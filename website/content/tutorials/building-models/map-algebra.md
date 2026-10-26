---
title: Map algebra from scratch
short: Map algebra
description: Compute new maps cell by cell with raster.calc, and build a habitat suitability index, two kinds of buffer and a forest edge mask from a land use map and its drivers.
order: 10
level: beginner
time: 45 min
ops: [raster.calc, raster.read, raster.write, view.plot]
---

In this tutorial you write your own maps from other maps. Starting from the land use of Sibuyan Island in the
Philippines and a stack of its environmental variables, you build a habitat suitability index for a forest
species, buffers around roads and farms, and a mask of the forest's edge. All of them come from one operator,
[[raster.calc]], and one short expression each. By the end you can read and write map algebra expressions, you know
how Terraflux treats missing cells, and you know how a threshold changes the area of habitat you report.

## The idea

A *raster* is a map made of a grid of square *cells*, each holding one number: a land use class, an elevation
in metres, a distance to the nearest road. Sibuyan's maps here have cells 250 m on a side, so each cell covers
6.25 hectares. Because every map of a study area can share the same grid, the cell in row 50 and column 70 is the
same piece of ground in all of them. Map algebra uses this: it computes a new map by applying the same formula to
every cell, reading the values the other maps hold at that cell.

C. Dana Tomlin gave the idea its name and grammar in the 1980s, in his thesis and then in his book
*Geographic Information Systems and Cartographic Modeling* (Tomlin, 1990; see also Tomlin, 1994). He sorted the
operations into a few families, and two of them cover most ecological work:

- **Local** operations use only the cell itself: "forest *and* below 1,200 m", "elevation divided by 1,000".
- **Focal** (neighbourhood) operations use the cell and the cells around it: "the share of forest within 500 m",
  "is any neighbour farmland?".

Land change models have been built from such expressions for decades. Dinamica EGO, for example, has a
*Calculate Map* functor that evaluates an expression over several maps (Soares-Filho et al., 2002), and
[[raster.calc]] follows the same model: its inputs are maps or numbers with names you choose, and its one setting
is an expression that uses those names.

![Map algebra cell by cell: a land map, the test land == 1, and nbsum(land == 1, 1), the number of forest cells in the 3 by 3 window around each cell](/img/tutorials/building-models/map-algebra/toy_grid.png "Three steps on a toy grid. Left: the land use (1 forest, 2 coconut, 4 rice; the cells marked None have no value). Middle: the comparison land == 1 gives 1 where it is true and 0 where it is false; the missing cells stay missing. Right: nbsum(land == 1, 1) adds the 1s in the 3 × 3 window around each cell (the orange square for the cell in the third row and third column: 5); the cells without a value get one from their neighbours. Computed by Terraflux.")

!!! science "Key idea"
    A map algebra expression is evaluated once for every cell \(i\) of the grid. Writing \(x_i\) for the value of
    map \(x\) at cell \(i\), the expression `0.6 * a + 0.4 * b` makes a map \(y\) with
    \(y_i = 0.6\,a_i + 0.4\,b_i\).

    A neighbourhood function such as `nbmean(x, r)` reads instead the square window of side \(2r + 1\) cells
    centred on \(i\):

    \[ \mathrm{nbmean}(x, r)_i = \frac{1}{n_i} \sum_{j \in W_i} x_j \]

    where \(W_i\) is the set of cells of that window that lie on the map and have a value, and \(n_i\) is how
    many there are.

## When to use it

Use map algebra whenever the map you need is a formula of maps you have. Some examples:

- **Habitat suitability indices.** The US Fish and Wildlife Service's habitat suitability index (HSI) models
  score each requirement of a species from 0 to 1 and combine the scores, often with a geometric mean
  (Brooks, 1997). That is a local expression.
- **Buffers and zones of influence.** "Forest within 500 m of a road" for road effects (Laurance et al., 2009),
  "cells within two cells of farmland" for hunting pressure or spillover.
- **Edges and interiors.** Edge effects reach tens to hundreds of metres into a forest (Murcia, 1995;
  Harper et al., 2005). A focal minimum finds the cells within that depth of a non-forest cell.
- **Reclassifying and combining.** Masks, weights, constraints and probabilities inside larger models, such as
  the transition probabilities of a land change simulation.

What it is *not* for: distances along a path (use the [[cost.acc_cost|cost distance]] operators), patches and
their sizes (use the landscape metrics or [[landscape.mspa|MSPA]]), and statistics over the whole map or over
zones (use [[vector.zonal]] or the metrics). A plain value-by-value recoding is easier with
[[raster.reclassify]]. Map algebra's neighbourhoods are squares of whole cells; for circular windows or
landscape metrics in a window, see [[metrics.window]].

## What you need

- **A land use map.** `landuse.tif` is Sibuyan's land use in 1997 and 2011, 128 × 108 cells of 250 m, classes
  1 forest, 2 coconut, 3 grassland, 4 rice and 5 other; the sea has no value. It is the example data of the
  CLUE-S model (Verburg et al., 2002) as R's lulcc package distributes it (Moulds et al., 2015).
- **A stack of drivers.** `drivers.tif` holds 13 bands on the same grid, each named: `elevation` (m),
  `slope` (degrees), `dist_roads` (m), `dist_streams` (m), `pop_density` and more. A *stack* is a set of maps on
  one grid, each with a name (see [Stacks and layers](../stacks-and-layers/)).

Both maps must be on the same grid: the same number of rows and columns. [[raster.calc]] stops with "*name* is not
on the grid of the other maps" otherwise; [[raster.match]] or [[raster.resample]] put a map on another's grid
first.

![Land use of Sibuyan in 1997](/img/tutorials/building-models/map-algebra/landuse.png "Sibuyan's land use in 1997: forest covers 295 km² of the island's 452 km², mostly the interior and the mountains; coconut, grassland and rice are on the coastal plains.")

```tfgallery
columns: 3
caption: Three of the drivers, layers of the stack drivers.tif. Their names are what an expression uses after the dot.
images:
  - { src: /img/tutorials/building-models/map-algebra/driver_elevation.png, caption: "elevation (m)" }
  - { src: /img/tutorials/building-models/map-algebra/driver_dist_roads.png, caption: "dist_roads (m)" }
  - { src: /img/tutorials/building-models/map-algebra/driver_slope.png, caption: "slope (degrees)" }
```

## Build the model

The goal is a habitat suitability index for a forest-dependent animal that avoids roads and prefers the lowland
and lower montane forest. It needs three things at a cell: forest, an elevation below about 1,200 m, and distance
from roads. Each becomes a score from 0 (unsuitable) to 1 (fully suitable).

These scores are an example made for this tutorial, not a published model of a species: in a real study the
shapes of the curves come from the literature on your species or from data.

### Step 1: the inputs

The model reads two files. In a model file they are declared as *inputs*: `land` is one band of
`landuse.tif` (the band named `1997`), and `drivers` is every band of `drivers.tif` as a stack. The nodes refer
to them as `$land` and `$drivers`.

```toml
format = "terraflux/1"

[inputs.land]
kind = "map"
path = "landuse.tif"
band = "1997"

[inputs.drivers]
kind = "stack"
path = "drivers.tif"
```

!!! app "In the application"
    Press ++ctrl+o++ (**File > Open a raster or vectors...**) or drag the file from the **Files** drawer: it becomes a
    [[raster.read]] node, whose `raster` port gives every band (or only the one its `band` setting chooses).

    Connect it to an input of [[raster.calc]] and the expression reads its layers by name
    (`d.elevation`), as it does with a stack declared as an input of the model.

### Step 2: a score for elevation

The first [[raster.calc]] node turns elevation into a score: 1 below 300 m, falling in a straight line to 0 at
1,200 m.

```tfgraph
#! caption: The first map algebra node: the stack of drivers comes in as d, and the expression reads its layer elevation.
#! highlight: si_elev
format = "terraflux/1"

[inputs.drivers]
kind = "stack"
path = "drivers.tif"

[[nodes]]
id = "si_elev"
op = "raster.calc"
inputs = { d = "$drivers" }
params = { expression = "clamp((1200 - d.elevation) / 900, 0, 1)" }
note = "1 below 300 m, 0 above 1,200 m, a straight line between."
```

Read the expression from the inside out:

- `d.elevation` is the layer named `elevation` of the stack that came in as `d`. The node's input is called
  `d` because that is the name given to it; any name made of letters, digits and `_` works.
- `(1200 - d.elevation) / 900` is 1 at 300 m, 0 at 1,200 m, above 1 lower down and negative higher up.
- `clamp(x, 0, 1)` keeps a value between 0 and 1: below 0 it gives 0, above 1 it gives 1.

!!! app "In the application"
    In the library, open the subject **Map algebra** and click [[raster.calc]]. In the Node pane, type a name
    (`d`) and a source (`$drivers`, or a node's port such as `si_elev.map`) under the inputs and click
    **Add input**; repeat for every map the expression reads. Type the expression in the `expression` setting.
    Press ++ctrl+enter++ to run the model up to the node and see its map under the canvas.

### Step 3: a score for roads

Roads bring hunting, noise, clearing and invasive species into a forest (Laurance et al., 2009). The second score
grows from 0 at a road to 1 at 3 km and beyond:

```text
clamp(d.dist_roads / 3000, 0, 1)
```

```tfgallery
columns: 2
caption: The two scores, from 0 (unsuitable) to 1. Each is one raster.calc node over one layer of the drivers.
images:
  - { src: /img/tutorials/building-models/map-algebra/si_elev.png, caption: "clamp((1200 - d.elevation) / 900, 0, 1)" }
  - { src: /img/tutorials/building-models/map-algebra/si_road.png, caption: "clamp(d.dist_roads / 3000, 0, 1)" }
```

### Step 4: combine them with if and else

The index is the geometric mean of the two scores where there is forest, and 0 elsewhere:

```text
sqrt(si_elev * si_road) if land == 1 else 0
```

- `land == 1` is a *comparison*: 1 where the land use is forest, 0 where it is not.
- `a if c else b` gives `a` where `c` is true (not 0) and `b` where it is false, as in Python.
- `sqrt(si_elev * si_road)` is the geometric mean of the two scores. A geometric mean is 0 as soon as one
  requirement is fully missing, which an arithmetic mean is not: a forest on a road is worth nothing to this
  animal, however low it lies.

```tfgraph
#! caption: The habitat suitability index: two scores from the drivers, combined with the land use. The scores come into the last node under the names si_elev and si_road.
#! highlight: hsi
#! numbered: true
format = "terraflux/1"

[inputs.land]
kind = "map"
path = "landuse.tif"
band = "1997"

[inputs.drivers]
kind = "stack"
path = "drivers.tif"

[[nodes]]
id = "si_elev"
op = "raster.calc"
inputs = { d = "$drivers" }
params = { expression = "clamp((1200 - d.elevation) / 900, 0, 1)" }

[[nodes]]
id = "si_road"
op = "raster.calc"
inputs = { d = "$drivers" }
params = { expression = "clamp(d.dist_roads / 3000, 0, 1)" }

[[nodes]]
id = "hsi"
op = "raster.calc"
inputs = { land = "$land", si_elev = "si_elev.map", si_road = "si_road.map" }
params = { expression = "sqrt(si_elev * si_road) if land == 1 else 0" }
note = "The geometric mean of the scores in forest, 0 outside it."

[[nodes]]
id = "plot"
op = "view.plot"
inputs = { value = "hsi.map" }
params = { title = "Habitat suitability" }
```

Node 3 takes three inputs: the land use and the outputs of nodes 1 and 2. The connection `si_elev.map` means "the
port `map` of the node `si_elev`". Node 4, [[view.plot]], shows the index in a tab of its own without writing a
file (see [Plots and tables](../plots-and-tables/)). You could also write the whole index as one expression;
splitting it into nodes lets you look at every score on its own, which is how errors are found.

![The habitat suitability index](/img/tutorials/building-models/map-algebra/hsi.png "The index: high in the lowland forest far from roads (yellow), lower up the mountains and near the roads of the coast, 0 outside forest. The sea has no value.")

### Step 5: two kinds of buffer

A *buffer* is the area within some distance of a feature. Map algebra makes it in two ways.

**From a distance map.** When a layer already holds the distance to the feature, a comparison is the buffer:

```text
d.dist_roads <= 500
```

**From a neighbourhood.** When there is no distance map, a focal maximum finds the cells near a class. To mark the
forest within two cells (500 m) of coconut or rice:

```text
nbmax(land == 2 or land == 4, 2) if land == 1 else None
```

`land == 2 or land == 4` is 1 on farmland and 0 elsewhere; `nbmax(..., 2)` is the largest value in the 5 × 5
window around each cell, so it is 1 wherever a farm cell lies within two cells. The `if` keeps only forest cells and
gives the others no value (`None`).

```tfgallery
columns: 2
caption: Two buffers. Left, from the distance layer of the drivers. Right, from a neighbourhood of the land use, only over forest.
images:
  - { src: /img/tutorials/building-models/map-algebra/road_buffer.png, caption: "d.dist_roads <= 500" }
  - { src: /img/tutorials/building-models/map-algebra/farm_buffer.png, caption: "nbmax(land == 2 or land == 4, 2), in forest" }
```

!!! note "Square windows"
    `nbmax(x, 2)` looks at a square of 5 × 5 cells, so a cell diagonal to the centre at a distance of
    \(2\sqrt{2} \approx 2.8\) cells counts as "within 2". For buffers that must be round, use a distance:
    [[distance.euclidean]] or [[distance.proximity]] give the distance to a class, and a comparison makes the
    buffer.

### Step 6: an edge mask

The forest's edge is the forest within some depth of a non-forest cell. With a focal minimum:

```text
2 - (nbmin(land == 1, 1) == 0) if land == 1 else 0
```

`nbmin(land == 1, 1)` is 0 when any cell of the 3 × 3 window is not forest, so `nbmin(...) == 0` is 1 at the
edge. The expression gives 2 for the interior, 1 for the edge and 0 outside forest: three classes in one map. A
radius of 1 cell is an edge 250 m deep here; a radius of 2 is 500 m.

```tfgraph
#! caption: Buffers and the edge mask: three raster.calc nodes reading the land use and the drivers.
#! highlight: road_buffer, farm_buffer, edge
format = "terraflux/1"

[inputs.land]
kind = "map"
path = "landuse.tif"
band = "1997"

[inputs.drivers]
kind = "stack"
path = "drivers.tif"

[[nodes]]
id = "road_buffer"
op = "raster.calc"
inputs = { d = "$drivers" }
params = { expression = "d.dist_roads <= 500" }

[[nodes]]
id = "farm_buffer"
op = "raster.calc"
inputs = { land = "$land" }
params = { expression = "nbmax(land == 2 or land == 4, 2) if land == 1 else None" }

[[nodes]]
id = "edge"
op = "raster.calc"
inputs = { land = "$land" }
params = { expression = "2 - (nbmin(land == 1, 1) == 0) if land == 1 else 0" }
note = "0 not forest, 1 edge, 2 interior."
```

## The language in full

Everything an expression can contain:

| | |
|---|---|
| Values | numbers (`3`, `0.5`, `1e-3`), `pi`, `None` (no value), `True` and `False` (1 and 0); the node's inputs by name (maps and numbers); a stack's layers as `stack.layer` |
| Arithmetic | `+ - * / %` (remainder) and `**` (power) |
| Comparisons | `< <= > >= == !=`, giving 1 (true) or 0 (false); chained as in Python (`5 <= x < 8`) |
| Logic | `and`, `or`, `not`; `a if c else b` |
| Functions | `abs sqrt exp log log10 floor ceil round` (`log` the natural logarithm), `min(a, b, ...)`, `max(a, b, ...)`, `clamp(x, lo, hi)`, `isnull(x)`, `coalesce(x, y)` |
| Neighbourhoods | `nbsum nbmean nbmin nbmax nbcount (x, r)`: the square of radius `r` cells around each cell; `r` is a whole number, written in the expression or a number input of the node |

Some rules that save time:

- **Precedence.** As in Python: `**` binds tightest, then the sign, then `* / %`, then `+ -`, then comparisons, then `not`,
  `and`, `or`, and `a if c else b` last. `2 ** 3 ** 2` is \(2^9 = 512\) (powers group from the right), and `-a ** 2` is \(-(a^2)\). Use
  parentheses when in doubt.
- **Comparisons use `==`.** A single `=` is refused with "use == to compare".
- **One syntax, Python's.** The forms of other languages are refused with the one to write: `^` ("write ** for
  a power"), `&&`, `||` and `!` (`and`, `or`, `not`), `null` (`None`), `ln` (`log`), `if c then a else b` and
  `c ? a : b` (`a if c else b`). A model written by an older Terraflux in those forms is rewritten in Python's
  when it is opened, and told.
- **Numbers can be inputs.** An input with a number instead of a connection (`k = 500` in the model file, or a
  value typed in the Node pane) is the same number at every cell. `d.dist_roads < k` then reads better, and a
  batch can change `k` without touching the expression. A number input can also be the radius of a
  neighbourhood: with `r = 2` connected, `nbmean(land == 1, r)` is the share of forest in the 5 × 5 window, and
  a scenario or a batch can try other windows (a radius that is not a whole number stops the run).
- **A model needs a map.** At least one input must be a map; the result has its grid.
- **Errors say what is wrong.** For example `expected "else" at the end`, `no function "foo"`,
  `the radius of nbmax is a whole number of cells, or a number input` or `no input "b" (a map or a number)`.

## Missing values

Real maps have holes: the sea around an island, clouds, cells outside a country. Terraflux calls a cell without a
value *missing* (or *no data*); in an expression it is `None`. The rules:

- **Most things give None when they meet None.** `None + 1`, `None > 3`, `None == 1` and `None and 1` are all
  None. So is `a if c else b` when `c` is None, and `min` or `max` when any argument is None.
- **`isnull(x)`** is 1 where `x` is missing and 0 elsewhere: `0 if isnull(land) else land`.
- **`coalesce(x, y)`** is `x` where it has a value and `y` where it does not: `coalesce(land, 0)` fills the
  sea with 0.
- **Neighbourhoods leave missing cells out.** `nbmean(x, r)` is the mean of the cells *with a value* in the
  window, `nbcount(x, r)` counts them, and the cells beyond the edge of the map are left out the same way. A
  missing cell gets a value from its neighbours (the bottom left cells of the toy grid above), unless every cell
  of its window is missing.
- **`None` in an expression is a way to say "no value here".** `nbmax(...) if land == 1 else None`
  keeps the result to forest only.

```tfgallery
columns: 2
caption: Two expressions about missing cells. Left, the share of forest in the 5 × 5 window, computed over the cells with a value. Right, the sea filled with 0 by coalesce.
images:
  - { src: /img/tutorials/building-models/map-algebra/share.png, caption: "nbmean(land == 1, 2)" }
  - { src: /img/tutorials/building-models/map-algebra/filled.png, caption: "coalesce(land, 0)" }
```

## Read the results

The habitat suitability index is a relative score, not a probability of presence: a cell of 0.8 is better than
a cell of 0.4 for this species under these assumptions, not "80 % likely to hold it". Its mean over the forest
of 1997 is 0.81: most of Sibuyan's forest is far from roads, and only the summits lose much.

The edge mask is a count you can report. With an edge 250 m deep (radius 1), 37 km² of the 295 km² of forest
is edge and 258 km² interior. The farm buffer says that 28 km² of forest lies within 500 m (two cells) of
coconut or rice. When you report numbers like these, give the rule that made them (the expression, the cell size
and the radius), since another rule gives another number.

To keep a result, write it with [[raster.write]] (`path = "hsi.tif"`), make it an output of the model, or save it
from its tab in the application (**Save output**).

## Settings that change the answer

### The threshold

A continuous index is often cut into "habitat" and "not habitat" at a threshold, to report an area or to feed a
patch analysis. The area depends strongly on where the cut is:

```tfslider
label: Threshold on the index
caption: "The cells with hsi >= threshold (one raster.calc node: hsi >= 0.5). Above 0.4 the area falls fast: the cut decides the answer, so report it and show how the area changes with it."
frames:
  - { value: 0.1, src: /img/tutorials/building-models/map-algebra/suit_0.1.png, stats: { "Suitable habitat": "272 km²" } }
  - { value: 0.2, src: /img/tutorials/building-models/map-algebra/suit_0.2.png, stats: { "Suitable habitat": "271 km²" } }
  - { value: 0.3, src: /img/tutorials/building-models/map-algebra/suit_0.3.png, stats: { "Suitable habitat": "257 km²" } }
  - { value: 0.4, src: /img/tutorials/building-models/map-algebra/suit_0.4.png, stats: { "Suitable habitat": "247 km²" } }
  - { value: 0.5, src: /img/tutorials/building-models/map-algebra/suit_0.5.png, stats: { "Suitable habitat": "221 km²" } }
  - { value: 0.6, src: /img/tutorials/building-models/map-algebra/suit_0.6.png, stats: { "Suitable habitat": "184 km²" } }
  - { value: 0.7, src: /img/tutorials/building-models/map-algebra/suit_0.7.png, stats: { "Suitable habitat": "138 km²" } }
  - { value: 0.8, src: /img/tutorials/building-models/map-algebra/suit_0.8.png, stats: { "Suitable habitat": "81 km²" } }
  - { value: 0.9, src: /img/tutorials/building-models/map-algebra/suit_0.9.png, stats: { "Suitable habitat": "36 km²" } }
start: 4
chart:
  type: line
  data: /data/tutorials/building-models/map-algebra/suitable_area.csv
  x: threshold
  y: [area]
  labels: { area: "Suitable habitat (km²)" }
  colors: { area: "#3b8a5a" }
  xlabel: Threshold
  ylabel: km²
```

### The edge depth

The radius of the neighbourhood is the depth of the edge. Studies of edge influence find depths from a few tens of
metres to more than a kilometre, depending on what is measured (Harper et al., 2005): the radius should come from
the process you care about, not from habit.

```tfgallery
columns: 3
caption: The edge mask with a radius of 1, 2 and 3 cells (250, 500 and 750 m). The interior shrinks from 258 to 230 and 205 km².
images:
  - { src: /img/tutorials/building-models/map-algebra/edge1.png, caption: "nbmin(land == 1, 1)" }
  - { src: /img/tutorials/building-models/map-algebra/edge2.png, caption: "nbmin(land == 1, 2)" }
  - { src: /img/tutorials/building-models/map-algebra/edge3.png, caption: "nbmin(land == 1, 3)" }
```

## Common pitfalls

- **The coast is not an edge.** Because neighbourhoods leave missing cells out, the forest next to the sea has
  no non-forest neighbour and is counted as interior. If the sea (or the area outside your study region) should
  count as "not forest", fill it first: `nbmin(coalesce(land, 0) == 1, 1)`. Whether it should is an ecological
  question: the sea is an edge for light and wind, but the edge of a study area is not an edge of the forest.
  The comparison below shows the difference.
- **The radius is in cells, not metres.** `nbmax(x, 2)` is 500 m with cells of 250 m but 60 m with cells of
  30 m. Convert distances to cells for your grid, and change the radius when you change the resolution.
- **Class codes.** `land == 1` is forest here because the legend says so. Check the legend of your own map;
  in a map coded as GuidosToolbox's, 2 is the foreground and 1 the background.
- **Units.** `d.dist_roads / 3000` assumes metres. Distances in a geographic coordinate system (degrees) give
  nonsense; reproject first with [[raster.reproject]].
- **There is no integer division.** `/` always divides exactly (`1 / 2` is 0.5); use `floor` or `round` for
  whole numbers.
- **A missing cell is not 0.** `land != 1` has no value over the sea, not 1. Counting "not forest" cells with it
  misses the sea; `isnull(land) or land != 1` does not.
- **Different grids.** Two maps with the same extent but different cell sizes are different grids. Match them
  first.

```tfcompare
caption: "The coast and the edge (radius 1; orange edge, dark green interior). Drag the handle: on the left the missing sea is left out and the coastal forest is interior; on the right the sea is filled with 0 by coalesce and the coastal forest becomes edge."
before: { src: /img/tutorials/building-models/map-algebra/edge1_plain.png, label: "Missing left out" }
after: { src: /img/tutorials/building-models/map-algebra/edge_coast_plain.png, label: "Sea as not forest" }
```

## Going further

- [The raster calculator](../calculator/): several expressions as one script, its outputs previewed as you type,
  a console, and the script saved as a node of your own.
- [Stacks and layers](../stacks-and-layers/): build your own stacks, replace a layer, and read a folder of maps.
- [Loops](../loops/): run an expression again at every step and write a cellular automaton.
- [Plugins in WebAssembly](../plugins/): when an expression is not enough.
- [Monte Carlo](../../uncertainty/monte-carlo/): the uncertainty of an expression with uncertain parameters.
- The operator reference: [[raster.calc]], [[raster.reclassify]], [[raster.match]], [[distance.euclidean]].

## References

- Brooks, R. P. (1997). Improving habitat suitability index models. *Wildlife Society Bulletin*, 25(1), 163–167.
- Harper, K. A., Macdonald, S. E., Burton, P. J., Chen, J., Brosofske, K. D., Saunders, S. C., Euskirchen, E. S., Roberts, D., Jaiteh, M. S., & Esseen, P.-A. (2005). Edge influence on forest structure and composition in fragmented landscapes. *Conservation Biology*, 19(3), 768–782. https://doi.org/10.1111/j.1523-1739.2005.00045.x
- Laurance, W. F., Goosem, M., & Laurance, S. G. W. (2009). Impacts of roads and linear clearings on tropical forests. *Trends in Ecology & Evolution*, 24(12), 659–669. https://doi.org/10.1016/j.tree.2009.06.009
- Moulds, S., Buytaert, W., & Mijic, A. (2015). An open and extensible framework for spatially explicit land use change modelling: the lulcc R package. *Geoscientific Model Development*, 8(10), 3215–3229. https://doi.org/10.5194/gmd-8-3215-2015
- Murcia, C. (1995). Edge effects in fragmented forests: implications for conservation. *Trends in Ecology & Evolution*, 10(2), 58–62. https://doi.org/10.1016/S0169-5347(00)88977-6
- Soares-Filho, B. S., Cerqueira, G. C., & Pennachin, C. L. (2002). DINAMICA: a stochastic cellular automata model designed to simulate the landscape dynamics in an Amazonian colonization frontier. *Ecological Modelling*, 154(3), 217–235. https://doi.org/10.1016/S0304-3800(02)00059-5
- Tomlin, C. D. (1990). *Geographic Information Systems and Cartographic Modeling*. Prentice Hall.
- Tomlin, C. D. (1994). Map algebra: one perspective. *Landscape and Urban Planning*, 30(1–2), 3–12. https://doi.org/10.1016/0169-2046(94)90063-9
- Verburg, P. H., Soepboer, W., Veldkamp, A., Limpiada, R., Espaldon, V., & Mastura, S. S. A. (2002). Modeling the spatial dynamics of regional land use: the CLUE-S model. *Environmental Management*, 30(3), 391–405. https://doi.org/10.1007/s00267-002-2630-x
