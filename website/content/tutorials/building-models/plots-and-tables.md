---
title: Plots and tables
description: Show maps in tabs of their own with view.plot, stack them with under and opacity, read, sort, filter and chart the tables operators give, write small tables into a model with table.of, and keep what you see as files.
order: 70
level: beginner
time: 30 min
ops: [view.plot, table.of, metrics.class, conn.graph, conn.node_importance, raster.legend]
---

A model is only as good as your ability to look at what it does. Terraflux gives you two kinds of things to look at:
**maps** (and stacks of maps) and **tables** (the class metrics of a landscape, the importance of each patch of a
network, the cells allocated at each step of a simulation). In this tutorial you will make a map tab that shows a
forest's MSPA over the land cover around it, read and chart a table of landscape metrics, and write two small
tables directly into a model, the areas of six ponds and the distances between them, to find which pond holds a
newt's network together.

## The idea

Every node's output can be looked at by selecting the node: the **Viewer** shows it. That is enough while you
build a model, but it shows one output at a time, of the node selected. Two operators of the library's subject
**Plot** and **Model flow** make looking part of the model:

- [[view.plot]] shows a map, a stack, the maps of a loop, rasters on disk or vector features **in a tab of its
  own**, refreshed at every run, without writing any file. Plots can be **stacked**: a plot connected to another
  plot's output is drawn over it, as transparent as you ask. A model with plots opens with its maps in view, which
  is why every template has them.
- [[table.of]] is the reverse: a **table you type** in the model's settings, for the small tables some operators
  take as input (targets, costs, demands, the nodes and links of a network).

Tables that operators give are shown in the Viewer as a spreadsheet you can sort and filter, and as **charts**:
bars, lines, points, box plots, a correlation matrix, a clustergram.

!!! science "Key idea"
    A figure is an analysis step like any other: what is shown, over what, and how transparent, are choices that
    change what a reader sees. Putting them in the model (a [[view.plot]] node with its title, `under` and
    `opacity`) makes them reproducible and reviewable, like the analysis itself.

## When to use it

- **[[view.plot]]**: for every result you want to see after each run (the template's final map, a map to compare
  with the input), and to compose maps: a habitat over the land cover, a corridor over a resistance surface, the
  current of Circuitscape over a satellite basemap, sampling points over a map.
- **Tables in the Viewer**: for every operator that gives a table: landscape metrics ([[metrics.class]],
  [[metrics.patch]], [[metrics.landscape]]), connectivity indices, a simulation's `info`, a prioritization's
  summary, a model's evaluation.
- **[[table.of]]**: for small tables that belong to the model (a few rows you would otherwise keep in a separate
  CSV file): a prioritization's targets, CRAFTY's demands, the agents of a model, a network of a few sites.
  Large tables are better produced by the operators that compute them.

## What you need

- `clc.tif`, the CORINE Land Cover excerpt of [Your first raster](../../getting-started/first-raster/). The model
  below clips a 15 x 15 km window of it.
- Nothing for the ponds: their table is typed in the model.

## Plots stacked: a forest over its land cover

```tfgraph
#! caption: Land cover in six named classes, the MSPA of its forest with the background removed, and two plots: the land cover, and the forest's MSPA drawn over it at 60% opacity.
#! highlight: plot_land, plot_forest
#! numbered: true
format = "terraflux/1"
name = "Forest over land cover"

[[nodes]]
id = "landcover"
op = "raster.read"
params = { path = "clc.tif" }

[[nodes]]
id = "window"
op = "raster.clip"
inputs = { raster = "landcover.raster" }
params = { extent = [4261250, 3267500, 4276250, 3282500], path = "window.tif" }
note = "A 15 x 15 km window in the north-west of the map."

[[nodes]]
id = "broad"
op = "raster.reclassify"
inputs = { map = "window.raster" }
params = { rules = "1..11 -> 1\n12..22 -> 2\n23..25 -> 3\n26..34 -> 4\n35..39 -> 5\n40..44 -> 6" }
note = "CORINE's classes in six groups."

[[nodes]]
id = "named"
op = "raster.legend"
inputs = { map = "broad.map" }
params = { classes = [[1, "Urban", "#b4508a"], [2, "Farmland", "#d9a441"], [3, "Forest", "#3b8a5a"], [4, "Open natural", "#a7c957"], [5, "Wetland", "#6f8fbf"], [6, "Water", "#2f6f9f"]] }

[[nodes]]
id = "mspa"
op = "landscape.mspa"
inputs = { map = "named.map" }
params = { foreground = [3], edge_width = 2 }

[[nodes]]
id = "forest_only"
op = "raster.calc"
inputs = { m = "mspa.map" }
params = { expression = "None if m == 0 or m == 100 or m == 220 else m" }
note = "MSPA's background (0) and openings (100, 220) without a value: transparent over the land cover."

[[nodes]]
id = "classes"
op = "metrics.class"
inputs = { map = "named.map" }
params = { metrics = ["pland", "np", "area_mn", "ed"], show = "pland" }

[[nodes]]
id = "plot_land"
op = "view.plot"
inputs = { value = "named.map" }
params = { title = "Land cover" }

[[nodes]]
id = "plot_forest"
op = "view.plot"
inputs = { value = "forest_only.map", under = "plot_land.shown" }
params = { title = "MSPA over the land cover", opacity = 0.6 }
```

[[view.plot]] has three inputs and two settings:

| | Name | What it is |
|---|---|---|
| input | `value` | the map (a map, a stack, a loop's series, rasters on disk) to show |
| input | `features` | vector features to show, over the map if there is one |
| input | `under` | **another plot's** output `shown`: that plot is drawn beneath this one |
| setting | `title` | the tab's title (by default the node's name) |
| setting | `opacity` | how opaque this plot is over what is under it, 0 (invisible) to 1 (opaque) |
| output | `shown` | what the plot shows, to connect to another plot's `under` |

Plot 9 (`plot_forest`) takes `plot_land.shown` as `under`: its tab draws the land cover first, then the forest's
MSPA over it at 60% opacity. Cells **without a value** are transparent; that is why node 6 removes MSPA's
background, which would otherwise cover the land cover in grey. Plots can be chained as deep as you like: a
hillshade under an elevation under a set of sampling points.

```tfgallery
columns: 3
caption: "The three layers of the plot, drawn by this page's figure script from the model's outputs, as the application stacks them. Left: the land cover (farmland yellow, forest green, urban purple, wetlands blue-grey, water and a river blue). Middle: the forest's MSPA (core green, edge black, bridges red, branches orange), its background without a value. Right: MSPA at 60% over the land cover: the forest's structure with its neighbours in view."
images:
  - { src: /img/tutorials/building-models/plots-and-tables/land.png, caption: "plot_land" }
  - { src: /img/tutorials/building-models/plots-and-tables/forest.png, caption: "forest_only.map" }
  - { src: /img/tutorials/building-models/plots-and-tables/stacked.png, caption: "plot_forest (under: plot_land)" }
```

!!! app "In the application"
    Select a node and click **plot** under **Plot** in the library (or ++slash++ `plot`): a [[view.plot]] comes
    connected to it, and its tab opens at the next run. A **double click** on any node opens its output in a tab
    too, without adding a node, for a quick look. In any map tab or the Viewer, **Maps over it** draws other maps
    of the run over the one shown and **Vectors over the map** the vector layers of the run, each with its own
    opacity, and **Basemap** puts satellite imagery or a world map beneath (see the [tour](../../getting-started/tour/#the-viewer)).
    Those choices are for looking; `under` and `opacity` in the model keep a composition with the model.

### Saving what a plot shows

A plot writes nothing. To keep a map, use **Save output** in the tab's header: **As a file...** writes the map
shown to a GeoTIFF (a table to CSV or JSON, vectors to GeoPackage, GeoJSON or Shapefile); **As a file, and a write
node in the model...** also adds a [[raster.write]] (or [[vector.write]]) node, so the file is written again at
every run. From the command line, plots are ignored: declare `[outputs]` for what `terraflux run` must write (see
[Your first model](../../getting-started/first-model/#the-model-as-a-file)).

## Tables in the Viewer

Select `classes`, the [[metrics.class]] node: the Viewer shows its table, a row per class, a column per metric.
Here are its values (the class names added from the legend):

| Class | pland (%) | np | area_mn (ha) | ed (m/ha) |
|---|---|---|---|---|
| Urban | 6.69 | 16 | 94.0 | 5.12 |
| Farmland | 64.78 | 9 | 1619.5 | 17.43 |
| Forest | 24.79 | 28 | 199.2 | 11.97 |
| Open natural | 0.40 | 2 | 45.1 | 0.41 |
| Wetland | 2.38 | 8 | 66.9 | 1.92 |
| Water | 0.97 | 4 | 54.3 | 1.88 |

`pland` is the share of the landscape, `np` the number of patches, `area_mn` the mean patch area and `ed` the edge
density (see the [Landscape metrics](../../landscape-metrics/) tutorials). In the Viewer you can:

- **sort** by any column, by clicking its header (again to reverse);
- **filter** the rows by text, or by a condition on a column: `pland > 1` keeps the four classes above 1%,
  `np >= 10` the fragmented ones;
- **chart** it: bars (a bar per row, a panel per column chosen), lines (columns along another, a date or a
  distance, a line per group), points (a column against another, coloured by group), box plots by group, and,
  for the operators that give them, a correlation matrix, a clustergram or the loadings of a principal component
  analysis.

```tfchart
type: bar
data: /data/tutorials/building-models/plots-and-tables/classes.csv
x: class
y: [pland]
labels: { pland: "Share of the landscape (pland, %)" }
colors: { pland: "#3b8a5a" }
ylabel: "% of the window"
height: 280
caption: "The column pland of the table as a bar chart, as the Viewer draws it with Bars: farmland dominates the window, forest covers a quarter of it."
```

`metrics.class` also gives a **map** of one metric (its setting `show`): here each class painted with its `pland`,
a quick way to see which classes dominate.

## Tables typed in the model: table.of

Some operators take tables as input. You could keep them in CSV files beside the model, but when a table has a
few rows, it is clearer to write it in the model itself with [[table.of]]. Its two settings are `columns`, the
names of the columns, and `rows`, a list of rows of numbers (`null` for a missing value).

The example: six ponds in an agricultural landscape, breeding sites of a newt, with their areas in hectares and
the distances between them. How much does each pond contribute to the connectivity of the whole network? That is
the question of Conefor's *probability of connectivity* (PC) and the importance of each node, dPC: the share of the
network's connectivity lost if that pond disappeared (Saura & Pascual-Hortal, 2007; Saura & Rubio, 2010).

```tfgraph
#! caption: Two tables typed in the model (the ponds and their areas; every pair of ponds and its distance), made a habitat graph, and the importance of each pond for a species that crosses 1 km with a probability of 0.5.
#! highlight: ponds, distances
#! numbered: true
format = "terraflux/1"
name = "Six ponds"

[[nodes]]
id = "ponds"
op = "table.of"
params = { columns = ["id", "attribute"], rows = [[1, 4.0], [2, 1.5], [3, 2.5], [4, 6.0], [5, 0.8], [6, 3.0]] }
note = "Six ponds and their area (ha)."

[[nodes]]
id = "distances"
op = "table.of"
params = { columns = ["from", "to", "value"], rows = [[1, 2, 762], [1, 3, 1315], [1, 4, 2602], [1, 5, 2102], [1, 6, 3612], [2, 3, 781], [2, 4, 1910], [2, 5, 1342], [2, 6, 2961], [3, 4, 1334], [3, 5, 1253], [3, 6, 2302], [4, 5, 1063], [4, 6, 1077], [5, 6, 2081]] }
note = "The distance between every two ponds (m)."

[[nodes]]
id = "graph"
op = "conn.graph"
inputs = { nodes = "ponds.table", pairs = "distances.table" }
params = { kind = "distance" }

[[nodes]]
id = "importance"
op = "conn.node_importance"
inputs = { graph = "graph.graph" }
params = { distance = 1000, probability = 0.5, metrics = ["PC"] }
note = "dPC: the share of the network's connectivity lost without each pond, for a newt whose chance of crossing 1 km is 0.5."
```

- Node 1 is the table of **nodes**: an `id` and an `attribute` per pond (its area). [[conn.graph]] reads those
  column names by default (its settings `id` and `attribute` name others).
- Node 2 is the table of **pairs**: `from`, `to` and `value`, here the distance in metres. A full table of all
  pairs of six ponds has 15 rows.
- Node 3, [[conn.graph]], makes a *habitat graph* of them; `kind = "distance"` says the values are distances.
- Node 4, [[conn.node_importance]], computes dPC with a dispersal kernel that gives a probability of 0.5 at 1,000 m
  (the probability falls exponentially with distance: 0.25 at 2 km).

In the application, a table's settings are typed as text in the Node pane, in the same form as in the file:
`[[1, 4.0], [2, 1.5], ...]`.

![The six ponds](/img/tutorials/building-models/plots-and-tables/ponds.png "The six ponds, their size as the circles' area, and the links whose probability of being crossed is above 0.2 (thicker: more likely). Under each pond its area and dPC.")

```tfchart
type: bar
data: /data/tutorials/building-models/plots-and-tables/importance.csv
x: id
y: [dPCintra, dPCflux]
labels: { dPCintra: "dPCintra (its own area)", dPCflux: "dPCflux (its links)" }
colors: { dPCintra: "#2f6f9f", dPCflux: "#e8901f" }
stacked: true
xlabel: Pond
ylabel: dPC (%)
height: 300
caption: "The importance of each pond (dPC, %), split into the part due to its own area (intra) and to the flow along its links (flux). Pond 4, the largest and the most central, holds 56% of the network's connectivity; pond 5, the smallest, 7%."
```

Pond 4 is the most important (dPC 55.8%): it is the largest (6 ha) and close to three others. Pond 1, also large
(4 ha) but at one end of the chain, comes second (28.8%). Pond 5, small and between others that are already
linked, matters least (7.1%). No pond is a stepping stone here (the *connector* fraction is 0 for all): with
straight-line distances for every pair, the direct route between two ponds is never longer than a route through a
third. With least-cost distances, over a resistance surface, connectors appear (see the
[Habitat networks](../../habitat-networks/) tutorials).

## Settings that change the answer

- **opacity** changes what a reader sees, not the data: at 1 the land cover under the forest is hidden, at 0.3 the
  forest's structure is hard to read. Choose it for the question the figure answers.
- In the ponds' example, the **dispersal distance** of the kernel changes the ranking: for a species that moves
  little, area dominates (intra); for one that moves far, position matters more (flux). Give `distance` a list,
  `[500, 1000, 2000]`, and the table has a row per pond and distance.

## Common pitfalls

- **A plot that covers everything.** A map drawn over another hides it except where it has no value. Remove the
  cells you do not want to show (as node 6 does), or lower `opacity`.
- **Maps that do not line up.** Maps drawn over a map are placed by their coordinates; maps of different grids
  or coordinate systems may not overlap as you expect. Put maps on one grid first
  ([One grid](../../getting-started/one-grid/)).
- **Rows of the wrong length.** Every row of [[table.of]] must have as many numbers as there are columns.
- **Text in tables.** [[table.of]] holds numbers; codes stand for names (class 3, pond 4). Keep the names in a
  note.
- **Expecting files from a plot.** [[view.plot]] never writes. Use **Save output**, [[raster.write]] or the
  model's `[outputs]`.

## Going further

- [Your first raster](../../getting-started/first-raster/): colours, legends and the viewer.
- [Landscape metrics](../../landscape-metrics/): what the columns of a class metrics table mean.
- [Habitat networks](../../habitat-networks/): habitat graphs from maps, and every connectivity index.
- The operators [[view.plot]], [[table.of]], [[metrics.class]], [[conn.graph]], [[conn.node_importance]].

## References

- Saura, S., & Pascual-Hortal, L. (2007). A new habitat availability index to integrate connectivity in landscape conservation planning: Comparison with existing indices and application to a case study. *Landscape and Urban Planning*, 83(2–3), 91–103. https://doi.org/10.1016/j.landurbplan.2007.03.005
- Saura, S., & Rubio, L. (2010). A common currency for the different ways in which patches and links can contribute to habitat availability and connectivity in the landscape. *Ecography*, 33(3), 523–537. https://doi.org/10.1111/j.1600-0587.2009.05760.x
