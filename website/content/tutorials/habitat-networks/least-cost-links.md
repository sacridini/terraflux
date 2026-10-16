---
title: Least-cost links between patches
short: Least-cost links
description: Replace the straight gaps between patches with the cost of the cheapest route through a map of resistance made from land cover, and see which links, components and stepping stones change when farmland and towns are not equally easy to cross.
order: 40
level: intermediate
time: 45 min
template: Habitat network (Conefor)
ops: [raster.reclassify, conn.graph_from_raster, conn.distances, conn.overall, conn.node_importance, conn.to_map]
series: Habitat networks
part: 4
---

So far the gap between two forests was a straight distance, as if 800 m of wheat, 800 m of pasture and 800 m of
town were the same obstacle. They are not, for almost any forest species. In this part you give every land cover
class a **resistance**, the cost of crossing one of its cells, and measure the distance between patches as the
cost of the **cheapest route** between them. You build the resistance from CORINE's classes, make the forest
graph with least-cost distances, and compare its components and the importance of its patches with those of
the straight graph. At the end you will know when least-cost links change the answer, how to choose the distance
that goes with them, and one trap they hide.

## The idea

### Resistance

A **resistance surface** is a raster in which every cell holds the difficulty, cost or risk of moving through it
for a species: low in the habitat, higher in land it avoids or where it dies more often, very high for barriers
such as motorways or lakes. Resistances are relative: what matters is that arable land is, say, twenty times
harder to cross than forest, not the absolute numbers. Zeller et al. (2012) reviewed how they are made:

- **Expert opinion**, as here: a value for each land cover class, from the literature and from people who know
  the species. Quick, transparent and the most common, but subjective.
- **Habitat suitability**: the inverse of a species distribution model's suitability (often transformed, since
  a cell of medium suitability is usually much easier to cross than its value suggests; Keeley et al., 2016).
- **Movement or genetic data**: resistances fitted so that the costs explain observed movements (GPS tracks,
  step selection) or genetic distances between populations (Spear et al., 2010).

The [circuit connectivity](/tutorials/circuit-connectivity/resistance-surfaces/) series has a whole part on
building and testing resistance surfaces.

### The least-cost distance

The **least-cost distance** between two places is the smallest sum of resistances along any route between them
on the grid, each step weighted by its length (a diagonal step is √2 times longer). Adriaensen et al. (2003)
made it popular in landscape ecology as a "functional" distance: two woods 1 km apart across pasture can be
closer, for a forest bird, than two woods 600 m apart across a town.

!!! science "Key idea: least-cost links"
    With the resistance \(r\) of each cell, a step from cell \(u\) to its neighbour \(v\) costs
    \(r_v\, \ell_{uv}\) (Makurhini's rule, the resistance of the cell entered), with \(\ell_{uv}\) the step's
    length: 100 m or 141 m with cells of 100 m. The least-cost distance \(c_{ij}\) between patches \(i\) and
    \(j\) is the cost of the cheapest chain of steps between them.

    With a resistance of 1 in the forest, \(c_{ij}\) is in **"metres of forest"**: crossing 1 km of arable land
    (resistance 20) costs 20,000, as much as 20 km of forest. The kernel of part 2 is applied to \(c_{ij}\) in
    place of \(d_{ij}\), so **the dispersal distance must be given in the same units**: a species that crosses
    1 km of arable land with probability one half has \(p = 0.5\) at a cost of 20,000.

The same model of movement underlies the [grid distances](/tutorials/grid-distances/cost-distance/) series, which
maps accumulated costs and draws the paths themselves; here we only need their costs, between every pair of
patches.

## When to use it

- **When the matrix is not uniform**: farmland with towns, motorways, rivers, a mix of pasture and crops. In a
  landscape of one kind of matrix (a forest in a sea of wheat), straight distances say almost the same thing.
- **When barriers matter**: a lake or a city between two forests makes them far apart however close they are.
- **When the result will be compared with movement or genetic data**, which follow the land, not straight lines.

What it is *not*: a least-cost distance assumes the disperser knows the best route, which a juvenile in unknown
country does not. Circuit theory spreads movement over all routes instead
([circuit connectivity](/tutorials/circuit-connectivity/circuit-theory/)), and Terraflux's habitat graphs can
also use **commute-time** distances, from a random walk over the same resistance (`distance = "commute-time"`).

## What you need

- The land cover of the series, [`clc_100m.tif`](/data/tutorials/habitat-networks/connectivity-as-a-graph/clc_100m.tif).
- **A resistance per class.** The values below are for a generalist forest mammal that uses woody vegetation
  readily, crosses grassland and mixed farming with some risk, open arable land with more, and avoids towns and
  water. They are an example: for your species, take them from the literature and test alternatives.

| CORINE classes | Land cover | Resistance |
|---|---|---|
| 23, 24, 25 | Forests | 1 |
| 29 | Transitional woodland-shrub | 2 |
| 21, 26, 27, 32, 35, 36 | Farmland with natural vegetation, grassland, heath, bogs | 5 |
| 18, 20 | Pastures, complex cultivation | 10 |
| 12 (and any other class) | Arable land | 20 |
| 10, 11 | Green urban areas, sport and leisure | 30 |
| 7, 8, 9 | Mines, dumps, construction sites | 50 |
| 1–6, 40, 41 | Towns, industry, roads, airports, rivers and lakes | 100 |

![The resistance made from the land cover](/img/tutorials/habitat-networks/least-cost-links/resistance.png "The resistance of each cell (logarithmic): 1 in the forests (pale yellow), 10 to 20 in the farmland (purple), 100 in towns, roads and Lake Steinhude (black). Hannover in the east and the lake in the west are barriers; the villages scatter smaller ones everywhere.")

## Build the model

### 1. The resistance

Add [[raster.reclassify]] after the [[raster.read]] of the land cover. Its `rules` are one line per rule, the
classes before the arrow and the new value after; `*` takes every value not yet given. In the Node pane the rules
are a table (**A rule per class** lists the map's classes); in the model file they are a text with `\n` between
lines:

```text
23 24 25 -> 1
29 -> 2
21 26 27 32 35 36 -> 5
18 20 -> 10
12 -> 20
10 11 -> 30
7 8 9 -> 50
1 2 3 4 5 6 40 41 -> 100
* -> 20
```

### 2. The graph with least-cost distances

Connect the land cover to the `map` of [[conn.graph_from_raster]], as before, **and** the resistance to its
`resistance` input. Set `distance = "least-cost"`. The other settings of least-cost distances:

- `cost_directions`: 8 (default) or 4 neighbours for the steps.
- `cost_function`: `makurhini` (default), the cost of a step is the resistance of the cell entered (Makurhini's
  rule); `mean`, the mean of the two cells' resistances (gdistance's usual rule).
- `unreachable`: pairs that no route joins (a patch on an island) get their straight distance (`euclidean`, the
  default) or no link (`drop`).

The least-cost distances run between the **centres** of the patches (as Makurhini computes them), so they include
the cheap crossing of each patch's own forest.

```tfgraph
#! caption: The forest graph twice, with straight (edge-to-edge) and least-cost distances over a resistance made from the land cover, and the importance of every patch in each.
#! numbered: true
#! highlight: resistance, least_cost, imp_cost
format = "terraflux/1"

[[nodes]]
id = "land"
op = "raster.read"
params = { path = "clc_100m.tif" }
note = "CORINE Land Cover, cells of 100 m."

[[nodes]]
id = "resistance"
op = "raster.reclassify"
inputs = { map = "land.raster" }
params = { rules = "23 24 25 -> 1\n29 -> 2\n21 26 27 32 35 36 -> 5\n18 20 -> 10\n12 -> 20\n10 11 -> 30\n7 8 9 -> 50\n1 2 3 4 5 6 40 41 -> 100\n* -> 20" }
note = "The cost of crossing each class, 1 (forest) to 100 (towns, water)."

[[nodes]]
id = "straight"
op = "conn.graph_from_raster"
inputs = { map = "land.raster" }
params = { habitat = [23, 24, 25], area_unit = "ha", distance = "edge", landscape_area = 250000 }
note = "Gaps between edges, in metres."

[[nodes]]
id = "least_cost"
op = "conn.graph_from_raster"
inputs = { map = "land.raster", resistance = "resistance.map" }
params = { habitat = [23, 24, 25], area_unit = "ha", distance = "least-cost", landscape_area = 250000 }
note = "Cheapest routes between the patches' centres, in metres of forest."

[[nodes]]
id = "imp_straight"
op = "conn.node_importance"
inputs = { graph = "straight.graph" }
params = { distance = 1000, probability = 0.5 }
note = "Half cross 1 km."

[[nodes]]
id = "imp_cost"
op = "conn.node_importance"
inputs = { graph = "least_cost.graph" }
params = { distance = 20000, probability = 0.5 }
note = "Half cross a cost of 20,000: 1 km of arable land."

[[nodes]]
id = "dpc_cost"
op = "conn.to_map"
inputs = { graph = "least_cost.graph", table = "imp_cost.table" }
params = { column = "dPC" }
```

### 3. Matching the two distances

To compare the two graphs, the species must be the same in both. A species that crosses 1 km of open land
with probability one half (part 3's straight graph at 1 km) crosses 1 km of arable land, the commonest matrix
here, at a cost of 20,000. So the least-cost graph is read at `distance = 20000`, and at 10,000, 40,000 and
100,000 for the straight 500 m, 2 km and 5 km. The ratio is a modelling choice: say what it is.

[[conn.distances]] with the same settings gives the pairs table itself, to see how the costs compare with the
gaps: here the least cost of a pair is a median 6.6 times its straight gap in metres (from 4 to 12 for most
pairs). Crossing pasture is cheaper than 20 per metre and towns dearer, and part of each cost is the forest of
the two patches.

## Read the results

### The network as a whole

| Species crossing (p = 0.5) | Components, straight | Components, least-cost | EC(PC) straight (ha) | EC(PC) least-cost (ha) | Step fraction straight | Step fraction least-cost |
|---|---|---|---|---|---|---|
| 500 m of farmland | 83 | 84 | 15,677 | 15,190 | 7.7% | 0.5% |
| 1 km | 56 | 46 | 17,911 | 17,681 | 17.4% | 0.7% |
| 2 km | 11 | 13 | 22,741 | 22,991 | 31.8% | 0.6% |
| 5 km | 2 | 3 | 32,807 | 33,401 | 42.6% | 0.0% |

The equivalent connected area is almost the same with both distances: the matching above works, and in this
landscape, where arable land dominates the matrix, the straight gap is a fair proxy *on average*. The components
differ more (46 instead of 56 at 1 km: some pairs are cheaper through pasture than their gap suggests, others
dearer around villages).

The striking difference is the **step fraction**: from 17–43% with straight gaps to under 1% with least costs.
This is not a property of the landscape but of the distances. A least-cost route between two patches may run
*through* a third patch on the way, at the low cost of forest; so the cost from A to C is never more than the
cost from A to B plus the cost from B to C, and the best path in the graph is almost always the direct link. The
stepping-stone role has moved *into the distances*.

### The importance of each patch

Drag the handle to compare the importance of each patch with straight gaps (left) and with least-cost links
(right), on the same scale:

```tfcompare
caption: "dPC of every patch, straight gaps at 1 km (left of the handle) and least-cost links at 20,000 (right), same logarithmic scale from 0.01% (dark) to 30% (yellow). The large forests keep their rank; the small woods of the northern chain lose most of their importance."
before: { src: /img/tutorials/habitat-networks/least-cost-links/dpc_straight.png, label: "Straight gaps, 1 km" }
after: { src: /img/tutorials/habitat-networks/least-cost-links/dpc_cost.png, label: "Least cost, 20,000" }
```

![The colour scale of the two maps](/img/tutorials/habitat-networks/least-cost-links/dpc-scale.png "The scale of both maps: dPC, the share of PC lost without the patch.")

![dPC of every patch with straight gaps against with least-cost links](/img/tutorials/habitat-networks/least-cost-links/straight-vs-cost.png "Each point is a patch: its dPC with straight gaps at 1 km (x) and with least-cost links at 20,000 (y), both logarithmic; the dashed line is equality. Most patches lie near it (rank correlation 0.95). The orange points are the small woods whose importance was mostly as stepping stones: it falls ten-fold.")

The rankings agree (Spearman's rank correlation 0.95): nine of the ten most important patches are the same in
both, in nearly the same order. The patches that fall are the small stepping stones of part 3, 9, 77, 81, 94 and 132
(24 to 55 ha): their dPCconnector was 1.0 to 1.6% with straight gaps and is about 0 with least costs.

### Which is right?

Both, for different questions, and here lies the trap. With least-cost distances, the routes between patches
were computed **once, with every patch in place**: the cost from A to C is low *because* the route runs through
forest B. When [[conn.node_importance]] removes B, it removes B as a node but **does not recompute the costs**, so
the route from A to C still benefits from B's forest. The connector role of B is lost from dPC. This is the same
in Conefor and Makurhini, which also compute the distances first.

So:

- to rank patches by their **area and flux**, either distance works, and least costs are more realistic;
- to find **stepping stones**, use straight (edge) distances, or compute the costs without the patch you are
  testing (remove it from the resistance by giving its cells the matrix's resistance, and build the graph again).

!!! paper "In the literature"
    Least-cost distances are common in habitat graphs (Galpern et al., 2011), and ProtConn can use them too
    ([part 6](../protconn/)). Report the resistance values and their source, the cost rule, how the dispersal
    distance was converted to cost units, and, ideally, the results with straight distances beside them so that
    readers see what the resistance changed.

## Settings that change the answer

- **The resistance values.** Least-cost results are more sensitive to the *ratios* between resistances than to
  their absolute values (Etherington, 2016): doubling every value doubles every cost and changes nothing once
  the dispersal distance is doubled too, but making towns 1,000 instead of 100 moves routes. Test two or three
  plausible sets.
- **The cost distance of the kernel.** It sets everything, and there is no single right conversion. Bracket it.
- **`cost_function`.** `makurhini` (the cell entered) and `mean` (the mean of the two cells) give slightly
  different costs at the edges of barriers.
- **Centroids.** Least costs run between patch centres; for a long, curved patch the centre can fall outside it,
  and the cost then includes matrix that a disperser leaving the patch's edge would not cross.

## Common pitfalls

- **A dispersal distance in metres on a graph in cost units.** With `distance = 1000` on the least-cost graph,
  almost nothing is linked (1 km of forest). Always convert.
- **Zero or negative resistances.** Resistances must be positive: give the habitat 1, not 0. Cells with no value
  cannot be crossed.
- **Barriers at the border.** A route that would go around a town just outside the map cannot; analyse a margin
  around the study area.
- **Reading dPCconnector from a least-cost graph** as the stepping-stone value of a patch (see above).

## Going further

- [Part 5: the MSPA network](../mspa-network/): structural corridors from the shape of the forest itself.
- [Grid distances](/tutorials/grid-distances/cost-distance/): the accumulated cost, the paths and the corridors
  between two forests.
- [Resistance surfaces](/tutorials/circuit-connectivity/resistance-surfaces/): how to make and test them.
- The operators: [[conn.graph_from_raster]], [[conn.distances]], [[raster.reclassify]], [[conn.graph_from_samc]]
  (links as the probability that a random walker leaving one patch settles in another).

## References

- Adriaensen, F., Chardon, J. P., De Blust, G., Swinnen, E., Villalba, S., Gulinck, H., & Matthysen, E. (2003). The application of 'least-cost' modelling as a functional landscape model. *Landscape and Urban Planning*, 64(4), 233–247. https://doi.org/10.1016/S0169-2046(02)00242-6
- Etherington, T. R. (2016). Least-cost modelling and landscape ecology: concepts, applications, and opportunities. *Current Landscape Ecology Reports*, 1(1), 40–53. https://doi.org/10.1007/s40823-016-0006-9
- Galpern, P., Manseau, M., & Fall, A. (2011). Patch-based graphs of landscape connectivity: a guide to construction, analysis and application for conservation. *Biological Conservation*, 144(1), 44–55. https://doi.org/10.1016/j.biocon.2010.09.002
- Keeley, A. T. H., Beier, P., & Gagnon, J. W. (2016). Estimating landscape resistance from habitat suitability: effects of data source and nonlinearities. *Landscape Ecology*, 31(9), 2151–2162. https://doi.org/10.1007/s10980-016-0387-5
- Spear, S. F., Balkenhol, N., Fortin, M.-J., McRae, B. H., & Scribner, K. (2010). Use of resistance surfaces for landscape genetic studies: considerations for parameterization and analysis. *Molecular Ecology*, 19(17), 3576–3591. https://doi.org/10.1111/j.1365-294X.2010.04657.x
- Zeller, K. A., McGarigal, K., & Whiteley, A. R. (2012). Estimating landscape resistance to movement: a review. *Landscape Ecology*, 27(6), 777–797. https://doi.org/10.1007/s10980-012-9737-0
