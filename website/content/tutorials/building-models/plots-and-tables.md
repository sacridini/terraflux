---
title: Plots and figures
description: See any result in the Viewer, keep maps on it as layers, compare two maps, save what the Viewer shows as a Plot of the model, find the Plots in the Document and the report, read, sort, filter and chart the tables operators give, and write small tables into a model with table.of.
order: 70
level: beginner
time: 30 min
ops: [table.of, metrics.class, conn.graph, conn.node_importance, raster.legend]
---

A model is only as good as your ability to look at what it does. Terraflux gives you two kinds of things to look at:
**maps** (and stacks of maps, and vector features) and **tables** (the class metrics of a landscape, the importance
of each patch of a network, the cells allocated at each step of a simulation). In this tutorial you will show a
forest's MSPA over the land cover around it, keep that view with the model as a **Plot**, read and chart a table of
landscape metrics, and write two small tables directly into a model, the areas of six ponds and the distances
between them, to find which pond holds a newt's network together.

## The idea

Every result is seen in one place, the **Viewer**. Click a node and the Viewer shows its output: a map with its
legend, a stack a layer at a time, the maps of a loop's steps, vector features, a table or a single value. That is
enough while you build a model, but it shows the output of the node clicked. Three things make looking part of the
model:

- **Layers** keep outputs on the map whatever node you click, each as opaque as you ask: a habitat over the land
  cover, a corridor over a resistance surface, sampling points over a map.
- **Compare** puts another map beside the one shown: as a curtain, side by side, or as their difference.
- A **Plot** keeps what the Viewer shows with the model: its layers, the map shown, its comparison, a table's
  chart, and a title. A model's Plots are listed in the Viewer's **Plots** menu, a click away, and the Document and
  the report show them as numbered figures. Every template has them, which is why a template opens with its result
  in view.

Tables that operators give are shown in the Viewer as a spreadsheet you can sort and filter, and as **charts**:
bars, lines, points, box plots, a correlation matrix, a clustergram. Small tables that a model needs as input
(targets, costs, demands, the nodes and links of a network) are typed in the model with [[table.of]].

!!! science "Key idea"
    A figure is an analysis step like any other: what is shown, over what, and how transparent, are choices that
    change what a reader sees. Keeping them in the model as a Plot, with its title, its layers and their opacity,
    makes them reproducible and reviewable, like the analysis itself.

## When to use it

- **A click on a node**: to look at any output while you build, edit and run the model.
- **Layers**: to compose maps: a habitat over the land cover, the current of Circuitscape over a satellite
  basemap, vector features over a raster.
- **Compare**: to set two maps against each other: two dates, two scenarios, a simulation and the observed map.
- **A Plot**: for every view you want back after each run, and for the figures of the Document and the report
  (the template's final map, a map compared with the input, a table's chart).
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

## A forest over its land cover

```tfgraph
#! caption: Land cover in six named classes and the MSPA of its forest with the background removed. Two Plots are kept with the model: the land cover, and the forest's MSPA drawn over it at 60% opacity.
#! highlight: named, forest_only
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

[[plots]]
id = "plot_land"
title = "Land cover"
layers = [{ from = "named.map" }]

[[plots]]
id = "plot_forest"
title = "MSPA over the land cover"
layers = [{ from = "named.map" }, { from = "forest_only.map", opacity = 0.6 }]
```

The graph has no node for looking: the nodes compute, and the **Plots** (the `[[plots]]` at the end of the model
file, under the toggle) say what to show. Here is how to make them.

### Seeing a result

- **Click** a node: the Viewer shows its output. If it has not run yet, the Viewer says so; ++ctrl+enter++ runs the
  model up to it. Click `named` to see the land cover in its six colours.
- **Double-click** a node: the Viewer shows its output, runs it first if it has not run, and **keeps it as a
  layer**. The pin (📌) beside the map's name in **Layers** does the same for the map shown.

### Layers

Double-click `named`, then click `forest_only`. The Viewer's side panel lists, under **Layers**, the outputs kept
on the map, from the top. The map shown is drawn over the layers, so the forest's MSPA is drawn over the land
cover. For each layer you can:

- show or hide it;
- move it **up** or **down**: the layers listed under the map shown are drawn under it, those over it over it;
- change a map's **opacity**, from 0 (invisible) to 1 (opaque), or the **style** of vector features;
- remove it.

Keep `forest_only` too (📌) and set its opacity to 0.6: the forest's structure with its neighbours in view.
**Add a layer** adds the run's other maps and vector features without clicking their nodes, and a click on a
layer's name shows that map, with its legend and settings. Cells **without a value** are transparent; that is why
node 6 removes MSPA's background, which would otherwise cover the land cover in grey. Layers can be as many as you
like: a hillshade under an elevation under a set of sampling points. A map layer in **another coordinate system**
than the map shown is reprojected as it is drawn, tile by tile, and its value under the pointer read back through
the same transformation; side by side, the views follow each other across systems, and the Figure, *Export the
view* and a layer's *zoom* handle them too. A rotated grid is carried the same way, and a map in degrees across the 180° meridian is
drawn on both sides of it (a map with no coordinate system can only be drawn with another that has none).

```tfgallery
columns: 3
caption: "The layers of the Plot, drawn by this page's figure script from the model's outputs, as the Viewer stacks them. Left: the land cover (farmland yellow, forest green, urban purple, wetlands blue-grey, water and a river blue). Middle: the forest's MSPA (core green, edge black, bridges red, branches orange), its background without a value. Right: MSPA at 60% over the land cover: the forest's structure with its neighbours in view."
images:
  - { src: /img/tutorials/building-models/plots-and-tables/land.png, caption: "named.map (the Plot Land cover)" }
  - { src: /img/tutorials/building-models/plots-and-tables/forest.png, caption: "forest_only.map" }
  - { src: /img/tutorials/building-models/plots-and-tables/stacked.png, caption: "the Plot MSPA over the land cover" }
```

**Basemap** puts satellite imagery or a world map beneath the layers (see the
[tour](../../getting-started/tour/#the-viewer)).

### Comparing two maps

**Compare**, in the Viewer's side panel, puts another map beside the one shown: another output of the run (a
scenario, another date, the observed map) or another layer of the same stack. Choose how:

- **Curtain**: the other map is right of a line you drag across the map;
- **Side by side**: two views that zoom and pan together, the pointer marked in both. *Another view* adds a third
  and a fourth map (another output, or another layer of the one shown), four views in two rows, all moving
  together. Each view draws the Viewer's layers unless you give it layers of its own: under *Compare*, the fold
  of the view says "same as the map shown" or "its own", a short list (started as a copy of the Viewer's) where
  you hide a layer, set its opacity, remove it or add another map or features of the run, differences included.
  The Plot keeps them;
- **Difference**: for maps of values, the other map minus this one; for maps of classes, what each cell changed
  from and to. The two maps may be of different coordinate systems: the other map is carried to the first one's
  cells, each cell taking the nearest cell of the other (nothing is interpolated). *Keep as a layer* makes it a layer of its own, drawn with the others and kept in a Plot. It works between two outputs and between two layers of one
  stack or steps of one series (the layer then says which band of each: "x.map step 5 minus step 1").

When the output compared is a stack or a series, a **band** choice under *Compare* (and under each view added)
picks which of its maps is shown, drawn and kept in the Plot; *Keep as a layer* of a difference then uses that band.

Both maps are drawn in the same colours, so that the same value is the same colour.

### The histogram of a map

Under the legend of a map of values, its histogram: the values in bars coloured as the map is, with the
minimum, maximum and mean, and how many cells have a value. A layer's settings have one too, folded.

### Three bands as one colour image

A layer of three maps or more (a stack, a series, a raster of several bands, such as a Sentinel-2 or Landsat
scene) can be drawn as a colour image: check **RGB** in its settings and choose the band of the red, green and
blue channels. When the bands' names tell them (`B4`, `SR_B4`, `B04`...), *composites* offers the satellite's
natural colour (B4 B3 B2), colour infrared (B8 B4 B3 for Sentinel-2, B5 B4 B3 for Landsat 8/9) and short-wave
infrared; switched on, the layer starts in natural colour when it can, else with its first three maps. Each channel
runs from black at the 2nd percentile of its band to full colour at the 98th; its two values can be set by hand
(*auto* goes back), and the gamma brightens or darkens the middle tones. A cell without a value in any of the three
is transparent. The legend under the row tells each channel's band and range, the pointer over the map the three
values. The Plot keeps the composite, and the figure, *Export the view* and `terraflux plot` draw it.

### Relief and blending

*Shade with this elevation*, in the settings of a map layer of values, draws its colours over the relief the map
itself makes (Horn's hillshade, as `gdaldem hillshade`): the sun's azimuth (315°, from the north-west) and
altitude (45°), the vertical exaggeration and how strong the shade is. On a grid in degrees the cells are measured in
metres. A layer can also be **blended** with those under it: *multiply* darkens them by its colours (a hillshade
layer multiplied over a land cover map gives its relief), *screen* lightens them. The Plot keeps both, and the
figure, *Export the view* and `terraflux plot` draw them.

### The style of vector features

A vector layer's style colours its features with one colour or **by an attribute**. Text, and numbers asked to be
*each value a category*, give a colour per value; other numbers run along a ramp (a click on it gives the Viewer's
others), continuously or in **classes**: equal intervals, quantiles (as many features in each) or natural breaks
(Jenks: the least variance within the classes), 2 to 12 of them. Points can be a circle, a square, a triangle, a
diamond or a cross, and **sized by** an attribute (their area grows with the value, between the two sizes set).
The legend under the layer's row tells the categories, the classes' ranges or the ramp, and the sizes; the figure
draws it under the map's. A Plot keeps the style, and the project written for QGIS has it as a categorized or a
graduated renderer, its symbol and a size by expression.

### Labels

*labels*, in a vector layer's style, writes an attribute of its features over them: polygons at a point inside
them, lines at their middle, points beside them. Only the features large enough for their label are labelled,
and a label that would cover another is left out; zoom in to see more of them. A Plot keeps the labels, and the
figure draws them.

### The attributes of vector features

The *table* button of a vector layer's row in the Layers opens its attributes under the map: a row per feature,
sorted and filtered like the other tables, copied or saved as CSV. A row clicked outlines its feature on the map
and takes the view to it; a feature clicked on the map selects its row. Ctrl+click adds or removes a row (or a
feature on the map) and shift+click takes the range from the last row clicked; the header counts the selected,
zooms to all of them (*Zoom to selection*), lets them go (*Clear selection*, or esc) and copies or saves only their
rows (*Copy selected*, *Save selected*). *Select all shown* (or ctrl+A with the pointer on the table) selects every
row the filter leaves. A ctrl+click that leaves one row selected does not move the view, only a plain click does.
The selection is outlined in every view, side by side too, and a feature clicked in a view side by side that
draws the Viewer's layers selects its row as in the main one (a view with layers of its own draws other features).

Features are also selected by area: **shift+drag** on the map draws a rectangle, **shift+alt+drag** a lasso, and
on release the features of the layer whose table is open (else the topmost vector layer shown, whose table then
opens) that meet the shape are selected: a point inside it, a line or a polygon that crosses it, or a polygon that
holds it. With ctrl held too, they are added to the selection; esc lets go of a shape still dragged. Ctrl+A selects
every row shown also when the keyboard is in the table. A saved Plot keeps the selection and opens the table with
it. **Filter the selection**, in the table's header, adds a [[vector.filter]] after the layer that keeps exactly the
features selected (by their number, `feature`, so that a change upstream asks for choosing them again), with a note
of what it keeps; it is drawn over the map, and one undo takes it away.

### Exporting the view

*Export the view...*, beside *Figure...*, writes the map as the Viewer shows it, with its layers, vectors and
labels and without a legend: a PNG, or a GeoTIFF of red, green, blue and alpha on the map's grid, which opens in
QGIS where the map is. A pixel to a cell, or twice or four times as many.

A saved Plot can be exported without the application, by its id: `terraflux plot model.toml cover -o cover.png`
on the command line (`.tif` for the GeoTIFF, `--scale 2` or `4` for a larger picture), or the assistant's
`export_plot` tool. It runs the model and draws the Plot's layers in their palettes; hidden layers and differences
are left out, and a Plot that compares maps gives its map only.

### The Viewer in a window of its own

**Pop out**, in the Viewer's header (or *View > Viewer in a window of its own*), moves the Viewer into a window of
its own, to put on a second screen beside the model; everything of it works there, files dropped on it too, and
what brings the Viewer forward (a double click on a node) brings that window. *Back into the window*, or closing it,
puts it back where it was. The window keeps its place and size, and the Viewer comes out again at the next start.

### Finding your way on the map

A **scale bar** sits at the bottom left of the map, in m or km (on a grid in degrees, metres at the latitude of the
view's centre; "cells" on a grid without a coordinate system); *On the map*, in the panel beside it, turns it off
and turns on a **coordinate grid** in the map's own coordinates. **‹** and **›** above the map (or alt+← and alt+→,
or the mouse's back and forward buttons) go back and forth along the views you stopped at. **Go to...** (ctrl+G)
centres the map on a coordinate and marks it until esc: `12.52, 122.40` (latitude, longitude, as web maps give
them), `122.40 12.52` (longitude first without a comma), `12°31'26"N 122°24'05"E`, or x and y in the map's
coordinates.

### Measuring

**Measure**, above the map (or M with the pointer over it), measures a line clicked point by point: each segment
and the whole length, and from three points the area and perimeter of the polygon they close. On a grid in metres
the lengths are in m and km and the areas in m², ha and km²; on a grid in degrees they are measured on the WGS84
ellipsoid. A right click takes the last point away, a double click ends the line, and esc clears it.

### Saving a Plot

What the Viewer shows is gone when you click another node and take the layers away. To keep it with the model,
open the **Plots** menu in the Viewer's header and choose **Save as a new plot**. The Plot keeps the layers (with
their opacity, band and style, the hidden ones too), the map shown and the comparison. Give it a **title** in the
same menu: the title is how the menu lists it and how the Document and the report name its figure.

The **Plots** menu lists the model's Plots. A click opens one: its layers, the map shown, its comparison, a table's
chart. With a Plot open, the menu shows its title, which you can edit, and two more items:

- **Update the plot** keeps in it what the Viewer shows now (after you added a layer, changed an opacity or a
  comparison);
- **Remove the plot** takes it away from the model; its layers stay in the Viewer.

In the model file, the Plots are kept outside the graph, as `[[plots]]`, the first layer at the bottom:

```toml
[[plots]]
id = "plot_forest"
title = "MSPA over the land cover"
layers = [{ from = "named.map" }, { from = "forest_only.map", opacity = 0.6 }]
```

| Key | What it is |
|---|---|
| `id` | the Plot's name in the model |
| `title` | its title, in the menu and over its figure (by default its `id`) |
| `layers` | what it shows, the first at the bottom: each an output (`node.port`) with its `opacity` (0 to 1), `band` (the layer of a stack or the step of a loop), `style` (vector features'), `hidden`, `minus` (another output: the layer is the difference, `from` minus it, or the transitions from it to `from` for classes) and `minus_band` (the layer of the stack or the step of the loop that `minus` is, so that two layers of one stack make a difference) |
| `compare` | a map compared with the top map: `{ with = "node.port", how = "curtain" }` (or `"side by side"`, `"difference"`); side by side, `also = ["node.port", ...]` adds up to two more views, `band` is the band of the stack (or step of the series) `with` is and `also_bands = [...]` those of the `also` maps (from 0; 0 is the first), and `layers = [[...], [...]]` gives the views side by side layers of their own (a list of layers like the Plot's, one per view: first the view of `with`, then those of `also`; an empty list is a view that draws the Plot's layers) |
| `chart` | a table's chart: `{ kind = "bars", x = "...", y = "...", by = "..." }` |
| `view` | the saved view, the frame it is drawn at: `{ extent = [west, south, east, north] }` in the coordinates of its map. The Viewer opens on it (*Keep this view in the plot* in the Plots menu keeps what is in view; *Forget the saved view* lets it go), and the figure, *Export the view*, `terraflux plot` / `export_plot` and the Document show only that part of the map, at the same proportions |
| `note`, `why` | what it shows and why, as a node's note |

A Plot follows a node renamed or put in a group, and loses a layer whose node is removed. `terraflux check` checks
what the Plots read.

### The Plots in the Document and the report

The [Document](../the-model-document/) shows the Plots as numbered figures, each with the step whose output it
shows, its title as the caption. The HTML and PDF report has the same figures, drawn from the last run; a Plot that
compares is a figure of the two maps side by side, and of their difference when it asks for one. The comparison of
scenarios offers the Plots' maps.

### Saving a map as a file

A Plot writes nothing. To keep a map as a file, use **Save output** in the Viewer's header: **As a file...** writes
the map shown to a GeoTIFF (a table to CSV or JSON, vectors to GeoPackage, GeoJSON or Shapefile); **As a file, and
a write node in the model...** also adds a [[raster.write]] (or [[vector.write]]) node, so the file is written
again at every run. From the command line, Plots are not drawn: declare `[outputs]` for what `terraflux run` must
write (see [Your first model](../../getting-started/first-model/#the-model-as-a-file)).

## Tables in the Viewer

Click `classes`, the [[metrics.class]] node: the Viewer shows its table, a row per class, a column per metric.
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

A table is kept as a Plot like a map: its layer is the table, and its `chart` says how to draw it, `kind` (`bars`,
`lines`, `points` or `boxes`; by default the one suggested), `x` (the column along the x axis), `y` (the columns
drawn, comma-separated) and `by` (the column whose values make the groups). Opening the Plot draws that chart:

```toml
[[plots]]
id = "plot_classes"
title = "Share of each class"
layers = [{ from = "classes.table" }]
chart = { kind = "bars", x = "class", y = "pland" }
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

## Models with view.plot nodes

Older models showed their results with `view.plot` nodes (a map, features or a table, another plot's `shown` under
it, a map `compared`). They still open: when a model is read, its `view.plot` nodes become Plots, with their
titles, maps, opacity, features, tables and charts, comparisons and notes; a chain of plots connected by `under`
becomes the layers of one Plot, the lowest first. What read a plot's `shown` reads what the plot showed. Plots
inside groups stay nodes. The library no longer offers `view.plot`, which is kept for old models until Terraflux 2.0.

## Common pitfalls

- **A layer that covers everything.** A map drawn over another hides it except where it has no value. Remove the
  cells you do not want to show (as node 6 does), lower its opacity, or move it down in **Layers**.
- **Maps that do not line up.** Layers are placed by their coordinates; maps of different grids or coordinate
  systems may not overlap as you expect. Put maps on one grid first
  ([One grid](../../getting-started/one-grid/)).
- **A view lost.** Layers stay in the Viewer while you work, not in the model. Save what you want back with
  **Plots › Save as a new plot**, and after a change, **Update the plot**.
- **Rows of the wrong length.** Every row of [[table.of]] must have as many numbers as there are columns.
- **Text in tables.** [[table.of]] holds numbers; codes stand for names (class 3, pond 4). Keep the names in a
  note.
- **Expecting files from a Plot.** A Plot never writes. Use **Save output**, [[raster.write]] or the model's
  `[outputs]`.

## Going further

- [Your first raster](../../getting-started/first-raster/): colours, legends and the Viewer.
- [The tour](../../getting-started/tour/#the-viewer): everything the Viewer does.
- [The model's document](../the-model-document/): the Plots as figures of the Document and the report.
- [Landscape metrics](../../landscape-metrics/): what the columns of a class metrics table mean.
- [Habitat networks](../../habitat-networks/): habitat graphs from maps, and every connectivity index.
- The operators [[table.of]], [[metrics.class]], [[conn.graph]], [[conn.node_importance]].

## References

- Saura, S., & Pascual-Hortal, L. (2007). A new habitat availability index to integrate connectivity in landscape conservation planning: Comparison with existing indices and application to a case study. *Landscape and Urban Planning*, 83(2–3), 91–103. https://doi.org/10.1016/j.landurbplan.2007.03.005
- Saura, S., & Rubio, L. (2010). A common currency for the different ways in which patches and links can contribute to habitat availability and connectivity in the landscape. *Ecography*, 33(3), 523–537. https://doi.org/10.1111/j.1600-0587.2009.05760.x
