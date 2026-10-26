---
title: Cost distance, least-cost paths and corridors
short: Cost distance and paths
description: Measure distances the way a disperser pays for them with R's gdistance operators, accumulate the cost of crossing a landscape from a forest, draw isochrones, trace the least-cost paths to four other forests and widen the best one into a corridor.
order: 10
level: beginner
time: 45 min
template: Least costs and random walks
ops: [cost.transition, cost.geo_correction, cost.acc_cost, cost.shortest_path, cost.distance, vector.read, vector.write, raster.calc]
series: Grid distances
part: 1
---

How far is one forest from another, for an animal that has to cross the fields, villages and roads between them?
Not the 29 km of a ruler: it depends on what lies in the way. In this part you measure distance as a **cost**, the
sum of the resistances crossed, the way the R package gdistance does and as Terraflux's `cost` operators
reproduce it to the last digit. You compute the **accumulated cost** of reaching every cell from the large
southern forest around Hannover, read it as **isochrones**, trace the **least-cost paths** to four other
forests, compare their costs with straight distances, and widen the best path into a **least-cost corridor**. At
the end you will have the tools behind most corridor designs, and know their assumptions.

## The idea

### A landscape as a grid of connections

gdistance (van Etten, 2017) describes a raster as a graph: every cell is a node connected to its 4, 8 or 16
neighbours, and each connection has a value, the **transition**. For movement the transition is a
**conductance**: how easily a walker passes from one cell to the next. It is made from the cells' values by a
function of the two (the mean, the minimum...), usually of the conductance of each cell, 1 / its resistance.

Two corrections follow. A diagonal step is longer than a side step (√2 times), so the conductance between two
cells is divided by the distance between their centres (**geographic correction**, type `c` for costs); on a
grid in degrees, those distances shrink towards the poles, which the correction takes into account. The cost of
a step is then its length divided by its conductance: the length times the mean resistance.

!!! science "Key idea: accumulated cost and least-cost path"
    The **accumulated cost** of a cell is the least sum of step costs along any route to it from the nearest
    origin, found by Dijkstra's algorithm (Dijkstra, 1959). The **least-cost path** to a goal is the route that
    achieves it. With a resistance of 1 in forest and cells of 100 m, a step through forest costs 100 and a step
    through arable land (resistance 20) costs 2,000: the cost is in "metres of forest".

    Least-cost modelling became a standard tool of landscape ecology with Adriaensen et al. (2003), who called
    the cost distance a *functional* distance: it measures the landscape as the species experiences it.

### Isochrones and corridors

An accumulated cost map can be read like a map of travel time: lines of equal cost, **isochrones**, enclose the
land a disperser can reach for a given effort. And instead of a single path, which a real animal is unlikely to
follow exactly, planners draw a **least-cost corridor**: every cell through which the best route between two
areas is at most a few percent dearer than the least cost (Beier et al., 2008). The corridor shows the room
there is on the ground and the alternatives a path hides.

## When to use it

- **Distances between populations or reserves** that account for the matrix: for habitat graphs
  ([least-cost links](/tutorials/habitat-networks/least-cost-links/)), for isolation in genetic studies, for
  dispersal models.
- **Designing corridors** between two areas (Beier et al., 2008), and comparing them.
- **Accessibility**: how far, in effort, every place is from a source (a reserve, a road, a town).

What it assumes: the animal knows the landscape and takes the best route. For animals exploring unknown country,
random walks and circuit theory are better models ([part 2](../random-walks/),
[circuit connectivity](/tutorials/circuit-connectivity/circuit-theory/)). Least-cost paths are also sensitive to
the resistance values (Etherington, 2016): treat the path as one plausible route among many.

## What you need

- **A resistance**: the class resistance of [resistance surfaces](/tutorials/circuit-connectivity/resistance-surfaces/),
  from [`clc_100m.tif`](/data/tutorials/habitat-networks/connectivity-as-a-graph/clc_100m.tif) (CORINE Land Cover
  around Hannover at 100 m): forest 1, arable land 20, towns and water 100, and so on.
- **Origins and goals**: areas (a raster mask) or points (vector features). Here the southern forest (patch 55,
  9,151 ha) as an area, and five forests as points,
  [`forests.geojson`](/data/tutorials/grid-distances/cost-distance/forests.geojson): one point in each of the
  southern forest (55), the north-eastern one (96), the south-western (8), the north-western (30) and the far
  north-eastern (110), at the cell of each forest nearest its centre.

The application's template **Least costs and random walks** (**Templates > Connectivity**) has the accumulated
cost and the passages of [part 2](../random-walks/) on a landscape drawn by chance.

![The resistance from the land cover](/img/tutorials/grid-distances/cost-distance/resistance.png "The resistance (logarithmic): forest 1 (pale), arable land 20 (purple), towns, roads and Lake Steinhude 100 (black).")

## Build the model

### 1. The transition

[[cost.transition]] (library subject **Grid distances**) takes the resistance as `map`. With `reciprocal = true`
it makes, for every pair of neighbours, 1 / the function of their values: with `function = "mean"`, 1 / the mean
resistance, the conductance of the connection (gdistance's `1 / transition(r, mean)`). `directions = 8`
connects each cell to its eight neighbours. Its output, a *transition*, is what every other `cost` operator reads.

[[cost.geo_correction]] then divides each conductance by the distance between the cells' centres (`type = "c"`):
after it, costs are in the map's units (metres) times resistance.

### 2. The accumulated cost from the southern forest

The southern forest as a mask: the patches of the forest numbered as in the
[habitat networks](/tutorials/habitat-networks/connectivity-as-a-graph/) series ([[conn.graph_from_raster]],
[[conn.nodes]], [[conn.to_map]]), then [[raster.calc]] `coalesce(p == 55, 0)`, 1 in the forest and 0 elsewhere.
[[cost.acc_cost]] takes the corrected `transition` and the mask as `origins` (cells not 0), and gives the
accumulated cost from the nearest origin cell to every cell (infinite where no route exists).

### 3. Least-cost paths and costs between points

[[vector.read]] reads the five points. [[cost.shortest_path]] takes the transition, an `origin` (the first point
of a layer) and `goals`, and gives the `lines` of the least-cost paths (one per goal) and a `map` of the share of
paths through each cell. [[cost.distance]] gives the least cost from every point to every other (a table).

```tfgraph
#! caption: The accumulated cost from the southern forest, the least-cost paths from it to four other forests and the least costs between all five, over the class resistance.
#! numbered: true
#! highlight: transition, corrected, cost, paths, least_costs
format = "terraflux/1"

[[nodes]]
id = "land"
op = "raster.read"
params = { path = "clc_100m.tif" }

[[nodes]]
id = "resistance"
op = "raster.reclassify"
inputs = { map = "land.raster" }
params = { rules = "23 24 25 -> 1\n29 -> 2\n21 26 27 32 35 36 -> 5\n18 20 -> 10\n12 -> 20\n10 11 -> 30\n7 8 9 -> 50\n1 2 3 4 5 6 40 41 -> 100\n* -> 20" }
note = "A resistance per class."

[[nodes]]
id = "transition"
op = "cost.transition"
inputs = { map = "resistance.map" }
params = { function = "mean", directions = 8, reciprocal = true }
note = "1 / the mean resistance of each pair of neighbours."

[[nodes]]
id = "corrected"
op = "cost.geo_correction"
inputs = { transition = "transition.transition" }
params = { type = "c" }
note = "Divided by the distance between the cells' centres."

[[nodes]]
id = "graph"
op = "conn.graph_from_raster"
inputs = { map = "land.raster" }
params = { habitat = [23, 24, 25] }

[[nodes]]
id = "patches"
op = "conn.nodes"
inputs = { map = "land.raster" }
params = { habitat = [23, 24, 25] }

[[nodes]]
id = "patch"
op = "conn.to_map"
inputs = { graph = "graph.graph", table = "patches.table" }
params = { column = "id" }

[[nodes]]
id = "south"
op = "raster.calc"
inputs = { p = "patch.map" }
params = { expression = "coalesce(p == 55, 0)" }
note = "The southern forest: 1 inside, 0 elsewhere."

[[nodes]]
id = "cost"
op = "cost.acc_cost"
inputs = { transition = "corrected.transition", origins = "south.map" }
note = "The cost of the cheapest route from the forest to every cell."

[[nodes]]
id = "forests"
op = "vector.read"
params = { path = "forests.geojson" }
note = "A point in each of five forests; the first is the southern one."

[[nodes]]
id = "paths"
op = "cost.shortest_path"
inputs = { transition = "corrected.transition", origin = "forests.vector", goals = "forests.vector" }

[[nodes]]
id = "least_costs"
op = "cost.distance"
inputs = { transition = "corrected.transition", from = "forests.vector" }
```

To keep the paths, connect `paths.lines` to a [[vector.write]] (a GeoPackage or GeoJSON); the application also
draws them over a map when you open them.

!!! note "gdistance and GuidosToolbox"
    The `restoration` operators of [costs and corridors](/tutorials/distance-restoration/costs-and-corridors/)
    compute costs too, with GuidosToolbox's conventions (cells as resistances, costs in cells). The `cost`
    operators follow gdistance's (transitions of conductances, costs in map units) and give its numbers exactly,
    so that a study done in R can be repeated in Terraflux.

## Read the results

### The accumulated cost and its isochrones

![The accumulated cost from the southern forest, with isochrones and the least-cost paths to four forests](/img/tutorials/grid-distances/cost-distance/acc-cost.png "The accumulated cost from the southern forest (in thousands: km of forest), with isochrones (white) at 25, 50, 100, 150, 200, 300 and 400, and the least-cost paths (orange) from the point in the southern forest to the four other forests.")

The isochrones are far from circles. They stretch north along the belt of woods and pastures in the centre-west,
and pinch against Hannover in the east, where the cost rises fastest: the land east of the city lies beyond the
300 and 400 lines though it is no farther than the north-west. Lake Steinhude makes a hole of high cost north of
the 100 line. The south-western forest lies just beyond the isochrone of 100 (as much effort as crossing 100 km
of forest, or 5 km of arable land); the north-eastern forest near the 200 line.

### Least-cost paths and costs

The paths follow the cheap land: from the southern forest they run north through the centre-west, then branch
west to the north-western forest and east to the north-eastern ones; the path to the south-western forest goes
north first, to cross the farmland where it is narrowest. They are far longer than straight lines: 45 km to the
north-eastern forest (29 km straight), 71 km to the far north-eastern one (43 km straight).

| Pair of forests | Straight distance (km) | Least cost (thousands) | Ratio |
|---|---|---|---|
| South – South-west | 19.2 | 102.3 | 5.3 |
| South – North-east | 29.3 | 218.9 | 7.5 |
| South – North-west | 33.3 | 192.4 | 5.8 |
| South – Far north-east | 42.6 | 279.5 | 6.6 |
| North-east – North-west | 24.5 | 75.2 | 3.1 |
| North-east – Far north-east | 14.2 | 61.1 | 4.3 |
| North-west – Far north-east | 35.3 | 132.0 | 3.7 |
| South-west – North-west | 27.3 | 156.6 | 5.7 |
| North-east – South-west | 37.9 | 230.8 | 6.1 |
| South-west – Far north-east | 52.1 | 287.6 | 5.5 |

```tfchart
type: bar
data: /data/tutorials/grid-distances/cost-distance/least-costs.csv
x: pair
y: [ratio]
labels: { ratio: "Least cost / straight distance" }
colors: { ratio: "#3b8a5a" }
ylabel: "Thousands of cost per km"
height: 280
caption: "The least cost of each pair over its straight distance. Pairs in the north (North-east – North-west: 3.1) are joined by woods and pastures; pairs that must cross the southern farmland or pass Hannover cost twice as much per kilometre."
```

The ratio of cost to distance is the landscape's "roughness" between two forests. The northern forests are
cheap to join (3 to 4 thousand per km: much of the way through woods and pasture); the southern forest is costly
to leave towards the north-east (7.5: open farmland, then Hannover's suburbs). A habitat graph with straight
distances treats these pairs as alike; with least costs it does not ([least-cost links](/tutorials/habitat-networks/least-cost-links/)).

### A least-cost corridor

The same model gives a corridor between two *areas*: the accumulated cost from the southern forest plus the
accumulated cost from the north-eastern forest (a second [[cost.acc_cost]] from a mask of patch 96, and a
[[raster.calc]] `a + b`) is, in each cell, the cost of the best route between the two forests through that cell.
Its least value (202,287 here) is the cost of the least-cost path; cells a little above it are on routes almost
as good.

![The least-cost corridor between the southern forest and the north-eastern one](/img/tutorials/grid-distances/cost-distance/corridor.png "The cells whose best route between the southern and the north-eastern forests (green) costs at most 2% (red), 5% (orange) or 10% (pale) more than the least-cost path, over the land cover (pale). The corridor runs north through the centre-west and turns east past Lake Steinhude.")

Within 2% of the least cost lie 4,003 cells (40 km²); within 5%, 7,133; within 10%, 14,771. The corridor is
narrow where every route must pass the same place, as between Lake Steinhude and the villages to its east (a
pinch point for this pair), and wide in the woods of the centre-west, where routes of almost equal cost run side
by side. The
threshold is a choice: Beier et al. (2008) discuss choosing it so that the corridor has a width the species needs
along its whole length.

!!! paper "In the literature"
    Report the resistance and its sources, the neighbourhood (4, 8 or 16), the transition function, the
    geographic correction, the origins and goals, and for corridors the threshold. Adriaensen et al. (2003),
    Beier et al. (2008) and Etherington (2016) are the references for the method; van Etten (2017) for gdistance,
    whose operators these are.

## Settings that change the answer

- **The resistance**, above all its contrasts (see the [sensitivity analysis](/tutorials/circuit-connectivity/resistance-surfaces/)).
- **Neighbours**: 4 (no diagonals: paths look like staircases and costs are higher), 8 (the usual), 16 (knight's
  moves too: smoother paths, a little cheaper).
- **The transition function**: the mean of the two cells' conductances, or the minimum (a barrier cell blocks the
  connection), or a harmonic or geometric mean.
- **The corridor's threshold**: 2%, 5% or 10% give corridors of very different widths.
- **Points or areas**: from a point, the path starts at that cell; from an area, from its cheapest edge. Use areas
  for reserves.

## Common pitfalls

- **Resistance of 0 or no value.** A cell without a value is not in the graph: a barrier. A resistance of 0 makes
  an infinite conductance; avoid it.
- **Forgetting the geographic correction.** Without it, diagonal steps cost the same as side steps, and costs are
  not in map units.
- **Comparing costs between resistance surfaces.** Costs scale with the resistance; compare paths and rankings,
  not raw costs.
- **Paths at the map's edge.** A path that hugs the edge may want to go around something outside the map. Use a
  study area larger than the region of interest.
- **One path as "the" corridor.** A path is one route; draw the corridor and say how wide it is.

## Going further

- [Part 2: random walks and randomised shortest paths](../random-walks/): from the best route to walkers that do
  not know the way.
- [Costs and corridors](/tutorials/distance-restoration/costs-and-corridors/): what restoring a corridor gains.
- [Least-cost links](/tutorials/habitat-networks/least-cost-links/): least costs in a habitat graph.
- [Circuitscape](/tutorials/circuit-connectivity/circuitscape/): currents instead of a path.
- The operators: [[cost.transition]], [[cost.geo_correction]], [[cost.acc_cost]], [[cost.shortest_path]],
  [[cost.distance]], [[cost.to_map]], [[cost.math]], [[cost.combine]].

## References

- Adriaensen, F., Chardon, J. P., De Blust, G., Swinnen, E., Villalba, S., Gulinck, H., & Matthysen, E. (2003). The application of 'least-cost' modelling as a functional landscape model. *Landscape and Urban Planning*, 64(4), 233–247. https://doi.org/10.1016/S0169-2046(02)00242-6
- Beier, P., Majka, D. R., & Spencer, W. D. (2008). Forks in the road: choices in procedures for designing wildland linkages. *Conservation Biology*, 22(4), 836–851. https://doi.org/10.1111/j.1523-1739.2008.00942.x
- Dijkstra, E. W. (1959). A note on two problems in connexion with graphs. *Numerische Mathematik*, 1(1), 269–271. https://doi.org/10.1007/BF01386390
- Etherington, T. R. (2016). Least-cost modelling and landscape ecology: concepts, applications, and opportunities. *Current Landscape Ecology Reports*, 1(1), 40–53. https://doi.org/10.1007/s40823-016-0006-9
- van Etten, J. (2017). R package gdistance: distances and routes on geographical grids. *Journal of Statistical Software*, 76(13). https://doi.org/10.18637/jss.v076.i13
