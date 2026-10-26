---
title: "What changed: transition matrices, gains and losses"
short: Transitions, gains and losses
description: Cross-tabulate two maps of the same place, read the transition matrix, and split the change into gains, losses, net change and swap, and into quantity, exchange and shift.
order: 10
level: beginner
time: 45 min
ops: [change.crosstab, change.transitions, change.components, change.suggest, raster.read, raster.calc, raster.resample]
series: Change detection
part: 1
---

Two maps of the same landscape at two dates are the most common data of land change science, and the first
question everyone asks of them is simple: *what changed?* In this tutorial you will compare the land use of
Sibuyan Island, in the Philippines, in 1997 and in 2011. You will build the **transition matrix** that counts
every cell's class at both dates, read from it each class's **gain**, **loss**, **net change** and **swap**, map
where each transition happened, and split the total change into its **quantity**, **exchange** and **shift**
components. At the end you will have a small model that does all of this for any pair of maps, the numbers a
paper needs, and a sense of how much the answer depends on the size of the cells.

## The idea

A land use map is a **raster**: a grid of square cells, each holding a number that stands for a class (1 forest,
2 coconut plantation, 3 grassland...). When two rasters of the same place share the same grid (the same cells,
the same size, the same position on the ground), every cell can be followed from the first date to the second.
A cell that was forest in 1997 and grassland in 2011 has made the **transition** forest → grassland; a cell that
was forest at both dates **persisted**.

Counting the cells of every pair of classes gives a square table, the **cross-tabulation** or **transition
matrix**: one row per class at the first date, one column per class at the second. The diagonal holds the
persistence; everything off the diagonal is change. Here is a toy landscape of 25 cells at two times, with three
classes: forest (F), farming (A) and grassland (G).

![A toy landscape of 25 cells at two times, and the six cells that changed](/img/tutorials/change-detection/transitions/toy.png "A toy landscape at two times. Six of the 25 cells changed (right): two forest cells became farming, two became grassland, and farming and grassland traded one cell each. Run through Terraflux, these two maps give the matrix below.")

Its transition matrix, as [[change.crosstab]] gives it (rows: time 1; columns: time 2):

| Time 1 \ Time 2 | Forest | Farming | Grassland | Total time 1 |
|---|---:|---:|---:|---:|
| Forest | **6** | 2 | 2 | 10 |
| Farming | 0 | **8** | 1 | 9 |
| Grassland | 0 | 1 | **5** | 6 |
| Total time 2 | 6 | 11 | 8 | 25 |

Everything a change analysis reports can be read from such a table. Robert Gilmore Pontius Jr. and colleagues
made this explicit in a paper that changed how land change is reported (Pontius et al., 2004). Most land in most
landscapes does not change in a decade, so the persistence on the diagonal dominates the matrix and hides the
transitions that matter. They proposed to describe each class by four numbers taken from its row and column:

!!! science "Key idea: gain, loss, net change and swap"
    For a class \(j\), with \(C_{ij}\) the cells in class \(i\) at time 1 and in class \(j\) at time 2:

    - the **gain** \(G_j = \sum_{i \ne j} C_{ij}\): its column total less its persistence \(C_{jj}\);
    - the **loss** \(L_j = \sum_{k \ne j} C_{jk}\): its row total less its persistence;
    - the **net change** \(N_j = |G_j - L_j|\): how much the class's area grew or shrank;
    - the **swap** \(S_j = 2 \min(G_j, L_j)\): change that leaves the area as it was, because the class gained
      in one place as much as it lost in another.

    The class's **total change** is \(G_j + L_j = N_j + S_j\). A class with a large swap and a small net change
    moved across the landscape without growing; comparing only the areas of the two maps would say that nothing
    happened to it.

In the toy, forest gained nothing and lost 4 cells: a net change of 4 and no swap. Farming gained 3 and lost 1: a
net change of 2 and a swap of 2. Grassland did the same. Over the whole landscape, the net change is the sum of
the classes' net changes divided by two (every cell lost by one class is gained by another), and the same holds
for the swap: 4 cells of net change and 2 of swap, 6 cells of change in all.

Pontius and Santacruz (2014) later split the swap further, in the R package **diffeR**. Part of a swap is an
**exchange**: a pair of classes trading cells, one cell of farming becoming grassland while another of grassland
becomes farming. The rest is **shift**: a class that moves by gaining from one class and losing to another (in
the Sibuyan maps below, grassland gains from forest and loses to coconut). The net change is called the
**quantity** component.

!!! science "Key idea: quantity, exchange and shift"
    The total difference between two maps (all the change) is the sum of three components:

    - **quantity**: the difference in the classes' areas, \(\tfrac{1}{2}\sum_j |G_j - L_j|\);
    - **exchange**: for every pair of classes \(i, k\), twice the smaller of \(C_{ik}\) and \(C_{ki}\), summed and
      halved;
    - **shift**: what remains, \(\text{total} - \text{quantity} - \text{exchange}\).

    Exchange and shift together are the **allocation** component: change in where the classes are, not in how
    much of each there is.

The toy's 6 cells of change are 4 of quantity, 2 of exchange and none of shift. Terraflux's
[[change.components]] computes these numbers exactly as diffeR does.

## When to use it

Cross-tabulation is the first step of nearly every study of land change, and often the only one needed:

- **Describing change.** How much forest was lost between two censuses, to what, and where? Which classes grew?
  A transition matrix with the gains and losses answers it and is the table reviewers expect.
- **Preparing a simulation.** Land change models (Dinamica EGO, CLUE-S, the Markov chain) are calibrated on the
  transitions observed between two dates. [[change.suggest]] ranks them and suggests those large enough to model.
- **Comparing two maps of the same date.** The same table compares a map with a reference (a simulated map with
  the observed one, a classification with field data). Then the rows are the map being judged and the columns
  the reference, and gain and loss become omission and commission (see
  [part 3](/tutorials/change-detection/comparing-maps/)).

What it is *not* for: a transition matrix counts cells, it says nothing about how fast the change was against the
time it took or whether a transition targeted a class more than its size would explain. That is the
[intensity analysis](/tutorials/change-detection/intensity-analysis/) of part 2. It also treats every cell
alike, so a forest that lost its core and a forest whose edge moved by one cell look the same; GuidosToolbox's
morphological change detection, in [part 4](/tutorials/change-detection/forest-gain-and-loss/), tells them
apart. And it does not tell real change from map error: if the maps are 90% accurate, most of a 10% change could
be error (Olofsson et al., 2014).

## What you need

Two maps of classes **on the same grid**, made with the same legend and, ideally, the same method. The file used
here, `sibuyan_landuse.tif`, is part of the example data of Terraflux's library: Sibuyan Island's land use in 1997
and 2011, from the demonstration data of the CLUE-S model (Verburg et al., 2002), **modified for teaching**: use
it to learn, not to draw conclusions about Sibuyan.

| Property | Value |
|---|---|
| Grid | 108 rows × 128 columns of 250 m cells (0.0625 km² each); no coordinate reference system |
| Bands | two, named `1997` and `2011` |
| Classes | 1 forest, 2 coconut, 3 grassland, 4 rice, 5 other; 255 no data (the sea) |
| Island | 7,226 cells with data, 451.6 km² |

![Sibuyan's land use in 1997 and 2011](/img/tutorials/change-detection/transitions/dates.png "Land use of Sibuyan Island in 1997 and 2011 (cells of 250 m). Forest covers the mountainous centre; coconut, grassland and rice the coastal plains.")

Drag the handle to see what changed:

```tfcompare
caption: "Sibuyan in 1997 (left) and 2011 (right). Forest in dark green, coconut in light green, grassland in ochre, rice in blue, other land in grey. The grassland in the north-west and along the east coast grew into the forest."
before: { src: /img/tutorials/change-detection/transitions/land_1997.png, label: "1997" }
after: { src: /img/tutorials/change-detection/transitions/land_2011.png, label: "2011" }
```

A **coordinate reference system** (CRS) says what the numbers of a raster's position mean on the Earth (metres
in a projection, or degrees of latitude and longitude). Sibuyan's file has none, so Terraflux takes its cell
sides, 250, as metres, which is what they are. With your own maps, check that both dates have the same CRS,
extent and cell size; if not, put one on the grid of the other first with [[raster.match]].

The template **Land change analysis** (menu **Templates → Land change**) builds the same kind of model on maps drawn
by chance; replace its first nodes with [[raster.read]] nodes of your files to use it with your own data.

## Build the model

### Read the two dates

A model in Terraflux is a graph: **nodes** are operations, and the lines join an output of one node to an input of
another. Start with the two maps. Add a [[raster.read]] node (library: **IO**), set its
`path` to `sibuyan_landuse.tif` and its `band` to `1997`; add a second one with the band `2011`. A `band` can be a
band's name or its number counted from 0. Each node has an output `map`: the band as a map of numbers, the sea
(255 in the file) as no value.

Then add [[change.crosstab]] from the **Change detection** subject of the library. Drag from the `map` port of the
1997 node to its `from` input, and from the 2011 node's `map` to its `to` input. The order matters: `from` gives
the rows, `to` the columns.

```tfgraph
#! caption: The two dates read from one file, and their transition matrix.
#! highlight: crosstab
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "land_1997"
op = "raster.read"
params = { path = "sibuyan_landuse.tif", band = "1997" }
note = "Land use in 1997: 1 forest, 2 coconut, 3 grassland, 4 rice, 5 other."

[[nodes]]
id = "land_2011"
op = "raster.read"
params = { path = "sibuyan_landuse.tif", band = "2011" }
note = "The same classes in 2011."

[[nodes]]
id = "crosstab"
op = "change.crosstab"
inputs = { from = "land_1997.raster", to = "land_2011.raster" }
note = "Rows: the class in 1997; columns: the class in 2011; cells: how many."
```

Select the crosstab node and press ++ctrl+enter++: the model runs up to it and its table appears in the Viewer
under the canvas. It has a row per class of 1997 (`from`) and a column per class of 2011, the cells in each pair.
Classes found in neither map are left out, and so are cells without a value at either date.

### Every transition, in km²

The matrix counts cells. To report areas, and to handle more than two dates, add [[change.transitions]]. It takes
the maps as one input, `maps`: a **stack** (several maps of one grid, a layer each). Add a third
[[raster.read]] node with the same path and no band: its `raster` holds both bands, named `1997` and `2011`.
Connect it to `maps`.

[[change.transitions]] reads the dates from the layers' names (or from its `times` setting) and a cell's area from
the grid (or from `cell_area`, in m²). It gives four tables: `table`, a row per pair of classes with the years,
the classes, the area in km² and the cells (OpenLand's contingency table; Exavier & Zeilhofer, 2020); `one_step`,
the same from the first date to the last when there are more than two; `areas`, every class's cells at every
date; and `matrix`, the transition matrix from the first date to the last.

Add [[change.suggest]] beside it, on the same stack. It lists every transition with its share of all the change
and of its class's area, largest first, and marks as suggested those that make at least `least` % of the change.
Set `least` to 5.

```tfgraph
#! caption: The stack of both dates gives the transitions in km² and the transitions worth modelling.
#! highlight: dates, transitions, suggest
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "land_1997"
op = "raster.read"
params = { path = "sibuyan_landuse.tif", band = "1997" }

[[nodes]]
id = "land_2011"
op = "raster.read"
params = { path = "sibuyan_landuse.tif", band = "2011" }

[[nodes]]
id = "dates"
op = "raster.read"
params = { path = "sibuyan_landuse.tif" }
note = "No band: the stack of both, layers named 1997 and 2011."

[[nodes]]
id = "crosstab"
op = "change.crosstab"
inputs = { from = "land_1997.raster", to = "land_2011.raster" }

[[nodes]]
id = "transitions"
op = "change.transitions"
inputs = { maps = "dates.raster" }
note = "Years from the layers' names; a cell's area from the grid (250 m x 250 m)."

[[nodes]]
id = "suggest"
op = "change.suggest"
inputs = { maps = "dates.raster" }
params = { least = 5 }
note = "Transitions making at least 5% of the change."
```

### The components, and a map of the transitions

Add [[change.components]]. It accepts either two maps (`from`, `to`) or a stack (`maps`, from its first layer to
its last); connect the two maps. Its `analysis` setting names the columns: `change` (the default) gives gain,
persistence and loss; `error` gives omission, agreement and commission, the words for comparing a map with a
reference. Its `table` has a row per class and an overall row at the end (with an empty class); `overall` has the
total split into quantity, exchange, shift and allocation, in cells.

A table cannot show *where* the change happened. Add a [[raster.calc]] node (library: **Map algebra**) with two inputs:
connect the 1997 map and name the input `a`, the 2011 map as `b` (in the Node pane, an input's name is what the
expression calls it). Its expression codes every transition with two digits:

```text
0 if a == b else a * 10 + b
```

so 13 is forest → grassland and 42 rice → coconut, 0 no change. Add a [[view.plot]] to see it.

```tfgraph
#! caption: The full model: the transition matrix, the transitions in km², the transitions worth modelling, the components of the change and a map of where each transition happened.
#! highlight: components, from_to, plot
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "land_1997"
op = "raster.read"
params = { path = "sibuyan_landuse.tif", band = "1997" }

[[nodes]]
id = "land_2011"
op = "raster.read"
params = { path = "sibuyan_landuse.tif", band = "2011" }

[[nodes]]
id = "dates"
op = "raster.read"
params = { path = "sibuyan_landuse.tif" }

[[nodes]]
id = "crosstab"
op = "change.crosstab"
inputs = { from = "land_1997.raster", to = "land_2011.raster" }

[[nodes]]
id = "transitions"
op = "change.transitions"
inputs = { maps = "dates.raster" }

[[nodes]]
id = "components"
op = "change.components"
inputs = { from = "land_1997.raster", to = "land_2011.raster" }
note = "Gain, persistence, loss; quantity, exchange, shift."

[[nodes]]
id = "suggest"
op = "change.suggest"
inputs = { maps = "dates.raster" }
params = { least = 5 }

[[nodes]]
id = "from_to"
op = "raster.calc"
inputs = { a = "land_1997.raster", b = "land_2011.raster" }
params = { expression = "0 if a == b else a * 10 + b" }
note = "13: forest to grassland, 42: rice to coconut... 0: no change."

[[nodes]]
id = "plot"
op = "view.plot"
inputs = { value = "from_to.map" }
params = { title = "Transitions 1997-2011" }
```

=== "Application"

    Save the model next to `sibuyan_landuse.tif` (its paths are relative to the model's folder) and press
    ++f5++ to run it all, or select a node and press ++ctrl+enter++ to run up to it. Every table opens in the
    Viewer, where it can be sorted, filtered (`km2 > 1`) and drawn as a chart.

=== "Command line"

    Name the tables you want written as outputs of the model, at the end of the file:

    ```toml
    [outputs.crosstab]
    from = "crosstab.table"

    [outputs.transitions]
    from = "transitions.table"

    [outputs.components]
    from = "components.table"

    [outputs.overall]
    from = "components.overall"
    ```

    then run it; every table is written as a CSV file in the output folder:

    ```bash
    terraflux run sibuyan_change.toml --out output
    ```

## Read the results

### The transition matrix

The crosstab node gives, in cells of 0.0625 km²:

| 1997 \ 2011 | Forest | Coconut | Grassland | Rice | Other | Total 1997 |
|---|---:|---:|---:|---:|---:|---:|
| Forest | **4160** | 126 | 320 | 116 | 1 | 4723 |
| Coconut | 0 | **1140** | 18 | 0 | 0 | 1158 |
| Grassland | 0 | 77 | **745** | 16 | 1 | 839 |
| Rice | 0 | 61 | 7 | **156** | 0 | 224 |
| Other | 0 | 0 | 0 | 0 | **282** | 282 |
| **Total 2011** | **4160** | **1404** | **1090** | **288** | **284** | **7226** |

Read it row by row. Of the 4,723 cells of forest in 1997, 4,160 were still forest in 2011, 320 had become
grassland, 126 coconut and 116 rice. The diagonal sums to 6,483 cells: **89.7% of the island persisted** and
743 cells (46.4 km², 10.3%) changed. Read the columns too: the forest column has nothing off the diagonal, so not
a single cell became forest. Coconut gained from every other class of farmland, and rice lost 61 of its 224
cells to coconut.

### Where it happened

![Where each transition happened between 1997 and 2011](/img/tutorials/change-detection/transitions/from_to.png "The from_to map, grouped: forest became grassland in the north-west and on the east coast, rice along the south-west coast, coconut at the edges of the plantations. The shares are of the island's cells.")

The map shows two different processes. Forest turned into grassland in large clumps in the north-west and along
the inner edge of the eastern plain, where cleared land is left to grass; forest became rice in a strip along the
south-west coast. Meanwhile the conversions from grassland and rice to coconut happened *within* the farmland of
the east, at the edges of the plantations. A transition matrix gives the first impression of these processes;
the map shows that they happen in different places, which is the first clue to their drivers.

### Gains, losses, net change and swap

The components table, converted to km² (cells × 0.0625):

```tfchart
type: bar
data: /data/tutorials/change-detection/transitions/gains_losses.csv
x: class
y: [gain, loss, net]
labels: { gain: "Gain", loss: "Loss", net: "Net change" }
colors: { gain: "#3b8a5a", loss: "#c0392b", net: "#48534d" }
ylabel: "km²"
height: 300
caption: "Each class's gain, loss (drawn below zero) and net change between 1997 and 2011, from change.components. Forest only lost; grassland and rice gained much more than their net change shows."
```

| Class | Gain (km²) | Loss (km²) | Net change (km²) | Swap (km²) | Total change (km²) |
|---|---:|---:|---:|---:|---:|
| Forest | 0.00 | 35.19 | −35.19 | 0.00 | 35.19 |
| Coconut | 16.50 | 1.13 | +15.38 | 2.25 | 17.63 |
| Grassland | 21.56 | 5.88 | +15.69 | 11.75 | 27.44 |
| Rice | 8.25 | 4.25 | +4.00 | 8.50 | 12.50 |
| Other | 0.13 | 0.00 | +0.13 | 0.00 | 0.13 |

Forest lost 35.2 km², 11.9% of its area in 1997. Its loss is all net change: it never gained, so its change is
pure **quantity**. Rice tells a different story. Its area grew by only 4 km² (from 14.0 to 18.0 km²), but
12.5 km² of rice changed: it gained 8.25 km² from forest and grassland and lost 4.25 km² to coconut. Two thirds
of rice's change is swap, invisible in a comparison of areas. Grassland is similar: its area grew by
15.7 km², but 27.4 km² of grassland was involved in change.

### Quantity, exchange and shift

```tfchart
type: bar
stacked: true
data: /data/tutorials/change-detection/transitions/components.csv
x: class
y: [quantity, exchange, shift]
labels: { quantity: "Quantity", exchange: "Exchange", shift: "Shift" }
colors: { quantity: "#2f6f9f", exchange: "#e8901f", shift: "#b4508a" }
ylabel: "km²"
height: 300
caption: "Each class's change split into quantity, exchange and shift (km²), from change.components. Exchange and shift are the swap of the table above."
```

Overall, the `overall` table gives 743 cells of change: **563 of quantity (7.8% of the island), 50 of exchange
(0.7%) and 130 of shift (1.8%)**, so 180 cells (2.5%) of allocation. Three quarters of Sibuyan's change was the
growth of the farmland at the expense of the forest; a quarter was the farmland rearranging itself. Exchange is
small because few pairs of classes traded in both directions: 18 cells went each way between coconut and
grassland, and 7 between grassland and rice, so 2 × 18 + 2 × 7 = 50 cells of exchange. Most of the allocation is shift: grassland and rice gained from forest in one place and lost to
coconut in another, so they moved towards the forest.

### The transitions worth modelling

[[change.suggest]] with `least = 5`:

| From | To | Cells | km² | % of the change | % of the class | Suggested |
|---|---|---:|---:|---:|---:|---:|
| Forest | Grassland | 320 | 20.00 | 43.1 | 6.8 | 1 |
| Forest | Coconut | 126 | 7.88 | 17.0 | 2.7 | 1 |
| Forest | Rice | 116 | 7.25 | 15.6 | 2.5 | 1 |
| Grassland | Coconut | 77 | 4.81 | 10.4 | 9.2 | 1 |
| Rice | Coconut | 61 | 3.81 | 8.2 | 27.2 | 1 |
| Coconut | Grassland | 18 | 1.13 | 2.4 | 1.6 | 0 |
| Grassland | Rice | 16 | 1.00 | 2.2 | 1.9 | 0 |
| Rice | Grassland | 7 | 0.44 | 0.9 | 3.1 | 0 |
| Forest | Other | 1 | 0.06 | 0.1 | 0.0 | 0 |
| Grassland | Other | 1 | 0.06 | 0.1 | 0.1 | 0 |

Five transitions make 94% of the change. The two columns of percentages answer different questions: forest →
grassland is the largest transition (43% of all change) but took only 6.8% of the forest, while rice → coconut is
small on the island's scale (8%) yet took 27% of the rice. A model of the island's future would model the first
five; the others have too few cells to fit anything to.

!!! paper "How to report it"
    Give the transition matrix in km² (or in % of the study area) with its row and column totals, then a table
    or bar chart of each class's gain, loss, net change and swap, and the overall split into quantity, exchange
    and shift. State the cell size, the number of cells with data and how the maps were made. A typical sentence:
    "Between 1997 and 2011, 10.3% of the island changed class: 7.8% as net change (quantity) and 2.5% as swap
    (0.7% exchange, 1.8% shift). Forest lost 35.2 km² (11.9%) and gained none; 57% of its losses became
    grassland." Pontius et al. (2004) and Pontius & Santacruz (2014) are the references for the measures.

## Settings that change the answer

The operators of this tutorial have few settings, and none of them changes the counts: `times` and `cell_area`
only convert cells into years and km². What changes the answer is the **data**: the legend (merge coconut and
rice into "farmland" and their exchange disappears), the method that made the maps, and the **resolution**.

The slider shows the same analysis on the two maps made coarser, each new cell taking the most common class of
the cells it covers ([[raster.resample]] with `resampling = "mode"`), from 250 m to 1 km:

```tfgraph
#! caption: The two dates made coarser before the components are computed; the resolution was set to 250, 500, 750 and 1000 m in turn.
#! highlight: coarser
format = "terraflux/1"

[[nodes]]
id = "dates"
op = "raster.read"
params = { path = "sibuyan_landuse.tif" }

[[nodes]]
id = "coarser"
op = "raster.resample"
inputs = { raster = "dates.raster" }
params = { resolution = 750, resampling = "mode" }
note = "Each new cell takes the most common class of those it covers."

[[nodes]]
id = "components"
op = "change.components"
inputs = { maps = "coarser.raster" }
```

```tfslider
label: Cell size (m)
caption: "The transitions of Sibuyan with cells of 250 m to 1 km, in the colours of the map of transitions above (light grey: persistence; ochre: forest to grassland; light green: forest to coconut; blue: forest to rice; olive: grassland or rice to coconut; red: other transitions), and the quantity, exchange and shift in % of the island's cells at each resolution."
frames:
  - { value: 250, src: /img/tutorials/change-detection/transitions/res_250.png, caption: "250 m: 7,226 cells", stats: { Change: "10.3%", Quantity: "7.8%", Exchange: "0.7%", Shift: "1.8%" } }
  - { value: 500, src: /img/tutorials/change-detection/transitions/res_500.png, caption: "500 m: 1,869 cells", stats: { Change: "10.5%", Quantity: "8.0%", Exchange: "0.5%", Shift: "1.9%" } }
  - { value: 750, src: /img/tutorials/change-detection/transitions/res_750.png, caption: "750 m: 861 cells", stats: { Change: "10.0%", Quantity: "7.8%", Exchange: "0.5%", Shift: "1.7%" } }
  - { value: 1000, src: /img/tutorials/change-detection/transitions/res_1000.png, caption: "1 km: 500 cells", stats: { Change: "11.0%", Quantity: "9.4%", Exchange: "0.8%", Shift: "0.8%" } }
chart:
  type: line
  data: /data/tutorials/change-detection/transitions/resolutions.csv
  x: resolution
  y: [quantity, exchange, shift]
  labels: { quantity: "Quantity", exchange: "Exchange", shift: "Shift" }
  colors: { quantity: "#2f6f9f", exchange: "#e8901f", shift: "#b4508a" }
  xlabel: Cell size (m)
  ylabel: "% of the island"
```

Two things happen. Small transitions vanish: the thin strips of forest → rice along the coast and the scattered
cells of coconut gains are absorbed by their neighbours, and at 1 km the shift falls from 1.8% to 0.8%. And the
change does not shrink steadily: at 1 km the quantity *grows*, to 9.4%, because the majority rule also redraws the
coast and turns mixed cells into whichever class happens to dominate them at each date. Coarsening by majority is
not a neutral operation. If maps must be compared at a coarser resolution, the multi-resolution methods of
[part 3](/tutorials/change-detection/comparing-maps/) do it without redrawing the classes.

## Common pitfalls

- **Maps that are not on the same grid.** A cross-tabulation pairs cell *k* of one map with cell *k* of the other.
  If the grids differ by half a cell, every boundary becomes false change. Terraflux refuses maps of different
  sizes, but it cannot know whether two grids of the same size cover the same ground: check the extents.
- **No-data codes counted as a class.** If 255 (or 0) means "no data" but the file does not say so, it becomes a
  class with its own row and column. Look at the classes of the matrix; set `nodata` on [[raster.read]] when the
  file does not declare it.
- **Rows and columns swapped.** Rows are `from` (the earlier map, or the map being compared), columns `to` (the
  later map, or the reference). Swapping them turns gains into losses.
- **Areas in degrees.** [[change.transitions]] takes a cell's area from the grid's units: on a map in latitude and
  longitude that is square degrees, and the km² are nonsense. Reproject the maps ([[raster.reproject]]) or give
  `cell_area` in m².
- **Percent of what?** "Forest lost 12%" (of the forest) and "forest loss was 7.8% of the island" are both true
  here. Say which denominator you use.
- **Map error taken for change.** Two independent maps, each 90% correct, may disagree on up to 20% of the cells
  over a landscape that did not change. Use maps made the same way, check them against reference data, and
  estimate the areas of change from a sample (Olofsson et al., 2014; see
  [part 3](/tutorials/change-detection/comparing-maps/)).
- **Swap read as no change.** Two maps with the same areas of each class can still differ everywhere. Report the
  swap, not only the net change.

## Going further

- [Part 2: intensity analysis](/tutorials/change-detection/intensity-analysis/): whether a period changed faster
  than another, which classes gained and lost actively, and which transitions targeted a class.
- [Part 3: comparing maps](/tutorials/change-detection/comparing-maps/): quantity and allocation disagreement,
  differences at several resolutions, kappa and the estimation of areas from a sample.
- [Part 4: forest gain and loss](/tutorials/change-detection/forest-gain-and-loss/): GuidosToolbox's simple and
  morphological change, and the change of fragmentation.
- [Land change simulation](/tutorials/#land-change): the transitions suggested here are what a simulation
  model is calibrated on.
- Operators: [[change.crosstab]], [[change.transitions]], [[change.components]], [[change.suggest]],
  [[change.count]] (how many times each cell changed along a series of maps).

## References

- Exavier, R., & Zeilhofer, P. (2020). OpenLand: software for quantitative analysis and visualization of land use and cover change. *The R Journal*, 12(2), 359. https://doi.org/10.32614/RJ-2021-021
- Olofsson, P., Foody, G. M., Herold, M., Stehman, S. V., Woodcock, C. E., & Wulder, M. A. (2014). Good practices for estimating area and assessing accuracy of land change. *Remote Sensing of Environment*, 148, 42–57. https://doi.org/10.1016/j.rse.2014.02.015
- Pontius, R. G., Jr., & Santacruz, A. (2014). Quantity, exchange, and shift components of difference in a square contingency table. *International Journal of Remote Sensing*, 35(21), 7543–7554. https://doi.org/10.1080/2150704X.2014.969814
- Pontius, R. G., Jr., Shusas, E., & McEachern, M. (2004). Detecting important categorical land changes while accounting for persistence. *Agriculture, Ecosystems & Environment*, 101(2–3), 251–268. https://doi.org/10.1016/j.agee.2003.09.008
- Verburg, P. H., Soepboer, W., Veldkamp, A., Limpiada, R., Espaldon, V., & Mastura, S. S. A. (2002). Modeling the spatial dynamics of regional land use: the CLUE-S model. *Environmental Management*, 30(3), 391–405. https://doi.org/10.1007/s00267-002-2630-x
