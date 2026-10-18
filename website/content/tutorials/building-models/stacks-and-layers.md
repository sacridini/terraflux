---
title: Stacks and layers
description: Keep many maps of one grid together as a stack of named layers, build your own with stack.of, replace a layer with stack.set, take one out with stack.extract_layer, read a folder or a time series, and give a stack's layers the meaning of classes.
order: 20
level: beginner
time: 35 min
ops: [stack.of, stack.set, stack.extract_layer, stack.classes, raster.read, raster.write, map.calc, dynamic.distance_to]
---

Many analyses need several maps at once: the drivers of a land change model, the species of a prioritization,
the years of a time series. Terraflux keeps such maps together as a *stack*. In this tutorial you look inside a
stack of environmental variables, make a stack of your own, replace one of its layers when the landscape
changes, read a folder of maps as one stack, and hand a land change model a stack of suitabilities computed
elsewhere. You will know how layers are named, how to reach one from an expression, and what goes wrong when the
names are not what an operator expects.

## The idea

A stack is a set of maps on one grid, each with a name. Picture the layers of a cake cut out of the same square of
ground: every layer has the same rows and columns, so the cell at row 50, column 70 is the same 6.25 hectares in
all of them, and each layer is something different measured there (elevation, slope, the distance to a road).

GIS software has long called these multi-band rasters; satellite images were the first, with one band per
wavelength. Ecology adopted them for the *explanatory variables* (predictors, covariates, drivers) of a model: a
species distribution model reads one value of each variable at every cell, as does a logistic regression of
where forest was cleared. More recently the idea has grown into *data cubes*, stacks with a time axis as well,
for analyses of how whole ecosystems change (Mahecha et al., 2020).

In Terraflux a stack is a value like a map or a number: it flows along a connection, and operators take it as
one input (`drivers` of [[glm.fit]], `features` of [[prioritize.zonation]], `capitals` of
[[crafty.allocate]]). What makes it more than a pile of maps is the **name of each layer**: an operator finds
the layer it needs by name, and an expression of [[map.calc]] reads the layer `elevation` of the stack `d` as
`d.elevation`.

![The 13 layers of Sibuyan's drivers stack, each with its name](/img/tutorials/building-models/stacks-and-layers/drivers_sheet.png "The stack drivers.tif: 13 layers of Sibuyan Island at 250 m, from the distance to the sea to the distance to streams. Each name is how an operator or an expression finds the layer (here as d.<name>, the stack being an input called d).")

!!! science "Key idea"
    A stack \(S\) of \(k\) layers on a grid of \(n\) cells is \(k\) maps \(S_1, \dots, S_k\) with names
    \(\ell_1, \dots, \ell_k\). At every cell \(i\) it gives a vector \((S_1(i), \dots, S_k(i))\): the values of
    all the variables there. A model that reads a stack (a regression, a suitability, an allocation) works on
    these vectors cell by cell, and finds each variable by its name, not by its position.

## When to use it

- **Explanatory variables.** The drivers of a land change model ([[glm.fit]], [[woe.fit]], [[molusce.total]]),
  the predictors of a species distribution model.
- **Many species or features.** Prioritization takes one layer per feature ([[prioritize.zonation]],
  [[prioritize.optimize]]).
- **Time series.** Maps of the same place at several dates: the bands of `landuse.tif` (1997 and 2011), the
  steps of a simulation written by [[raster.write]], a folder of yearly maps.
- **Per-class surfaces.** The suitability of each land use class, the probability of each transition.

When you need only one variable, use a map: a stack is for keeping many together. When the maps are of
different grids, they cannot be a stack until [[raster.match]] puts them on one.

## What you need

- `drivers.tif`: Sibuyan's 13 drivers as one GeoTIFF with named bands (128 × 108 cells of 250 m).
- `landuse.tif`: Sibuyan's land use in 1997 and 2011, two bands named `1997` and `2011` (classes 1 forest,
  2 coconut, 3 grassland, 4 rice, 5 other), from the CLUE-S example (Verburg et al., 2002; Moulds et al., 2015).
- A folder `landuse/` with the same two dates as separate files, `landuse_1997.tif` and `landuse_2011.tif`.

## Reading a stack

A raster file with several bands is read as a stack in two ways.

**As an input of the model.** `kind = "stack"` reads every band (or the bands listed in `bands`); the nodes
refer to it as `$drivers`.

```toml
[inputs.drivers]
kind = "stack"
path = "drivers.tif"
```

**With [[raster.read]].** Its `raster` gives every band of the file to a node that takes a stack, and one band
(the `band` setting: a name such as `"slope"`, or a number from 0; the first by default) to a node that takes a
raster, which is how you take one layer out of a stack when you need it as a map.

The names of the layers are the names of the bands in the file (GDAL's band descriptions, which R's
terra and Python's rasterio can write). A band without a name is called by its position from 0: `"0"`, `"1"`...
Check the names before you build a model on them: in the application, select the node and the maps of its stack
are shown under the canvas; [[view.plot]] shows them in a tab of their own.

Whichever way it is read, a stack reaches [[map.calc]] whole, and the expression reads its layers by name
(`d.slope`). So does a stack connected to an input that takes a value of any kind (such as the predictors of a
species distribution model): every band arrives, not only the first.

## Make a stack: stack.of

[[stack.of]] puts maps of one grid into a stack. Its inputs take any names, and **each input's name becomes its
layer's name**. Here three maps describe a habitat: the slope, whether a cell is lowland (below 300 m) and
whether it was forest in 1997.

```tfgraph
#! caption: Three maps made by map algebra, put together by stack.of and written as one GeoTIFF with three named bands.
#! highlight: habitat
format = "terraflux/1"

[inputs.t0]
kind = "map"
path = "landuse.tif"
band = "1997"

[inputs.drivers]
kind = "stack"
path = "drivers.tif"

[[nodes]]
id = "slope"
op = "map.calc"
inputs = { d = "$drivers" }
params = { expression = "d.slope" }

[[nodes]]
id = "lowland"
op = "map.calc"
inputs = { d = "$drivers" }
params = { expression = "d.elevation < 300" }

[[nodes]]
id = "forest"
op = "map.calc"
inputs = { land = "$t0" }
params = { expression = "land == 1" }

[[nodes]]
id = "habitat"
op = "stack.of"
inputs = { slope = "slope.map", lowland = "lowland.map", forest = "forest.map" }
note = "A layer per input, named as the input, in the order of the names."

[[nodes]]
id = "write"
op = "raster.write"
inputs = { value = "habitat.stack" }
params = { path = "habitat.tif" }
```

Two things to know:

- **The layers are in the order of their names**, alphabetical, not in the order you connected them. The file
  written here has its bands as `forest`, `lowland`, `slope`. Operators find layers by name, so the order
  rarely matters inside Terraflux; it matters when another program reads the file by band number.
- **The maps must share the grid.** [[stack.of]] refuses maps of different sizes.

[[raster.write]] writes a stack as a GeoTIFF with one band per layer and the layer's name as the band's
description, so the names survive the trip to R, Python or QGIS and back.

![The stack made by stack.of, written as three bands](/img/tutorials/building-models/stacks-and-layers/habitat_sheet.png "habitat.tif as written: the bands come out in alphabetical order of the names (forest, lowland, slope), whatever the order of the connections.")

!!! app "In the application"
    In the library, the subject **Map algebra** holds [[map.calc]] and the stack operators. Add [[stack.of]],
    then in the Node pane add one input per map: a name (the layer's name) and the port it comes from
    (`slope.map`), and **Add input**.

## Replace a layer: stack.set

Some drivers change while the landscape changes. The distance to farmland is a classic one: farms spread, and the
forest that was far from them becomes near. A drivers file computed in 1997 holds the distances of 1997.
[[stack.set]] takes a stack and a map and returns the stack with the map in the layer of a given name: replacing
that layer if it exists, or adding it at the end if it does not.

```tfgraph
#! caption: The distance to farmland (coconut or rice) computed from each map, put into the drivers stack under the name dist_farm: added the first time, replaced the second.
#! highlight: with_farms, updated
#! numbered: true
format = "terraflux/1"

[inputs.t0]
kind = "map"
path = "landuse.tif"
band = "1997"

[inputs.t1]
kind = "map"
path = "landuse.tif"
band = "2011"

[inputs.drivers]
kind = "stack"
path = "drivers.tif"

[[nodes]]
id = "farm_1997"
op = "dynamic.distance_to"
inputs = { landscape = "$t0" }
params = { classes = [2, 4] }

[[nodes]]
id = "farm_2011"
op = "dynamic.distance_to"
inputs = { landscape = "$t1" }
params = { classes = [2, 4] }

[[nodes]]
id = "with_farms"
op = "stack.set"
inputs = { stack = "$drivers", map = "farm_1997.distance" }
params = { name = "dist_farm" }
note = "No layer dist_farm yet: added at the end (14 layers)."

[[nodes]]
id = "updated"
op = "stack.set"
inputs = { stack = "with_farms.stack", map = "farm_2011.distance" }
params = { name = "dist_farm" }
note = "dist_farm exists: replaced (still 14 layers)."

[[nodes]]
id = "gained"
op = "map.calc"
inputs = { before = "with_farms.stack", after = "updated.stack" }
params = { expression = "after.dist_farm - before.dist_farm" }
```

[[dynamic.distance_to]] (nodes 1 and 2) gives the straight-line distance from every cell to the nearest cell of
some classes, in metres for this grid. Node 3 adds it to the 13 drivers as a 14th layer; node 4 puts the distance
of 2011 in its place. Node 5 subtracts the two layers to show what changed: up to almost 5 km less in the
southwest, where forest became farmland between the two dates.

```tfgallery
columns: 3
caption: The layer dist_farm before and after stack.set replaced it, and the difference (blue where farms came closer).
images:
  - { src: /img/tutorials/building-models/stacks-and-layers/dist_farm_1997.png, caption: "before.dist_farm (1997)" }
  - { src: /img/tutorials/building-models/stacks-and-layers/dist_farm_2011.png, caption: "after.dist_farm (2011)" }
  - { src: /img/tutorials/building-models/stacks-and-layers/dist_farm_change.png, caption: "after - before" }
```

This is the pattern of a *dynamic variable* in a simulation: inside a loop, every step computes the driver from
the map of the step before and puts it into the stack under the name the fitted model expects, so the model reads
the new distance without knowing anything changed. The [Loops](../loops/) tutorial builds such a loop, and
[[dynamic.share_of]] is the other common dynamic driver (the share of a class around each cell).

## Take one layer out: stack.extract_layer

[[stack.extract_layer]] gives one layer of a stack as a map, by its name or its number from 0: a band of rasters read by
[[raster.read]], a layer of a stack an operator made, a step of a loop's series (1, 2...). Names that are numbers
work here too (`"1997"`, a class's `"3"`), which an expression cannot reach. It is how a stack that an operator
gives becomes the one map you write or pass on: [[prioritize.evaluate]]'s `ferrier` is Ferrier's irreplaceability
of every feature and their `total`, and only the total is wanted.

```tfgraph
#! caption: The 2011 band of the time series as a map, without a model input of its own.
#! highlight: y2011
format = "terraflux/1"

[[nodes]]
id = "series"
op = "raster.read"
params = { path = "landuse.tif" }

[[nodes]]
id = "y2011"
op = "stack.extract_layer"
inputs = { stack = "series.raster" }
params = { layer = "2011" }

[outputs.landuse_2011]
from = "y2011.map"
```

To write one layer as an output of the model, no node is needed: `layer` in the output does the same.

```toml
[outputs.importance]
from = "evaluation.ferrier"
layer = "total"
```

In the application, to see the layers of a stack is no work at all: show the output and step through its layers
with the arrows or the slider under the map.

## A folder as a stack

Data often arrive as one file per date, per tile or per species. [[raster.read]] reads a folder, or a pattern
such as `landuse/*.tif`, as one stack: a layer per band of each file, named after the file (its name without the
extension, and the band's name too when a file has several bands). In the application, **Open a folder as a
stack...** in the library does the same.

```tfgraph
#! caption: A folder of two maps read as one stack and written as one file of two bands, landuse_1997 and landuse_2011.
#! highlight: series
format = "terraflux/1"

[[nodes]]
id = "series"
op = "raster.read"
params = { path = "landuse" }
note = "Every raster of the folder landuse/: a layer per file."

[[nodes]]
id = "write"
op = "raster.write"
inputs = { value = "series.raster" }
params = { path = "series.tif" }

[[nodes]]
id = "plot"
op = "view.plot"
inputs = { value = "series.raster" }
params = { title = "Land use, 1997 and 2011" }
```

The files of a folder may be of different grids (tiles to join, maps from different sources). Terraflux shows
them one by one, and [[raster.match]] puts them on one grid before they can be used as a stack.

## The bands of a time series

`landuse.tif` holds two dates as two bands named `1997` and `2011`. To compare them cell by cell, read each band as
a map:

```toml
[inputs.t0]
kind = "map"
path = "landuse.tif"
band = "1997"

[inputs.t1]
kind = "map"
path = "landuse.tif"
band = "2011"
```

and code every change by where it came from and where it went:

```text
if t0 == t1 then 0 else t0 * 10 + t1
```

A cell that went from forest (1) to grassland (3) gets 13, coconut (2) to grassland gets 23, and so on.

```tfgallery
columns: 2
caption: The two layers of the folder's stack, 1997 and 2011 (green forest, ochre coconut, light green grassland, yellow rice, purple other).
images:
  - { src: /img/tutorials/building-models/stacks-and-layers/landuse_1997.png, caption: "landuse_1997" }
  - { src: /img/tutorials/building-models/stacks-and-layers/landuse_2011.png, caption: "landuse_2011" }
```

![The transitions between 1997 and 2011](/img/tutorials/building-models/stacks-and-layers/transitions.png "Where the land use changed between 1997 and 2011, coded from-to. The largest change is forest to grassland (13: 320 cells, 20 km²), then forest to coconut (12) and forest to rice (14). Transitions of fewer than 7 cells are not drawn.")

```tfchart
type: bar
data: /data/tutorials/building-models/stacks-and-layers/shares.csv
x: year
y: [Forest, Coconut, Grassland, Rice, Other]
colors: { Forest: "#3b8a5a", Coconut: "#d9a441", Grassland: "#a7c957", Rice: "#e6d36a", Other: "#b4508a" }
stacked: true
percent: true
ylabel: "% of the island"
caption: The share of each class in the two layers of the stack. Forest fell from 65.4 % to 57.6 % of the island in 14 years; grassland and coconut grew.
```

!!! warning "Layers named by numbers"
    An expression reaches a layer as `stack.name`, and a name there must start with a letter or `_`.
    `lu.1997` is read as `lu` followed by the number `.1997`, and fails with *unexpected text before 0.1997*.
    Read a band named by a year as a map (`band = "1997"`, as above), or give the layers names such as
    `landuse_1997` (the folder's stack does).

The steps of a simulation come out the same way: [[raster.write]] of a loop's collected maps writes one band per
step (see [Loops](../loops/)), and [[demand.from_maps]] reads a stack of observed maps as a series of dates.

## Layers that are classes: stack.classes

Some operators read a stack whose layers *mean* classes: the suitability of each land use class for a CLUE-S
allocation ([[lulcc.step]]), the probability of each transition for Dinamica EGO's ([[dinamica.step]]). When
Terraflux computes them ([[glm.predict]], [[woe.probability]]), the layers already carry their classes. When they
come from a file, perhaps computed in R with another method, the layers are just named `"1"`, `"2"`... and
[[stack.classes]] declares that these names are classes (or, written `"1_to_3"`, transitions).

To see it work, the CLUE-S model that `terraflux new clue` writes for Sibuyan was run twice: once as it is, with
[[glm.predict]] computing the suitabilities, and once with the suitabilities read from the file the first run
wrote:

```tfgraph
#! caption: The suitability of each class read from a file of five bands named 1 to 5, declared classes, and given to CLUE-S's allocation. Part of the model; the full one is written by terraflux new clue.
#! fragment: true
#! highlight: classes
format = "terraflux/1"

[inputs.suitability]
kind = "stack"
path = "suitability.tif"

[[nodes]]
id = "classes"
op = "stack.classes"
inputs = { stack = "$suitability" }
note = "Layers 1..5 become the suitability of classes 1..5."

[[nodes]]
id = "simulation"
op = "control.repeat"
inputs = { times = "demand.steps", landscape = "$start", history = "history.history" }

[nodes.body]
carry = { landscape = "allocate.landscape", history = "allocate.history" }
collect = { maps = "allocate.landscape" }

[[nodes.body.nodes]]
id = "allocate"
op = "lulcc.step"
inputs = { landscape = "@landscape", history = "@history", suitability = "classes.stack", demand = "demand.demand", rng = "rng.rng", step = "@step" }
params = { allocation = { method = "clue" } }
```

The 14 simulated maps are identical, cell for cell, to those of the first run. Without [[stack.classes]] the run
stops with *the potential gives no suitability for classes ["1", "2", "3", "4", "5"]*: the allocation found
layers named "1" to "5" but no layer that *is* class 1.

Maps made inside the model need no [[stack.classes]]: name the inputs of [[stack.of]] after the classes or the
transitions and its layers mean them at once. An input named `2` makes the layer the suitability of class 2; an
input named `1_to_2` makes it the probability of the transition from 1 to 2, which [[dinamica.step]] reads. So a
probability written by hand in [[map.calc]] (a rule of thumb, a scenario, a map from another program) can drive
Dinamica EGO's allocation through `stack.of` with `inputs = { 1_to_2 = "rule.map" }`.

![The five suitability layers of the file](/img/tutorials/building-models/stacks-and-layers/suitability_sheet.png "suitability.tif: the suitability of each class from CLUE-S's logistic regressions on Sibuyan's drivers, one band per class, named 1 to 5. Forest is suitable in the interior, coconut and grassland on the coastal plains.")

## Read the results

A stack has no "result" of its own; what you check is that its layers are the ones you meant:

- **The names.** [[raster.write]] the stack and look at the band descriptions (in QGIS: Layer Properties,
  Information; with GDAL: `gdalinfo`), or show it with [[view.plot]].
- **The count.** [[stack.set]] with a misspelt name adds a layer instead of replacing one: a stack of 14 layers
  where you expected 13 is the sign.
- **The values.** Each layer should have the range and units you expect (metres, degrees, 0 to 1).

When you report a model that read a stack, list its layers with their names, units, sources and dates, and say
which were updated during a simulation.

## Common pitfalls

- **A misspelt name in stack.set.** `name = "dist_roads "` (with a space) or `"Dist_roads"` adds a new layer
  and leaves the old one, which the model goes on reading.
- **Expecting the connection order in stack.of.** The layers are ordered by name. If a program outside
  Terraflux reads the file by band number, check the order.
- **Numbers as names.** `"1997"`, `"0"` and `"1"` cannot be reached as `stack.layer` in an expression; take
  the layer out with [[stack.extract_layer]], read the band as a map or rename the layers. In [[stack.of]], an input named by a number (`2`, `1_to_2`) also gives
  its layer the meaning of a class or a transition (above).
- **Unnamed bands.** A file written without band descriptions gives layers `"0"`, `"1"`...; an operator that
  looks for `slope` will not find it. Name the bands when you write them, or build the stack with [[stack.of]].
- **Different grids.** A stack's layers share one grid. Maps of different extent, resolution or coordinate
  system must be matched first ([[raster.match]], [[raster.reproject]]).
- **Classes as plain names.** A stack of suitabilities read from a file needs [[stack.classes]] before an
  allocation can use it.
- **Large stacks in memory.** Every layer of a stack is held in memory when an operator needs it. A stack of 13
  layers of 10,000 × 10,000 cells is about 5 GB in 32-bit values; see [Maps larger than memory](../../large-data/larger-than-memory/).

## Going further

- [Map algebra from scratch](../map-algebra/): expressions over maps and stack layers.
- [Loops](../loops/): carry a map and a stack along a simulation, with a driver updated at every step.
- [Batch processing](../../large-data/batch-processing/): the same analysis on every file of a folder.
- The operator reference: [[stack.of]], [[stack.set]], [[stack.extract_layer]], [[stack.classes]], [[raster.read]], [[raster.write]].

## References

- Mahecha, M. D., Gans, F., Brandt, G., Christiansen, R., Cornell, S. E., Fomferra, N., Kraemer, G., Peters, J., Bodesheim, P., Camps-Valls, G., Donges, J. F., Dorigo, W., Estupinan-Suarez, L. M., Gutierrez-Velez, V. H., Gutwin, M., Jung, M., Londoño, M. C., Miralles, D. G., Papastefanou, P., & Reichstein, M. (2020). Earth system data cubes unravel global multivariate dynamics. *Earth System Dynamics*, 11(1), 201–234. https://doi.org/10.5194/esd-11-201-2020
- Moulds, S., Buytaert, W., & Mijic, A. (2015). An open and extensible framework for spatially explicit land use change modelling: the lulcc R package. *Geoscientific Model Development*, 8(10), 3215–3229. https://doi.org/10.5194/gmd-8-3215-2015
- Verburg, P. H., Soepboer, W., Veldkamp, A., Limpiada, R., Espaldon, V., & Mastura, S. S. A. (2002). Modeling the spatial dynamics of regional land use: the CLUE-S model. *Environmental Management*, 30(3), 391–405. https://doi.org/10.1007/s00267-002-2630-x
