---
title: "Systematic conservation planning: the ideas"
short: 1. The ideas
description: Why "protect the richest places" fails, what complementarity, irreplaceability and efficiency mean, the ingredients of a planning problem, and a first plan made in Terraflux.
order: 10
level: beginner
time: 40 min
ops: [prioritize.optimize, prioritize.evaluate, prioritize.zonation, raster.read, view.plot]
series: Systematic conservation planning
part: 1
---

Money, land and time for conservation are always short, so someone has to decide *where*. This series is about
making that decision with methods that can be explained, repeated and defended: **systematic conservation
planning** and the **spatial prioritization** at its heart. In this first part you will meet the ideas every
method shares, using a small example you can follow by eye and a synthetic region, Serra Verde, that the whole
series uses. At the end you will have made your first plan in Terraflux and read the table that says whether it
does its job.

## The idea

### Reserves chosen by chance

For most of the twentieth century, protected areas were chosen *ad hoc*: for their scenery, for recreation, or
because the land was too steep, remote or poor to be worth anything else. Pressey (1994) showed what this left
behind in New South Wales: reserves piled up on rugged, unproductive land while lowland habitats, and the
species that live only there, were left out. A decade later a global **gap analysis**, which asks what existing
reserves already hold, found hundreds of threatened species occurring in no protected area at all, most of them
with small ranges (Rodrigues et al., 2004).

**Systematic conservation planning** (Margules & Pressey, 2000) replaced these choices with an explicit,
repeatable procedure: state clear goals for what should be conserved, measure what the existing reserves already
achieve, choose new areas that fill the gaps as efficiently as possible, then implement, manage and monitor, and
revise the plan as things change. The step in the middle, the choice of areas, is **spatial prioritization**
(Moilanen, Wilson & Possingham, 2009), and it is what the `prioritize` operators of Terraflux do.

### Complementarity

The idea that most separates systematic planning from intuition is **complementarity**: the value of a site
depends on what the other sites in the plan already hold (Kirkpatrick, 1983; Pressey et al., 1993). A second
site with the same species as the first adds little; a poor site with a species found nowhere else adds a lot.

Take four sites, A to D, and four species. A holds three species, B two, C and D one each. If only two sites can
be protected, the obvious choice is the two richest, A and B. But B holds nothing that A does not already have,
so the pair protects three species. The pair A and C protects all four:

![Four sites and four species: the two richest sites hold three species, the two complementary ones all four.](/img/tutorials/spatial-prioritization/systematic-conservation-planning/complementarity.png "Four sites, four species. Left: the two richest sites, A and B, hold three species; species 4 lives only in C and is lost. Right: the pair Terraflux chose (`prioritize.optimize` with the max_cover objective and a budget of two sites) holds all four. Zonation, run on the same four sites, also keeps A and C to the end (ranks 0.5 and 0.75) and gives up D first (rank 0).")

!!! science "Key idea: sets, not sites"
    A plan is judged as a **set** of places. A site's value is what it adds to the set: its *marginal* gain,
    given the sites already chosen. Scoring every site on its own (richness, endemism, a threat index) and taking
    the best scores cannot see this, however good the score.

Both families of methods in this series use complementarity. **Zonation** (part 2) removes the sites one by one,
always the one whose loss matters least given what remains; **Marxan** and **prioritizr** (parts 3 to 5) search
for the best set directly.

### The core concepts

Kukkala & Moilanen (2013) reviewed the concepts behind spatial prioritization. Five do most of the work:

Representation
:   Every feature (species, habitat, ecosystem service) should be in the plan, not only the common or
    charismatic ones.

Complementarity
:   Places are chosen for what they add to the others, as above.

Irreplaceability
:   How much the goals depend on a place. A site with the only population of a species is irreplaceable; a site
    with features found in many others is not. Part 3 and part 6 measure it.

Efficiency
:   Reaching the goals at the least cost, which leaves the rest of the landscape, and the rest of the budget, for
    other uses.

Adequacy and persistence
:   Being *in* a reserve is not the same as *persisting* there. Features need enough area, in large and connected
    pieces, to last (Cabeza & Moilanen, 2001). Representation is the first step; part 2 (connectivity in Zonation)
    and part 5 (compact and connected plans) take the next ones.

## The ingredients of a planning problem

Every prioritization, from a watershed to a continent, is the answer to five questions.

| Ingredient | The question | In Terraflux |
|---|---|---|
| **Planning units** | Into which pieces is the landscape divided? | the cells of a cost raster with a value, polygons (properties, watersheds), H3 hexagons, or rows of a table |
| **Features** | What should be conserved? | a raster layer per species, habitat or service: a **stack** |
| **Cost** | What does it take to conserve each piece? | the values of the cost raster (or a column) |
| **Targets** | How much of each feature is enough? | `relative_targets` (a share of each feature's total), `absolute_targets`, by hand |
| **Decisions** | What can be done with each piece? | in or out (binary), a share of it (proportion), or an action against a threat |

A **raster** is a map stored as a grid of square **cells**, each with a value: a habitat suitability, a land
price, a class of land cover. A **stack** is several rasters on the same grid, one per feature. A **planning
unit** is the smallest area a plan can select or leave out; on a raster it is usually a cell.

**Costs** can be the price of land, the income lost by not farming or logging it (the *opportunity cost*), the
cost of managing it, or simply its area when nothing better is known (Naidoo et al., 2006). Without costs the best
plan is to protect everything; with them the question becomes which places give the most for what they cost.

**Targets** say how much is enough: 30 % of the habitat of every species, 500 km² of mangrove, 2 million tonnes of
carbon. They come from policy (Target 3 of the Kunming-Montreal Global Biodiversity Framework asks for 30 % of land
and sea by 2030), from ecology (the area a viable population needs), or from the people involved.

### Serra Verde

The series uses **Serra Verde**, a synthetic coastal region that comes with landschaft, the library behind
Terraflux's prioritization. It is imaginary but built to behave like real planning data: 60 × 90 cells of 1 km
(4825 planning units on land; the sea has no value and is not part of the problem), mountains in the west where
land is cheap, a coastal plain with three cities, a highway and two rivers where land is dear, three protected
areas, and eight features from widespread to very rare.

![Serra Verde: the cost of every cell, its protected areas and cities.](/img/tutorials/spatial-prioritization/systematic-conservation-planning/serra-verde.png "Serra Verde. Left: the cost of every planning unit, from 25 to 256 thousand USD per km²: cheap in the western mountains, dear around the three cities and along the highway and the coast. Right: the three existing protected areas and the three cities.")

![The habitat suitability of the eight features of Serra Verde.](/img/tutorials/spatial-prioritization/systematic-conservation-planning/features.png "The eight features: the habitat suitability (0 to 1) of six species and the extent of two ecosystems. The jaguar and the maned wolf are widespread; the golden lion tamarin lives in two small lowland fragments (105 cells), the mangrove in a thin coastal strip (97 cells), the otter along the two rivers.")

Download the rasters to follow along: [serra-verde.zip](/data/tutorials/spatial-prioritization/systematic-conservation-planning/serra-verde.zip)
(the features, the cost, the protected areas, the cities, the habitat condition and two threats, with a short
README). They are GeoTIFFs in a projected **coordinate reference system** (CRS: how the numbers of a map relate to
places on Earth), UTM zone 23 South, with cells of 1000 m.

## The richest cells against a plan

Here is the experiment that shows why methods are needed. The goal is to protect **30 % of the habitat of every
feature**. Two strategies spend exactly the same money:

A. **The richest cells.** Add up the eight suitability maps (a "hotspot" score), then select cells from the
   highest score down until the money runs out.

B. **An optimized plan.** Ask Terraflux for the cheapest set of cells that meets all eight targets (a *minimum set*
   problem, part 4). Its cost, 37.1 million USD, is the budget of strategy A.

![The richest cells and an optimized plan for the same money.](/img/tutorials/spatial-prioritization/systematic-conservation-planning/hotspots-vs-plan.png "A: the 919 richest cells crowd into the western mountains, where four features overlap. B: the optimized plan (812 cells) spreads across the region, picking up the tamarin's fragments, a piece of every river, and the mangrove. Both cost 37.1 million USD.")

```tfchart
type: bar
data: /data/tutorials/spatial-prioritization/systematic-conservation-planning/hotspots.csv
x: feature
y: [Richest cells, Optimized plan]
colors: { "Richest cells": "#e8901f", "Optimized plan": "#2e7d4f" }
ylabel: "% of the feature held"
height: 300
caption: "How much of each feature the two strategies hold (from `prioritize.evaluate`). The target is 30 % for all. The richest cells hold 82 % of the araucaria forest and 70 % of the harpy eagle's habitat, but 7 % of the tapir's and **nothing** of the tamarin and the mangrove: 5 targets of 8. The plan meets all eight."
```

The richest cells are in the mountains, where the jaguar, the harpy eagle, the maned wolf and the araucaria forest
overlap. Every mountain cell adds a little more of the *same* features. The golden lion tamarin, the mangrove and
the tapir live in places with few other features, so those cells never reach the top of a richness list. The
optimized plan values *sets*: once the mountain cells hold 30 % of the araucaria forest, more of them add nothing to
the targets, and the next dollar goes to a tamarin fragment or a stretch of mangrove. That is complementarity at
the scale of a region. The plan is also **efficient**: where two places hold the same features it takes the cheaper
one, and it stays away from the dear land around the cities whenever there is an alternative.

## Two families of methods

| Approach | The question it answers | In Terraflux | Part |
|---|---|---|---|
| **Scoring** | Which places score highest on an index? | not a method: richness, endemism (see [Biodiversity analysis](/tutorials/biodiversity/)) | |
| **Ranking** | In what order would we give up the landscape, keeping as much of everything for as long as possible? | [[prioritize.zonation]] | 2 |
| **Minimum set** | What is the cheapest set of areas that meets a target for every feature? | [[prioritize.marxan]], [[prioritize.optimize]] | 3, 4 |
| **Maximal coverage** | With this budget, which set of areas achieves the most? | [[prioritize.optimize]] (budget objectives) | 4 |

Scoring ignores complementarity, as the experiment showed. The other three use it, and they belong to two
families:

- **Ranking** (Zonation; Moilanen et al., 2005, 2022) orders every cell from the least to the most important. It
  needs no target and no budget: the best 10 % lies inside the best 30 %, so one map answers every budget. This
  suits the early stages of planning, while targets are still discussed, and very large maps.
- **Optimization** finds the best *set* of areas for a target or a budget. The two classic formulations are the
  **minimum set** problem (meet every target at the least cost) and the **maximal coverage** problem (do the most
  within a budget; Church et al., 1996). Marxan (Ball et al., 2009) solves them with *simulated annealing*, a
  heuristic that improves a plan by random changes and gives good answers without knowing how good. prioritizr
  (Hanson et al., 2025) writes them as *integer linear programs* and solves them exactly, with a proof of how close
  the plan is to the best possible (Underhill, 1994; Rodrigues & Gaston, 2002; Beyer et al., 2016).

![A ranking and a plan of Serra Verde.](/img/tutorials/spatial-prioritization/systematic-conservation-planning/two-families.png "The two families on the same data. Left: Zonation's rank map, a priority for every cell (1 the most important). The rivers, the tamarin's fragments and the coast stand out. Right: prioritizr's minimum set for 30 % of every feature, a single plan.")

The two families are complementary, not rivals: a ranking to explore, and an optimized plan to decide. Part 6
compares them on Serra Verde.

## When to use it

Spatial prioritization answers questions such as:

- **Where should new protected areas go** to represent every ecosystem and species, at the least cost? This is the
  classic use, from national reserve systems to the global analyses of where protected areas should grow
  (Montesino Pouzols et al., 2014).
- **What does a target cost?** The cost of 17 %, 30 % or 50 % of every feature, and what a given budget achieves.
- **Which places are irreplaceable**, so that development should avoid them (impact avoidance, offsets)?
- **Where should restoration go**, for the most biodiversity or carbon per dollar (Strassburg et al., 2020)? See
  [Distance and restoration](/tutorials/distance-restoration/).
- **Which actions against which threats, where?** (multi-action planning, part 7).
- **How good is an existing or proposed reserve network?** A gap analysis is an evaluation (part 6).

What it is *not*:

- **It is a decision-support tool, not a decision.** The answer depends on the goals, data and costs you give it.
  Different, equally reasonable goals give different maps, which is why stakeholders should take part in choosing
  them.
- **It is only as good as its data.** Species distribution models have errors, costs are estimates, and both pass
  into the plan.
- **High threat is not high priority.** A very threatened place may be a poor investment if action there is
  unlikely to work (Game et al., 2013).
- **It does not replace field knowledge.** A selected area may be impractical for reasons no map holds.

## What you need

- **Features**: a raster per feature, all on the same grid: species distribution models or ranges, habitat maps,
  carbon. Values are amounts: presence (0/1), probability or suitability (0 to 1), hectares of habitat, tonnes.
  The [Species distributions](/tutorials/species-distributions/) tutorials make such maps.
- **A cost raster** on the same grid. Cells with **no value** are not planning units (the sea in Serra Verde). A
  cost of zero is a free planning unit, which the solver will gladly select.
- **Optionally**: existing protected areas (to lock in), places that cannot be protected (cities, to lock out),
  threats, habitat condition.
- **A projected coordinate system** with cells of equal area (metres, not degrees), so that a cell means the same
  area everywhere and boundaries are measured in metres.

In the application, the templates of the **Templates** menu, group *Conservation and restoration*, start the main
methods with data drawn by chance: *Zonation*, *Marxan and irreplaceability*, *Conservation planning (prioritizr)*,
*Irreplaceability of a plan* and *Multi-action planning (prioriactions)*. Replace their `nlm` nodes with
`raster.read` nodes of your own maps.

## Build the model

The first plan of the series: the cheapest set of cells that holds 30 % of each of the eight features, and its
evaluation.

```tfgraph
#! caption: A first plan. Read the features and the cost, find the cheapest cells that hold 30 % of every feature, evaluate the plan and show it.
#! numbered: true
#! highlight: plan, evaluation
format = "terraflux/1"

[[nodes]]
id = "features"
op = "raster.read"
params = { path = "features.tif" }
note = "Serra Verde's eight features, a band each."

[[nodes]]
id = "cost"
op = "raster.read"
params = { path = "cost.tif" }
note = "Thousand USD per km²; the sea has no value."

[[nodes]]
id = "plan"
op = "prioritize.optimize"
inputs = { features = "features.raster", cost = "cost.raster" }
params = { objective = "min_set", relative_targets = 0.3, gap = 0.001 }

[[nodes]]
id = "evaluation"
op = "prioritize.evaluate"
inputs = { features = "features.raster", cost = "cost.raster", solution = "plan.solution" }
params = { relative_targets = 0.3 }

[[nodes]]
id = "show"
op = "view.plot"
inputs = { value = "plan.solution" }
params = { title = "Plan for 30 % of every feature" }

[outputs.coverage]
from = "evaluation.coverage"
```

<div class="steps" markdown>

1. **Read the features** (node 1). [[raster.read]] reads `features.tif`. A multi-band file gives a **stack** on the
   `stack` output port, one layer per band, named by the bands' descriptions (`jaguar`, `lowland_tapir`...). Those
   names follow the features through every table the plan produces.
2. **Read the cost** (node 2), a single band: its `map` output. Every cell with a value becomes a planning unit.
3. **Solve the problem** (node 3). [[prioritize.optimize]] takes the features on its `features` input and the cost on
   `cost`. `objective = "min_set"` asks for the cheapest plan that meets every target; `relative_targets = 0.3`
   sets the target of every feature to 30 % of its total amount; `gap = 0.001` asks the solver to prove the plan
   is within 0.1 % of the cheapest possible (part 4 explains the gap). The `solution` output is a map: 1 selected,
   0 not.
4. **Evaluate it** (node 4). [[prioritize.evaluate]] takes the same problem (features, cost, targets) and a solution
   on its `solution` input. Its `coverage` table says, for every target, whether it is met.
5. **Show it** (node 5). [[view.plot]] draws the plan in a map tab of its own.

</div>

!!! app "In the application"
    Press **New**, then open each raster with ++ctrl+o++ (*Open a raster or vectors*): each becomes a
    `raster.read` node. Find
    *Spatial prioritization* in the library on the left, or press ++slash++ and type `optimize`, and click
    `prioritize.optimize` to add it. Drag from the `stack` port of the features to the `features` port of the new
    node, and from the `map` port of the cost to `cost`. Select the node and set its settings in the **Node** pane:
    objective, relative targets, gap. Press ++ctrl+enter++ to run up to the selected node; its result appears under
    the canvas (a map, or a table you can sort). Or start from **Templates ▸ Conservation and restoration ▸
    Conservation planning (prioritizr)** and replace its random maps with your own.

From the command line, save the model as `first-plan.toml` next to the rasters and run it:

```bash
terraflux run first-plan.toml --out output
```

The outputs of the model (`[outputs.coverage]` above) are written to the output folder, tables as CSV files.

## Read the results

The plan selects **812 cells** (17 % of the land) for **37.1 million USD**, and the solver proves it is within
0.01 % of the cheapest plan possible. The `coverage` table is the first thing to read after any solve:

| Feature | Target (% of total) | Held | Met |
|---|---|---|---|
| Jaguar | 30 % | 30.0 % | yes |
| Lowland tapir | 30 % | 30.0 % | yes |
| Harpy eagle | 30 % | 43.7 % | yes |
| Maned wolf | 30 % | 30.2 % | yes |
| Golden lion tamarin | 30 % | 30.0 % | yes |
| Neotropical otter | 30 % | 30.0 % | yes |
| Araucaria forest | 30 % | 60.2 % | yes |
| Mangrove | 30 % | 30.0 % | yes |

Most targets are met *exactly*: a minimum set buys no more than it needs. Two features are well above their target,
the harpy eagle and the araucaria forest. They come "for free": they live in the cheap western mountains where the
plan must go anyway for the jaguar and the maned wolf. A feature far above its target is a sign of where the plan's
cheap land is, not a sign of a problem.

What to report in a paper, at a minimum: the planning units (their size and number), the features and their data,
the cost layer and its units, the targets, the objective and the solver with its optimality gap, then the plan's
cost, area and the share of every feature it holds, and a map. Part 6 gives a full checklist and shows how to
compare plans.

## Settings that change the answer

The **target** sets the price. The slider shows the cheapest plan for targets from 10 % to 50 % of every feature:

```tfslider
label: Target (% of every feature)
caption: "The cheapest plan for each target (`prioritize.optimize`, min_set, gap 0.5 %). The cost rises a little faster than the target: from 10.9 million USD at 10 % to 68.0 million at 50 %, because the cheapest land goes first and every further step must buy dearer land. The mountains enter at once; the coast and the land around the cities only at high targets."
frames:
  - { value: "10 %", src: /img/tutorials/spatial-prioritization/systematic-conservation-planning/target_10.png, stats: { Cost: "10.9 M USD", Area: "5.3 %" } }
  - { value: "15 %", src: /img/tutorials/spatial-prioritization/systematic-conservation-planning/target_15.png, stats: { Cost: "17.1 M USD", Area: "8.2 %" } }
  - { value: "20 %", src: /img/tutorials/spatial-prioritization/systematic-conservation-planning/target_20.png, stats: { Cost: "23.6 M USD", Area: "11.1 %" } }
  - { value: "25 %", src: /img/tutorials/spatial-prioritization/systematic-conservation-planning/target_25.png, stats: { Cost: "30.2 M USD", Area: "13.9 %" } }
  - { value: "30 %", src: /img/tutorials/spatial-prioritization/systematic-conservation-planning/target_30.png, stats: { Cost: "37.2 M USD", Area: "16.8 %" } }
  - { value: "35 %", src: /img/tutorials/spatial-prioritization/systematic-conservation-planning/target_35.png, stats: { Cost: "44.5 M USD", Area: "19.8 %" } }
  - { value: "40 %", src: /img/tutorials/spatial-prioritization/systematic-conservation-planning/target_40.png, stats: { Cost: "52.0 M USD", Area: "22.9 %" } }
  - { value: "45 %", src: /img/tutorials/spatial-prioritization/systematic-conservation-planning/target_45.png, stats: { Cost: "59.8 M USD", Area: "25.9 %" } }
  - { value: "50 %", src: /img/tutorials/spatial-prioritization/systematic-conservation-planning/target_50.png, stats: { Cost: "68.0 M USD", Area: "29.1 %" } }
start: 4
chart:
  type: line
  data: /data/tutorials/spatial-prioritization/systematic-conservation-planning/targets.csv
  x: target
  y: [cost]
  labels: { cost: "Cost (million USD)" }
  colors: { cost: "#2e7d4f" }
  xlabel: Target (%)
  ylabel: Million USD
```

The other settings that change a plan, each the subject of a part of this series:

- **the method**: a ranking (part 2) or an optimized set (parts 3 and 4);
- **the cost layer**: area, land price or opportunity cost give different plans (part 2 shows the effect in
  Zonation);
- **the objective**: meet every target, or do the most with a budget (part 4);
- **the shape**: compact, connected plans cost more but last better (part 5);
- **what is already protected**, and what cannot be (part 4).

## Common pitfalls

- **Zero instead of no value.** Cells that are not part of the problem (sea, outside the study area) must have no
  value in the cost raster. A cost of 0 makes them free, and every method will select them.
- **Features on different grids.** All features and the cost must share one grid (extent, cell size, CRS). Align
  them first with `raster.match` (see [Getting started](/tutorials/getting-started/)).
- **Geographic coordinates.** Cells of 0.01° shrink towards the poles, so their areas differ and boundaries are in
  degrees. Reproject to an equal-area or a local projected CRS before planning.
- **Features of very different totals.** A widespread species has a far larger total than a rare one. Relative
  targets handle this, but budget objectives and rankings without weights favour the widespread (parts 2 and 4).
- **The edge of the study area.** A species rare inside your borders may be common next door; a plan within
  administrative borders gives priority to the edges of ranges. Plan within the ecological context when you can.
- **Threat is not priority.** Do not use a threat map as a feature to protect. Threats belong in the cost, in
  locked-out areas, in the condition of habitat (part 2), or as threats to act against (part 7).

## Going further

The series, in order:

1. **The ideas** (this page).
2. [Ranking a landscape with Zonation](/tutorials/spatial-prioritization/zonation/): rank maps, performance curves,
   weights, costs, connectivity, condition and Zonation 5 projects.
3. [Reserves with Marxan, and irreplaceability](/tutorials/spatial-prioritization/marxan/): simulated annealing,
   the boundary length modifier and its calibration, selection frequency, ProxIRR.
4. [Optimal plans with prioritizr](/tutorials/spatial-prioritization/prioritizr/): objectives, targets, budgets,
   locked areas, gaps and solvers, portfolios.
5. [Compact and connected plans, and zones](/tutorials/spatial-prioritization/spatial-design/): penalties,
   constraints, connectivity, hexagons and management zones.
6. [Evaluating, comparing and reporting plans](/tutorials/spatial-prioritization/evaluating-plans/).
7. [Multi-action planning](/tutorials/spatial-prioritization/multi-action-planning/): threats and actions.

Restoration planning (plangea) is in [Distance and restoration](/tutorials/distance-restoration/). The operators are
documented in the [operator reference](/operators/).

## References

- Ball, I. R., Possingham, H. P., & Watts, M. E. (2009). Marxan and relatives: software for spatial conservation prioritization. In A. Moilanen, K. A. Wilson & H. P. Possingham (Eds.), *Spatial Conservation Prioritization* (pp. 185–195). Oxford University Press. https://doi.org/10.1093/oso/9780199547760.003.0014
- Beyer, H. L., Dujardin, Y., Watts, M. E., & Possingham, H. P. (2016). Solving conservation planning problems with integer linear programming. *Ecological Modelling*, 328, 14–22. https://doi.org/10.1016/j.ecolmodel.2016.02.005
- Cabeza, M., & Moilanen, A. (2001). Design of reserve networks and the persistence of biodiversity. *Trends in Ecology & Evolution*, 16(5), 242–248. https://doi.org/10.1016/S0169-5347(01)02125-5
- Church, R. L., Stoms, D. M., & Davis, F. W. (1996). Reserve selection as a maximal covering location problem. *Biological Conservation*, 76(2), 105–112. https://doi.org/10.1016/0006-3207(95)00102-6
- Game, E. T., Kareiva, P., & Possingham, H. P. (2013). Six common mistakes in conservation priority setting. *Conservation Biology*, 27(3), 480–485. https://doi.org/10.1111/cobi.12051
- Hanson, J. O., Schuster, R., Strimas-Mackey, M., Morrell, N., Edwards, B. P. M., Arcese, P., Bennett, J. R., & Possingham, H. P. (2025). Systematic conservation prioritization with the prioritizr R package. *Conservation Biology*, 39(1), e14376. https://doi.org/10.1111/cobi.14376
- Kirkpatrick, J. B. (1983). An iterative method for establishing priorities for the selection of nature reserves: an example from Tasmania. *Biological Conservation*, 25(2), 127–134. https://doi.org/10.1016/0006-3207(83)90056-3
- Kukkala, A. S., & Moilanen, A. (2013). Core concepts of spatial prioritisation in systematic conservation planning. *Biological Reviews*, 88(2), 443–464. https://doi.org/10.1111/brv.12008
- Margules, C. R., & Pressey, R. L. (2000). Systematic conservation planning. *Nature*, 405(6783), 243–253. https://doi.org/10.1038/35012251
- Moilanen, A., Franco, A. M. A., Early, R. I., Fox, R., Wintle, B., & Thomas, C. D. (2005). Prioritizing multiple-use landscapes for conservation: methods for large multi-species planning problems. *Proceedings of the Royal Society B*, 272(1575), 1885–1891. https://doi.org/10.1098/rspb.2005.3164
- Moilanen, A., Lehtinen, P., Kohonen, I., Jalkanen, J., Virtanen, E. A., & Kujala, H. (2022). Novel methods for spatial prioritization with applications in conservation, land use planning and ecological impact avoidance. *Methods in Ecology and Evolution*, 13(5), 1062–1072. https://doi.org/10.1111/2041-210X.13819
- Moilanen, A., Wilson, K. A., & Possingham, H. P. (Eds.) (2009). *Spatial Conservation Prioritization: Quantitative Methods and Computational Tools*. Oxford University Press. https://doi.org/10.1093/oso/9780199547760.001.0001
- Montesino Pouzols, F., Toivonen, T., Di Minin, E., Kukkala, A. S., Kullberg, P., Kuusterä, J., Lehtomäki, J., Tenkanen, H., Verburg, P. H., & Moilanen, A. (2014). Global protected area expansion is compromised by projected land-use and parochialism. *Nature*, 516(7531), 383–386. https://doi.org/10.1038/nature14032
- Naidoo, R., Balmford, A., Ferraro, P. J., Polasky, S., Ricketts, T. H., & Rouget, M. (2006). Integrating economic costs into conservation planning. *Trends in Ecology & Evolution*, 21(12), 681–687. https://doi.org/10.1016/j.tree.2006.10.003
- Pressey, R. L. (1994). Ad hoc reservations: forward or backward steps in developing representative reserve systems? *Conservation Biology*, 8(3), 662–668. https://doi.org/10.1046/j.1523-1739.1994.08030662.x
- Pressey, R. L., Humphries, C. J., Margules, C. R., Vane-Wright, R. I., & Williams, P. H. (1993). Beyond opportunism: key principles for systematic reserve selection. *Trends in Ecology & Evolution*, 8(4), 124–128. https://doi.org/10.1016/0169-5347(93)90023-I
- Rodrigues, A. S. L., Andelman, S. J., Bakarr, M. I., Boitani, L., Brooks, T. M., Cowling, R. M., et al. (2004). Effectiveness of the global protected area network in representing species diversity. *Nature*, 428(6983), 640–643. https://doi.org/10.1038/nature02422
- Rodrigues, A. S. L., & Gaston, K. J. (2002). Optimisation in reserve selection procedures: why not? *Biological Conservation*, 107(1), 123–129. https://doi.org/10.1016/S0006-3207(02)00042-3
- Strassburg, B. B. N., Iribarrem, A., Beyer, H. L., Cordeiro, C. L., Crouzeilles, R., Jakovac, C. C., et al. (2020). Global priority areas for ecosystem restoration. *Nature*, 586(7831), 724–729. https://doi.org/10.1038/s41586-020-2784-9
- Underhill, L. G. (1994). Optimal and suboptimal reserve selection algorithms. *Biological Conservation*, 70(1), 85–87. https://doi.org/10.1016/0006-3207(94)90302-6
