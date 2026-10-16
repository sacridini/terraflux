---
title: Compact and connected plans, and zones
short: 5. Spatial design
description: Give a plan a shape - boundary penalties and their calibration, neighbour and contiguity constraints, connectivity along a species' habitat, hexagons of H3 as planning units, and management zones with their own costs and contributions - with prioritize.optimize.
order: 50
level: advanced
time: 60 min
ops: [prioritize.optimize, prioritize.evaluate, prioritize.coarsen, prioritize.refine, prioritize.connectivity_matrix, prioritize.h3_units, prioritize.boundary_matrix, prioritize.adjacency_matrix, stack.of, map.calc]
series: Systematic conservation planning
part: 5
---

The plans of [part 4](../prioritizr/) meet every target at the least cost, and nothing else. Nothing tells the solver
that two neighbouring cells are worth more than two cells far apart, so the plans come in 30 pieces, some of them a
single square kilometre. In this part you will give plans a shape: compact with a **boundary penalty** (and calibrate
it with a dashboard), free of isolated cells with **neighbour constraints**, in one piece with **contiguity**, and
connected along a species' habitat with a **connectivity penalty**. Then you will change the planning units themselves,
to **H3 hexagons**, and let a plan choose between two **management zones**, strict protection and sustainable use.

## The idea

### Why shape matters

Reserves are not abstract sets of cells. A fragmented plan has more **edge**, exposed to wind, fire, hunting and
invasive species; small pieces hold small populations that wink out; every separate piece needs its own fences,
patrols and negotiations; and animals must cross the land between them. Spatial reserve design asks for plans that are
**compact** (little edge for their area) and **connected** (pieces close together or linked), at the least extra cost
(Williams, ReVelle & Levin, 2005). The question is always how much compactness is worth: a plan of one round block
is easy to manage and may miss half the features.

### Penalties and constraints

There are two ways to ask for a shape, and they behave differently:

- a **penalty** adds a price to the objective for what you do not want (boundary, broken links). The solver
  trades it against the cost: a preference, as strong as its weight. Its value must be **calibrated**;
- a **constraint** is a rule the plan must obey (every selected cell has two selected neighbours; the plan is one
  piece). No value to calibrate, but every rule raises the cost, and some make the problem much harder to solve.

!!! science "Key idea: the boundary penalty"
    With \(x_i\) the decision for unit \(i\), \(b_{ik}\) the length of the boundary units \(i\) and \(k\) share, and
    \(e_i\) the length of unit \(i\)'s edges that face no unit (the sea, the edge of the map), the minimum set with a
    boundary penalty \(p\) is

    \[ \min \sum_i c_i x_i + p \Big( \sum_i \epsilon\, e_i x_i + \sum_{i<k} b_{ik}\,(x_i + x_k - 2 x_i x_k) \Big) \]

    The bracket is the **boundary of the plan**: an edge between a selected and an unselected unit counts, an edge
    between two selected units does not. \(\epsilon\) is the `edge_factor` (0.5 by default): how much the edges to
    nothing count. The product \(x_i x_k\) is made linear with an extra variable per pair, so the problem stays an
    integer program (Beyer et al., 2016). Marxan's BLM is the same idea ([part 3](../marxan/)).

Constraints on shape: **neighbours** (every selected unit has at least \(k\) selected neighbours) removes isolated
cells; **contiguity** (all selected units form one connected patch, Önal & Briers, 2006) forces a single network;
**feature contiguity** asks it of every feature's units separately.

**Connectivity penalties** generalise the boundary: a matrix of how strongly each pair of units is linked (by
adjacency, by distance, by a dispersal probability, by the habitat between them), and a reward for selecting
strongly linked pairs together. Asymmetric versions express links with a direction: a reach of river upstream of
another (Beger et al., 2010; Hermoso et al., 2011).

### Planning units and zones

**Planning units** need not be cells. Properties, watersheds or hexagons are often better: they match how land is
managed, and hexagons have equal areas and six equal neighbours. **H3** is a global grid of hexagons at sixteen
resolutions (resolution 7 cells cover about 5 km², resolution 6 about 36 km²). [[prioritize.h3_units]] covers a cost
map with them; [[prioritize.optimize]] takes any polygons as units and sums every feature over each one exactly.

**Management zones** let each unit take one of several uses, each with its own cost and its own value for the
features (Watts et al., 2009, Marxan with Zones; Hanson et al., 2025, prioritizr's zones). A feature may count fully
under strict protection, half under sustainable use; the use zone may cost less, since some income continues. The
plan decides, for every unit, which zone (or none).

## When to use it

- **Almost every plan on fine units** needs a boundary penalty: without it, plans scatter. Calibrate it.
- **Neighbour constraints** when isolated cells make no sense on the ground but several pieces are acceptable.
- **Contiguity** for small problems or few features close together, or when one network is a requirement (a
  corridor system); it is slow on large ones.
- **Connectivity penalties** when you know how a feature moves: rivers, dispersal distances, least-cost distances
  ([[prioritize.decay_connectivity]] turns any distances into probabilities).
- **Polygons or hexagons** when decisions are made on properties or management units, or for regional and global
  analyses (hexagons of equal area).
- **Zones** for zoning plans: marine spatial plans, land-use plans with protection, sustainable use and restoration.

## What you need

- The features and the cost of the previous parts:
  [serra-verde.zip](/data/tutorials/spatial-prioritization/systematic-conservation-planning/serra-verde.zip).
- For hexagons, a cost map in a known coordinate system (Serra Verde's is UTM zone 23 South).
- For zones, a cost map per zone and the zones' contributions to the features.
- The boundary and adjacency of raster cells are computed by [[prioritize.optimize]] itself; for other matrices,
  [[prioritize.boundary_matrix]], [[prioritize.adjacency_matrix]], [[prioritize.connectivity_matrix]],
  [[prioritize.proximity_matrix]] and [[prioritize.decay_connectivity]] make them as tables (`i`, `j`, `x`).

## Build the model

### A boundary penalty

```tfgraph
#! caption: The 30 % minimum set with a boundary penalty (node 3), and its evaluation (node 4), whose summary gives the plan's cost and boundary apart.
#! numbered: true
#! highlight: plan, evaluation
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
params = { objective = "min_set", relative_targets = 0.3, boundary_penalty = 0.01, edge_factor = 0.5, gap = 0.01 }

[[nodes]]
id = "evaluation"
op = "prioritize.evaluate"
inputs = { features = "features.raster", cost = "cost.raster", solution = "plan.solution" }
params = { relative_targets = 0.3 }

[[nodes]]
id = "show"
op = "view.plot"
inputs = { value = "plan.solution" }
```

<div class="steps" markdown>

1. **The penalty** (node 3): `boundary_penalty` is \(p\), in units of cost per unit of boundary. Costs are in thousand
   USD and the boundary in metres, so 0.01 is 10 USD per metre, 10 000 USD per kilometre of the plan's outline.
   `edge_factor` weighs the edges facing the sea or the map's edge.
2. **The evaluation** (node 4): with a penalty, the plan's `objective` is no longer its cost. [[prioritize.evaluate]]
   reports them apart: its `summary` has the units selected, the cost and the boundary.

</div>

The `spatial` setting takes several penalties and constraints at once, in order: `spatial = [{ kind = "boundary",
penalty = 0.01, edge_factor = 0.5 }, { kind = "neighbors", k = 1 }]`.

### Neighbours and contiguity

Constraints of shape add many variables (contiguity adds a flow through the network) and are best tried on coarser
units first. [[prioritize.coarsen]] sums the cost and the features over blocks of `factor` × `factor` cells,
[[prioritize.refine]] brings the coarse plan back to the fine grid:

```tfgraph
#! caption: A minimum set of 20 % on blocks of 3 x 3 km (nodes 3 and 4) in which every selected block has at least two selected neighbours, brought back to the 1 km grid (node 5).
#! numbered: true
#! highlight: blocks, plan, fine
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
params = { factor = 3 }

[[nodes]]
id = "plan"
op = "prioritize.optimize"
inputs = { features = "blocks.features", cost = "blocks.cost" }
params = { objective = "min_set", relative_targets = 0.2, neighbors = 2, gap = 0.01, time_limit = 120 }

[[nodes]]
id = "fine"
op = "prioritize.refine"
inputs = { selection = "plan.solution", like = "cost.raster" }
```

`neighbors = 2` asks for two selected neighbours (of the four sharing an edge) for every selected block; `clamp`
(true) lowers the number for blocks that have fewer neighbours, on the coast. `contiguity = true` asks for one patch
instead (with `adjacency_data` for polygons).

### Connectivity along a species' habitat

```tfgraph
#! caption: The tapir's habitat on the blocks (node 1), a connectivity matrix of the blocks weighted by it (node 2), and a plan rewarded for keeping well-linked blocks together (node 3). It continues the graph above.
#! fragment: true
#! numbered: true
#! highlight: links, plan
[[nodes]]
id = "tapir"
op = "map.calc"
inputs = { f = "blocks.features" }
params = { expression = "f.lowland_tapir" }

[[nodes]]
id = "links"
op = "prioritize.connectivity_matrix"
inputs = { units = "blocks.cost", y = "tapir.map" }

[[nodes]]
id = "plan"
op = "prioritize.optimize"
inputs = { features = "blocks.features", cost = "blocks.cost", connectivity_data = "links.table" }
params = { objective = "min_set", relative_targets = 0.2, connectivity_penalty = 0.005, gap = 0.01 }
```

[[prioritize.connectivity_matrix]] gives every pair of adjacent units the length of their shared boundary times the
mean of `y` in the two: here the tapir's habitat suitability, so the links run along its rivers. Its `table` (`i`,
`j`, `x`) goes to the `connectivity_data` input, and `connectivity_penalty` weighs it. A positive value **rewards**
selecting linked pairs together (prioritizr's sign convention), unlike the boundary penalty, which punishes the
outline. For one-way links, `asym_connectivity_data` and `asym_connectivity_penalty`.

### Hexagons

```tfgraph
#! caption: H3 hexagons of resolution 7 over Serra Verde (node 3) as planning units: the features summed over each hexagon, the minimum set of 30 %.
#! numbered: true
#! highlight: hexagons, plan
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
id = "hexagons"
op = "prioritize.h3_units"
inputs = { cost = "cost.raster" }
params = { resolution = 7 }

[[nodes]]
id = "plan"
op = "prioritize.optimize"
inputs = { polygons = "hexagons.units", features = "features.raster" }
params = { objective = "min_set", relative_targets = 0.3, cost_column = "cost", gap = 0.01 }

[outputs.selection]
from = "plan.selection"
```

[[prioritize.h3_units]] returns the hexagons as polygons (`units`, with their H3 index and their cost, the costs of the
cells under each weighted by the share of each cell inside it) and a `table`. Connect `units` to the `polygons` input
of [[prioritize.optimize]] and name the cost column (`cost_column = "cost"`): the features are summed over each
hexagon exactly, and the boundaries and adjacency come from the geometries. The plan is the `selection` table, a row
per hexagon in the order of the polygons; write the polygons with [[vector.write]] to map it. Any polygons work the
same way: properties, watersheds, municipalities read with [[vector.read]].

### Zones

```tfgraph
#! caption: Two management zones: strict protection at the full cost of the land, sustainable use at 45 % of it (nodes 3 and 4), in which the features count for half (node 5).
#! numbered: true
#! highlight: zone_costs, plan
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
id = "use_cost"
op = "map.calc"
inputs = { c = "cost.raster" }
params = { expression = "c * 0.45" }

[[nodes]]
id = "zone_costs"
op = "stack.of"
inputs = { protect = "cost.raster", use = "use_cost.map" }

[[nodes]]
id = "plan"
op = "prioritize.optimize"
inputs = { zone_costs = "zone_costs.stack", features = "features.raster" }
params = { objective = "min_set", relative_targets = 0.3, contribution = { use = 0.5 }, gap = 0.01 }

[[nodes]]
id = "show"
op = "view.plot"
inputs = { value = "plan.solution" }
```

<div class="steps" markdown>

1. **The zones' costs** (nodes 3 and 4): a stack with a layer per zone, **named by the zone** ([[stack.of]] names
   each layer after its input: `protect`, `use`). A cell with no value in a zone's layer cannot go to that zone.
2. **Contributions** (node 5): `contribution = { use = 0.5 }`: in the use zone, every feature counts for half its
   amount; zones not listed count fully. A list gives a value per feature; `contribution_maps` a raster per zone.
3. **Targets**: `relative_targets` applies to the sum over the zones. A target in one zone needs `manual_targets`,
   a list such as `{ feature = "jaguar", zone = "protect", type = "relative", sense = ">=", target = 0.2 }` (with
   one entry per feature and zone).
4. **Outputs**: `solution` is a map of the zone of every cell (1 the first zone, 2 the second, 0 none), `allocation`
   a stack with every zone's share.

</div>

`mandatory_allocation = true` puts every unit in some zone (for plans that zone a whole region); `boundary_zones` says
how much an edge between two zones counts in the boundary penalty.

## Read the results

### Compactness and its price

```tfslider
label: Boundary penalty (thousand USD per metre)
caption: "The 30 % minimum set as the boundary penalty grows (solved within 1 %). The cost of the plan and its boundary, from prioritize.evaluate (edges to the sea at half their length), and the number of patches (8-connected)."
frames:
  - { value: 0, src: /img/tutorials/spatial-prioritization/spatial-design/boundary_0.png, stats: { "Cost": "37.20 M USD", "Boundary": "490 km", "Patches": "30" } }
  - { value: 0.001, src: /img/tutorials/spatial-prioritization/spatial-design/boundary_0.001.png, stats: { "Cost": "37.26 M USD", "Boundary": "448 km", "Patches": "26" } }
  - { value: 0.002, src: /img/tutorials/spatial-prioritization/spatial-design/boundary_0.002.png, stats: { "Cost": "37.39 M USD", "Boundary": "416 km", "Patches": "24" } }
  - { value: 0.005, src: /img/tutorials/spatial-prioritization/spatial-design/boundary_0.005.png, stats: { "Cost": "37.40 M USD", "Boundary": "368 km", "Patches": "21" } }
  - { value: 0.01, src: /img/tutorials/spatial-prioritization/spatial-design/boundary_0.01.png, stats: { "Cost": "37.59 M USD", "Boundary": "340 km", "Patches": "13" } }
  - { value: 0.02, src: /img/tutorials/spatial-prioritization/spatial-design/boundary_0.02.png, stats: { "Cost": "38.71 M USD", "Boundary": "274 km", "Patches": "9" } }
  - { value: 0.05, src: /img/tutorials/spatial-prioritization/spatial-design/boundary_0.05.png, stats: { "Cost": "39.97 M USD", "Boundary": "227 km", "Patches": "5" } }
start: 4
chart:
  type: line
  data: /data/tutorials/spatial-prioritization/spatial-design/boundary.csv
  x: penalty
  y: ["Boundary (% of no penalty)", "Cost (% above the optimum)"]
  colors: { "Boundary (% of no penalty)": "#2f6f9f", "Cost (% above the optimum)": "#e8901f" }
  xlabel: Boundary penalty
  ylabel: "%"
```

Unlike Marxan's runs in part 3, the cost now only rises with the penalty: the solver finds the cheapest plan for every
value. The first steps are almost free: a penalty of 0.01 removes 31 % of the boundary and more than half the patches
for 1.2 % more cost. Beyond it, compactness gets dear: halving the boundary costs 7.6 %.

```tfchart
type: scatter
data: /data/tutorials/spatial-prioritization/spatial-design/boundary-tradeoff.csv
x: Boundary (km)
y: [Cost (million USD)]
colors: { "Cost (million USD)": "#2e7d4f" }
xlabel: Boundary of the plan (km)
ylabel: Cost (million USD)
height: 270
caption: "The trade-off between cost and boundary, a point per penalty (from no penalty, right, to 0.05, left). The curve bends near a penalty of 0.01 (340 km, 37.6 million USD): left of it, each kilometre of boundary removed costs more and more."
```

A penalty near the bend, here 0.005 to 0.01, is a defensible default. [Part 6](../evaluating-plans/) finds it
reproducibly with [[prioritize.calibrate]] (the knee of the curve, or TOPSIS). Report the value, how it was chosen, and
the cost and boundary apart.

### Rules on the shape

![Three rules on blocks of 3 x 3 km.](/img/tutorials/spatial-prioritization/spatial-design/rules.png "The minimum set of 20 % on blocks of 9 km² under three rules. No rule: 64 blocks in 18 pieces, 24.9 million USD. At least two selected neighbours: no block alone, 7 pieces, 27.8 million (15 s). One contiguous network: a single winding network that reaches every feature, 34.0 million, not proven optimal after 3 minutes (a gap of 23 %).")

| Rule | Cost (million USD) | Pieces | Solved |
|---|---|---|---|
| none | 24.9 | 18 | 1 s, gap 0.6 % |
| `neighbors = 2` | 27.8 (+12 %) | 7 | 15 s, gap 1 % |
| `contiguity = true` | 34.0 (+37 %) | 1 | stopped at 3 min, gap 23 % |

Contiguity is the most demanding rule, in money and in time: the network must thread from the mountains to the coast
through dear land, and the solver cannot prove its plan within 23 % of the best in three minutes. Use it on small
problems, or when the rule is a requirement; elsewhere a boundary penalty gets most of the compactness for a fraction
of the cost. Report the gap: a contiguous plan "within 23 % of the optimum" is still a valid plan, but not a proven
cheapest one.

### Connectivity along the rivers

```tfgallery
columns: 3
caption: "Plans of 20 % on blocks of 3 x 3 km over the tapir's habitat (orange, darker where more suitable). Without a connectivity penalty the plan touches the rivers in a few places. With a penalty of 0.005, linked blocks along the southern river are kept together: the connectivity of the plan along the tapir's habitat nearly doubles (from 0.43 to 0.77 million) for 0.3 % more cost. With 0.02 it more than quintuples (2.3 million), for 46 % more."
images:
  - { src: /img/tutorials/spatial-prioritization/spatial-design/connectivity_0.png, caption: "No penalty: 24.9 M USD" }
  - { src: /img/tutorials/spatial-prioritization/spatial-design/connectivity_0.005.png, caption: "0.005: 25.0 M USD" }
  - { src: /img/tutorials/spatial-prioritization/spatial-design/connectivity_0.02.png, caption: "0.02: 36.4 M USD" }
```

The connectivity of a plan, `connectivity` in the summary of [[prioritize.evaluate]] given the same matrix on its
`connectivity_data` input, is the sum of the links between selected units. Like the boundary penalty, the value of the
connectivity penalty is an exchange rate between money and links, and needs the same calibration.

### Hexagons

```tfgallery
columns: 2
caption: "The 30 % minimum set on H3 hexagons. Resolution 7 (left): 984 hexagons of 5.3 km² on average, 185 selected for 38.5 million USD, the same places as the plan on cells (the mountains, the rivers, the tamarin's fragments, the mangrove coast) in units a planner can name. Resolution 6 (right): 159 hexagons of 37 km², 36 selected for 42.8 million."
images:
  - { src: /img/tutorials/spatial-prioritization/spatial-design/hexagons_7.png, caption: "Resolution 7" }
  - { src: /img/tutorials/spatial-prioritization/spatial-design/hexagons_6.png, caption: "Resolution 6" }
```

| Planning units | Units | Mean area | Plan | Cost (million USD) |
|---|---|---|---|---|
| Cells of 1 km² | 4 825 | 1 km² | 812 cells | 37.1 |
| H3, resolution 7 | 984 | 5.3 km² | 185 hexagons | 38.5 (+4 %) |
| H3, resolution 6 | 159 | 37 km² | 36 hexagons | 42.8 (+15 %) |

Larger units cost more for the same targets: a unit is bought whole, with the poor land inside it. They also give
plans that are compact by construction. Choose the unit by the decision: cells for analysis, hexagons or properties
for plans that will be implemented, and report the units with the results.

### Zones

![A plan with two management zones.](/img/tutorials/spatial-prioritization/spatial-design/zones_strict.png "A zoning plan for 30 % of every feature, counting sustainable use for half, with at least 20 % of every feature under strict protection: 533 cells of strict protection (dark) and 569 of sustainable use (light), for 36.3 million USD. Strict protection takes the cores of the mountain blocks, the tamarin's fragments and the mangrove; sustainable use surrounds them, like the buffer zones of a biosphere reserve.")

![The zoning plan without a target of strict protection.](/img/tutorials/spatial-prioritization/spatial-design/zones.png "The same zoning plan without the 20 % of strict protection: 452 cells of strict protection and 735 of sustainable use.")

Without the 20 % of strict protection, the plan has 452 strictly protected cells and 735 of sustainable use, for
36.2 million USD: cheaper per cell, sustainable use covers more land. The strict targets cost little here (0.4 %), but
they move 81 cells into strict protection. Zoning plans make visible a choice that a plan of protected areas hides:
how much of the targets may be met by land that is used.

## Settings that change the answer

- **The penalty's value**, as the slider shows; it has units and depends on the landscape: calibrate it, never copy
  it.
- **`edge_factor`**: 0 when the edge of the study area is not a real edge (a political border through continuous
  forest), 1 when it is (a coast that the plan should not hug); the default 0.5 is in between.
- **The size of the units**, for constraints (blocks of 3 km here) and for the plan itself (hexagons).
- **The contributions of zones**, which are assumptions about how much a use harms each feature: take them from
  studies or experts, and test other values.

## Common pitfalls

- **Reporting the objective as the cost.** With penalties, the objective includes them; read the cost from
  [[prioritize.evaluate]].
- **A penalty copied from another study,** or chosen for the map it gives. Calibrate on the curve.
- **Contiguity on a large problem** without a time limit: it may run for hours. Set `time_limit`, read the gap, and
  try coarser units.
- **Zone layers named differently** in `zone_costs`, `contribution` and `manual_targets`: the names tie them together.
- **Comparing boundaries computed with different edge factors**: Marxan's maps use 1 by default, prioritizr's 0.5.

## Going further

- Part 6, [Evaluating, comparing and reporting plans](../evaluating-plans/): calibration with the knee and TOPSIS,
  trade-off curves, and a checklist.
- Part 7, [Multi-action planning](../multi-action-planning/): actions against threats, with the boundary of each
  action.
- Connectivity from least-cost or resistance distances: [Distance and restoration](/tutorials/distance-restoration/)
  and [[prioritize.decay_connectivity]].
- The operators: [[prioritize.optimize]], [[prioritize.h3_units]], [[prioritize.connectivity_matrix]],
  [[prioritize.coarsen]], [[prioritize.refine]].

## References

- Beger, M., Linke, S., Watts, M., Game, E., Treml, E., Ball, I., & Possingham, H. P. (2010). Incorporating asymmetric connectivity into spatial decision making for conservation. *Conservation Letters*, 3(5), 359–368. https://doi.org/10.1111/j.1755-263X.2010.00123.x
- Beyer, H. L., Dujardin, Y., Watts, M. E., & Possingham, H. P. (2016). Solving conservation planning problems with integer linear programming. *Ecological Modelling*, 328, 14–22. https://doi.org/10.1016/j.ecolmodel.2016.02.005
- Hanson, J. O., Schuster, R., Strimas-Mackey, M., Morrell, N., Edwards, B. P. M., Arcese, P., Bennett, J. R., & Possingham, H. P. (2025). Systematic conservation prioritization with the prioritizr R package. *Conservation Biology*, 39(1), e14376. https://doi.org/10.1111/cobi.14376
- Hermoso, V., Linke, S., Prenda, J., & Possingham, H. P. (2011). Addressing longitudinal connectivity in the systematic conservation planning of fresh waters. *Freshwater Biology*, 56(1), 57–70. https://doi.org/10.1111/j.1365-2427.2009.02390.x
- Önal, H., & Briers, R. A. (2006). Optimal selection of a connected reserve network. *Operations Research*, 54(2), 379–388. https://doi.org/10.1287/opre.1060.0272
- Watts, M. E., Ball, I. R., Stewart, R. S., Klein, C. J., Wilson, K., Steinback, C., et al. (2009). Marxan with Zones: software for optimal conservation based land- and sea-use zoning. *Environmental Modelling & Software*, 24(12), 1513–1521. https://doi.org/10.1016/j.envsoft.2009.06.005
- Williams, J. C., ReVelle, C. S., & Levin, S. A. (2005). Spatial attributes and reserve design models: a review. *Environmental Modeling & Assessment*, 10(3), 163–181. https://doi.org/10.1007/s10666-005-9007-5
