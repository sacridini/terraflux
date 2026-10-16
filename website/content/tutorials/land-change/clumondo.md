---
title: "CLUMondo: land systems and the services they supply"
short: CLUMondo
description: Simulate land systems that compete to supply food and wood, and see when a demand is met by intensifying the land rather than clearing more of it.
order: 600
level: advanced
time: 60 min
ops: [clumondo.ages, clumondo.step, glm.fit, control.repeat, raster.read]
series: Other land change models
part: 1
---

In this tutorial you build a CLUMondo model of Sibuyan Island, in the Philippines. Instead of telling the model
how many hectares of each land use there must be, you tell it how much **food** and **wood** the island must
produce each year. The model then decides, cell by cell, which land system produces them: it may clear forest
for grassland, or turn grassland into rice paddies that produce twice as much food on the same land. At the end
you will have ten simulated years, a chart of the supply against the demand, and a feel for the one setting that
decides between clearing and intensifying.

## The idea

Most land change models ask for an **area demand**: so many cells of cropland, so many of forest, every year.
That works when the classes are simple land covers, but land is used with very different intensities. A
hectare of irrigated rice and a hectare of extensive grazing are both "agriculture", yet one may feed ten times
more people than the other. When a region needs more food, it can expand agriculture into forest, or it can
intensify the land it already farms. Which of the two happens matters enormously for biodiversity, and an area
demand cannot even express the choice.

**CLUMondo** (van Asselen & Verburg, 2013) answers this by changing what the classes and the demand are:

- The classes are **land systems**: combinations of land cover, livestock and management intensity (for
  example "mosaic of cropland and forest", "intensive cropland", "extensive grazing"). A cell holds one system.
- The demand is for **goods and services**: tonnes of crops, heads of cattle, cubic metres of wood, area of
  protected forest, carbon stored.
- Each land system **supplies** a fixed amount of each service per cell. The supply of the landscape is the sum
  over its cells.

The model must then find a map whose supply matches the demand, and there are usually many. The choice
between them comes from the **suitability** of each cell for each system (a logistic regression on drivers, as
in CLUE-S), from the **conversion resistance** of each system (how hard it is to change it), from rules about
which conversions are allowed, and from the **conversion order**, which says which systems are "more" of each
service.

CLUMondo is the latest member of the CLUE family. CLUE (Veldkamp & Fresco, 1996) modelled land use at coarse
resolution in Costa Rica; CLUE-S (Verburg et al., 2002) brought it to fine resolution for regions, with a demand in
areas (see [the CLUE-S series](/tutorials/land-change/clue-s-ideas/)); Dyna-CLUE (Verburg & Overmars, 2009)
added changes that happen without a demand, such as the regrowth of abandoned fields; and CLUMondo replaced the
areas with services, so that intensification and expansion compete (van Vliet & Verburg, 2018).

![Land systems in 1997](/img/tutorials/land-change/clumondo/year_0.png "Sibuyan Island in 1997: the five classes of the map used as land systems. The forest covers the mountainous centre; coconut, grassland and rice the coastal plains.")

!!! science "Key idea: the score of each system in each cell"
    In every iteration, each cell \(i\) that has system \(o\) takes the system \(k\) of highest score

    \[ T(i,k) = S(i,k) + \sum_{d} w_d \, E_d \, \operatorname{sign}(r_{k,d} - r_{o,d}) + \begin{cases} R_o & k = o \\ 0 & k \neq o \end{cases} \]

    - \(S(i,k)\): the suitability of cell \(i\) for system \(k\) (0 to 1, from a logistic regression);
    - \(E_d\): the **iteration variable** of service \(d\). It starts at 0 and grows while the supply of \(d\)
      falls short of its demand, shrinks while it exceeds it;
    - \(r_{k,d}\): the **conversion order**, the rank of system \(k\) for service \(d\) (higher supplies more). A
      move towards a system of higher rank gains \(E_d\), a move towards a lower one loses it;
    - \(w_d\): the weight of service \(d\) (1 here);
    - \(R_o\): the **conversion resistance** of the current system, added only for staying.

    After each iteration the supply \(\sum_i s_{k(i),d}\) is compared with the demand \(D_d\); the iterations stop
    when the mean deviation over the services and the largest deviation are below their limits (in percent of
    the demand). The values \(E_d\) are carried to the next year, so a year starts where the last one ended.

The resistance makes systems with a high value (a forest that is hard to clear, an orchard that took years to
grow) stay put unless the demand pushes hard. Because the iteration variables act on *pairs* of systems through
their ranks, a growing food demand raises the score of every move from a system that produces less food to one
that produces more: from forest to grassland (expansion) as well as from grassland to rice (intensification).
The suitability and the resistances decide which of those moves wins in each cell.

## When to use it

Use CLUMondo when the question is about **how much land a demand needs**, and not only where it goes:

- **Intensification against expansion.** Will a rising demand for crops clear forest, or will it intensify
  existing farmland? van Asselen and Verburg (2013) ran CLUMondo globally and found that much of the future demand
  could be met by intensification, with large regional differences.
- **Several demands at once.** Food, wood, protected area and carbon can all be services. Eitelberg et al.
  (2016) added demands for biodiversity protection and carbon storage to the global model and showed how they
  shift agriculture elsewhere. Ornetsmüller et al. (2016) compared demands for rice, maize and forest products
  in Laos.
- **Mosaic landscapes**, where classes such as "forest with shifting cultivation" are more meaningful than pure
  land covers.

It is *not* the model for reproducing the exact shape of patches (see [PLUS](/tutorials/land-change/plus-cars/)
or Dinamica EGO, on [the category page](/tutorials/#land-change)), nor for urban growth driven by population
(see [FUTURES](/tutorials/land-change/futures/)). If your demand really is in areas, CLUE-S is simpler and
needs fewer assumptions ([CLUE-S series](/tutorials/land-change/clue-s-ideas/)). [Choosing a
model](/tutorials/land-change/choosing-a-model/) compares them all.

## What you need

- **A map of land systems** at the start: whole numbers, one per system (here `sibuyan_landuse.tif`, band
  `1997`, 128 by 108 cells of 250 m; 1 forest, 2 coconut, 3 grassland, 4 rice, 5 other). A *raster* is a grid of
  cells, each with a value; here the value is the class.
- **Drivers**: rasters of the same grid that explain where each system is (`sibuyan_drivers.tif`: distance to
  the sea, elevation, slope, distance to roads, distance to the town, and others).
- **A supply table**: what one cell of each system supplies of each service. This is the heart of the model and
  usually comes from statistics (yields, livestock densities, timber increments) divided among the systems.
- **A demand table**: each service for each year, the first row being the supply at the start.
- The **conversion resistances**, the **conversion matrix** and the **conversion order** (below).

The data come from the CLUE-S study of Sibuyan (Verburg & Veldkamp, 2004). The supply table here is invented for
the example: a forest cell supplies 1 unit of wood; a coconut cell 0.2 of food and 0.3 of wood; grassland (grazing)
1 of food; rice 2 of food; "other" nothing. In a real study, each number would be justified from data.

## Build the model

The model has five nodes; the last is a loop that runs one CLUMondo year per step.

### 1. The map and the drivers

Read the map of 1997 and the drivers with two `raster.read` nodes. In the application, open the library's
**IO** subject, click `raster.read` to add it, and in the Node pane set `path` to the file and `band` to `1997`.
The drivers node gives a **stack** (all the bands, by name), which is what the regression and the allocation read.

### 2. The suitability of each system

[[glm.fit]] fits a logistic regression of the presence of each system on the drivers. We use five drivers for
every system: distance to the sea, elevation, slope, distance to roads and distance to the town. The regression
gives, for each cell, the probability that a system is there given its drivers: forest on high, steep land far
from roads; rice on flat land near the coast. The model's suitability is these probabilities, recomputed by
`clumondo.step` every year from the regressions and the drivers (connect `model` and `drivers`).

!!! tip "The suitability has more choices"
    `clumondo.step` takes either `model` with `drivers` (logistic regressions, computed in single precision as
    CLUMondo does) or `suitability`, a stack with a layer per system made any other way (a random forest, a
    multi-criteria map). `preferences` adds location-specific preferences (CLUMondo's *locspec* files), for
    example a bonus for forest inside a planned reserve.

### 3. The ages

CLUMondo remembers how long each cell has had its system: some conversions are only allowed after (or before) a
number of years. [[clumondo.ages]] makes the starting ages. Give it a `history` map if you know them; otherwise
`max_age` draws a random age from 1 to `max_age` for every cell, as CLUMondo itself does (with the `seed`). Here
ages from 1 to 10.

```tfgraph
#! caption: The pieces outside the loop: the map, the drivers, the regressions and the starting ages.
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "land"
op = "raster.read"
params = { path = "sibuyan_landuse.tif", band = "1997" }
note = "The land systems at the start."

[[nodes]]
id = "drivers"
op = "raster.read"
params = { path = "sibuyan_drivers.tif" }
note = "The drivers, a band each."

[[nodes]]
id = "suitability"
op = "glm.fit"
inputs = { landscape = "land.raster", drivers = "drivers.raster" }
params = { formulas = { 1 = ["dist_sea", "elevation", "slope", "dist_roads", "dist_urban"], 2 = ["dist_sea", "elevation", "slope", "dist_roads", "dist_urban"], 3 = ["dist_sea", "elevation", "slope", "dist_roads", "dist_urban"], 4 = ["dist_sea", "elevation", "slope", "dist_roads", "dist_urban"], 5 = ["dist_sea", "elevation", "slope", "dist_roads", "dist_urban"] } }
note = "A logistic regression per land system."

[[nodes]]
id = "ages"
op = "clumondo.ages"
inputs = { landscape = "land.raster", seed = 1 }
params = { max_age = 10 }
note = "Random ages from 1 to 10 years, as CLUMondo draws them."
```

### 4. The loop: one year per step

A `control.repeat` node runs its **body** once per step. CLUMondo needs three things carried from one year to the
next, and each becomes a *carried value* of the loop:

- `@landscape` starts as `land.raster` (1997) and comes back from `allocate.landscape`: the map of the year before.
- `@age` starts as `ages.age` and comes back from `allocate.age`: the ages, one more where the system stayed,
  back to 1 where it changed.
- `@elasticity` starts as `[0.0, 0.0]` and comes back from `allocate.elasticity`: the iteration variables
  \(E_d\), one per service (CLUMondo calls them the demand's elasticities).

The third one is easy to miss: the iteration variables found in one year are the starting point of the next, so
a year that needed a strong push toward food starts the next year with that push. Starting them at zero each
year would make every year search from scratch, and it would no longer be CLUMondo.

The body also collects `maps` (the map of every year) and `info` (the iterations, the deviations, the supply
of each service and the cells of each system). And `until = "allocate.stop"`: when a year has no solution (the
demand cannot be met within `max_loops` iterations), CLUMondo stops the simulation there, and so does the loop:
that year is neither collected nor carried, and the outputs end with the last year that worked.

```tfgraph
#! caption: The loop: a CLUMondo year per step. The map, the ages and the iteration variables are carried; the loop ends early if a year has no solution.
#! highlight: years, allocate
format = "terraflux/1"

[[nodes]]
id = "land"
op = "raster.read"
params = { path = "sibuyan_landuse.tif", band = "1997" }

[[nodes]]
id = "drivers"
op = "raster.read"
params = { path = "sibuyan_drivers.tif" }

[[nodes]]
id = "suitability"
op = "glm.fit"
inputs = { landscape = "land.raster", drivers = "drivers.raster" }

[[nodes]]
id = "ages"
op = "clumondo.ages"
inputs = { landscape = "land.raster", seed = 1 }
params = { max_age = 10 }

[[nodes]]
id = "years"
op = "control.repeat"
inputs = { times = 10, landscape = "land.raster", age = "ages.age", elasticity = [0.0, 0.0] }
note = "Ten years; the map, the ages and the iteration variables carried."

[nodes.body]
carry = { landscape = "allocate.landscape", age = "allocate.age", elasticity = "allocate.elasticity" }
collect = { maps = "allocate.landscape", info = "allocate.info" }
until = "allocate.stop"

[[nodes.body.nodes]]
id = "allocate"
op = "clumondo.step"
inputs = { landscape = "@landscape", age = "@age", elasticity = "@elasticity", step = "@step", model = "suitability.model", drivers = "drivers.raster" }
note = "One year of CLUMondo."

[nodes.body.nodes.params.settings]
systems = [1, 2, 3, 4, 5]
services = ["food", "wood"]
supply = [{ step = 1, rows = [[0.0, 1.0], [0.2, 0.3], [1.0, 0.0], [2.0, 0.0], [0.0, 0.0]] }]
demand = [[1518.6, 5070.4], [1564.16, 5070.4], [1611.08, 5070.4], [1659.42, 5070.4], [1709.2, 5070.4], [1760.47, 5070.4], [1813.29, 5070.4], [1867.69, 5070.4], [1923.72, 5070.4], [1981.43, 5070.4], [2040.87, 5070.4]]
order = [[0, 2], [1, 1], [2, 0], [3, 0], [-1, -1]]
weights = [1.0, 1.0]
resistance = [0.8, 0.5, 0.3, 0.6, 1.0]
allowed = [[1, 1, 1, 1, 0], [1, 1, 1, 1, 0], [110, 1, 1, 1, 0], [110, 1, 1, 1, 0], [0, 0, 0, 0, 1]]
deviation = "percent"
mean_deviation = 0.5
max_deviation = 1.0
speed = 0.05
max_loops = 20000
```

In the application, add `control.repeat` from the **Model flow** subject, then double-click it (or use **Edit
the body**) to edit its body. Inside, the column on the left lists what the body can read: `@step`, the carried
values and the outer nodes. CLUMondo's operators are not in the Land change subject of the library; type
`clumondo` in the library's search to find them. Connect `@landscape`, `@age`, `@elasticity` and `@step` to the
inputs of the same names, `suitability.model` to `model` and `drivers.raster` to `drivers`. Then set the loop's
carried values and collected outputs in the Node pane of the loop.

### 5. The settings, one by one

The `settings` of `clumondo.step` are CLUMondo's files in one place. Each entry is a row per land system (in the
order of `systems`) or per service (in the order of `services`).

`supply`
:   What a cell of each system supplies: a row per system, a value per service (CLUMondo's *lusmatrix*). A
    list of `{ step, rows }` lets the supply change over time (yields that improve from step 5, for example);
    the table in force at a step is the last one given at or before it.

`demand`
:   A row per year, a value per service, the first row being the start. Here the food demand grows 3 % a year
    (from the 1518.6 units the island supplied in 1997 to 2040.9 after ten years) and the wood demand stays at
    5070.4.

`order`
:   The conversion order (*lusconv*): per system and service, its rank. For food: forest 0, coconut 1,
    grassland 2, rice 3; for wood: forest 2, coconut 1, the others 0. A rank of `-1` keeps a system out of the
    competition for that service; "other" (villages, roads) takes part in none.

`resistance`
:   The conversion resistance of each system, added to the score of staying: forest 0.8 (hard to clear),
    coconut 0.5 (a plantation of several decades), grassland 0.3 (easy to convert), rice 0.6 (irrigation
    works), other 1.0.

`allowed`
:   The conversion matrix, rows the current system, columns the next, with CLUMondo's codes: `1` allowed, `0`
    not; `100 + t` allowed only once the cell has had its system for `t` years; `-100 - t` only until then;
    `1000 + t` an autonomous change after `t` years, whatever the demand (succession); 2 to 99 the layer of the
    `allow` stack that says where (a map of 0 and 1). Here nothing becomes "other" or leaves it, and grassland
    and rice may return to forest only after 10 years (`110`): a field must be abandoned long enough for forest
    to come back.

`deviation`, `mean_deviation`, `max_deviation`
:   When a year is good enough: the deviations in percent of the demand (`"percent"`) or in its units
    (`"absolute"`); here a mean deviation of 0.5 % and a largest of 1 %.

`speed`, `max_loops`
:   The starting step of the iteration variables (CLUMondo's default 0.05) and the iterations allowed per year
    before the simulation stops.

Optional entries add CLUMondo's neighbourhood functions (`neighbourhood`), location preferences
(`preferences`), patches (`patches`), and landschaft's extensions: caps on the cells a transition may take per
year (`caps`) and resistances per transition (`pair_resistance`).

!!! tip "Writing the settings from Python"
    The settings are long to write by hand. [landschaft](https://github.com/sacridini/landschaft) builds them
    from tables (`ls.simulate.CLUMondo`, `Supply`, `ServiceDemand`, `Rules`) and `Model.save` writes the whole
    model as a Terraflux graph, which `terraflux run` runs with the same result.

Run the model with ++ctrl+enter++ on the loop, or from the command line:

```bash
terraflux run clumondo.toml --out output      # output/maps.tif: a band per year; output/steps.csv
```

## Read the results

Move the slider to follow the ten years. The chart shows the supply of each service against its demand, as an
index (the start = 100).

```tfslider
label: Year
caption: "Ten years of CLUMondo on Sibuyan (forest dark green, coconut light green, grassland ochre, rice blue, other grey). The food demand grows 3 % a year, the wood demand stays. Most of the new food comes from grassland turned into rice paddies; the forest barely moves."
frames:
  - { value: 0, src: /img/tutorials/land-change/clumondo/frame_0.png, caption: "1997, the start", stats: { Forest: "4723", Grassland: "839", Rice: "224" } }
  - { value: 1, src: /img/tutorials/land-change/clumondo/frame_1.png, caption: "Year 1", stats: { Forest: "4736", Grassland: "767", Rice: "277" } }
  - { value: 2, src: /img/tutorials/land-change/clumondo/frame_2.png, caption: "Year 2", stats: { Forest: "4728", Grassland: "775", Rice: "296" } }
  - { value: 3, src: /img/tutorials/land-change/clumondo/frame_3.png, caption: "Year 3", stats: { Forest: "4725", Grassland: "756", Rice: "333" } }
  - { value: 4, src: /img/tutorials/land-change/clumondo/frame_4.png, caption: "Year 4", stats: { Forest: "4718", Grassland: "724", Rice: "374" } }
  - { value: 5, src: /img/tutorials/land-change/clumondo/frame_5.png, caption: "Year 5", stats: { Forest: "4716", Grassland: "689", Rice: "419" } }
  - { value: 6, src: /img/tutorials/land-change/clumondo/frame_6.png, caption: "Year 6", stats: { Forest: "4714", Grassland: "645", Rice: "468" } }
  - { value: 7, src: /img/tutorials/land-change/clumondo/frame_7.png, caption: "Year 7", stats: { Forest: "4712", Grassland: "613", Rice: "513" } }
  - { value: 8, src: /img/tutorials/land-change/clumondo/frame_8.png, caption: "Year 8", stats: { Forest: "4710", Grassland: "573", Rice: "562" } }
  - { value: 9, src: /img/tutorials/land-change/clumondo/frame_9.png, caption: "Year 9", stats: { Forest: "4710", Grassland: "533", Rice: "612" } }
  - { value: 10, src: /img/tutorials/land-change/clumondo/frame_10.png, caption: "Year 10", stats: { Forest: "4708", Grassland: "483", Rice: "668" } }
chart:
  type: line
  data: /data/tutorials/land-change/clumondo/services.csv
  x: year
  y: [food_demand, food_supply, wood_demand, wood_supply]
  labels: { food_demand: "Food demand", food_supply: "Food supply", wood_demand: "Wood demand", wood_supply: "Wood supply" }
  colors: { food_demand: "#e8901f", food_supply: "#b4508a", wood_demand: "#3b8a5a", wood_supply: "#2f6f9f" }
  ylabel: "Index (1997 = 100)"
```

Three things to read from it:

1. **The supply follows the demand.** Every year converged (`converged = 1` in `steps.csv`) within the limits:
   the largest deviation was below 1 % and the mean below 0.5 %. The food supply ends at 134.1 against a
   demand of 134.4, the wood at 99.3 against 100. If a year fails, the loop stops and the maps end there:
   always check that the number of years in `maps.tif` is the number you asked for.
2. **The demand was met by intensification.** Grassland went from 839 to 483 cells and rice from 224 to 668:
   each conversion doubles the food of the cell. The forest lost only 15 cells in ten years.
3. **Some changes go the "wrong" way.** A few coconut cells became forest and some grassland too: cells
   whose grassland had been there for 10 years or more (the `110` code) and where the forest's suitability is
   high. The wood lost to coconut-to-rice conversions has to be made up somewhere.

The map of changes shows where each transition happened:

![Transitions after ten years](/img/tutorials/land-change/clumondo/change.png "The changes between 1997 and year 10. Grassland to rice (5.3 % of the island) dominates, on the flat land of the east coast; forest to coconut (1 %) in the north.")

The cells of each system over the years:

```tfchart
type: line
data: /data/tutorials/land-change/clumondo/cells.csv
x: year
y: [Coconut, Grassland, Rice]
colors: { Coconut: "#a7c957", Grassland: "#d9a441", Rice: "#56b4e9" }
xlabel: Year
ylabel: Cells (6.25 ha each)
caption: The cells of coconut, grassland and rice. The forest (about 4,710 cells) and "other" (282) are left out to keep the scale readable.
```

**How to report it.** Give the supply table and where its numbers come from, the demand scenario, the
resistances and the conversion matrix, and the deviations reached each year. Report the areas of each system,
and the transitions (a cross table of the start and the end, or the map above). Because many maps meet the same
demand, a CLUMondo result is a *scenario*: say which assumptions drive it (here, the rice yield of twice the
grassland's is what makes intensification attractive).

## Settings that change the answer

The conversion resistance is the setting that most changes how a demand is met. Here the forest's resistance
goes from 0 (the forest is converted as easily as anything else) to 0.9, the rest of the model unchanged.

```tfslider
label: Forest resistance
caption: "The island after ten years with four resistances of the forest (colours as above). With no resistance, forest becomes coconut, which supplies wood and some food, and grassland almost disappears."
frames:
  - { value: 0.0, src: /img/tutorials/land-change/clumondo/forest_res_0.0.png, stats: { Forest: "4585", Coconut: "1489", Grassland: "1", Rice: "869" } }
  - { value: 0.3, src: /img/tutorials/land-change/clumondo/forest_res_0.3.png, stats: { Forest: "4608", Coconut: "1408", Grassland: "101", Rice: "827" } }
  - { value: 0.6, src: /img/tutorials/land-change/clumondo/forest_res_0.6.png, stats: { Forest: "4653", Coconut: "1255", Grassland: "284", Rice: "752" } }
  - { value: 0.9, src: /img/tutorials/land-change/clumondo/forest_res_0.9.png, stats: { Forest: "4727", Coconut: "1035", Grassland: "536", Rice: "646" } }
chart:
  type: line
  data: /data/tutorials/land-change/clumondo/resistance.csv
  x: resistance
  y: [forest, coconut, grassland, rice]
  labels: { forest: "Forest", coconut: "Coconut", grassland: "Grassland", rice: "Rice" }
  colors: { forest: "#3b8a5a", coconut: "#a7c957", grassland: "#d9a441", rice: "#56b4e9" }
  xlabel: Forest resistance
  ylabel: Cells after 10 years
```

The same food demand is met in very different landscapes. With a low resistance, the model meets the demand
partly by converting forest into coconut, which keeps the wood supply (coconut supplies 0.3 wood) while adding
food; it then needs fewer rice paddies on grassland, and grassland ends as rice or coconut. With a high
resistance, the forest is kept and the grassland survives only where rice is not suitable. The forest loss
differs by 142 cells (890 ha) between the two ends, with the same demand. This is why the resistances must be
justified, and calibrated if a past period allows it (run the model over an observed period and compare it
with the map at the end; see the validation tutorials on [the category page](/tutorials/#land-change)).

Other settings that matter:

- **The supply table.** Doubling the rice yield halves the paddies needed. The supply is the model's economy:
  treat it as an assumption to test, not a constant.
- **The conversion order.** A wrong rank (rice ranked below grassland for food) sends the iteration variables
  the wrong way and the year cannot converge.
- **`mean_deviation` and `max_deviation`.** Tighter limits take more iterations and may stop the simulation;
  looser ones let the supply drift from the demand.
- **The `allowed` matrix and its time codes.** `110` for grassland to forest is a statement about succession;
  `1000 + t` codes make such changes happen whatever the demand.

## Common pitfalls

- **Not carrying the iteration variables.** If `elasticity` is given as `[0.0, 0.0]` inside the body instead of
  being carried, every year starts from zero: the model is slower and no longer CLUMondo.
- **The first demand row.** It is the start, not the first simulated year. If it differs much from the supply of
  the starting map, the first year has to make a jump and may fail. Compute the starting supply from the map and
  the supply table, as here.
- **A loop that ends early.** With `until`, a year without a solution ends the run silently with fewer maps;
  check `steps.csv` and the number of bands. Raise `max_loops`, loosen the deviations or revisit a demand
  that the land cannot supply.
- **Units of the demand.** With `deviation = "percent"` the limits are percentages; with `"absolute"` they are in
  the demand's units, so a limit of 1 means one tonne, one head of cattle.
- **Systems missing from the regressions.** Every system needs a suitability; a system with no cells in the map
  cannot be fitted by `glm.fit` (give its suitability another way).
- **Projected coordinates.** As for every model on a grid, cells must have the same area: use a projected
  coordinate reference system (CRS) for the maps, not degrees of longitude and latitude.

## Going further

- [CLUE-S and lulcc](/tutorials/land-change/clue-s-ideas/): the area-based ancestor, with the logistic
  regressions in detail ([Suitability from logistic regressions](/tutorials/land-change/logistic-suitability/)).
- [A demand per region](/tutorials/land-change/regional-demand/): different demands for parts of the map.
- [Choosing a model](/tutorials/land-change/choosing-a-model/): CLUMondo next to the other models.
- [Agents](/tutorials/land-change/agents/) and [CRAFTY](/tutorials/land-change/crafty/): services supplied by
  agents who decide, rather than by systems that compete.
- The operators: [[clumondo.step]], [[clumondo.ages]], [[glm.fit]], [[control.repeat]].

## References

- Eitelberg, D. A., van Vliet, J., Doelman, J. C., Stehfest, E., & Verburg, P. H. (2016). Demand for biodiversity protection and carbon storage as drivers of global land change scenarios. *Global Environmental Change*, 40, 101–111. https://doi.org/10.1016/j.gloenvcha.2016.06.014
- Ornetsmüller, C., Verburg, P. H., & Heinimann, A. (2016). Scenarios of land system change in the Lao PDR: Transitions in response to alternative demands on goods and services provided by the land. *Applied Geography*, 75, 1–11. https://doi.org/10.1016/j.apgeog.2016.07.010
- van Asselen, S., & Verburg, P. H. (2013). Land cover change or land-use intensification: simulating land system change with a global-scale land change model. *Global Change Biology*, 19(12), 3648–3667. https://doi.org/10.1111/gcb.12331
- van Vliet, J., & Verburg, P. H. (2018). A short presentation of CLUMondo. In M. T. Camacho Olmedo, M. Paegelow, J.-F. Mas, & F. Escobar (Eds.), *Geomatic Approaches for Modeling Land Change Scenarios* (pp. 485–492). Springer. https://doi.org/10.1007/978-3-319-60801-3_34
- Veldkamp, A., & Fresco, L. O. (1996). CLUE: a conceptual model to study the Conversion of Land Use and its Effects. *Ecological Modelling*, 85(2–3), 253–270. https://doi.org/10.1016/0304-3800(94)00151-0
- Verburg, P. H., & Overmars, K. P. (2009). Combining top-down and bottom-up dynamics in land use modeling: exploring the future of abandoned farmlands in Europe with the Dyna-CLUE model. *Landscape Ecology*, 24(9), 1167–1181. https://doi.org/10.1007/s10980-009-9355-7
- Verburg, P. H., Soepboer, W., Veldkamp, A., Limpiada, R., Espaldon, V., & Mastura, S. S. A. (2002). Modeling the spatial dynamics of regional land use: the CLUE-S model. *Environmental Management*, 30(3), 391–405. https://doi.org/10.1007/s00267-002-2630-x
- Verburg, P. H., & Veldkamp, A. (2004). Projecting land use transitions at forest fringes in the Philippines at two spatial scales. *Landscape Ecology*, 19(1), 77–98. https://doi.org/10.1023/B:LAND.0000018370.57457.58
