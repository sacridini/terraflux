---
title: Your first raster
description: What a raster is (cells, resolution, extent, coordinates, bands, no data, data types), how to open one in Terraflux, and how to read what the viewer shows.
order: 20
level: beginner
time: 30 min
ops: [raster.read, raster.legend, view.plot, raster.write]
---

Almost every map you will use in Terraflux is a **raster**: a land cover map made from satellite images, an
elevation model, a map of forest cover. In this tutorial you will learn what a raster is made of, open a real land
cover map of northern Germany, look at its cells one by one, colour it, name its classes, and meet the two
things that cause the most trouble for beginners: cells with no value and files with many bands. At the end you
will know how to read a raster in Terraflux and what every number the viewer shows means.

## The idea: a map made of cells

A raster is a grid of square **cells** (also called pixels), in rows and columns, where every cell holds **one
number**. What the number means depends on the map: on a land cover map it is the code of a class (1 for forest,
2 for farmland...); on an elevation model it is a height in metres; on a map of a species' habitat suitability
it is a probability between 0 and 1.

![The parts of a raster: cells in rows and columns, an origin, a cell size, an extent, a coordinate system and cells with no value](/img/tutorials/getting-started/first-raster/anatomy.png "The parts of a raster, on a toy map of 8 x 6 cells. Rows and columns are counted from 0, from the top-left corner. Each cell holds one value (here 1 forest, 2 farming, 3 water); the hatched cells hold none.")

The figure above shows everything a raster file stores besides its values:

Cell size (resolution)
:   The length of a cell's side on the ground, for example 30 m. A *fine* resolution means small cells and much
    detail; a *coarse* one means large cells. The cell size decides the smallest thing the map can show: a
    hedge 5 m wide does not exist on a map of 30 m cells.

Origin and extent
:   The coordinates of the top-left corner (the **origin**), and the rectangle the whole grid covers (the
    **extent**: its west, south, east and north edges). With the origin and the cell size, every cell has a
    place on Earth: the cell in row 1, column 5 starts 5 x 30 m east and 1 x 30 m south of the origin.

Coordinate reference system (CRS)
:   What the coordinates mean: metres east and north in a map projection, or degrees of longitude and latitude.
    Without it, the numbers of the origin are just numbers. The tutorial
    [Coordinate systems and reprojection](../coordinate-systems/) is all about it.

Bands
:   A raster file may hold several grids of the same size, one over the other: the years of a land use series,
    the drivers of a model, the channels of a satellite image. Each grid is a **band**.

No data
:   A special value that means "no value here": outside the study area, under a cloud, in the sea. Those cells are
    left out of every count and every calculation.

Data type
:   How the numbers are stored. Whole numbers take less room than decimal ones:

| Type | Holds | Typical use | Bytes per cell |
|---|---|---|---|
| `uint8` (Byte) | whole numbers 0 to 255 | land cover classes, binary maps | 1 |
| `int16`, `uint16` | whole numbers to about ±32,000 or 0 to 65,535 | elevation in metres, counts | 2 |
| `int32` | whole numbers to about ±2 billion | patch identifiers | 4 |
| `float32` | decimal numbers, about 7 digits | probabilities, distances, indices | 4 |
| `float64` | decimal numbers, about 15 digits | precise calculations | 8 |

!!! science "Key idea: areas come from counting cells"
    On a map in metres, the area of a class is its number of cells times the area of one cell:

    \[ A_k = n_k \times s^2 \]

    where \(n_k\) is the number of cells of class \(k\) and \(s\) the cell size. With cells of 25 m
    (\(s^2 = 625\ \text{m}^2\)), 100,000 cells of forest are 62.5 km². This is why the resolution, and a CRS in
    metres, matter for every measure of a landscape.

### Classes and measurements

The most important question to ask about a raster is: **are its values classes or measurements?**

- A **categorical** raster holds *codes* of classes. The numbers are labels: class 24 (coniferous forest) is
  not "twice" class 12 (arable land), and the mean of 12 and 24 is meaningless. Land cover, land use, soil types
  and MSPA's classes are categorical.
- A **continuous** raster holds *measurements*: elevation, temperature, NDVI, a distance, a probability. Means,
  differences and interpolation make sense.

Landscape ecology mostly works with categorical maps of habitat and land cover, where a landscape is a mosaic of
patches of classes (Turner & Gardner, 2015), and with continuous surfaces for terrain and gradients. The
difference decides how a map is coloured (below), how it may be resampled
([One grid for all your maps](../one-grid/)) and which methods apply.

## When to use a raster

Use rasters for anything that covers the whole study area cell by cell: classified land cover, elevation,
climate, distances, the probability of a change, the output of a simulation. Almost every method in Terraflux
(MSPA, landscape metrics, circuit connectivity, prioritization, land change models) reads and writes rasters.
Things that are discrete objects with exact boundaries, such as sampling plots, roads, rivers and the polygons of
reserves, are better stored as **vector data**: see [Vector data](../vector-data/). Terraflux turns them into
rasters when a method needs it.

## What you need

A raster file. **GeoTIFF** (`.tif`) is the usual format and the one Terraflux writes, but Terraflux reads every
format GDAL reads (GDAL is the open-source library behind almost every GIS; Terraflux brings its own copy). This
tutorial uses four real datasets:

- **CORINE Land Cover**: an excerpt of 50 x 50 km around Hanover, Lower Saxony, with the lake Steinhuder Meer,
  at 25 m. CORINE maps 44 classes of land cover over Europe, from satellite images, for the European Environment
  Agency (Büttner, 2014); you can download it from the Copernicus Land Monitoring Service.
- **Elevation** from the Shuttle Radar Topography Mission (SRTM; Farr et al., 2007), in south-western Oregon,
  cells of about 30 m.
- **NDVI** of the same area: the normalised difference vegetation index, a measure of green vegetation from the
  red and near-infrared light a surface reflects (Tucker, 1979), from 0 (bare) to about 0.9 (dense forest).
- **Sibuyan Island** (Philippines), mapped in 1997 and 2011 in five classes, the classic example of the CLUE-S
  land change model (Verburg et al., 2002), and a stack of 13 explanatory variables, as distributed with R's
  lulcc package (Moulds et al., 2015).

## Open a raster

In Terraflux, every step of an analysis is a **node** in a **model**: a box that does one thing. Opening a raster
adds a node of the operator [[raster.read]].

<div class="steps" markdown>

1. Start a new model with **New** in the toolbar.
2. Open the raster: **Open a raster...** at the top of the **Operators** pane (the drawer that slides out when
   the pointer touches the window's left edge), or ++ctrl+o++, or **Operators > Open a raster or vectors...** in
   the toolbar. Choose `clc.tif`.
3. A node `raster.read` appears on the canvas, already selected. Its map is shown in the **Viewer** under the
   canvas at once.

</div>

```tfgraph
#! caption: Read a raster and show it in a tab of its own.
#! highlight: landcover
format = "terraflux/1"

[[nodes]]
id = "landcover"
op = "raster.read"
params = { path = "clc.tif" }
note = "CORINE Land Cover around Hanover: 2000 x 2000 cells of 25 m, EPSG:3035."

[[nodes]]
id = "plot"
op = "view.plot"
inputs = { value = "landcover.raster" }
params = { title = "Land cover" }
note = "The map in a tab of its own, refreshed at every run."
```

The node has one output, `raster`: the file **on disk**. Only its header (size, cells, coordinates) has been
read, so a raster of any size opens at once, and the next node reads what it needs:

- a node that takes a raster (most analyses) reads **one band**, the first unless the setting `band` says
  otherwise (by name, such as `1997`, or by number counted from 0);
- a node that takes a stack reads **every band** of the file, as a stack of maps (see
  [Stacks](#stacks-many-maps-in-one-file));
- the operators that work on files (reprojecting, resampling, clipping) take the file itself.

Its settings, in the **Node** pane on the left, are `path` (the file, relative to the model's folder; **Choose...**
opens a dialog), `band` and `nodata` (a value that replaces the file's own no-data value).

The second node, [[view.plot]], is optional: it shows a map in a tab of its own, refreshed every time the
model runs, without writing any file. To add it, keep `landcover` selected and click **plot** under **Plot**
in the operators' library: a new node comes already connected to the selected one.

## What the viewer shows

![The CORINE land cover excerpt around Hanover, with its 26 classes and their shares](/img/tutorials/getting-started/first-raster/landcover.png "CORINE Land Cover around Hanover (50 x 50 km, cells of 25 m). Arable land covers half of the area; the city is the red mass on the right, the Steinhuder Meer the lake on the left, surrounded by peat bogs.")

This is the map as Terraflux reads it: 2000 rows by 2000 columns, four million cells of 25 x 25 m, in
`uint8`, with 26 of CORINE's 44 classes present. Beside the map the viewer lists the classes with their colour
and their number of cells; here the shares are written as percentages: non-irrigated arable land covers 50.3%
of the area, coniferous forest 10.0%, broad-leaved forest 7.7%, discontinuous urban fabric 10.8%.

Move the pointer over the map. In the bottom-left corner of the viewer, Terraflux writes what is under it:

```text
x 4264162.50   y 3277587.50   ·   row 196  column 116   ·   40
```

- `x` and `y` are the **coordinates** of the pointer in the map's CRS (here metres of the European projection
  EPSG:3035; for a map in degrees, longitude and latitude with six decimals);
- `row` and `column` are the **cell** under the pointer, counted from 0 at the top-left corner;
- the last number is the cell's **value**, here 40 (water courses), or `no value`.

Zoom with the mouse wheel (around the pointer), pan by dragging, and double-click to see the whole map again.
When you are close enough to see the cells, the one under the pointer is outlined. The three images below are
three zooms of the same place, down to single cells:

```tfgallery
columns: 3
caption: "Zooming in on the north-west of the map: a 15 x 15 km window (the box is the next zoom), 2.5 x 2.5 km, and 12 x 12 cells of 25 m with their values: 40 water courses (a river), 41 water bodies, 24 coniferous forest, 12 arable land, 20 complex cultivation, 2 discontinuous urban fabric."
images:
  - { src: /img/tutorials/getting-started/first-raster/zoom_1.png, caption: "15 km: 600 x 600 cells" }
  - { src: /img/tutorials/getting-started/first-raster/zoom_2.png, caption: "2.5 km: 100 x 100 cells" }
  - { src: /img/tutorials/getting-started/first-raster/zoom_3.png, caption: "300 m: 12 x 12 cells" }
```

The edges of the river and the forest are **staircases**: a raster can only follow a boundary cell by cell. The
river, 25 to 75 m wide on the ground, is one to three cells wide. Anything narrower than a cell either takes the
whole cell or disappears; this is the first thing to keep in mind when you choose a resolution.

!!! app "In the application: the facts of a raster"
    The **Metadata** button in the viewer's header lists what the file says about itself: the file and the
    library that read it, the **coordinate system**, the **size** (columns x rows), the **cell size**, the
    **extent** and the **origin**, the **layers** (bands), the **data type** and the **no data** value. **Copy**
    copies them as text, for your notes or a methods section. For `clc.tif`: EPSG:3035, 2000 x 2000 cells, cell
    size 25 x 25, extent from x 4,261,250 to 4,311,250 and y 3,232,500 to 3,282,500, `uint8`, no data: none.

## Colours: classes or a ramp

Terraflux chooses colours by **what the values are**, as landschaft's plots do: a map of a few whole numbers is
drawn as classes, a colour each (and a legend of classes); any other map is drawn with a **colour ramp** and a
bar. Elevation, hillshades and NDVI are recognised by the names of their nodes, files and bands and get their own
ramps; the products of GuidosToolbox (MSPA, fragmentation) get GuidosToolbox's colours and names.

```tfgallery
columns: 2
caption: "Two continuous rasters of the same area of Oregon (0.1 x 0.1 degrees). Left: SRTM elevation, 433 to 1,390 m, with a terrain ramp. Right: summer NDVI, 0.10 to 0.84, from red (bare) to green (dense vegetation); the pale patches are clear-cuts."
images:
  - { src: /img/tutorials/getting-started/first-raster/elevation.png, caption: "Elevation (m)" }
  - { src: /img/tutorials/getting-started/first-raster/ndvi.png, caption: "NDVI" }
```

You can override the choice under **Colours** beside the map: *by what they are* (the default), or a ramp by
name (*viridis*, *blue to red, centred on 0* for differences, *terrain*, *grey*, *red to green*, *rank*), each
also on a logarithmic scale for values spread over orders of magnitude (distances, densities, current flows).
The choice is remembered for that output.

!!! tip "Which ramp?"
    For a continuous map use a ramp whose lightness changes steadily, such as viridis, so that the eye reads
    higher values as "more". For differences (a change, a residual) use a diverging ramp centred on 0. Never use
    a ramp for classes: the colours suggest an order the codes do not have.

## Names and colours of classes: legends

Codes are hard to read: you will not remember that 24 is coniferous forest. A **legend** gives each class a name
and a colour. In Terraflux a legend is a node, [[raster.legend]], placed after the map; it carries the names
with the map to everything downstream that keeps its classes, and into the GeoTIFF the map is written to.

You rarely type it: under the legend beside the map, **Edit the legend** opens a colour and a name per class;
**Apply** writes them into the model as a `raster.legend` node after the node shown (or changes it, if the node
shown is one). Here is the legend of Sibuyan's land use, as the model file keeps it:

```tfgraph
#! caption: Read the land use of Sibuyan in 1997 (a band of the file, by name), name its classes, show it.
#! highlight: legend
format = "terraflux/1"

[[nodes]]
id = "landuse"
op = "raster.read"
params = { path = "sibuyan_landuse.tif", band = "1997" }
note = "Two bands, 1997 and 2011: the setting band chooses one by its name."

[[nodes]]
id = "legend"
op = "raster.legend"
inputs = { map = "landuse.raster" }
params = { classes = [[1, "Forest", "#3b8a5a"], [2, "Coconut", "#d9a441"], [3, "Grassland", "#a7c957"], [4, "Rice", "#e8d36a"], [5, "Other", "#b4508a"]] }
note = "The classes of lulcc's Sibuyan data, with their names and colours."

[[nodes]]
id = "plot"
op = "view.plot"
inputs = { value = "legend.map" }
params = { title = "Sibuyan, 1997" }
```

![Sibuyan's land use in 1997 with its legend](/img/tutorials/getting-started/first-raster/sibuyan_legend.png "Sibuyan Island in 1997, cells of 250 m, with the legend of the model above. Forest covers 65.4% of the island's cells, coconut plantations 16.0%, mostly near the coast.")

The setting `classes` is a list of `[value, "name", "#rrggbb"]`. Colours are written as hexadecimal red, green
and blue, as on the web.

## No data

Sibuyan is an island: most of its rectangle is sea. The file marks the sea with the value 255 and says, in its
header, that 255 means **no data**. Terraflux reads that and leaves those cells out: of the 128 x 108 = 13,824
cells of the grid, 7,226 have a value, and the shares of the legend above are shares of the island, not of the
rectangle.

If the file did not say it (or said it wrong), 255 would be read as a class like any other. The setting `nodata`
of [[raster.read]] replaces the file's value; giving it a value that never occurs (here -1) shows what happens
then:

```tfcompare
caption: "Drag the handle. Left: 255 read as no data (the sea left out, white). Right: the same file with the setting nodata = -1, so that 255 is read as a class: 6,598 cells of \"sea class\" (dark grey), 48% of the map. Every share, every metric of the landscape and every model fitted on it would be wrong."
before: { src: /img/tutorials/getting-started/first-raster/nodata_on.png, label: "255 = no data" }
after: { src: /img/tutorials/getting-started/first-raster/nodata_off.png, label: "255 read as a value" }
```

!!! warning "Check the no-data value of every new file"
    Open **Metadata** and look at *no data*. Common values are 0, 255 (for `uint8`), -9999, -32768 and NaN. If a
    file uses one but does not declare it, set `nodata` on [[raster.read]]. A map of classes that suddenly has a huge class at its border is
    the usual sign.

## Stacks: many maps in one file

The file `sibuyan_drivers.tif` holds 13 bands on the same grid: the variables that explain where land use
changed (distances to the sea, roads, towns and streams, elevation, slope, soils, erosion, population density).
A set of maps on one grid is a **stack**, and each map a **layer**. The `raster` of [[raster.read]] is them all
where a node takes a stack, and one of them (`band`) where it takes a raster. Move the slider to go through them:

```tfslider
label: Band
caption: "The 13 bands of sibuyan_drivers.tif, a stack of the variables of Sibuyan's land change model. Continuous variables are drawn with viridis, the soil and erosion classes as 0 and 1."
frames:
  - { value: 1, src: /img/tutorials/getting-started/first-raster/driver_1.png, caption: "1 dist_sea: distance to the sea, 0 to 8,325 m" }
  - { value: 2, src: /img/tutorials/getting-started/first-raster/driver_2.png, caption: "2 pop_density: population density" }
  - { value: 3, src: /img/tutorials/getting-started/first-raster/driver_3.png, caption: "3 diorite: diorite soils" }
  - { value: 4, src: /img/tutorials/getting-started/first-raster/driver_4.png, caption: "4 ultramafic: ultramafic soils" }
  - { value: 5, src: /img/tutorials/getting-started/first-raster/driver_5.png, caption: "5 sediments" }
  - { value: 6, src: /img/tutorials/getting-started/first-raster/driver_6.png, caption: "6 no_erosion" }
  - { value: 7, src: /img/tutorials/getting-started/first-raster/driver_7.png, caption: "7 moderate_erosion" }
  - { value: 8, src: /img/tutorials/getting-started/first-raster/driver_8.png, caption: "8 elevation: 0 to 1,786 m" }
  - { value: 9, src: /img/tutorials/getting-started/first-raster/driver_9.png, caption: "9 slope: 0 to 54 degrees" }
  - { value: 10, src: /img/tutorials/getting-started/first-raster/driver_10.png, caption: "10 aspect: the direction a slope faces, 0 to 360 degrees" }
  - { value: 11, src: /img/tutorials/getting-started/first-raster/driver_11.png, caption: "11 dist_roads: distance to roads, 0 to 7,458 m" }
  - { value: 12, src: /img/tutorials/getting-started/first-raster/driver_12.png, caption: "12 dist_urban: distance to towns, 0 to 12,733 m" }
  - { value: 13, src: /img/tutorials/getting-started/first-raster/driver_13.png, caption: "13 dist_streams: distance to streams, 0 to 2,305 m" }
```

In the viewer, a stack is shown one layer at a time, with **<** and **>** buttons and a slider beside the map. **Clicking a cell** of a stack draws a chart of that cell's value in every layer (by the layers' names when
they are numbers, as years are): the history of one place along a time series.

A stack can also come from **many files**. Give [[raster.read]] a folder, or a pattern such as `forest/*.tif`, and
every band of every file becomes a layer, named after the files (**Open a folder as a stack...** in the
operators' pane, or **A folder...** beside `path`). The files must share a grid to form a stack; files of
different grids are shown one by one, and [[raster.match]] puts them on one grid first (see
[One grid for all your maps](../one-grid/)).

## Large rasters

Opening a raster reads only its header. The viewer then reads just the tiles in view, at the screen's
resolution, in threads, showing a coarser level while it waits: a raster of billions of cells appears in under a
second and zooms smoothly. Nothing is loaded whole until a node needs a `map` or a `stack`. Operators that work on
files ([[raster.reproject]], [[raster.resample]], [[raster.clip]]...) never load it, and several analyses (landscape
metrics, zonal statistics, rasterizing, MSPA by tiles) read rasters of more than 250 million cells by strips, so
that maps larger than memory still work. See the category [Large data and batches](../../large-data/).

## Save a raster

What you see can be kept. In the viewer's header, **Save output > As a file...** writes the map shown to a GeoTIFF
(in the background); **As a file, and a write node in the model...** also adds a [[raster.write]] node, so that
the model writes it again at every run. [[raster.write]] picks the smallest data type that holds the values,
unless its setting `type` says otherwise. The tutorial [Reclassify, calculate and save](../reclassify-calculate-save/)
covers writing in detail.

## Common pitfalls

- **No data not declared.** The border, the sea or the clouds become a class, and every share is wrong. Check
  *no data* in Metadata; set `nodata` on [[raster.read]].
- **Averaging class codes.** The mean of codes 12 and 24 is 18, which in CORINE is "pastures": a class that is
  not there. Never interpolate or average a categorical map (see [Coordinate systems](../coordinate-systems/)).
- **The wrong band.** A file of several bands gives its first band as `map`. Set `band` (by name, `1997`, or
  number from 0) when you want another, and check the layer names in Metadata.
- **Areas in degrees.** On a map in longitude and latitude, cells are not squares on the ground and their area
  changes with latitude. Count areas on a map in metres, or in an equal-area projection.
- **A file without a CRS.** `sibuyan_landuse.tif` has coordinates (x from 436,000 to 468,000) but its header
  does not say in which system; Metadata shows *none*. Terraflux can still use it on its own, but cannot place a
  basemap under it or reproject it until you say what it is (the setting `source_crs` of [[raster.reproject]]).
- **Classes stored as decimals.** A map of classes in `float32` may hold 2.9999998 instead of 3 after some
  processing; its legend shows many "classes". Round it ([[map.calc]] with `round(x)`) or write it as an integer
  type.

## Going further

- [A tour of Terraflux](../tour/): the panes, templates and keys of the application.
- [Vector data](../vector-data/): points, lines and polygons, and how they meet rasters.
- [Coordinate systems and reprojection](../coordinate-systems/): degrees, metres and equal areas.
- [One grid for all your maps](../one-grid/): resolution, alignment and resampling.
- The operators: [[raster.read]], [[raster.legend]], [[view.plot]], [[raster.write]].
- The [Glossary](/glossary/) for every term in one place.

## References

- Büttner, G. (2014). CORINE land cover and land cover change products. In I. Manakos & M. Braun (Eds.), *Land Use and Land Cover Mapping in Europe* (pp. 55–74). Springer. https://doi.org/10.1007/978-94-007-7969-3_5
- Farr, T. G., Rosen, P. A., Caro, E., Crippen, R., Duren, R., Hensley, S., et al. (2007). The Shuttle Radar Topography Mission. *Reviews of Geophysics*, 45(2), RG2004. https://doi.org/10.1029/2005RG000183
- Moulds, S., Buytaert, W., & Mijic, A. (2015). An open and extensible framework for spatially explicit land use change modelling: the lulcc R package. *Geoscientific Model Development*, 8(10), 3215–3229. https://doi.org/10.5194/gmd-8-3215-2015
- Tucker, C. J. (1979). Red and photographic infrared linear combinations for monitoring vegetation. *Remote Sensing of Environment*, 8(2), 127–150. https://doi.org/10.1016/0034-4257(79)90013-0
- Turner, M. G., & Gardner, R. H. (2015). *Landscape Ecology in Theory and Practice: Pattern and Process* (2nd ed.). Springer. https://doi.org/10.1007/978-1-4939-2794-4
- Verburg, P. H., Soepboer, W., Veldkamp, A., Limpiada, R., Espaldon, V., & Mastura, S. S. A. (2002). Modeling the spatial dynamics of regional land use: the CLUE-S model. *Environmental Management*, 30(3), 391–405. https://doi.org/10.1007/s00267-002-2630-x
