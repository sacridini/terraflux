---
title: "A demand per region: municipalities, basins and their own trends"
short: A demand per region
description: Give every municipality, basin or biome of a map its own demand or its own transition rates, with CLUE-S, CARS and Dinamica EGO's model, see what a single demand for the whole map gets wrong, and write a scenario for one region.
order: 640
level: intermediate
time: 40 min
ops: [demand.regional, demand.table, lulcc.step, plus.demand, cars.step, dinamica.step, glm.fit, glm.predict]
series: Other land change models
part: 5
---

Land does not change at the same pace everywhere: one municipality clears its forest, its neighbour protects
it; a basin urbanises while the next one is abandoned. A model with one demand (or one Markov chain) for the
whole map spreads the change wherever the suitability is highest, regardless of where it really happened. In this
part you will cut Sibuyan Island into three sectors, give each its own demand with [[demand.regional]], run CLUE-S
with one demand for the island and with one per sector, compare where the forest is lost, write a scenario in
which one sector keeps its forest, and run PLUS's CARS with a Markov chain per sector. The same operator serves
Dinamica EGO's Expander and Patcher, whose "regionalization" works the same way. The part ends with a table of
which model answers which question, and a link to the page that compares them in full.

## The idea

### Why regions

Every land change model in this category has a **quantity** (how much changes) and an **allocation** (where).
With one quantity for the map, the allocation alone decides how the change is shared among the parts of the map,
following the suitability or the probability. That is right when the whole map is one system. It is wrong when the
parts have their own drivers that the maps do not show: a municipality's policy, a protected area's enforcement,
a road's construction, the population of a county. Then the quantity itself must be regional.

!!! science "Key idea: a regional demand"
    With regions \(r = 1 \dots R\) (a map of region ids), the demand becomes a set of demands: the cells of each
    class \(u\) at each step \(t\) in each region, \(D_{r,u,t}\), or a Markov chain per region. The allocation
    runs **region by region**: the cells of region \(r\) meet \(D_{r,\cdot,t}\) only, and cells outside every
    region keep their class. The suitabilities and probabilities stay global (one regression or one set of weights
    for the map), unless you fit them per region too.

This is how Dinamica EGO's users model deforestation across municipalities with different rates (its
"regionalization": a transition matrix per region; Soares-Filho et al., 2006, used regions of the Amazon basin),
how CLUE-S, whose demand comes from outside the map (Verburg et al., 2002), is given one demand per province, and how FUTURES gives each county its demand
([FUTURES](/tutorials/land-change/futures/)).

### Sibuyan's three sectors

Sibuyan has three municipalities. Their boundaries are not in the example data, so the island is cut here into
three sectors around its centre, in their place: the north (1), the south-east (2) and the south-west (3).

```tfgallery
columns: 2
caption: The three sectors (in the place of the island's municipalities) and the land use of 1997.
images:
  - { src: /img/tutorials/land-change/regional-demand/regions.png, caption: "Three sectors" }
  - { src: /img/tutorials/land-change/regional-demand/observed_1997.png, caption: "Land use, 1997" }
```

They changed very differently between 1997 and 2011:

| Sector | Forest 1997 | Forest 2011 | Change | Share lost |
|---|---:|---:|---:|---:|
| North | 1,554 | 1,205 | −349 | 22% |
| South-east | 1,385 | 1,287 | −98 | 7% |
| South-west | 1,784 | 1,668 | −116 | 7% |
| Island | 4,723 | 4,160 | −563 | 12% |

The north lost a fifth of its forest, the two southern sectors a fourteenth.

### Where the regional demands come from

A regional demand is only as good as its source. The common ones:

- **The trend of each region**, from its own observed maps, as here: each region continues its own past.
- **Statistics by administrative unit**: agricultural censuses (cropland per municipality), forest inventories,
  housing permits. They often exist for more years than maps, and for the units decisions are taken in.
- **Projections**: population per county turned into developed land per county (as FUTURES does), or the land
  demands of an economic model given per country or province.
- **Scenarios** written for one region: a protected area enforced, a road paved, a municipality's zoning plan.

Whichever the source, check that the regional demands add up to a plausible total, and that each starts from the
region's cells on the start map.

## When to use it

Use regional demands when:

- the change rates are known (or assumed) per administrative unit: deforestation per municipality, population per
  county, cropland targets per province;
- a policy applies to some regions only (a scenario: one municipality enforces its forest code);
- a single demand visibly misplaces change across regions (below).

Do not cut regions so small that each has a handful of changes: the regional rates become noise. And remember
that a region with no change in the calibration period will have none in the simulation.

## What you need

- A **map of regions**: whole ids, on the grid of the land use map (a municipalities' layer rasterized with
  [[vector.rasterize]], or a basins' map). Cells without a region keep their class.
- A **demand per region**: a table of cells per class and step ([[demand.table]]'s form), or a Markov chain per
  region (as [[markov.matrix]]'s), each written into [[demand.regional]].

The demands below are linear trends of each sector's areas from 1997 to 2011, written by the figure script from the
counts of each sector (the same interpolation [[demand.from_maps]] does for a whole map).

## Build the model

### CLUE-S with a demand per sector

```tfgraph
#! caption: CLUE-S with a demand per sector: the regions (node 3) and demand.regional (node 6) replace the single demand; the loop's lulcc.step (node 10) reads it through its regional input. Settings hidden: open the model file for the tables.
#! numbered: true
#! highlight: regions, demand, allocate
#! params: false
format = "terraflux/1"

[[nodes]]
id = "lu1997"
op = "raster.read"
params = { path = "sibuyan_landuse.tif", band = "1997" }

[[nodes]]
id = "drivers"
op = "raster.read"
params = { path = "sibuyan_drivers.tif" }

[[nodes]]
id = "regions"
op = "raster.read"
params = { path = "regions.tif" }

[[nodes]]
id = "regression"
op = "glm.fit"
inputs = { landscape = "lu1997.raster", drivers = "drivers.raster" }
params = { formulas = { 1 = ["dist_sea", "elevation", "slope", "dist_roads", "dist_urban"], 2 = ["dist_sea", "elevation", "slope", "dist_roads", "dist_urban"], 3 = ["dist_sea", "elevation", "slope", "dist_roads", "dist_urban"], 4 = ["dist_sea", "elevation", "slope", "dist_roads", "dist_urban"], 5 = ["dist_sea", "elevation", "slope", "dist_roads", "dist_urban"] } }

[[nodes]]
id = "suitability"
op = "glm.predict"
inputs = { model = "regression.model", drivers = "drivers.raster", landscape = "lu1997.raster" }

[[nodes]]
id = "demand"
op = "demand.regional"
inputs = { regions = "regions.raster" }
params = { ids = [1, 2, 3], demands = [{ table = { classes = [1, 2, 3, 4, 5], rows = [[1554.0, 158.0, 209.0, 129.0, 148.0], [1529.0, 166.0, 224.0, 131.0, 148.0], [1504.0, 174.0, 238.0, 134.0, 148.0], [1479.0, 183.0, 252.0, 136.0, 148.0], [1454.0, 191.0, 267.0, 138.0, 148.0], [1429.0, 199.0, 281.0, 141.0, 148.0], [1404.0, 207.0, 295.0, 143.0, 149.0], [1380.0, 216.0, 309.0, 145.0, 148.0], [1355.0, 224.0, 324.0, 147.0, 148.0], [1330.0, 232.0, 338.0, 149.0, 149.0], [1305.0, 240.0, 352.0, 152.0, 149.0], [1280.0, 248.0, 367.0, 154.0, 149.0], [1255.0, 257.0, 381.0, 156.0, 149.0], [1230.0, 265.0, 395.0, 159.0, 149.0], [1205.0, 273.0, 410.0, 161.0, 149.0]], times = [0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0] } }, { table = { classes = [1, 2, 3, 4, 5], rows = [[1385.0, 869.0, 568.0, 72.0, 33.0], [1378.0, 877.0, 570.0, 69.0, 33.0], [1371.0, 884.0, 573.0, 66.0, 33.0], [1364.0, 892.0, 575.0, 63.0, 33.0], [1357.0, 900.0, 577.0, 60.0, 33.0], [1350.0, 907.0, 580.0, 57.0, 33.0], [1343.0, 915.0, 582.0, 54.0, 33.0], [1336.0, 923.0, 584.0, 51.0, 33.0], [1329.0, 930.0, 586.0, 49.0, 33.0], [1322.0, 938.0, 588.0, 46.0, 33.0], [1315.0, 945.0, 591.0, 43.0, 33.0], [1308.0, 953.0, 593.0, 40.0, 33.0], [1301.0, 961.0, 595.0, 37.0, 33.0], [1294.0, 968.0, 598.0, 34.0, 33.0], [1287.0, 976.0, 600.0, 31.0, 33.0]], times = [0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0] } }, { table = { classes = [1, 2, 3, 4, 5], rows = [[1784.0, 131.0, 62.0, 23.0, 101.0], [1776.0, 133.0, 63.0, 28.0, 101.0], [1767.0, 134.0, 65.0, 34.0, 101.0], [1759.0, 136.0, 66.0, 39.0, 101.0], [1751.0, 138.0, 67.0, 44.0, 101.0], [1743.0, 140.0, 68.0, 49.0, 101.0], [1734.0, 141.0, 70.0, 54.0, 102.0], [1726.0, 143.0, 71.0, 60.0, 101.0], [1718.0, 145.0, 72.0, 65.0, 101.0], [1709.0, 146.0, 74.0, 70.0, 102.0], [1701.0, 148.0, 75.0, 75.0, 102.0], [1693.0, 150.0, 76.0, 80.0, 102.0], [1685.0, 152.0, 77.0, 85.0, 102.0], [1676.0, 153.0, 79.0, 91.0, 102.0], [1668.0, 155.0, 80.0, 96.0, 102.0]], times = [0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0] } }] }

[[nodes]]
id = "rng"
op = "rng.r"
inputs = { seed = 1 }

[[nodes]]
id = "history"
op = "lulcc.history"
inputs = { landscape = "lu1997.raster" }

[[nodes]]
id = "simulation"
op = "control.repeat"
inputs = { times = 14, landscape = "lu1997.raster", history = "history.history" }

[nodes.body]
carry = { landscape = "allocate.landscape", history = "allocate.history" }
collect = { maps = "allocate.landscape", info = "allocate.info" }

[[nodes.body.nodes]]
id = "allocate"
op = "lulcc.step"
inputs = { landscape = "@landscape", history = "@history", suitability = "suitability.suitability", regional = "demand.regional", step = "@step", rng = "rng.rng" }
params = { allocation = { method = "clue", jitter = 0.0001, scale = 0.0005, max_iter = 1000, max_diff = 20, mean_diff = 10 }, elasticity = [0.3, 0.3, 0.3, 0.3, 0.3] }

[outputs.maps]
from = "simulation.maps"

[outputs.steps]
from = "simulation.info"
```

It is the CLUE-S model of [part 4 of the CLUE-S series](/tutorials/land-change/clue-s-model/) with two changes:

1. **`regions`** (node 3): a [[raster.read]] of the map of sector ids, on the land use map's grid.
2. **`demand`** (node 6, [[demand.regional]]): its `regions` input is that map; its settings are `ids`, the region
   ids (`[1, 2, 3]`), and `demands`, a list in the same order with one entry per region. Each entry is either
   `{ table = { classes, rows, times } }` (cells of each class, a row per step, the first the start) or
   `{ markov = { classes, single, multi, steps } }` (a chain). Its outputs: `regional` and `steps`.
3. **`allocate`** (node 10, [[lulcc.step]]): its `regional` input takes `demand.regional` in place of `demand`.
   It allocates each region separately, with its own iteration variables, on the global suitability.

The rows of each region must add up, in every step, to that region's cells with a class (here 2,198, 2,927 and
2,101), and each region's first row must be its counts on the start map.

### CARS and Dinamica EGO

The same [[demand.regional]] node serves the other allocations:

- **CARS** (PLUS): [[plus.demand]] reads it in its `regional` input and projects each region's chain from the start
  map into cells per class and step; [[cars.step]] reads the result in its own `regional` input.
- **Dinamica EGO**: [[dinamica.step]] takes `regional` (a Markov chain per region) in place of `rates`; each region
  changes its rate times its own cells, the regions in the order of their ids, each with a seed of its own.

```toml
[[nodes]]
id = "chains"
op = "demand.regional"
inputs = { regions = "regions.raster" }
params = { ids = [1, 2, 3], demands = [
  { markov = { classes = [1, 2, 3, 4, 5], single = [[0.7754, 0.0483, 0.1416, 0.0347, 0.0], ...], steps = 14 } },
  ...
] }

[[nodes]]
id = "demand"
op = "plus.demand"
inputs = { start = "lu1997.raster", regional = "chains.regional" }
params = { steps = 14 }
```

(The full CARS model is in [cars.toml](/data/tutorials/land-change/regional-demand/cars.toml).) The chain of each
sector here is its observed matrix of 1997-2011 (`single`, a row per class: the share of its cells that hold each
class in 2011), with `steps = 14`.

## Read the results

### One demand against a demand per sector

```tfchart
type: bar
data: /data/tutorials/land-change/regional-demand/forest_change.csv
x: region
y: [forest_observed, forest_island, forest_regional]
labels: { forest_observed: "Observed", forest_island: "One demand for the island", forest_regional: "A demand per sector" }
colors: { forest_observed: "#1c2420", forest_island: "#e8901f", forest_regional: "#3b8a5a" }
xlabel: Sector
ylabel: Change of forest, 1997-2011 (cells)
height: 300
caption: The forest gained or lost in each sector from 1997 to 2011: observed, simulated by CLUE-S with one demand for the island, and with a demand per sector.
```

With one demand for the island, CLUE-S takes the island's forest loss where the suitabilities point: 468 cells in
the north (observed 349), 214 in the south-west (observed 116), and it **grows** forest by 91 cells in the
south-east, which lost 98. The island's total is right; its geography is not. With a demand per sector, each
sector loses what it lost (330, 81 and 100 cells; within the tolerance of 20 cells per class and step of the
allocation, plus the elasticity's resistance). The island's run also had steps that did not converge (see the
pitfalls), but they miss the demand by a few tens of cells: the misplacement between sectors is ten times larger.

```tfgallery
columns: 3
caption: Sibuyan in 2011: observed, simulated by CLUE-S with one demand for the island, and with a demand per sector (forest green, coconut ochre, grassland light green, rice blue, other purple).
images:
  - { src: /img/tutorials/land-change/regional-demand/observed_2011.png, caption: "Observed 2011" }
  - { src: /img/tutorials/land-change/regional-demand/island_2011.png, caption: "One demand" }
  - { src: /img/tutorials/land-change/regional-demand/regional_2011.png, caption: "A demand per sector" }
```

### CARS with a chain per sector

![CARS with a Markov chain per sector](/img/tutorials/land-change/regional-demand/cars_2011.png "Sibuyan in 2011 simulated by CARS with a Markov chain per sector: every class meets each sector's projected cells exactly (forest -349, -98 and -116 cells, as observed).")

[[plus.demand]] projects each sector's chain from its own cells, and CARS meets each sector's demand to the cell:
the forest of the three sectors changes by exactly the observed −349, −98 and −116 cells.

### A scenario for one sector

A regional demand is also the natural place for a scenario that concerns one region. Here: the north keeps its
forest of 1997, its other classes growing as in the trend, with the grassland that would have replaced the forest
not appearing. Only the north's table changes in [[demand.regional]]:

![A scenario: the north keeps its forest](/img/tutorials/land-change/regional-demand/scenario_2011.png "Sibuyan in 2011 under the scenario: the north keeps its 1,554 cells of forest (simulated +4), the other sectors follow their trends.")

The north ends with its forest of 1997 (4 cells more, within the tolerance), while the south-east and the
south-west lose 81 and 103 cells as before: the regions do not exchange their demands.

### What to report

The regions and why (administrative, ecological), their demands (tables or chains, and their source), whether the
potential is global or per region, and the simulated change per region next to the observed one.

## Settings that change the answer

- **The regions themselves.** Different boundaries give different trends; regions should be the units whose
  decisions or data drive the demand.
- **Global or regional potential.** The suitability here is fitted on the whole island. If the drivers act
  differently in each region (the east of [FUTURES' example](/tutorials/land-change/futures/#step-1-the-pressure-the-potential-and-the-patch-library)
  responds twice as strongly to the pressure), fit one per region, as FUTURES does.
- **The allocation's settings per region.** CLUE-S's tolerances (`max_diff`, `mean_diff`) apply to each region:
  small regions need small tolerances.

## Common pitfalls

- **Rows that do not add up** to the region's cells: the allocation cannot meet the demand.
- **Cells outside every region**, or a region id not in `ids`: they keep their class silently.
- **Too small regions**, with few observed changes: their rates are noise.
- **Non-convergence.** With one demand for the island, 6 of the 14 steps of the run above did not converge in
  1,000 iterations (in the last, the largest difference was 28 cells against a tolerance of 20): with five classes
  competing over the whole island, the allocation's `scale` of 0.0005 makes the iteration variables overshoot.
  With a demand per sector, every step converged in 11 to 31 iterations. Read `converged` in `steps.csv` in both
  cases, and lower `scale` when steps fail.

## Which model for which question

The land change models of this category answer different questions. A summary; [Choosing a land change
model](/tutorials/land-change/choosing-a-model/) compares them in full.

| Your question | Model | Quantity from | Shape of the change |
|---|---|---|---|
| Where will deforestation go if the trend continues, and in what patches? | [Dinamica EGO](/tutorials/land-change/dinamica-on-sibuyan/) | Markov rates | patches: expansion and new |
| Where do the areas of a scenario go, several classes competing? | [CLUE-S](/tutorials/land-change/clue-s-model/) | a demand of areas | cell by cell |
| Is a demand for food or wood met by clearing or by intensifying? | [CLUMondo](/tutorials/land-change/clumondo/) | a demand of goods | land systems |
| How do several classes expand, with new patches seeded by chance? | [PLUS (CARS)](/tutorials/land-change/plus-cars/) | Markov or areas | random patch seeds |
| How does a town grow, infill or sprawl, county by county? | [FUTURES](/tutorials/land-change/futures/) | developed cells per subregion | patches from a library |
| What would a MOLUSCE study give, or a quick first model? | [MOLUSCE](/tutorials/land-change/molusce/) | the observed cross table | cell by cell |
| Different rates in different municipalities or basins? | any of the above with [[demand.regional]] | per region | the model's |
| How do decisions of farmers or managers change the land? | [agents](/tutorials/land-change/agents/), [CRAFTY](/tutorials/land-change/crafty/) | the agents' decisions, services | cell or parcel |
| Does a driver that the change itself alters matter? | any, with [dynamic drivers](/tutorials/land-change/dynamic-drivers/) | the model's | the model's |

## Going further

- [Choosing a land change model](/tutorials/land-change/choosing-a-model/), the next part.
- [Dynamic drivers](/tutorials/land-change/dynamic-drivers/) and [agents](/tutorials/land-change/agents/), for
  drivers and decisions that vary with the simulation.
- The operators: [[demand.regional]], [[demand.table]], [[plus.demand]], [[lulcc.step]], [[cars.step]],
  [[dinamica.step]].

## References

- Soares-Filho, B. S., Nepstad, D. C., Curran, L. M., Cerqueira, G. C., Garcia, R. A., Ramos, C. A., et al. (2006). Modelling conservation in the Amazon basin. *Nature*, 440(7083), 520–523. https://doi.org/10.1038/nature04389
- Verburg, P. H., Soepboer, W., Veldkamp, A., Limpiada, R., Espaldon, V., & Mastura, S. S. A. (2002). Modeling the spatial dynamics of regional land use: the CLUE-S model. *Environmental Management*, 30(3), 391–405. https://doi.org/10.1007/s00267-002-2630-x
