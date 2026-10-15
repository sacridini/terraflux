---
title: Movement as electric current
short: Circuit theory
description: Why ecologists model movement across a landscape as electric current, what effective resistance, current and voltage mean for a disperser, and how a current map differs from a least-cost path, on circuits small enough to check by hand.
order: 10
level: beginner
time: 35 min
template: Circuit connectivity
ops: [circuit.network_pairwise, circuit.pairwise, circuit.omniscape, table.of, raster.read, restoration.least_cost_path]
series: Circuit connectivity
part: 1
---

A least-cost path draws the single best route between two places, as if every animal knew the map. A young
lynx leaving its mother's range does not: it wanders, takes many routes, and is funnelled where the landscape
leaves it little choice. **Circuit theory** models that kind of movement by treating the landscape as an
electrical circuit, in which current flows through every route at once, more through the easy ones, and
concentrates where routes are few. In this first part you learn the ideas on circuits of four nodes that you can
solve by hand, then on a small landscape with a wall and two gaps, where you compare a least-cost path with
Circuitscape's current and Omniscape's normalised current. At the end you will know what effective resistance,
current and voltage mean ecologically, and when to prefer them to least costs.

## The idea

### From random walkers to electric circuits

Mathematicians have long known that **random walks and electric circuits are the same thing** (Doyle & Snell,
1984). Take a network of nodes joined by resistors, and a walker that moves from node to node, choosing each
neighbour with a probability proportional to the conductance (1 / resistance) of the resistor to it. Then:

- the **current** through a resistor, when 1 ampere enters at a node A and leaves at a node B, is the net number
  of times a walker starting at A crosses that resistor before it reaches B;
- the **voltage** at a node is the probability that a walker starting there reaches A before B;
- the **effective resistance** between A and B is proportional to the *commute time*, the expected number of
  steps a walker needs to go from A to B and back (Chandra et al., 1996).

McRae (2006) saw what this means for ecology. If dispersers move like random walkers over a landscape where each
cell is a resistor (its resistance to movement), then circuit theory gives, in one calculation, how isolated two
populations are (the effective resistance) and where the movement between them goes (the current). He proposed
**isolation by resistance**, the effective resistance as a measure of genetic isolation, and McRae and Beier
(2007) showed that it explained the genetic differentiation of populations of wolverines and of a tree
(big-leaf mahogany) better than least-cost or straight distances. McRae et al. (2008) set out the uses of circuit
theory in ecology, evolution and conservation, and the software that does it, **Circuitscape** (Shah & McRae,
2008; Anantharaman et al., 2020).

!!! science "Key idea: Ohm's law and effective resistance"
    Ohm's law: the voltage \(V\) across a resistor of resistance \(R\) carrying a current \(I\) is \(V = I R\).
    Two rules give the resistance of a whole circuit:

    - in **series** (one after another) resistances add: \(R = R_1 + R_2\);
    - in **parallel** (side by side) *conductances* add: \(1/R = 1/R_1 + 1/R_2\).

    The **effective resistance** between two nodes is the resistance of the whole circuit between them. Unlike a
    least cost, it *falls* when routes are added: two equal routes make the nodes twice as connected as one.

### Three small circuits

![Three circuits between nodes 1 and 3, with their effective resistances](/img/tutorials/circuit-connectivity/circuit-theory/resistors.png "Three circuits between nodes 1 and 3 (orange), solved by circuit.network_pairwise. One route of two 1-ohm resistors: 2 ohms. Two such routes side by side: 1 ohm, half as much. A second route twice as resistant as the first: 1.33 ohms, and it carries a third of the current.")

The circuits show the three properties that make circuit theory useful in ecology:

1. **Redundancy counts.** A least-cost distance is the same for one route or for ten equal routes; the effective
   resistance falls with each route added. Two populations joined by several corridors are better connected than
   two joined by one, and circuit theory says so.
2. **Current splits by ease.** With routes of 2 and 4 ohms, two thirds of the current takes the easy route and a
   third the hard one. Movement is spread, not all on the best route.
3. **Narrow passages concentrate current.** Where all routes must pass through a few nodes, all the current
   goes through them: a **pinch point**, where losing a little habitat would cut a lot of movement.

### Current, voltage and resistance as ecology

| Electrical | Ecological meaning |
|---|---|
| Resistance of a cell | how hard or risky it is to move through it |
| Conductance (1 / resistance) | how easily it is crossed |
| Effective resistance between two areas | their isolation, taking every route into account |
| Current through a cell | how much of the movement between the areas passes there: its importance as a route |
| Current density at a pinch point | how much movement is squeezed through a narrow place |
| Voltage | how likely a walker from a cell is to reach one area before the other |

## When to use it

- **Gene flow and population isolation**: effective resistances between populations, compared with genetic
  distances (isolation by resistance; McRae & Beier, 2007).
- **Corridors and pinch points** for animals that do not know the way: dispersing juveniles, range shifts under
  climate change, invasive species.
- **Where to restore**: the places where current is high and the land is degraded, or where a barrier blocks a
  strong flow.
- **Connectivity everywhere**, without choosing patches: Omniscape ([part 4](../omniscape/)).

When *not*: for animals that know their route (seasonal migration along traditional paths), least-cost paths
([grid distances](/tutorials/grid-distances/cost-distance/)) may be the better model; randomised shortest paths
sit between the two ([part 2 of grid distances](/tutorials/grid-distances/random-walks/)). For the connectivity
of a set of patches for a dispersal distance, the habitat graphs of
[habitat networks](/tutorials/habitat-networks/connectivity-as-a-graph/) are simpler.

## What you need

For this part nothing but Terraflux: the circuits are typed as tables, and the small landscape is three rasters
you can download, [`wall.tif`](/data/tutorials/circuit-connectivity/circuit-theory/wall.tif) (the resistance:
1 everywhere, no value in the wall), [`focal.tif`](/data/tutorials/circuit-connectivity/circuit-theory/focal.tif)
(the two areas to connect, coded 1 and 2) and
[`wall_0.tif`](/data/tutorials/circuit-connectivity/circuit-theory/wall_0.tif) (the same resistance with 0 in the
wall, for the least-cost path). The application's template **Circuit connectivity** (**Templates >
Connectivity**) has Circuitscape and Omniscape on a landscape drawn by chance.

## Build the models

### A circuit as a table

[[circuit.network_pairwise]] (library subject **Circuit connectivity**) is Circuitscape's *network* mode: it
reads a circuit as a table of edges, `from`, `to` and `resistance`, and solves it between the nodes listed in
`focal_nodes`. Make the table with [[table.of]] and connect it to the `graph` input:

```tfgraph
#! caption: The third circuit of the figure: two routes between nodes 1 and 3, one of 2 ohms and one of 4, solved with Circuitscape's network mode.
#! numbered: true
#! highlight: circuit
format = "terraflux/1"

[[nodes]]
id = "edges"
op = "table.of"
params = { columns = ["from", "to", "resistance"], rows = [[1, 2, 1], [2, 3, 1], [1, 4, 2], [4, 3, 2]] }
note = "Four resistors: 1-2-3 of 1 ohm each, 1-4-3 of 2 ohms each."

[[nodes]]
id = "circuit"
op = "circuit.network_pairwise"
inputs = { graph = "edges.table" }
params = { focal_nodes = [1, 3], current_maps = true }
note = "The effective resistance between 1 and 3, and the current through every resistor."
```

Its outputs: `resistances`, a row per focal node with its effective resistance to each other one (1.333 here);
with `current_maps = true`, `cum_branch_currents` (the current through each resistor: 0.667 on 1-2 and 2-3,
0.333 on 1-4 and 4-3) and `cum_node_currents`. Change a resistance and predict the result before you run it:
that is the quickest way to an intuition for circuits.

### A landscape as a circuit

On a raster, every cell with a value is a node, joined to its eight neighbours by resistors (the mean of the two
cells' resistances, adjusted for the longer diagonals). Cells without a value are not in the circuit: a wall.
The landscape here is 100 × 60 cells of 100 m, with a wall down the middle broken by a narrow gap (3 cells) in the
north and a wide gap (18 cells) in the south; two short lines of cells near each end are the areas to connect.

[[circuit.pairwise]] is Circuitscape's *pairwise* mode on rasters: `habitat` takes the resistance and `points` the
areas (cells with the same number form one area, Circuitscape's *focal region*). With `cum_map = true` it gives
the current through every cell (`cum`). For comparison, the same model computes the least-cost path
([[restoration.least_cost_path]], which treats 0 as a wall) and Omniscape's normalised current
([[circuit.omniscape]], [part 4](../omniscape/)).

```tfgraph
#! caption: A wall with two gaps: Circuitscape's current between the two areas, Omniscape's normalised current, and the least-cost path for comparison.
#! numbered: true
#! highlight: current, omniscape, path
format = "terraflux/1"

[[nodes]]
id = "resistance"
op = "raster.read"
params = { path = "wall.tif" }
note = "1 everywhere, no value in the wall."

[[nodes]]
id = "focal"
op = "raster.read"
params = { path = "focal.tif" }
note = "Area 1 in the west, area 2 in the east."

[[nodes]]
id = "current"
op = "circuit.pairwise"
inputs = { habitat = "resistance.map", points = "focal.map" }
params = { cum_map = true }
note = "1 A from area 1 to area 2: the current through every cell."

[[nodes]]
id = "omniscape"
op = "circuit.omniscape"
inputs = { resistance = "resistance.map" }
params = { radius = 15, block_size = 1, flow_potential = true, normalized_current = true }
note = "Current from every cell to those within 15 cells, over what it would be without the wall."

[[nodes]]
id = "open"
op = "raster.read"
params = { path = "wall_0.tif" }
note = "The same, with 0 in the wall."

[[nodes]]
id = "left"
op = "map.calc"
inputs = { f = "focal.map" }
params = { expression = "f == 1" }

[[nodes]]
id = "right"
op = "map.calc"
inputs = { f = "focal.map" }
params = { expression = "f == 2" }

[[nodes]]
id = "path"
op = "restoration.least_cost_path"
inputs = { resistance = "open.map", a = "left.map", b = "right.map" }
note = "The single cheapest route."
```

## Read the results

```tfgallery
columns: 3
caption: "The same landscape seen three ways. Left: the least-cost path (orange) takes the wide gap, which is closer to the straight line, and ignores the narrow one. Middle: Circuitscape's current (logarithmic) uses both gaps, and is densest in the narrow one. Right: Omniscape's normalised current, red where current is channelled above what an open landscape would carry, blue where it is impeded."
images:
  - { src: /img/tutorials/circuit-connectivity/circuit-theory/wall-path.png, caption: "Least-cost path" }
  - { src: /img/tutorials/circuit-connectivity/circuit-theory/wall-current.png, caption: "Current (Circuitscape)" }
  - { src: /img/tutorials/circuit-connectivity/circuit-theory/wall-normalized.png, caption: "Normalised current (Omniscape)" }
```

The three maps answer different questions:

- **The least-cost path** is one line through the wide gap, at its northern end, the point nearest the straight
  line between the areas. It says nothing about the narrow gap: if the wide gap were closed, the path would jump
  to the narrow one, but the map gives no hint that it matters.
- **The current** spreads over the whole landscape and passes through **both** gaps: 26% of it through the narrow
  gap and 74% through the wide one. Per cell, though, the narrow gap carries **twice the current** of the wide
  one (0.089 A against 0.041 A): it is a pinch point. A planner reading this map knows that both gaps carry
  movement and that the narrow one is the more fragile.
- **The normalised current** of Omniscape compares the current with what the same windows would carry with no
  wall at all. In the open land it is 1 (as expected); in the narrow gap 3.4 and in the wide gap 1.3; around the
  ends of the wall, blue shadows where movement is blocked.

!!! paper "In the literature"
    The comparison above is a classic. McRae et al. (2008) and Dickson et al. (2019) show that least-cost
    paths and current maps agree on the main routes but that current maps also show redundant routes and pinch
    points, which least-cost corridors cannot. Marrotte and Bowman (2017) compared least-cost and resistance
    distances over many simulated landscapes and found them strongly correlated in most. As measures of isolation
    they often agree; as maps of where movement goes, they do not.

## Settings that change the answer

- **The resistance values**: the next part, [resistance surfaces](../resistance-surfaces/).
- **4 or 8 neighbours** (`four_neighbors`): with 4, diagonal moves are not allowed, and current cannot slip
  through gaps one cell wide at a corner.
- **How neighbours are joined**: by the mean of their conductances (the default) or of their resistances
  (`avg_resistances`). The difference shows at sharp boundaries between very different cells.
- **Focal areas or points**: a focal region (many cells with one id) injects current over its whole extent,
  a single cell at one point. Use regions for reserves and patches, as here.

## Common pitfalls

- **Zero resistance.** A resistance must be positive. Use "no value" for barriers and small values (1) for the
  best habitat.
- **Reading current as a probability.** Current is the expected net number of passages of a walker between the
  two areas, for 1 A injected. Its absolute value depends on how much current enters; compare cells of one map,
  and normalise before comparing maps.
- **Edges of the map.** Walkers cannot go around the map's edge, so current crowds along it. Leave a buffer around
  the study area (Koen et al., 2010).
- **Too-small gaps.** A one-cell gap in a raster of 100 m may be a road crossing or an artefact of the map. Check
  pinch points on the ground.

## Going further

- [Part 2: resistance surfaces](../resistance-surfaces/).
- [Part 3: Circuitscape between forests](../circuitscape/): its four modes on a real landscape.
- [Part 4: Omniscape](../omniscape/): connectivity everywhere, without choosing areas.
- [Grid distances](/tutorials/grid-distances/cost-distance/): least-cost paths, random walks and the randomised
  shortest paths between them.
- The operators: [[circuit.network_pairwise]], [[circuit.network_advanced]], [[circuit.pairwise]],
  [[circuit.circuitscape_ini]] (Circuitscape's INI files as they are).

## References

- Anantharaman, R., Hall, K., Shah, V. B., & Edelman, A. (2020). Circuitscape in Julia: high performance connectivity modelling to support conservation decisions. *JuliaCon Proceedings*, 1(1), 58. https://doi.org/10.21105/jcon.00058
- Chandra, A. K., Raghavan, P., Ruzzo, W. L., Smolensky, R., & Tiwari, P. (1996). The electrical resistance of a graph captures its commute and cover times. *Computational Complexity*, 6(4), 312–340. https://doi.org/10.1007/BF01270385
- Dickson, B. G., Albano, C. M., Anantharaman, R., Beier, P., Fargione, J., Graves, T. A., Gray, M. E., Hall, K. R., et al. (2019). Circuit-theory applications to connectivity science and conservation. *Conservation Biology*, 33(2), 239–249. https://doi.org/10.1111/cobi.13230
- Doyle, P. G., & Snell, J. L. (1984). *Random walks and electric networks*. Mathematical Association of America. https://doi.org/10.5948/UPO9781614440222
- Koen, E. L., Garroway, C. J., Wilson, P. J., & Bowman, J. (2010). The effect of map boundary on estimates of landscape resistance to animal movement. *PLoS ONE*, 5(7), e11785. https://doi.org/10.1371/journal.pone.0011785
- Marrotte, R. R., & Bowman, J. (2017). The relationship between least-cost and resistance distance. *PLoS ONE*, 12(3), e0174212. https://doi.org/10.1371/journal.pone.0174212
- McRae, B. H. (2006). Isolation by resistance. *Evolution*, 60(8), 1551–1561. https://doi.org/10.1111/j.0014-3820.2006.tb00500.x
- McRae, B. H., & Beier, P. (2007). Circuit theory predicts gene flow in plant and animal populations. *Proceedings of the National Academy of Sciences*, 104(50), 19885–19890. https://doi.org/10.1073/pnas.0706568104
- McRae, B. H., Dickson, B. G., Keitt, T. H., & Shah, V. B. (2008). Using circuit theory to model connectivity in ecology, evolution, and conservation. *Ecology*, 89(10), 2712–2724. https://doi.org/10.1890/07-1861.1
- Shah, V. B., & McRae, B. H. (2008). Circuitscape: a tool for landscape ecology. In *Proceedings of the 7th Python in Science Conference* (pp. 62–65). https://doi.org/10.25080/EWDJ4033
