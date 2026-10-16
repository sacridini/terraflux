---
title: Ranking a landscape with Zonation
short: 2. Zonation
description: Zonation 5's priority ranking in Terraflux - how cells are removed by marginal loss, how to read rank maps and performance curves, and how weights, costs, protected areas, connectivity and habitat condition change the answer.
order: 20
level: intermediate
time: 60 min
ops: [prioritize.zonation, prioritize.zonation_project, raster.read, stack.of, view.plot]
series: Systematic conservation planning
part: 2
---

A ranking gives every cell of a landscape a priority, from the first to give up to the last to keep. It needs no
target and no budget, and the best 10 % lies inside the best 30 %, so one map answers every "what if we could
protect *x* %?" at once. In this part you will rank Serra Verde with Zonation, read its rank map and performance
curves with a dashboard, see what the marginal loss rules, costs, weights, existing reserves, connectivity and
habitat condition do to the ranking, and run the same analysis as a Zonation 5 project.

## The idea

Zonation was developed by Atte Moilanen and colleagues at the University of Helsinki to rank very large maps of
many species, where optimization was then out of reach (Moilanen et al., 2005). Zonation 5, the current version,
added a faster algorithm and many new analyses (Moilanen et al., 2022). [[prioritize.zonation]] gives the same rank
maps as the `z5` program.

### Removing the landscape one cell at a time

Zonation builds the ranking **backwards**. It starts with the whole landscape and removes cells one after
another, always the cell whose loss would hurt the features least, *given what is still left*. The first cells to
go get rank 0; the last ones, the core that could not be given up, get ranks near 1.

The loss of a cell is measured relative to what remains of each feature. Every feature \(j\) is first scaled so that
its values sum to 1 over the landscape: \(q_{ij}\) is the share of feature \(j\) in cell \(i\), and \(Q_j\) the share
of it still in the remaining cells (1 at the start). A cell holding 1 % of a feature takes 1 % of it at the start,
but a third of what is left once only 3 % remains. This is how Zonation keeps complementarity: as a feature
shrinks, the cells that still hold it become more and more valuable.

!!! science "Key idea: the marginal loss rules"
    With \(w_j\) the weight of feature \(j\), \(c_i\) the cost of cell \(i\), and the relative loss of feature \(j\)
    when cell \(i\) goes, \(w_j q_{ij} / Q_j\), the **rules** combine the features' losses into the cell's marginal
    loss \(\delta_i\):

    \[ \textbf{CAZMAX:}\quad \delta_i = \frac{1}{c_i} \max_j \frac{w_j q_{ij}}{Q_j} \qquad\qquad
       \textbf{CAZ1:}\quad \delta_i = \frac{1}{c_i} \sum_j \frac{w_j q_{ij}}{Q_j} \]

    \[ \textbf{CAZ2:}\quad \delta_i = \frac{1}{c_i} \sqrt{\sum_j \Big(\frac{w_j q_{ij}}{Q_j}\Big)^2} \qquad\qquad
       \textbf{ABF:}\quad \delta_i = \frac{1}{c_i} \sum_j w_j \big[Q_j^{z} - (Q_j - q_{ij})^{z}\big] \]

    The cell with the smallest \(\delta_i\) goes next. **CAZMAX**, the *core-area* rule of the original Zonation,
    values a cell by its single most important feature: it protects the core of every feature's distribution, even
    the rarest. **CAZ1** adds the losses, so cells with many features rise. **CAZ2**, Zonation 5's default, sits in
    between: large single losses dominate, but several moderate ones still count. The three are the same formula,
    a p-norm of the relative losses with p = ∞, 1 and 2 (`CAZP` takes any p). **ABF**, the *additive benefit
    function* (Arponen et al., 2005; Moilanen, 2007), assumes that a feature's value grows with the share kept as
    \(Q_j^{z}\), with diminishing returns: with \(z = 0.25\), keeping half of a species gives 84 % of its value.

In the four-site example of [part 1](/tutorials/spatial-prioritization/systematic-conservation-planning/),
site D holds a third of species 1 and nothing else (a core-area loss of 1/3), B half of species 2 (1/2), and A
and C a whole species each (1). D goes first. Species 1 now has \(Q_1 = 2/3\) left, so B's loss is
max((1/3)/(2/3), 1/2) = 1/2, and B goes next. A and C remain: the complementary pair.

### Sorting until the order holds

Recomputing every loss after every removal is slow on millions of cells. Zonation 5 turns the problem around:
given an order of removal, it knows how much of every feature remains when each cell goes, and so every cell's
loss at that moment. Sorting the cells by those losses gives a new order; Zonation repeats until the order stops
changing (the mean **ranking error** falls below `ranking_error`, 0.01 % by default, or after `max_iterations`).
Serra Verde needs 18 sorts.

### The rank map and the performance curves

The **rank map** gives every cell a value from 0 to 1. The **top fraction** of the landscape is a threshold on it:
the best 17 % are the cells ranked 0.83 or more. The **performance curves** follow every feature as the landscape is
removed: for every rank, the share of the feature still in the remaining cells. They answer the practical
question "if the best 30 % were protected, how much of each species would it hold?" The **area under the curve**
(AUC) of the mean of the features sums up a whole ranking in one number, to compare alternatives.

## When to use it

- **While targets are still negotiated**: the curves show what 10, 17, 30 or 50 % of the land would hold for
  every feature, so a group can discuss trade-offs before fixing a target.
- **Very large maps**: millions of cells and thousands of features (national and continental analyses, such as the
  global analysis of protected area expansion by Montesino Pouzols et al., 2014).
- **Impact avoidance and land use planning**: which places development should avoid, and what a project's
  footprint would take from every feature (Moilanen et al., 2022).
- **Expanding an existing network**: rank the rest of the landscape for what the reserves lack.

It is *not* the tool when the plan must meet exact targets at the least cost, when the plan's shape must obey
rules (contiguity, corridors), or when actions differ between places. Use optimization then:
[Marxan](/tutorials/spatial-prioritization/marxan/) or [prioritizr](/tutorials/spatial-prioritization/prioritizr/).
Lehtomäki & Moilanen (2013) describe a full Zonation workflow, from data to interpretation.

## What you need

- **Features**: a raster per feature on one grid (a stack). Zonation works with any non-negative amounts:
  probabilities from species distribution models, presences, habitat areas. Kujala et al. (2018) show that the
  kind of data (ranges, models, records) changes the ranking more than its amount: use the best distribution data
  you can.
- **Optionally a cost** raster on the same grid (no value counts as 0), a **hierarchic mask** (existing reserves,
  levels 0 to 255), an **analysis area** (the cells to rank), **condition** maps (0 to 1).
- Serra Verde's rasters ([serra-verde.zip](/data/tutorials/spatial-prioritization/systematic-conservation-planning/serra-verde.zip),
  from part 1).
- The template **Templates ▸ Conservation and restoration ▸ Zonation** ranks four random features with a cost.

## Build the model

### A first ranking

```tfgraph
#! caption: Zonation's ranking of Serra Verde's eight features with the cost of land.
#! numbered: true
#! highlight: zonation
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
id = "zonation"
op = "prioritize.zonation"
inputs = { features = "features.raster", cost = "cost.raster" }
params = { rule = "CAZ2" }

[[nodes]]
id = "show"
op = "view.plot"
inputs = { value = "zonation.rank" }
params = { title = "Priority rank" }

[outputs.curves]
from = "zonation.curves"
```

<div class="steps" markdown>

1. **The features** (node 1): the eight bands of `features.tif`, a stack on the `stack` port.
2. **The cost** (node 2): its `map` port goes to Zonation's `cost` input. With a cost, the marginal loss of a cell is
   divided by its cost, so cheap cells that hold a little of everything rise. Without one, every cell costs the
   same, and the ranking is by area.
3. **Zonation** (node 3): [[prioritize.zonation]] with `rule = "CAZ2"` (the default). Its outputs: `rank` (a map,
   0 to 1), `curves` (a table: the rank, the cost still remaining and the share of every feature remaining at each
   step), `features` (each feature's weight, sum, number of cells and the error of storing it in 8 bits), `auc`
   and `iterations` (numbers).
4. **Show the rank** (node 4) with [[view.plot]]; the curves are an output of the model.

</div>

!!! app "In the application"
    In the library, *Spatial prioritization* holds `prioritize.zonation`. Connect the features' `stack` to its
    `features` port and the cost's `map` to `cost`, choose the rule in the **Node** pane, and press
    ++ctrl+enter++. The rank map appears under the canvas; select the `curves` port to see the table, sort it, or
    draw it as lines. **Templates ▸ Conservation and restoration ▸ Zonation** is the same model with random maps.

From the command line, `terraflux run zonation.toml --out output` writes `output/curves.csv`; add a
[[raster.write]] node to keep the rank map as a GeoTIFF.

### The full model

Most real analyses add existing reserves, condition and connectivity. The settings that take a value per feature
(`weights`, `condition`, `connectivity`, `retention`...) are lists in the order of the features' layers.

```tfgraph
#! caption: A ranking that keeps the protected areas on top, discounts degraded habitat and values habitat with habitat around it (a negative exponential kernel of 2 cells for every feature), with the tamarin and the mangrove weighted 3.
#! numbered: true
#! highlight: zonation, conditions
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

[[nodes]]
id = "condition"
op = "raster.read"
params = { path = "condition.tif" }

[[nodes]]
id = "conditions"
op = "stack.of"
inputs = { condition = "condition.raster" }
note = "Condition maps are found by name."

[[nodes]]
id = "zonation"
op = "prioritize.zonation"
inputs = { features = "features.raster", cost = "cost.raster", hierarchic_mask = "protected.raster", condition_maps = "conditions.stack" }
params = { rule = "CAZ2", weights = [1, 1, 1, 1, 3, 1, 1, 3], condition = ["condition", "condition", "condition", "condition", "condition", "condition", "condition", "condition"], connectivity = [{ shape = "negexp", distance = 2 }, { shape = "negexp", distance = 2 }, { shape = "negexp", distance = 2 }, { shape = "negexp", distance = 2 }, { shape = "negexp", distance = 2 }, { shape = "negexp", distance = 2 }, { shape = "negexp", distance = 2 }, { shape = "negexp", distance = 2 }] }

[[nodes]]
id = "show"
op = "view.plot"
inputs = { value = "zonation.rank" }
```

- **`hierarchic_mask`** (node 3 into node 6): cells of a higher level are removed only after all cells of lower
  levels. With the protected areas at level 1, they rank on top and the rest of the landscape is ranked for what
  they lack.
- **`condition_maps`** and **`condition`** (nodes 4 and 5): every feature's values are multiplied by the named
  condition map (Zonation's `-c`). [[stack.of]] names the layer after its input, `condition`.
- **`connectivity`**: every feature smoothed by a dispersal kernel before the ranking (Zonation's `-d`), so a cell
  is worth the habitat around it.
- **`weights`**: the tamarin (5th) and the mangrove (8th) count three times.

## Read the results

![The rank map of Serra Verde.](/img/tutorials/spatial-prioritization/zonation/rank.png "Zonation's rank map of Serra Verde (CAZ2, with costs). The last cells kept (yellow) are the tamarin's two fragments, the coast with the mangrove, the two rivers and the western mountains. The interior east of the rivers, which holds little of any feature, goes first.")

The dashboard below protects the best fraction of the landscape. Move the slider: the map shows the cells kept
(green) over the rest of the ranking (grey, darker for higher ranks); the chart, the performance curves, with a
line at the fraction chosen.

```tfslider
label: Best fraction kept
caption: "The best fractions of Serra Verde by Zonation's ranking, and how much of every feature each holds (the performance curves, from the `curves` table; toggle features in the legend). Rare features in distinct places (tamarin, mangrove, otter) are almost entirely in the best 17 to 30 %; the widespread jaguar and tapir trail behind."
frames:
  - { value: "5 %", src: /img/tutorials/spatial-prioritization/zonation/top_5.png, stats: { "Mean of features": "18.5 %", "Worst off": "6.4 % (tapir)", "Cost": "3.6 %" } }
  - { value: "10 %", src: /img/tutorials/spatial-prioritization/zonation/top_10.png, stats: { "Mean of features": "34.0 %", "Worst off": "12.8 % (tapir)", "Cost": "7.5 %" } }
  - { value: "17 %", src: /img/tutorials/spatial-prioritization/zonation/top_17.png, stats: { "Mean of features": "50.5 %", "Worst off": "23.4 % (tapir)", "Cost": "13.0 %" } }
  - { value: "20 %", src: /img/tutorials/spatial-prioritization/zonation/top_20.png, stats: { "Mean of features": "56.4 %", "Worst off": "27.8 % (tapir)", "Cost": "15.4 %" } }
  - { value: "30 %", src: /img/tutorials/spatial-prioritization/zonation/top_30.png, stats: { "Mean of features": "71.3 %", "Worst off": "43.7 % (jaguar)", "Cost": "23.2 %" } }
  - { value: "40 %", src: /img/tutorials/spatial-prioritization/zonation/top_40.png, stats: { "Mean of features": "82.2 %", "Worst off": "57.9 % (jaguar)", "Cost": "31.0 %" } }
  - { value: "50 %", src: /img/tutorials/spatial-prioritization/zonation/top_50.png, stats: { "Mean of features": "90.3 %", "Worst off": "69.8 % (jaguar)", "Cost": "39.9 %" } }
  - { value: "60 %", src: /img/tutorials/spatial-prioritization/zonation/top_60.png, stats: { "Mean of features": "96.7 %", "Worst off": "82.2 % (jaguar)", "Cost": "49.2 %" } }
  - { value: "70 %", src: /img/tutorials/spatial-prioritization/zonation/top_70.png, stats: { "Mean of features": "99.6 %", "Worst off": "96.7 % (jaguar)", "Cost": "57.8 %" } }
  - { value: "80 %", src: /img/tutorials/spatial-prioritization/zonation/top_80.png, stats: { "Mean of features": "100 %", "Worst off": "100 %", "Cost": "73.7 %" } }
  - { value: "90 %", src: /img/tutorials/spatial-prioritization/zonation/top_90.png, stats: { "Mean of features": "100 %", "Worst off": "100 %", "Cost": "88.1 %" } }
start: 2
chart:
  type: line
  data: /data/tutorials/spatial-prioritization/zonation/top-fractions.csv
  x: top
  y: [Jaguar, Tapir, Harpy eagle, Maned wolf, Tamarin, Otter, Araucaria, Mangrove]
  colors: { Jaguar: "#0072b2", Tapir: "#e69f00", "Harpy eagle": "#009e73", "Maned wolf": "#cc79a7", Tamarin: "#56b4e9", Otter: "#d55e00", Araucaria: "#f0e442", Mangrove: "#785ef0" }
  xlabel: Best fraction of the land kept (%)
  ylabel: "% of the feature kept"
  height: 320
```

How to read it:

- **The steep part of a curve** is where the landscape cannot be given up without losing the feature. The
  tamarin's curve is flat at 100 % down to the best 30 % and then plunges: its two fragments are among the last
  cells. The jaguar's curve falls steadily: its habitat is spread over half the region and no part of it is
  irreplaceable on its own.
- **The worst-off feature** at a fraction tells you whether a given area target leaves something behind. At 17 %
  of the land (the Aichi target of 2010), the best cells hold half of the features on average, but only 23 % of the
  tapir's habitat; at 30 % (the 2030 target of the Kunming-Montreal framework), at least 44 % of every feature.
- **The cost column** of the `curves` table is the cost of the cells *still remaining* at each rank (the total at
  rank 0). The best 17 % costs 13 % of the region's total: the ranking prefers cheap land where it serves as well.
- **The rank is a priority, not a plan.** A threshold of the rank map gives a candidate network for an area
  target, but it does not promise any feature's target; the curves say what it holds.

The whole curves, every 1 % of the cells, with their mean, are in
[curves.csv](/data/tutorials/spatial-prioritization/zonation/curves.csv):

```tfchart
type: line
data: /data/tutorials/spatial-prioritization/zonation/curves.csv
x: top
y: [Mean, Tamarin, Jaguar]
colors: { Mean: "#1c2420", Tamarin: "#56b4e9", Jaguar: "#0072b2" }
xlabel: Best fraction of the land kept (%)
ylabel: "% of the feature kept"
height: 260
caption: "The mean performance curve and two contrasting features. The area under the mean curve, as a share of the whole square, is Zonation's AUC: 0.78 for this ranking."
```

### Reporting a ranking

In a paper, report: the rule (and z for ABF), the features and their weights, the cost layer, the hierarchic mask
and any transforms (condition, connectivity kernels with their shapes and distances), and Zonation's version (here
the Zonation 5 algorithm in Terraflux). Show the rank map with a colour ramp from 0 to 1 and the performance
curves, and give a table of the share of every feature in the top fractions that matter for the decision (often
10, 17 and 30 %). The `features` output gives every feature's sum, cells and quantization error, worth a line in the
supplementary material.

## Settings that change the answer

### The rule

![Four rules on the same data.](/img/tutorials/spatial-prioritization/zonation/rules.png "The rank maps of four marginal loss rules, all with costs. They agree on the core (tamarin, coast, rivers, mountains) and differ in the middle ranks.")

```tfchart
type: bar
data: /data/tutorials/spatial-prioritization/zonation/rules-top17.csv
x: feature
y: [CAZMAX, CAZ1, CAZ2, ABF]
colors: { CAZMAX: "#0072b2", CAZ1: "#e69f00", CAZ2: "#2e7d4f", ABF: "#cc79a7" }
ylabel: "% kept in the best 17 %"
height: 290
caption: "The share of every feature in the best 17 % under each rule. Averages are close (48 to 51 %), but the rules share the land differently. CAZMAX keeps 96 % of the tamarin and 94 % of the mangrove, and less of the features that overlap in the mountains (harpy eagle, maned wolf, araucaria); CAZ1 and ABF do the reverse and let the tapir fall to 16 to 19 %. CAZ2 sits between."
```

Use **CAZ2** (or CAZMAX) when every feature matters, rare ones included: for threatened species and when a few
features must not be left behind. Use **ABF** when the goal is the most *total* value, accepting that a few
features fall behind, for instance for ecosystem services or very many species where the sum matters. CAZ1 lies
in between, pulled towards places with many features. The AUCs (0.778 to 0.784) hide these differences: always look
at the worst-off features too.

### Costs

![The ranking without and with costs.](/img/tutorials/spatial-prioritization/zonation/cost.png "Left: every cell costs the same, so the ranking is about area. Right: with the cost of land, dear cells near the cities and the highway fall in the ranking, and cheap cells in the mountains rise.")

```tfchart
type: line
data: /data/tutorials/spatial-prioritization/zonation/cost-curves.csv
x: budget
y: [Without costs, With costs]
colors: { "Without costs": "#e8901f", "With costs": "#2e7d4f" }
xlabel: Budget (% of the cost of the whole region)
ylabel: "Mean % of the features"
height: 260
caption: "What each budget buys: the best cells of each ranking, taken in order of rank until the money runs out, and the mean share of the features they hold. The ranking with costs buys more for the same money: 65.9 % against 62.4 % with a fifth of the budget."
```

When the decision is about money (buying land, paying owners), give Zonation the cost and read the curves against
the cost; when it is about area (a share of a country under protection), leave the cost out. `cost_in_ranking =
false` keeps the cost in the curves without letting it change the order (Zonation's `-x`).

### Weights

Weights multiply a feature's losses. With the tamarin and the mangrove weighted 3 and the others 1, the best 10 %
holds 93 % of the tamarin's habitat (instead of 55 %) and 90 % of the mangrove (instead of 68 %), while every other
feature loses 2 to 6 points. Weights express **responsibility** (a region's share of a species' world range),
**threat status** (an IUCN category), or **policy**. A negative weight marks something to avoid (a threat, an
invasive species): its cells are removed early. `weight_groups` and `group_weights` give a weight to a group of
features (all birds, all ecosystem services) shared among them.

### What is already protected

![The ranking with the protected areas as a hierarchic mask.](/img/tutorials/spatial-prioritization/zonation/mask.png "The protected areas (outlined in white) in the hierarchic mask: they rank on top (yellow), and the rest of the landscape is ranked for what they lack.")

The protected areas cover 3.6 % of Serra Verde. The performance curves at that fraction are a **gap analysis**:
they hold 4 % of the jaguar's habitat, 7 % of the harpy eagle's and of the araucaria forest, 10 % of the otter's,
and **nothing** of the tamarin, the mangrove and almost nothing of the tapir. The cells ranked just below the mask
are the best extensions of the network.

```tfchart
type: bar
data: /data/tutorials/spatial-prioritization/zonation/gap-analysis.csv
x: feature
y: [Protected areas, Protected + best extension to 17 %]
colors: { "Protected areas": "#785ef0", "Protected + best extension to 17 %": "#2e7d4f" }
ylabel: "% of the feature"
height: 270
caption: "A gap analysis and its remedy: what the existing protected areas hold, and what they hold with the best extension to 17 % of the land, both from the curves of the masked ranking."
```

### Connectivity

A cell of forest surrounded by forest is worth more to a forest species than the same cell alone in a field.
Zonation smooths every feature with a **dispersal kernel** before ranking (Moilanen et al., 2005), so that each cell
receives value from the habitat around it, at the scale the species moves. The `connectivity` setting takes, per
feature, a kernel: `shape` (`uniform`, `triangular`, `negexp` or `gaussian`), `distance` in cells, and an optional
`cut`.

```tfslider
label: Kernel distance (cells of 1 km)
caption: "The best 17 % (green) with every feature smoothed by a negative exponential kernel. As the distance grows, thin strips (the rivers, the coast) give way to compact blocks of habitat: the outline of the network shrinks from 616 to 448 km, at a cost in the raw habitat held."
frames:
  - { value: 0, src: /img/tutorials/spatial-prioritization/zonation/conn_0.png, stats: { "Outline": "616 km", "Mean of features": "50.5 %", "Worst off": "23.4 %" } }
  - { value: 1, src: /img/tutorials/spatial-prioritization/zonation/conn_1.png, stats: { "Outline": "590 km", "Mean of features": "50.3 %", "Worst off": "23.5 %" } }
  - { value: 2, src: /img/tutorials/spatial-prioritization/zonation/conn_2.png, stats: { "Outline": "558 km", "Mean of features": "50.2 %", "Worst off": "23.0 %" } }
  - { value: 4, src: /img/tutorials/spatial-prioritization/zonation/conn_4.png, stats: { "Outline": "514 km", "Mean of features": "49.0 %", "Worst off": "22.8 %" } }
  - { value: 8, src: /img/tutorials/spatial-prioritization/zonation/conn_8.png, stats: { "Outline": "448 km", "Mean of features": "44.5 %", "Worst off": "18.9 %" } }
```

The numbers are those of the *raw* features in the best 17 %: smoothing trades a little representation now for
larger, more connected areas that should hold their species longer. Choose the distance from the species'
dispersal (a few hundred metres for a frog, tens of kilometres for a jaguar): it can differ per feature. Zonation 5
also has **interactions** between features (`interaction`, a predator and its prey) and **matrix connectivity**
(`matrix_connectivity` with `matrices`: features of related habitats counting for one another).

### Habitat condition

![Habitat condition and its effect on the ranking.](/img/tutorials/spatial-prioritization/zonation/condition.png "Left: the condition of the habitat, low along the highway, around the cities and the coast. Right: the rank with every feature multiplied by the condition, minus the rank without. Cells in degraded habitat drop (blue), and good habitat elsewhere rises a little (red).")

The `condition` setting names, per feature, a layer of `condition_maps` with values from 0 to 1 that multiply it:
a cell of habitat in poor condition counts for less. `renormalize` brings a conditioned feature back to its full
weight. **Retention** (`retention`) works the other way: the share of a feature that *stays* in a cell even when it
is lost to the network (a production forest keeps part of its birds), everywhere or as a map.

### Impact assessment

Zonation 5 can also tell what a development would take. An `impact` scenario gives, in `impact_maps`, the places
of the projects (sources) and per group of features their footprint and buffers of decreasing loss; the outputs
`impact_zones` and `impact_maps` give every feature's loss in the footprint and in each buffer zone, and
`cumulative_impact` combines several pressures. This is Zonation's tool for impact avoidance and offsets
(Moilanen et al., 2022); the operator reference lists every field.

### Ties and plateaus

Cells with exactly the same values have the same loss, and like any sort Zonation orders them by position. Large
uniform areas (every cell of a land cover class worth the same) get arbitrary stripes in the rank map; ranks within
such ties mean nothing. Serra Verde shows one: the straight bands in the far north-east, where no feature lives.
Smoothing (connectivity) or any more informative feature breaks ties. The `flexibility` output shows how far each
cell could move in the order before its loss differs by a given error.

## Zonation 5 projects

Analyses prepared for the `z5` program run as they are with [[prioritize.zonation_project]]: a settings file
(`.z5`), its feature list and link files, with the switches of the `z5` command line. Here is Serra Verde as a
project ([serra-verde-z5.zip](/data/tutorials/spatial-prioritization/zonation/serra-verde-z5.zip)):

```text
# settings.z5
feature list file = features.txt
hierarchic mask layer = protected.tif
cost layer = cost.tif
```

```text
# features.txt: a weight, a file and a name per feature
weight filename name
1 features/jaguar.tif jaguar
1 features/lowland_tapir.tif lowland_tapir
...
3 features/golden_lion_tamarin.tif golden_lion_tamarin
...
3 features/mangrove.tif mangrove
```

```tfgraph
#! caption: A Zonation 5 project run as z5 runs it: weights (-w), the hierarchic mask (-h) and the cost in the ranking (-X), with CAZ2.
#! highlight: project
format = "terraflux/1"

[[nodes]]
id = "project"
op = "prioritize.zonation_project"
params = { path = "settings.z5", flags = "-whX", mode = "CAZ2" }

[[nodes]]
id = "show"
op = "view.plot"
inputs = { value = "project.rank" }
```

The `flags` are those of `z5`: `-w` weights, `-W` weight groups, `-c` conditions, `-r` retention, `-x`/`-X` the cost
tracked or in the ranking, `-h` hierarchic mask, `-a` analysis area, `-d`/`-i`/`-I`/`-m` connectivity, `-s`/`-e`
impact scenarios, `-p` a post-processing file. `mode` is z5's `--mode` (`CAZ2`, `"ABF 0.25"`...). The project above
gives exactly the rank map of [[prioritize.zonation]] with the same inputs (the largest difference between the two
maps is 0), which the figure script checks.

## Common pitfalls

- **Reading a threshold of the rank as a plan that meets targets.** The best 17 % holds what the curves say, no
  more. If targets matter, check them with the curves, or use optimization.
- **Features with no data marked as zero, or zero as no data.** `zero_mode` says whether zeros are data;
  `nodata` per feature gives a file's no-data value. A cell with no value in the features is not ranked.
- **Features with long tails.** Zonation stores features in 8 bits by default; the `features` table reports the
  quantization error per feature. If it is large (a feature with a few very high values), use `bits = 16`.
- **Extent effects.** A species rare in the study area may be common next door; the ranking favours the edges of
  ranges. Weight by the share of the global range the region holds, or rank in the ecological context.
- **Comparing AUCs only.** Rankings with close AUCs can treat the rarest features very differently: compare the
  worst-off curves too.
- **Costs and area mixed up.** With a cost, the curves' "fraction of the landscape" is still a fraction of cells;
  read them against the `cost` column to speak of budgets.

## Going further

- Part 3, [Reserves with Marxan, and irreplaceability](/tutorials/spatial-prioritization/marxan/): sets of reserves
  for targets, by simulated annealing.
- Part 6, [Evaluating, comparing and reporting plans](/tutorials/spatial-prioritization/evaluating-plans/):
  Zonation's best fraction against an optimized plan of the same cost.
- The habitat around a cell can also be measured with the tools of
  [Habitat networks](/tutorials/habitat-networks/) and [Circuit connectivity](/tutorials/circuit-connectivity/).
- The operators: [[prioritize.zonation]], [[prioritize.zonation_project]].

## References

- Arponen, A., Heikkinen, R. K., Thomas, C. D., & Moilanen, A. (2005). The value of biodiversity in reserve selection: representation, species weighting, and benefit functions. *Conservation Biology*, 19(6), 2009–2014. https://doi.org/10.1111/j.1523-1739.2005.00218.x
- Kujala, H., Lahoz-Monfort, J. J., Elith, J., & Moilanen, A. (2018). Not all data are equal: influence of data type and amount in spatial conservation prioritisation. *Methods in Ecology and Evolution*, 9(11), 2249–2261. https://doi.org/10.1111/2041-210X.13084
- Lehtomäki, J., & Moilanen, A. (2013). Methods and workflow for spatial conservation prioritization using Zonation. *Environmental Modelling & Software*, 47, 128–137. https://doi.org/10.1016/j.envsoft.2013.05.001
- Moilanen, A. (2007). Landscape Zonation, benefit functions and target-based planning: unifying reserve selection strategies. *Biological Conservation*, 134(4), 571–579. https://doi.org/10.1016/j.biocon.2006.09.008
- Moilanen, A., Franco, A. M. A., Early, R. I., Fox, R., Wintle, B., & Thomas, C. D. (2005). Prioritizing multiple-use landscapes for conservation: methods for large multi-species planning problems. *Proceedings of the Royal Society B*, 272(1575), 1885–1891. https://doi.org/10.1098/rspb.2005.3164
- Moilanen, A., Lehtinen, P., Kohonen, I., Jalkanen, J., Virtanen, E. A., & Kujala, H. (2022). Novel methods for spatial prioritization with applications in conservation, land use planning and ecological impact avoidance. *Methods in Ecology and Evolution*, 13(5), 1062–1072. https://doi.org/10.1111/2041-210X.13819
- Montesino Pouzols, F., Toivonen, T., Di Minin, E., Kukkala, A. S., Kullberg, P., Kuusterä, J., Lehtomäki, J., Tenkanen, H., Verburg, P. H., & Moilanen, A. (2014). Global protected area expansion is compromised by projected land-use and parochialism. *Nature*, 516(7531), 383–386. https://doi.org/10.1038/nature14032
