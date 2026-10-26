---
title: Costs, paths and corridors
short: Costs and corridors
description: Turn distance into a resistance to movement, accumulate it into costs, find the least-cost path between two forests, and measure how much restoring it would reconnect the habitat, as GuidosToolbox's Restoration Planner does.
order: 20
level: intermediate
time: 50 min
template: Restoring a corridor
ops: [restoration.resistance, restoration.cost_distance, restoration.least_cost_path, restoration.corridor, restoration.isochrone, restoration.status, landscape.accounting]
series: Distance and restoration
part: 2
---

In this part you plan a corridor. Starting from the Oregon forest of the [first part](../distance-to-habitat/),
you score how connected the forest is today, turn the distance to the forest into a resistance to movement, and
find the cheapest route between the largest forests. Then you ask the question a restoration planner asks: if the
open land along that route were restored, how much more of the forest would be connected, and which of three
possible corridors gives the most for the effort? At the end you have a map of each corridor, its cost, and the
gain in connectivity it would bring, the numbers that GuidosToolbox's *Restoration Planner* reports.

## The idea

### Resistance and accumulated cost

A straight line ignores what lies between two places. An animal leaving a forest may cross a narrow strip of
pasture easily and avoid a wide one, and a corridor through land that is easy to cross is worth more than one
through land that is not. A **resistance** map gives every cell the cost of crossing it: low in the habitat,
higher in land the species avoids. Resistance values are relative, and here they run from 1 to 100.

The **accumulated cost** of a cell is the least total resistance of any route from a source (a reserve, a
forest) to it. It is computed on the grid seen as a network: every cell is linked to its eight neighbours, and
a step between two cells costs the mean of their resistances times the length of the step (1 for a side, √2 for
a diagonal). Dijkstra's algorithm (Dijkstra, 1959) finds every least cost at once: starting from the sources, it
repeatedly settles the cheapest unsettled cell and updates its neighbours.

![A toy grid of 5 by 6 cells with its resistance and the accumulated cost from the top-left cell, and the cheapest path to the bottom-right cell](/img/tutorials/distance-restoration/costs-and-corridors/toy.png "Left: the resistance of each cell. Right: the accumulated cost from the top-left cell, by Dijkstra's algorithm; the line is the cheapest route to the bottom-right cell, which goes around the costly cells in the middle. The first step down costs (2 + 2) / 2 = 2; the diagonal step from 2 to 5 costs (2 + 5) / 2 × √2 ≈ 4.9.")

!!! science "Key idea"
    For neighbouring cells \(i\) and \(j\) with resistances \(r_i\) and \(r_j\), a step costs
    \( c_{ij} = \frac{r_i + r_j}{2}\, \ell_{ij} \), with \(\ell_{ij} = 1\) for a side and \(\sqrt{2}\) for a
    diagonal. The accumulated cost of a cell is the least sum of \(c_{ij}\) along any route from a source to it.
    The **least-cost path** between areas \(A\) and \(B\) runs through the cells where the cost from \(A\) plus
    the cost from \(B\) is smallest.

Least-cost modelling has been used for wildlife corridors since the early 2000s (Adriaensen et al., 2003), and
the choices it involves (the resistance, the sources, one path or a band of near-optimal paths) are reviewed by
Beier et al. (2008) and Etherington (2016).

### How connected is the habitat, and how much could a corridor add?

To compare corridors we need a measure of connectivity that rises when habitat is reconnected. GuidosToolbox's
*Restoration Status* uses the **equivalent connected area** (ECA) of a species that cannot cross open land at
all: habitat is reachable only inside the object (the group of touching habitat cells) it belongs to. The ECA is
the size of a single object that would offer the same amount of reachable habitat as the whole landscape
(Saura & Pascual-Hortal, 2007; Saura et al., 2011).

!!! science "Key idea"
    With \(a_i\) the area of habitat object \(i\),

    \[ \mathrm{ECA} = \sqrt{\textstyle\sum_i a_i^2} \qquad \text{coherence} = 100\,\frac{\mathrm{ECA}}{\sum_i a_i} \]

    Coherence is 100% when all the habitat is one object. The **restoration potential** is
    \(100 - \text{coherence}\): what reconnecting every piece would add. Joining two objects of areas \(a\) and
    \(b\) adds \(2ab\) to the sum under the square root, so joining **large** objects gains the most.

## When to use it

- **Designing corridors** between reserves or large forests for species that move through the landscape in a
  directed way (seasonal migrations, animals that know their home range).
- **Comparing restoration options**: each candidate corridor has a gain (in ECA or coherence) and a cost (the
  open cells to restore, or their resistance); ranking them by gain per unit of cost is the logic of
  connectivity-based restoration planning (Tambosi et al., 2014).
- **Travel costs**: the accumulated cost from a reserve shows how far a disperser can get for a given effort.

What it is *not* for: dispersers exploring unknown ground do not "know" the best route; circuit theory, which
spreads movement over all routes, suits them better ([circuit connectivity](/tutorials/#circuit-connectivity)).
For many sources and costs in map units, as R's gdistance does, see [grid distances](/tutorials/#grid-distances).
ECA here treats open land as impassable; for species that cross some distance of it, use the probability of
connectivity of [habitat networks](/tutorials/#habitat-networks). And to decide *how much* of each converted cell
to restore for species and carbon, see the plans of the [fifth part](../restoration-plan/).

## What you need

- **A habitat map** in GuidosToolbox's coding, 0 missing, 1 background, 2 habitat: the forest of the first part
  (Oregon's summer NDVI of 0.7 or more, projected to UTM zone 10 N with cells of 30 m).
- **Two areas to connect**: reserves, or here the largest forest objects, found with [[landscape.accounting]].
- **A resistance**, 1 to 100 (0 or no value: impassable). Without information on the species, GuidosToolbox's
  default grows with the distance from the habitat, assuming forest animals avoid moving far from cover. With
  information, build one from land cover or from a habitat model.

The application's template **Restoring a corridor** (Templates menu, group *Connectivity*) builds the same chain
on a landscape drawn by chance; the model below replaces it with the Oregon forest.

## Build the model

### How connected is the forest now?

Start from the three nodes of the first part (`ndvi`, `projected`, `forest`). Add [[restoration.status]] and
connect `forest.map` to its `map`. Its one output, `stats`, is a table of one row: the area with data, the number
of objects, the habitat, the largest object, the average patch size (APS), the critical new object area (CNOA),
the ECA, the coherence, the restoration potential and the habitat's share of the area with data (RAC), all in
cells or in percent.

### Find the forest objects

Add [[landscape.accounting]] with `forest.map` as its `map`. It labels every object (`ids`: the number of each
cell's object) and lists them with their areas (`objects`). Run it (++ctrl+enter++ on the node) and sort the
`objects` table by `cells`: the three largest objects are numbers 36 (9,508 cells), 284 (7,035) and 169 (4,995).
Call them A, B and C. Two [[raster.calc]] nodes mark A and B with the expressions `ids == 36` and `ids == 284` on
`objects.ids`.

![The three largest forest objects, A, B and C](/img/tutorials/distance-restoration/costs-and-corridors/objects.png "The three largest forest objects (dark green): A with 9,508 cells (8.6 km²), B with 7,035 and C with 4,995. Pale green: the 565 other pieces of forest.")

### A resistance from the distance to the forest

[[restoration.resistance]] makes GuidosToolbox's resistance: 2 on the habitat, then the rounded distance to the
habitat plus 2 on the background (3 next to the forest, growing outwards up to 100), and 0 on missing cells,
which the cost operators treat as impassable. Connect `forest.map` to its `map`.

![GuidosToolbox's resistance: 2 in the forest, growing with the distance from it](/img/tutorials/distance-restoration/costs-and-corridors/resistance.png "The resistance from restoration.resistance. In this fine-grained landscape no open cell is far from forest, so the resistance stays at 16 or below: crossing open land costs a few times more than crossing forest.")

### The accumulated cost from A

[[restoration.cost_distance]] takes the `resistance` and the `sources` (cells above 0: here `a.map`) and gives
the accumulated cost from A to every cell, by the eight-neighbour rule of the key idea above.

![The accumulated cost of reaching every cell from object A](/img/tutorials/distance-restoration/costs-and-corridors/cost_a.png "The accumulated cost from A (0 inside it). It grows slowly through forest and faster across open land; B, in the north-west, is first reached at a cost of 233; the far north-east corner costs 674.")

### The least-cost path from A to B

[[restoration.least_cost_path]] takes the `resistance` and the two areas `a` and `b`. Its outputs: `path` (1 on
the path's cells), `cost` (the cost from A plus the cost from B: least, and the same, all along the path) and
`stats` (the path's length in cells, the cells of it on the background, their total resistance or *expense*, and
the mean distance of those cells to the habitat when a [[distance.euclidean]] map is connected to `distance`).

### What restoring it would gain

[[restoration.corridor]] takes the same inputs plus the `habitat` map. It turns the background cells of the
least-cost path into habitat and computes the restoration status before and after: its `stats` hold the sizes of
A and B, the cells restored, the expense, the ECA and the coherence before and after, their gains, and the
*efficiency*, the ECA gained per unit of expense. Its `restored` output is the habitat map with the corridor
restored, ready for another round.

```tfgraph
#! caption: The corridor model: how connected the forest is, its objects, a resistance, the cost from A, the least-cost path from A to B and what restoring it would gain. The first three nodes are those of the first part.
#! highlight: status, objects, resistance, path, corridor
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "ndvi"
op = "raster.read"
params = { path = "ndvi.tif" }

[[nodes]]
id = "projected"
op = "raster.reproject"
inputs = { raster = "ndvi.raster" }
params = { crs = "EPSG:32610", resolution = 30 }

[[nodes]]
id = "forest"
op = "raster.calc"
inputs = { n = "projected.raster" }
params = { expression = "0 if isnull(n) else 2 if n >= 0.7 else 1" }

[[nodes]]
id = "status"
op = "restoration.status"
inputs = { map = "forest.map" }
note = "ECA, coherence and restoration potential now."

[[nodes]]
id = "objects"
op = "landscape.accounting"
inputs = { map = "forest.map" }
note = "Every forest object numbered and measured."

[[nodes]]
id = "resistance"
op = "restoration.resistance"
inputs = { map = "forest.map" }
note = "2 in the forest, growing with the distance from it."

[[nodes]]
id = "a"
op = "raster.calc"
inputs = { ids = "objects.ids" }
params = { expression = "ids == 36" }
note = "Object A, the largest."

[[nodes]]
id = "b"
op = "raster.calc"
inputs = { ids = "objects.ids" }
params = { expression = "ids == 284" }
note = "Object B, the second."

[[nodes]]
id = "cost_a"
op = "restoration.cost_distance"
inputs = { resistance = "resistance.map", sources = "a.map" }

[[nodes]]
id = "path"
op = "restoration.least_cost_path"
inputs = { resistance = "resistance.map", a = "a.map", b = "b.map" }

[[nodes]]
id = "corridor"
op = "restoration.corridor"
inputs = { resistance = "resistance.map", a = "a.map", b = "b.map", habitat = "forest.map" }
note = "The path's open cells restored: ECA and coherence before and after."
```

### The isochrone map

GuidosToolbox draws the same costs as an *isochrone map*. [[restoration.isochrone]] takes the `resistance`
(2 habitat, 3 to 100 background, as [[restoration.resistance]] makes it) and a `marker` map coded 0 background,
1 object A, 2 object B, 3 missing (not crossed). With both objects it gives the cost of the cheapest route from A
to B through every cell, scaled from 0 to 100 in GuidosToolbox's codes (101 the objects, 102 missing, 103 the
least-cost path, 104 unreachable), the cost itself (`cost`), the path (`path`) and the least and largest costs
(`stats`). The marker is one more [[raster.calc]]:

```text
3 if f == 0 else 1 if a else 2 if b else 0
```

with `f` the forest, `a` and `b` the two objects. The node would come after those of the graph above:

```tfgraph
#! caption: GuidosToolbox's isochrone map from A to B: a marker map of the two objects, then the cost through every cell.
#! highlight: marker, isochrone
#! fragment: true
[[nodes]]
id = "marker"
op = "raster.calc"
inputs = { f = "forest.map", a = "a.map", b = "b.map" }
params = { expression = "3 if f == 0 else 1 if a else 2 if b else 0" }

[[nodes]]
id = "isochrone"
op = "restoration.isochrone"
inputs = { resistance = "resistance.map", marker = "marker.map" }
```

## Read the results

### The forest today

```tfchart
type: bar
csv: |
  measure,value
  Coherence,32.97
  Restoration potential,67.03
  Habitat in the area with data (RAC),41.24
x: measure
y: value
colors: { value: "#3b8a5a" }
ylabel: "%"
percent: true
height: 240
caption: The restoration status of the forest, from restoration.status. The forest (41% of the map) is split into 568 objects whose ECA is 13,650 cells: as much reachable habitat as one object of a third of the forest's area.
```

A coherence of 33% means that the forest offers as much reachable habitat as a single object one third of its
size: two thirds of its potential connectivity is lost to fragmentation, and is what restoration could recover.

### The least-cost path and the cost surface

![The cost from A plus the cost from B, above its least: the dark band is the corridor of near-optimal routes](/img/tutorials/distance-restoration/costs-and-corridors/path_cost.png "The cost from A plus the cost from B, minus its least value (the cost of the path). The least-cost path runs where it is 0; the dark band around it holds routes almost as cheap, the room there is for a corridor on the ground. Pale: routes more costly than most.")

The `cost` output of [[restoration.least_cost_path]] is worth looking at as well as the path. Its least value is
the cost of the best route; cells a little above it are on routes almost as good. Thresholding it (for example
at 5% above the least) gives a **least-cost corridor**, a band rather than a line, which is more robust than a
single path and shows the room available on the ground (Adriaensen et al., 2003; Beier et al., 2008).

### Three corridors compared

Repeat the model with the other pairs of objects (change the two expressions to `ids == 36` and `ids == 169`,
then `ids == 284` and `ids == 169`). Each frame is a real run:

```tfslider
label: Corridor
caption: The least-cost path between each pair of the three largest objects (drawn wider than its one cell so that it shows). Red: open cells to restore; orange: forest it already crosses.
frames:
  - { value: "A-B", src: /img/tutorials/distance-restoration/costs-and-corridors/corridor_A-B.png, stats: { "Cells restored": "71", "Expense": "191", "Coherence": "33.0% → 50.3%", "ECA gained (cells)": "7,188" } }
  - { value: "A-C", src: /img/tutorials/distance-restoration/costs-and-corridors/corridor_A-C.png, stats: { "Cells restored": "3", "Expense": "10", "Coherence": "33.0% → 40.5%", "ECA gained (cells)": "3,125" } }
  - { value: "B-C", src: /img/tutorials/distance-restoration/costs-and-corridors/corridor_B-C.png, stats: { "Cells restored": "5", "Expense": "19", "Coherence": "33.0% → 38.7%", "ECA gained (cells)": "2,373" } }
```

| Corridor | Path length (cells) | Open cells restored | Expense | Coherence gained (points) | ECA gained (cells) | Efficiency (ECA per expense) |
|---|---|---|---|---|---|---|
| A-B | 73 | 71 | 191 | 17.3 | 7,188 | 38 |
| A-C | 5 | 3 | 10 | 7.5 | 3,125 | 312 |
| B-C | 7 | 5 | 19 | 5.7 | 2,373 | 125 |

The options differ on both axes, as in real planning:

- **A-B gives the largest gain**, 17 points of coherence, more than the two objects alone would give. Joining
  9,508 and 7,035 cells adds \(2 \times 9{,}508 \times 7{,}035\) to ECA², which would raise ECA to about 17,900
  cells; the corridor raises it to 20,838 because the path also crosses smaller forests on its way and joins them
  too. But it needs 71 cells (6.4 ha) of restoration.
- **A-C is by far the most efficient**: A and C are only three open cells apart, and closing that gap adds 7.5
  points for 3 cells. A planner would do it first.
- **B-C is cheap but gains less**, because neither object is as large as A.

Ranking options by gain per unit of cost, then checking whether they complement each other (after A-C, does A-B
still gain as much?), is the logic of spatial restoration planning. Run [[restoration.corridor]] again on its own
`restored` output to answer the second question.

### How to report it

Report the habitat map and its coding, the resistance (its source and values), the areas connected, and for each
corridor the open cells (or hectares) to restore, the expense, and the ECA and coherence before and after. Give
the coherence gain *and* the efficiency: the first says what a corridor achieves, the second what it costs.

## Settings that change the answer

- **The resistance.** The default resistance assumes only that open land far from forest is harder to cross. A
  resistance from land cover (pasture 10, crops 30, a highway 100) or from a habitat suitability model can move
  the path entirely. Least-cost paths are sensitive to the ratios between resistances, and less to their
  absolute values (Etherington, 2016); try two or three plausible sets and report whether the corridor moves.
- **The habitat definition.** With an NDVI threshold of 0.6 almost all the forest is one object (92,651 of its
  cells) and there is little left to reconnect; with 0.75 the large objects fall apart. The coherence and every
  gain depend on the definition, so keep the same one when you compare options.
- **The areas to connect.** Objects found by [[landscape.accounting]] depend on the connectivity rule (8
  neighbours by default; `connectivity = 4` splits objects that touch only at a corner).

## Common pitfalls

- **Zero is a wall.** Cells of resistance 0 or without a value cannot be crossed. Code the outside of the study
  area as missing on purpose, not by accident (a cloud coded 0 cuts a path).
- **ECA treats open land as impassable.** A corridor of one cell joins two objects completely in this measure.
  Real corridors need a width that depends on the species: widen the restored path (a buffer around it) before
  you report hectares.
- **The objects must not touch.** [[restoration.isochrone]] stops with a message if A and B touch, and if missing
  cells separate them.
- **Projected coordinates.** Costs are per cell; compare corridors on maps of the same cell size, and report
  lengths in metres (cells times the cell's side).

## Going further

- Distances without resistance: [Distance to habitat](../distance-to-habitat/), the first part.
- Where human pressure is highest and where removing it would help most:
  [Ecological intactness](../ecological-intactness/), the next part.
- Many sources and costs in metres, and random walks: [grid distances](/tutorials/#grid-distances).
- Current flow instead of a single path: [circuit connectivity](/tutorials/#circuit-connectivity).
- The operators: [[restoration.status]], [[restoration.resistance]], [[restoration.cost_distance]],
  [[restoration.least_cost_path]], [[restoration.corridor]], [[restoration.isochrone]].

## References

- Adriaensen, F., Chardon, J. P., De Blust, G., Swinnen, E., Villalba, S., Gulinck, H., & Matthysen, E. (2003). The application of 'least-cost' modelling as a functional landscape model. *Landscape and Urban Planning*, 64(4), 233–247. https://doi.org/10.1016/S0169-2046(02)00242-6
- Beier, P., Majka, D. R., & Spencer, W. D. (2008). Forks in the road: choices in procedures for designing wildland linkages. *Conservation Biology*, 22(4), 836–851. https://doi.org/10.1111/j.1523-1739.2008.00942.x
- Dijkstra, E. W. (1959). A note on two problems in connexion with graphs. *Numerische Mathematik*, 1(1), 269–271. https://doi.org/10.1007/BF01386390
- Etherington, T. R. (2016). Least-cost modelling and landscape ecology: concepts, applications, and opportunities. *Current Landscape Ecology Reports*, 1(1), 40–53. https://doi.org/10.1007/s40823-016-0006-9
- Saura, S., & Pascual-Hortal, L. (2007). A new habitat availability index to integrate connectivity in landscape conservation planning: comparison with existing indices and application to a case study. *Landscape and Urban Planning*, 83(2–3), 91–103. https://doi.org/10.1016/j.landurbplan.2007.03.005
- Saura, S., Estreguil, C., Mouton, C., & Rodríguez-Freire, M. (2011). Network analysis to assess landscape connectivity trends: application to European forests (1990–2000). *Ecological Indicators*, 11(2), 407–416. https://doi.org/10.1016/j.ecolind.2010.06.011
- Tambosi, L. R., Martensen, A. C., Ribeiro, M. C., & Metzger, J. P. (2014). A framework to optimize biodiversity restoration efforts based on habitat amount and landscape connectivity. *Restoration Ecology*, 22(2), 169–177. https://doi.org/10.1111/rec.12049
- Vogt, P., & Riitters, K. (2017). GuidosToolbox: universal digital image object analysis. *European Journal of Remote Sensing*, 50(1), 352–361. https://doi.org/10.1080/22797254.2017.1330650
