---
title: Reserves with Marxan, and irreplaceability
short: 3. Marxan
description: Marxan's minimum set by simulated annealing in Terraflux - the species penalty factor, the boundary length modifier and how to calibrate it, selection frequency over many runs, how far the runs are from the optimum, and ProxIRR's irreplaceability.
order: 30
level: intermediate
time: 60 min
template: Marxan and irreplaceability
ops: [prioritize.marxan, prioritize.proxirr, prioritize.coarsen, map.calc, raster.read]
series: Systematic conservation planning
part: 3
---

Marxan is the most used software of systematic conservation planning: it has helped design marine parks from the
Great Barrier Reef to California and terrestrial reserve systems on every continent. In this part you will run
Marxan on Serra Verde with [[prioritize.marxan]], learn the three settings that decide its answer (the species
penalty factor, the boundary length modifier and the number of iterations), calibrate the boundary with a dashboard,
read the selection frequency of many runs, compare Marxan's plans with the proven optimum of the next part, and map
which places the targets cannot do without, with ProxIRR's irreplaceability.

## The idea

### A minimum set with a boundary

Marxan answers the **minimum set** question of [part 1](../systematic-conservation-planning/): which planning units
meet a target for every feature at the least cost? It adds a second wish: a plan whose pieces hang together. Ian
Ball and Hugh Possingham developed it at the University of Queensland around 2000, from their work on reserve
selection algorithms (Possingham, Ball & Andelman, 2000), and it was first used at scale to zone the Great
Barrier Reef Marine Park, where it helped raise the no-take area from under 5 % to a third of the park (Fernandes et
al., 2005). Ball, Possingham & Watts (2009) describe it and its relatives, Marxan with Zones among them (Watts et al.,
2009).

The **boundary** of a plan is the length of the edges between selected units and the rest. Two plans of the same
area can have very different boundaries:

![Two selections of eight cells, scattered and compact, with their boundaries.](/img/tutorials/spatial-prioritization/marxan/boundary-toy.png "Two plans of eight cells. Scattered, every cell is exposed on four sides: a boundary of 32 cell sides. Compact, the same area has a boundary of 12. A long boundary means more edge habitat, more fences and patrols, and more neighbours to negotiate with.")

!!! science "Key idea: Marxan's objective"
    Marxan looks for the set of planning units with the lowest **score**

    \[ \text{score} = \sum_{i} c_i x_i \;+\; \text{BLM} \sum_{i}\sum_{h} b_{ih}\, x_i (1 - x_h) \;+\; \sum_j \text{SPF}_j \, \text{FPF}_j \, s_j \]

    where \(x_i\) is 1 if unit \(i\) is in the plan and 0 if not, \(c_i\) its cost, and \(b_{ih}\) the length of the
    boundary units \(i\) and \(h\) share. The middle term is the boundary of the plan, weighted by the **boundary
    length modifier** (BLM). The last term is a penalty for every feature \(j\) whose target is not met: \(s_j\) is
    the share of the target missing, \(\text{FPF}_j\) roughly what it would cost to meet that target alone (Marxan
    estimates it with a quick greedy pass), and \(\text{SPF}_j\), the **species penalty factor**, how much a missed
    target hurts. Targets are not rules for Marxan: they are paid for when missed.

### Simulated annealing

A region of 4 825 units has \(2^{4825}\) possible plans. Marxan searches them with **simulated annealing**, a
method borrowed from physics (Kirkpatrick, Gelatt & Vecchi, 1983): a metal cooled slowly settles into a stronger
crystal than one quenched.

<div class="steps" markdown>

1. Start from a random plan.
2. Pick a unit at random and flip it: in if it was out, out if it was in. Compute the change \(\Delta\) of the score.
3. If the score falls, keep the change. If it rises, keep it anyway with probability \(e^{-\Delta / T}\), where \(T\)
   is the **temperature**. Early on \(T\) is high and many bad moves are accepted, which lets the search escape
   poor plans; \(T\) falls in steps (`num_temp` of them) until almost only improvements pass.
4. Repeat for `num_iters` iterations, then **iterative improvement**: try every unit once more in random order,
   keeping only changes that lower the score, until none does.

</div>

Each **run** starts from a different random plan and ends in a different good plan. Marxan makes many runs
(`num_reps`) and reports the best, every run's plan, and the **selection frequency**: how often each unit was in the
plans. Units chosen in every run are hard to do without; units chosen in a few are interchangeable.

Annealing gives good plans fast, but it cannot say how good: it never proves that a better plan does not exist.
The [next part](../linear-programming/) solves the same problem exactly and measures how far Marxan's plans are from the
best one.

### Irreplaceability

**Irreplaceability** is the extent to which the targets depend on a place (Ferrier, Pressey & Barrett, 2000). A
unit with the last population of a species is irreplaceable: every plan that meets the targets needs it. A unit
with features found in many other units is not. Marxan's selection frequency is a measure of it, but it depends on
the settings and the runs. Baisero, Schuster & Plumptre (2022) proposed a fast, explicit one, **ProxIRR**:

!!! science "Key idea: ProxIRR"
    For feature \(j\) in unit \(i\), with \(r_{ij}\) its amount there, \(R_j\) its total and \(T_j\) its target,

    \[ \alpha_{ij} = \min\left(1, \frac{r_{ij}}{R_j - T_j}\right) \]

    is the share of the feature's *spare* amount (what is left beyond the target) that the unit holds. If the unit
    holds all the spare, \(\alpha = 1\): without it the target cannot be met. Over the features of the unit,

    \[ \beta_i = 1 - \prod_j \big(1 - \alpha_{ij}\big) \qquad \gamma_i = \sum_j \alpha_{ij} \]

    \(\beta_i\) is the chance that the unit is needed for at least one target, and \(\gamma_i\) roughly how many
    targets need it.

## When to use it

- **To reproduce or extend a Marxan analysis.** Thousands of plans and much published guidance use Marxan
  (Ardron, Possingham & Klein, 2010). [[prioritize.marxan]] runs Marxan 4's algorithm with its settings, so an
  analysis can be repeated, with the same data, inside a larger model.
- **Selection frequency as a summary of many good plans,** for workshops where people want to see how flexible
  the choice is.
- **Problems an exact solver finds hard**: very large boundary problems, or objectives that are not linear (Marxan
  also has cost thresholds, `cost_thresh`).
- **ProxIRR** to map which places a set of targets depends on, before any plan is made, at the scale of a country or
  the globe (Baisero et al., 2022, mapped it worldwide).

When you need *the* cheapest plan, a proof of how close a plan is to it, or hard rules (budgets, contiguity, locked
areas as rules), use [linear programming](../linear-programming/): Schuster et al. (2020) found exact solvers faster and better than
simulated annealing on the same problems. When targets are not yet agreed, [Zonation](../zonation/)'s ranking
explores all of them at once.

## What you need

- **Features** as a stack, one layer per feature, and a **cost** map on the same grid: every cell with a cost is a
  planning unit, its shared edges with other units its boundaries (cells of 1 km share 1 000 m). Serra Verde's
  rasters: [serra-verde.zip](/data/tutorials/spatial-prioritization/systematic-conservation-planning/serra-verde.zip).
- Optionally a **status** map: 0 available, 1 in the starting plan, 2 locked in (existing reserves), 3 locked out
  (cities).
- The template **Templates ▸ Conservation and restoration ▸ Marxan and irreplaceability** runs Marxan and ProxIRR
  on four features drawn by chance.

Marxan's own text files (`pu.dat`, `spec.dat`, `puvspr.dat`, `bound.dat`) describe planning units of any shape:
read them as tables and connect them to the `pu`, `spec`, `puvspr` and `bound` inputs instead of the maps (ProxIRR
takes `pu`, `spec` and `puvspr`). With tables, the results are tables only: Marxan's `map` and `frequency_map`, and
ProxIRR's `beta` and `gamma` maps, are given only from maps. This tutorial uses maps.

## Build the model

```tfgraph
#! caption: Marxan on Serra Verde: 30 % of every feature, the protected areas locked in and the cities locked out, a boundary length modifier of 0.01, ten runs; and ProxIRR's irreplaceability of every cell for the same targets.
#! numbered: true
#! highlight: status, marxan, irreplaceability
format = "terraflux/1"

[[nodes]]
id = "features"
op = "raster.read"
params = { path = "features.tif" }
note = "Eight features, a band each."

[[nodes]]
id = "cost"
op = "raster.read"
params = { path = "cost.tif" }
note = "Thousand USD per km²; the sea has no value."

[[nodes]]
id = "protected"
op = "raster.read"
params = { path = "protected.tif" }

[[nodes]]
id = "urban"
op = "raster.read"
params = { path = "urban.tif" }

[[nodes]]
id = "status"
op = "map.calc"
inputs = { p = "protected.raster", u = "urban.raster" }
params = { expression = "if u > 0 then 3 else if p > 0 then 2 else 0" }
note = "Marxan's status: 2 locked in, 3 locked out."

[[nodes]]
id = "marxan"
op = "prioritize.marxan"
inputs = { features = "features.raster", cost = "cost.raster", status = "status.map" }
params = { prop = [0.3, 0.3, 0.3, 0.3, 0.3, 0.3, 0.3, 0.3], spf = 10, blm = 0.01, num_reps = 10, num_iters = 10000000, seed = 1 }

[[nodes]]
id = "irreplaceability"
op = "prioritize.proxirr"
inputs = { features = "features.raster" }
params = { prop = 0.3 }

[[nodes]]
id = "show"
op = "view.plot"
inputs = { value = "marxan.frequency_map" }
params = { title = "Selection frequency" }
```

<div class="steps" markdown>

1. **Features and cost** (nodes 1 and 2): [[raster.read]] of the eight bands and of the cost. The `stack` of node 1
   goes to Marxan's `features`, the `map` of node 2 to its `cost`.
2. **The status** (nodes 3 to 5): the protected areas and the cities, combined by [[map.calc]] into Marxan's codes.
   The protected areas (175 cells) cost money too: locked in, they are part of every plan and of its cost.
3. **Marxan** (node 6): [[prioritize.marxan]], in the library's *Spatial prioritization* subject. Its settings:
    - `prop`: the target of every feature, as a share of its total (a list in the order of the layers);
      `targets` gives amounts instead;
    - `spf`: the species penalty factors (one number for all, or a list);
    - `blm`: the boundary length modifier, in units of cost per unit of boundary (here thousand USD per metre);
    - `num_reps` runs of `num_iters` iterations each, from the random `seed` (the same seed gives the same runs);
    - the annealing schedule: `start_temp` (-1, adaptive, by default), `cool_fac`, `num_temp` (10 000 steps);
    - `run_mode` (1: annealing then iterative improvement, the default), `heur_type`, `itimp_type`;
    - `misslevel` (1: a target counts as met only in full), `edge_factor` (1: the edges of a unit facing the sea or
      the edge of the map count fully in its boundary).
4. **ProxIRR** (node 7): [[prioritize.proxirr]] with the same features and `prop = 0.3`.
5. **Show** (node 8): the selection frequency in a map tab.

</div>

Marxan's outputs: `map` (the best run's plan, 1 in, 0 out), `frequency_map` (the share of the runs selecting each
cell), and tables: `best`, `solutions` (every run's plan), `frequency`, `summary` (per run: its score, cost, number of
units, boundary, penalty, shortfall, features missed and the least proportion of a target met) and `penalties` (each
feature's target and the penalty it costs when missed). ProxIRR gives `beta` and `gamma` maps and `units`, `alpha` and
`targets` tables.

!!! app "In the application"
    Add `prioritize.marxan` from the *Spatial prioritization* subject of the library (or press ++slash++ and type
    `marxan`), drag the features' `stack` to `features` and the cost's `map` to `cost`, and set `prop` in the Node
    pane. Select the node and press ++ctrl+enter++: the `summary` table appears under the canvas, one row per run;
    click the `map` port to see the best plan. **Templates ▸ Conservation and restoration ▸ Marxan and
    irreplaceability** is the same model on maps drawn by chance.

From the command line, `terraflux run marxan.toml --out output` with `[outputs.summary]` (`from =
"marxan.summary"`) writes `output/summary.csv`; a [[raster.write]] of `marxan.frequency_map` keeps the map.

### First, the species penalty factor

Before anything else, check that the targets are met. With Marxan's default SPF of 1, the penalty for a missed
target is about what meeting it alone would cost, and annealing often finds it cheaper to pay the penalty than to
buy the land. Serra Verde without a boundary (BLM 0), ten runs of 10 million iterations:

| SPF | Best run (million USD) | Targets missed by the best run | Runs meeting every target |
|---|---|---|---|
| 1 (default) | 33.9 | 4 | 0 of 10 |
| 2 | 38.8 | 2 | 0 of 10 |
| 5 | 40.7 | 0 | 6 of 10 |
| 10 | 41.6 | 0 | 8 of 10 |
| 20 | 41.6 | 0 | 8 of 10 |
| 50 | 41.8 | 0 | 9 of 10 |

The cheap plans at SPF 1 and 2 are cheap because they **fail**: they are not answers to the question. The Marxan
Good Practices Handbook recommends raising the SPF until (nearly) every run meets every target, and no further, since
very large penalties make the search rougher (Ardron et al., 2010). Here 10 does it. The `penalties` table shows
the scale: meeting the jaguar's target alone would cost 26.6 million USD, the mangrove's 1.7 million. A feature that
still misses its target at a high SPF may have an impossible target (more than exists outside the locked-out
cells).

### Then, the number of iterations

Annealing improves with time. The same problem with more and more iterations per run:

```tfchart
type: line
data: /data/tutorials/spatial-prioritization/marxan/iterations.csv
x: iterations
xlog: true
xlabel: Iterations per run (millions)
y: [Best run, Worst run, Proven optimum]
colors: { "Best run": "#2e7d4f", "Worst run": "#e8901f", "Proven optimum": "#1c2420" }
ylabel: Cost (million USD)
height: 280
caption: "The cost of the best and the worst of ten runs (SPF 10, BLM 0) against the proven optimum of the same problem (prioritize.optimize with a gap of 0, part 4). From 0.1 to 50 million iterations the best run falls from 48.1 to 40.5 million USD, still 9 % above the optimum. Ten runs of 10 million iterations take about 15 s here; of 50 million, a minute and a half."
```

The returns diminish: each fivefold increase gains less. The test of enough iterations is that more do not change the
best plan much; Ardron et al. (2010) suggest checking it for every new data set. Notice also that the runs agree
with each other (best and worst within 1 %), yet all are 9 to 12 % above the optimum: consistent runs are not proof
of a good plan.

## Read the results

![The best of ten runs with a BLM of 0.01.](/img/tutorials/spatial-prioritization/marxan/best.png "The best of ten Marxan runs (SPF 10, BLM 0.01, 10 million iterations): 818 cells for 38.3 million USD, in compact blocks in the western mountains, along the two rivers, around the tamarin's fragments and in short stretches of mangrove.")

![Selection frequency of the ten runs.](/img/tutorials/spatial-prioritization/marxan/frequency.png "The selection frequency of the same ten runs: the share of the runs selecting each cell. Yellow cells (370) are in every run: the tamarin's fragments, the core of the mountain blocks, the rivers' best stretches. Green and blue cells (1 154) are chosen by some runs: alternatives of similar value.")

How to read them:

- **The best run** is one plan, the run with the lowest score. Read its `summary` row: the cost (38.3 million USD),
  the units (818), the boundary (648 km) and the targets missed. Here the best run misses one target by 0.02 %
  (its least proportion met, `mpm`, is 0.9998): with `misslevel = 1` it counts as missed, with `misslevel = 0.99`
  it would not.
- **The selection frequency** is the more robust product. Cells in every run are the backbone of any good plan;
  cells chosen in some runs are where negotiation can happen; cells never chosen are not needed for these targets.
  It is not a probability or a ranking: with other settings or more runs the shares change.
- **All runs are above the optimum.** The ten runs without a boundary cost from 41.6 to 42.0 million USD; the
  cheapest plan that meets the same targets costs 37.1 million (proven in [part 4](../linear-programming/)), 12 % less.

```tfchart
type: bar
data: /data/tutorials/spatial-prioritization/marxan/vs-optimum.csv
x: run
y: [Marxan run, Proven optimum]
colors: { "Marxan run": "#2e7d4f", "Proven optimum": "#1c2420" }
ylabel: Cost (million USD)
height: 260
caption: "Ten Marxan runs (SPF 10, BLM 0, 10 million iterations each) and the proven optimum of the same minimum set problem. Every run is 12 to 13 % dearer, and nothing in Marxan's output tells you so."
```

### Reporting a Marxan analysis

In the methods: the planning units, features, costs and targets; the SPF and how it was chosen; the BLM and how it
was calibrated; the number of runs and iterations, the annealing schedule and the run mode; and the version (here
Marxan 4's algorithm in Terraflux). In the results: the best run's cost, area, boundary and the share of every target
met, the selection frequency map, and how many runs met every target. Ardron et al. (2010) give a full list of good
practices, among them: never present the best run alone, since many runs are almost as good.

## Settings that change the answer

### The boundary length modifier

The BLM is an exchange rate: how much cost you would pay to remove one unit of boundary. Here costs are in thousand
USD and boundaries in metres, so a BLM of 0.01 means 10 USD per metre, 10 000 USD per kilometre of boundary. The
slider runs the same problem from no weight on the boundary to a large one:

```tfslider
label: Boundary length modifier
caption: "The best of ten Marxan runs (SPF 10, 10 million iterations) as the BLM grows. Scattered cells disappear first; from 0.01 the plan is a few blocks, and beyond 0.05 the blocks merge into fewer, larger ones. The chart shows the boundary (as a share of the plan with BLM 0) and the cost above the proven optimum."
frames:
  - { value: 0, src: /img/tutorials/spatial-prioritization/marxan/blm_0.png, stats: { "Cost": "41.6 M USD", "Boundary": "1 872 km", "Cells": "859", "In every run": "31 cells" } }
  - { value: 0.002, src: /img/tutorials/spatial-prioritization/marxan/blm_0.002.png, stats: { "Cost": "40.2 M USD", "Boundary": "1 442 km", "Cells": "840", "In every run": "104 cells" } }
  - { value: 0.005, src: /img/tutorials/spatial-prioritization/marxan/blm_0.005.png, stats: { "Cost": "38.9 M USD", "Boundary": "966 km", "Cells": "827", "In every run": "238 cells" } }
  - { value: 0.01, src: /img/tutorials/spatial-prioritization/marxan/blm_0.01.png, stats: { "Cost": "38.3 M USD", "Boundary": "648 km", "Cells": "818", "In every run": "370 cells" } }
  - { value: 0.02, src: /img/tutorials/spatial-prioritization/marxan/blm_0.02.png, stats: { "Cost": "38.2 M USD", "Boundary": "442 km", "Cells": "817", "In every run": "482 cells" } }
  - { value: 0.05, src: /img/tutorials/spatial-prioritization/marxan/blm_0.05.png, stats: { "Cost": "39.6 M USD", "Boundary": "314 km", "Cells": "828", "In every run": "485 cells" } }
  - { value: 0.1, src: /img/tutorials/spatial-prioritization/marxan/blm_0.1.png, stats: { "Cost": "40.7 M USD", "Boundary": "276 km", "Cells": "829", "In every run": "483 cells" } }
start: 3
chart:
  type: line
  data: /data/tutorials/spatial-prioritization/marxan/blm.csv
  x: blm
  y: ["Boundary (% of BLM 0)", "Cost (% above the optimum)"]
  colors: { "Boundary (% of BLM 0)": "#2f6f9f", "Cost (% above the optimum)": "#e8901f" }
  xlabel: BLM
  ylabel: "%"
```

Something odd happens: from BLM 0 to 0.02 the plans get **cheaper** as well as more compact. A boundary cannot make
land cheaper; what changes is the search. A scattered plan has thousands of nearly equivalent swaps, and annealing
wanders among them; a weight on the boundary makes the score smoother, and the search settles better. The exact
solver of [part 5](../spatial-design/) shows the true trade-off: with a boundary penalty the cost can only rise. With
Marxan, part of what you see along a calibration curve is the heuristic, not the landscape.

### Calibrating the BLM

There is no right BLM: it depends on the units of cost and boundary and on the landscape, so a value cannot be
copied from another study. The usual calibration (Stewart & Possingham, 2005; Ardron et al., 2010) runs Marxan over a
range of BLMs and plots the cost against the boundary:

```tfchart
type: scatter
data: /data/tutorials/spatial-prioritization/marxan/tradeoff.csv
x: Boundary (km)
y: [Cost (million USD)]
colors: { "Cost (million USD)": "#2e7d4f" }
xlabel: Boundary of the best run (km)
ylabel: Cost (million USD)
height: 280
caption: "Cost against boundary for BLMs from 0 (right) to 0.1 (left). Between 1 872 and 442 km the boundary falls by three quarters without any cost (the annealing artefact above); beyond, every further kilometre of compactness costs money: 1.4 million USD for the next 128 km, 1 million for the 38 km after."
```

Choose the BLM where the curve turns: past it, compactness gets expensive. Here that is about 0.01 to 0.02, a plan
with a boundary about a third or a quarter of the scattered one for no more cost. Choosing by eye is common;
[part 6](../evaluating-plans/) shows reproducible choices (the knee of the curve and TOPSIS) with
[[prioritize.calibrate]]. Calibrate with the SPF and the iterations already fixed, and say in the methods how the
value was chosen.

### Selection frequency and the BLM

The slider's last number is worth a look: the cells chosen in every run grow from 31 at BLM 0 to about 480 at 0.02
and above. Without a boundary, many cells are interchangeable and the runs disagree everywhere; with one, the runs
agree on whole blocks. A frequency map is a property of the settings as much as of the landscape: compare
frequencies only between runs with the same settings.

### ProxIRR and the size of the planning units

![ProxIRR's beta on cells of 1 km² and on blocks of 5 x 5 km.](/img/tutorials/spatial-prioritization/marxan/proxirr.png "ProxIRR's beta for targets of 30 % of every feature. Left, on Serra Verde's cells of 1 km²: the highest value is 0.026, on the coast (mangrove) and in the tamarin's fragments. Right, on blocks of 25 km² made by prioritize.coarsen: the same places, now up to 0.33. Note the colour scales.")

On cells of 1 km², no cell is irreplaceable: every feature is spread over hundreds of cells, and a single cell holds a
tiny share of its spare amount (beta of 0.026 at most). The **pattern** is still useful: the coast with the mangrove,
the tamarin's fragments and the rivers stand out, the same places Marxan selects in every run (a Spearman correlation
of 0.65 between beta and the selection frequency at BLM 0.01). On larger units, irreplaceability rises: on blocks of
5 × 5 km the tamarin's two fragments and the mangrove coast reach betas of 0.2 to 0.33, and the correlation with the
selection frequency (averaged over each block) rises to 0.77.

Irreplaceability depends on the **size of the planning units** and on the **targets**: with higher targets, there
is less spare and every unit counts more; with a target above what exists, ProxIRR counts it as needed everywhere
unless `triage = true` sets it aside. Compare values only at the same scale and targets.

To compute it on blocks, [[prioritize.coarsen]] sums the cost and the features over blocks of `factor` × `factor`
cells:

```tfgraph
#! caption: ProxIRR on blocks of 5 x 5 cells: prioritize.coarsen sums the cost and every feature over the blocks.
#! numbered: true
#! highlight: blocks, irreplaceability
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
id = "blocks"
op = "prioritize.coarsen"
inputs = { cost = "cost.raster", features = "features.raster" }
params = { factor = 5 }

[[nodes]]
id = "irreplaceability"
op = "prioritize.proxirr"
inputs = { features = "blocks.features" }
params = { prop = 0.3 }

[[nodes]]
id = "show"
op = "view.plot"
inputs = { value = "irreplaceability.beta" }
```

## Common pitfalls

- **Targets missed without noticing.** Read the `missing` column of the `summary` for every run. A cheap plan that
  misses targets answers another question. Raise the SPF.
- **Too few iterations.** Runs that agree with each other can all be far from the best. Increase `num_iters` until
  the best run stops improving much, and compare with an exact solution when you can.
- **A BLM copied from another study.** It has units (cost per unit of boundary) and depends on the landscape.
  Calibrate it.
- **The best run alone.** Present the selection frequency, or several runs; the best run is one of many almost as
  good.
- **The edge of the study area.** With `edge_factor = 1`, cells on the coast or the map's edge carry a boundary on
  their open sides, and the BLM pushes the plan away from them. Use a smaller factor when the edge is not a real
  edge (a political border through continuous forest).
- **Irreplaceability compared across scales.** ProxIRR and selection frequency change with the planning units,
  targets and settings: compare only like with like.

## Going further

- Part 4, [Optimal plans with linear programming](../linear-programming/): the same minimum set solved exactly, with a proof of
  how close the plan is to the best.
- Part 5, [Compact and connected plans, and zones](../spatial-design/): boundary penalties in an exact solver.
- Part 6, [Evaluating, comparing and reporting plans](../evaluating-plans/): calibration with the knee and TOPSIS,
  Ferrier's irreplaceability and replacement cost.
- The operators: [[prioritize.marxan]], [[prioritize.proxirr]], [[prioritize.coarsen]].

## References

- Ardron, J. A., Possingham, H. P., & Klein, C. J. (Eds.) (2010). *Marxan Good Practices Handbook*, Version 2. Pacific Marine Analysis and Research Association, Victoria, Canada.
- Baisero, D., Schuster, R., & Plumptre, A. J. (2022). Redefining and mapping global irreplaceability. *Conservation Biology*, 36(2), e13806. https://doi.org/10.1111/cobi.13806
- Ball, I. R., Possingham, H. P., & Watts, M. E. (2009). Marxan and relatives: software for spatial conservation prioritization. In A. Moilanen, K. A. Wilson & H. P. Possingham (Eds.), *Spatial Conservation Prioritization* (pp. 185–195). Oxford University Press. https://doi.org/10.1093/oso/9780199547760.003.0014
- Fernandes, L., Day, J., Lewis, A., Slegers, S., Kerrigan, B., Breen, D., et al. (2005). Establishing representative no-take areas in the Great Barrier Reef: large-scale implementation of theory on marine protected areas. *Conservation Biology*, 19(6), 1733–1744. https://doi.org/10.1111/j.1523-1739.2005.00302.x
- Ferrier, S., Pressey, R. L., & Barrett, T. W. (2000). A new predictor of the irreplaceability of areas for achieving a conservation goal, its application to real-world planning, and a research agenda for further refinement. *Biological Conservation*, 93(3), 303–325. https://doi.org/10.1016/S0006-3207(99)00149-4
- Kirkpatrick, S., Gelatt, C. D., & Vecchi, M. P. (1983). Optimization by simulated annealing. *Science*, 220(4598), 671–680. https://doi.org/10.1126/science.220.4598.671
- Possingham, H. P., Ball, I. R., & Andelman, S. (2000). Mathematical methods for identifying representative reserve networks. In S. Ferson & M. Burgman (Eds.), *Quantitative Methods for Conservation Biology* (pp. 291–306). Springer. https://doi.org/10.1007/0-387-22648-6_17
- Schuster, R., Hanson, J. O., Strimas-Mackey, M., & Bennett, J. R. (2020). Exact integer linear programming solvers outperform simulated annealing for solving conservation planning problems. *PeerJ*, 8, e9258. https://doi.org/10.7717/peerj.9258
- Stewart, R. R., & Possingham, H. P. (2005). Efficiency, costs and trade-offs in marine reserve system design. *Environmental Modeling & Assessment*, 10(3), 203–213. https://doi.org/10.1007/s10666-005-9001-y
- Watts, M. E., Ball, I. R., Stewart, R. S., Klein, C. J., Wilson, K., Steinback, C., et al. (2009). Marxan with Zones: software for optimal conservation based land- and sea-use zoning. *Environmental Modelling & Software*, 24(12), 1513–1521. https://doi.org/10.1016/j.envsoft.2009.06.005
