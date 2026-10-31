---
title: Vector data
description: Points, lines and polygons with their attributes; how to read them, show them over a map, burn them into a grid, measure a map in each polygon, read values at points and measure distances to features.
order: 30
level: beginner
time: 30 min
ops: [vector.read, view.plot, vector.rasterize, vector.zonal, vector.extract, vector.distance, vector.buffer, vector.write]
---

Not everything in a landscape study is a grid of cells. Sampling plots are points, roads and rivers are lines,
reserves and parcels are polygons. In this tutorial you will read such **vector data** in Terraflux, draw it
over a land cover map, and then do the four things that join vectors and rasters in almost every study: burn
polygons into a grid, measure a map inside each polygon, read a map at points, and measure the distance from
every cell to a road. At the end you will have a table of the forest in each reserve, the land cover at each plot,
and a map of the distance to a planned road.

## The idea: features with attributes

Vector data describes the world as **features**: objects with an exact geometry and a row of **attributes**
(values that describe them) in a table. There are three kinds of geometry:

![Points, lines and polygons](/img/tutorials/getting-started/vector-data/kinds.png "The three kinds of features. Points have a position, lines a length, polygons an area. A layer holds features of one kind, each with its attributes.")

- **Points**: a position. Sampling plots, occurrence records of a species, nests, camera traps.
- **Lines**: a sequence of positions. Roads, rivers, fences, transects, movement paths.
- **Polygons**: a closed outline, possibly with holes. Reserves, parcels, lakes, municipalities, the range of a
  species.

A raster says what is in *every cell* of a grid; a vector says *where each object is*, as precisely as its
coordinates allow, and nothing about the space between objects. That makes vectors the natural form for things
that are discrete and have sharp boundaries, and for anything measured at a few places. A road 8 m wide is a
line on a vector layer, but on a raster of 25 m cells it is either a row of whole cells or nothing.

The way GIS software stores features follows the **Simple Features** standard of the Open Geospatial Consortium
(Herring, 2011): every program, from QGIS to R's sf package (Pebesma, 2018), reads the same geometries. Terraflux
reads and writes them through GDAL's vector library, OGR (GDAL/OGR contributors).

| Format | Files | Notes |
|---|---|---|
| GeoPackage | `.gpkg` | One file, many layers, any attribute names. The best default. |
| Shapefile | `.shp` with `.shx`, `.dbf`, `.prj` | The oldest and most common. Keep all the files together; attribute names of 10 characters at most. |
| GeoJSON | `.geojson`, `.json` | Text, readable, common on the web; usually in longitude and latitude. |
| FlatGeobuf | `.fgb` | One file, fast for large layers. |
| KML | `.kml` | Google Earth's format. |

## When to use it

Vector data comes into a landscape study in four ways, and Terraflux has one operator for each:

| Question | Operator | Example |
|---|---|---|
| Which cells does each feature cover? | [[vector.rasterize]] | protected areas as a map, for a prioritization or a land change model's restrictions |
| What is the map like inside each polygon? | [[vector.zonal]] | the forest share of every reserve, the mean elevation of every parcel |
| What is the map's value at each point? | [[vector.extract]] | the land cover at each plot, the climate at each occurrence record |
| How far is every cell from the features? | [[vector.distance]] | the distance to roads or rivers, a driver of deforestation |

Many methods also take vectors directly: landscape metrics of sample plots (`metrics.sample`), planning units of
polygons for spatial prioritization (`prioritize.optimize`), occurrence records for species distribution models. For
anything that varies continuously across the area, use rasters: see [Your first raster](../first-raster/).

## What you need

A vector file in any format OGR reads. This tutorial draws three layers over the 15 x 15 km north-western window
of the CORINE land cover around Hanover used in [Your first raster](../first-raster/) (cells of 25 m,
EPSG:3035). The layers were **made for the tutorial** and are not real places:

- `reserves.gpkg`: three polygons, "South", "North" and "West", drawn around the three largest forests of the
  window, with the attributes `id` (1 to 3), `name` and `year` (of designation);
- `road.gpkg`: one line, a "planned road" of 18 km, with the attributes `name` and `lanes`;
- `plots.gpkg`: twelve sampling plots placed at random, with the attribute `plot`.

All three are in the same coordinate system as the map. When they are not, Terraflux reprojects the features
to the map's system before rasterizing, extracting or measuring distances (see the pitfalls below).

## Read features and show them over a map

In the application, **Open vectors...** in the **Operators** pane (or ++ctrl+o++, which opens rasters and vectors
alike) adds a [[vector.read]] node for the file chosen. Its settings are `path` and `layer` (a GeoPackage may
hold several layers; the first is read by default). It has two outputs:

- `vector`: the features, with their geometries, attributes and coordinate system;
- `table`: their attributes **that are numbers**, one row per feature numbered from 1, for operators that take
  tables. Text attributes (here `name`) stay with the features.

To see features, select the [[vector.read]] node: the viewer draws them. To see them **over a map**, use plots
that stack: a [[view.plot]] whose `under` input is connected to another plot's `shown` output is drawn on top of
it. Here the land cover is at the bottom, the reserves over it, then the road and the plots:

```tfgraph
#! caption: Read a map and three vector layers, and stack them in plots: each plot's under is the plot drawn beneath it.
#! highlight: reserves, road, plots
format = "terraflux/1"

[[nodes]]
id = "landcover"
op = "raster.read"
params = { path = "landcover.tif" }
note = "The 15 x 15 km window of CORINE around Hanover, cells of 25 m."

[[nodes]]
id = "reserves"
op = "vector.read"
params = { path = "reserves.gpkg" }
note = "Three polygons made for the tutorial: id, name, year."

[[nodes]]
id = "road"
op = "vector.read"
params = { path = "road.gpkg" }

[[nodes]]
id = "plots"
op = "vector.read"
params = { path = "plots.gpkg" }

[[nodes]]
id = "map"
op = "view.plot"
inputs = { value = "landcover.raster" }
params = { title = "Land cover" }

[[nodes]]
id = "with_reserves"
op = "view.plot"
inputs = { features = "reserves.vector", under = "map.shown" }
params = { title = "Reserves" }

[[nodes]]
id = "with_road"
op = "view.plot"
inputs = { features = "road.vector", under = "with_reserves.shown" }
params = { title = "Road" }

[[nodes]]
id = "everything"
op = "view.plot"
inputs = { features = "plots.vector", under = "with_road.shown" }
params = { title = "Everything" }
```

![The three layers over the land cover](/img/tutorials/getting-started/vector-data/overlay.png "The reserves (black outlines), the planned road and the twelve plots (orange, numbered) over the land cover of the window. Colours of the land cover as in CORINE: yellow arable land, greens forests, red towns, blue peat bogs.")

!!! app "In the application: vectors without a plot"
    You do not need plots to look. Beside any map, the viewer's **Vectors over the map** lists every vector
    output of the run: tick one to draw it over the map, reprojected to it, and choose its style. Hover over a
    feature to read its attributes (`feature 2`, `id: 2`, `name: North`, `year: 1998`). **Maps over it** does the
    same for other maps, each with its opacity.

!!! app "In the application: draw a study area"
    **Draw on the map**, under those, draws features on the map shown: **polygons** (a click a vertex; Enter, a
    double click or a click on the first vertex closes one), **rectangles** (a corner, then the opposite one),
    **points** (a click each) or **lines**. Backspace takes back the last vertex, Escape the feature being drawn.
    **Add to the model** writes them as a GeoPackage beside the model (`drawn/study_area.gpkg`, an `id` each, in
    the map's coordinate system) and adds a [[vector.read]] node that reads it, so the model keeps them: clip by
    them, rasterize them, sample in them as with any vector layer, and open them in QGIS.

## Burn polygons into the grid

Most methods work on rasters, so polygons often have to become a map: a map of reserves for a prioritization,
a map of restrictions for a land change model. [[vector.rasterize]] **burns** features into the grid of a map
given at its `like` input (only its grid is read: its CRS, extent and cells). Each cell takes the value of the
feature over it: the attribute named by `field`, or the setting `value` (1 by default) for every feature. Cells
no feature takes get `fill` (0 by default; `null` for no value).

```tfgraph
#! caption: Burn the reserves into the land cover's grid, each cell taking the reserve's id.
#! highlight: burnt
format = "terraflux/1"

[[nodes]]
id = "landcover"
op = "raster.read"
params = { path = "landcover.tif" }

[[nodes]]
id = "reserves"
op = "vector.read"
params = { path = "reserves.gpkg" }

[[nodes]]
id = "burnt"
op = "vector.rasterize"
inputs = { vector = "reserves.vector", like = "landcover.raster" }
params = { field = "id" }
note = "1, 2 or 3 inside a reserve, 0 outside."
```

![The reserves burnt into the grid](/img/tutorials/getting-started/vector-data/burnt.png "The reserves as a raster on the land cover's grid: each cell holds the id of the reserve whose polygon covers its centre. The three reserves cover 14.0% of the window.")

Which cells does a polygon take? By default, those whose **centre** falls inside it. With `all_touched = true`,
every cell the outline touches as well. Drag the handle to compare them on the western tip of the reserve "West":

```tfcompare
caption: "A corner of the reserve West, cells of 25 m. Left: the cells whose centre is inside the polygon (the default). Right: every cell the polygon touches (all_touched = true). The reserves grow by 2.0% (South) to 3.2% (West): 16,837 cells against 17,178 for South."
before: { src: /img/tutorials/getting-started/vector-data/edge_centre.png, label: "Cell centres" }
after: { src: /img/tutorials/getting-started/vector-data/edge_touched.png, label: "Every cell touched" }
```

!!! tip "Which rule?"
    Cell centres give the closest area to the polygon's own, with no bias: about as many cells are added at
    the edges as are left out. Every cell touched always overestimates, but makes sure that **small or thin
    features** (a narrow reserve, a stream, a road) are not lost: use it for lines, and for polygons smaller
    than a few cells.

Two more settings matter when features overlap: `merge = "replace"` (the default: the last feature wins) or
`merge = "add"` (values added up, for example to count how many polygons cover each cell).

## Statistics of a map in each polygon

How much forest does each reserve hold? [[vector.zonal]] computes **zonal statistics**: statistics of a map's
cells within each polygon (each polygon's cells taken as [[vector.rasterize]] burns them). Its `stats` setting
lists them: `count`, `nodata`, `sum`, `mean`, `min`, `max`, `range`, `std`, `var`, `median`, `majority`,
`minority`, `unique` and percentiles such as `percentile_90`.

A trick turns a mean into a **share**: a map that is 1 where there is forest and 0 elsewhere (made with
[[raster.reclassify]]) has, in each polygon, a mean equal to the share of its cells that are forest.

```tfgraph
#! caption: The share of forest in each reserve (the mean of a 1/0 forest map), and the most common land cover class.
#! highlight: forest, forest_share, cover
format = "terraflux/1"

[[nodes]]
id = "landcover"
op = "raster.read"
params = { path = "landcover.tif" }

[[nodes]]
id = "reserves"
op = "vector.read"
params = { path = "reserves.gpkg" }

[[nodes]]
id = "forest"
op = "raster.reclassify"
inputs = { map = "landcover.raster" }
params = { rules = "23..25 -> 1\n* -> 0" }
note = "CORINE's three forest classes (23 broad-leaved, 24 coniferous, 25 mixed) become 1, the rest 0."

[[nodes]]
id = "forest_share"
op = "vector.zonal"
inputs = { vector = "reserves.vector", map = "forest.map" }
params = { stats = ["count", "mean"] }
note = "count: the reserve's cells; mean: the share of them that is forest."

[[nodes]]
id = "cover"
op = "vector.zonal"
inputs = { vector = "reserves.vector", map = "landcover.raster" }
params = { stats = ["majority", "unique"] }
note = "majority: the most common class; unique: how many classes."

[outputs.forest_share]
from = "forest_share.table"

[outputs.cover]
from = "cover.table"
```

The result is a **table**, one row per feature in the order of the file (`feature` 1, 2, 3...). The Viewer shows
a table with its rows filtered and sorted as you ask and its columns drawn as charts; `terraflux run` writes it
as a CSV file. Joined with the
reserves' names and areas:

| Feature | Reserve | Cells | Area (ha) | Forest share | Most common class | Classes |
|---|---|---|---|---|---|---|
| 1 | South | 16,837 | 1,052 | 79.0% | 24 coniferous forest | 7 |
| 2 | North | 17,703 | 1,106 | 71.3% | 24 coniferous forest | 9 |
| 3 | West | 15,861 | 991 | 76.7% | 24 coniferous forest | 6 |

The area is the number of cells times 625 m² (25 x 25 m). The North reserve is the largest but holds the least
forest: its outline takes in part of a town (the red patch inside it on the map above). That is the kind of
check a zonal table makes easy.

## Values of a map at points

At each sampling plot, what is the land cover? [[vector.extract]] reads a map (or every layer of a stack) at
points: the cell that holds each point (`method = "nearest"`, the default) or a value interpolated from the four
nearest cell centres (`bilinear`, for continuous maps only). With a `buffer` (a radius in map units) it gives
statistics of the cells within that distance instead, the usual way to describe the surroundings of a plot.

```tfgraph
#! caption: The land cover at each plot, and the share of forest within 250 m of each.
#! highlight: at_plots, around_plots
format = "terraflux/1"

[[nodes]]
id = "landcover"
op = "raster.read"
params = { path = "landcover.tif" }

[[nodes]]
id = "plots"
op = "vector.read"
params = { path = "plots.gpkg" }

[[nodes]]
id = "forest"
op = "raster.reclassify"
inputs = { map = "landcover.raster" }
params = { rules = "23..25 -> 1\n* -> 0" }

[[nodes]]
id = "at_plots"
op = "vector.extract"
inputs = { vector = "plots.vector", map = "landcover.raster" }

[[nodes]]
id = "around_plots"
op = "vector.extract"
inputs = { vector = "plots.vector", map = "forest.map" }
params = { buffer = 250, stats = ["mean"] }
note = "The mean of the 1/0 forest map within 250 m: the share of forest around each plot."
```

| Plot | Land cover at the plot | Forest within 250 m |
|---|---|---|
| 1, 2, 3, 6, 7, 8, 10 | 12 non-irrigated arable land | 0% |
| 4 | 36 peat bogs | 0% |
| 5 | 18 pastures | 22.2% |
| 9 | 2 discontinuous urban fabric | 0% |
| 11 | 21 agriculture with natural vegetation | 13.0% |
| 12 | 24 coniferous forest | 66.3% |

Seven of the twelve random plots fell on arable land, about what you would expect when half of the area is arable.
The output `table` holds the plot's attributes and the values; the output `vector` is the points with the values
as new attributes, to write or to show.

## Distances to features

Distances to roads, rivers and towns are among the most used explanatory variables of land change and species
distribution models. [[vector.distance]] gives, for every cell of the grid at `like`, the distance from its
centre to the nearest cell of the features (burnt as [[vector.rasterize]] burns them), in map units: here
metres. [[vector.buffer]] does the opposite job: it makes the **area** within a distance of each feature, a new
polygon layer (a corridor along a line, circles around points), its distance in the features' units.

```tfgraph
#! caption: The distance from every cell to the planned road, and a 500 m corridor along it, written to a GeoPackage.
#! highlight: distance, corridor, save
format = "terraflux/1"

[[nodes]]
id = "landcover"
op = "raster.read"
params = { path = "landcover.tif" }

[[nodes]]
id = "road"
op = "vector.read"
params = { path = "road.gpkg" }

[[nodes]]
id = "distance"
op = "vector.distance"
inputs = { vector = "road.vector", like = "landcover.raster" }
note = "Metres from every cell's centre to the road (0 on it)."

[[nodes]]
id = "corridor"
op = "vector.buffer"
inputs = { vector = "road.vector" }
params = { distance = 500 }
note = "The land within 500 m of the road: a polygon."

[[nodes]]
id = "save"
op = "vector.write"
inputs = { vector = "corridor.vector" }
params = { path = "corridor.gpkg" }
```

![The distance to the planned road and its 500 m corridor](/img/tutorials/getting-started/vector-data/distance.png "The distance from every cell to the planned road, from 0 on the road to 10.9 km in the north-west corner, with the 500 m corridor (dashed) made by vector.buffer. 8.4% of the window lies within 500 m of the road.")

[[vector.write]] writes features to a file, its format chosen by the extension: `.gpkg` (the default), `.shp`,
`.geojson`, `.fgb`, `.kml` or `.gml`. Distances can also be measured on a raster: [[distance.proximity]] gives
the distance to cells of some values (to class 40, water courses, for example), on a file of any size.

!!! app "In the application: Prepare"
    Select a [[vector.read]] node and open the **Prepare** pane (beside the Node pane, on the left). Once the model
    has a reference grid (usually its land cover map), Prepare offers the vector steps in one click each:
    **Distance to these features**, **Burn into the grid** and **Statistics of the reference map in each
    polygon**. Each adds the operator's node, already connected, with a note saying why. See
    [One grid for all your maps](../one-grid/).

## Common pitfalls

- **Distances in degrees.** [[vector.buffer]] measures in the features' own units. A GeoJSON in longitude and
  latitude buffered by `500` makes circles of 500 *degrees*. Reproject the features to a projected CRS first, or
  give a distance in degrees knowing that it is not the same in every direction. [[vector.distance]] measures in
  the *map's* units, so a map in metres gives metres whatever the features' system.
- **A missing `.prj`.** A shapefile without its `.prj` has no coordinate system; Metadata shows *none* and the
  features cannot be reprojected onto the map. Keep the files of a shapefile together, or use GeoPackage.
- **Small features vanish.** A polygon smaller than a cell, or a thin one, may cover no cell centre at all and
  disappear from the raster. Use `all_touched = true`, or a finer grid.
- **Rows by order, not by name.** Zonal and extract tables number features 1, 2, 3 in the order of the file.
  Keep an `id` attribute that is a number, so you can join the results back to the features (with
  `vector.join`, or in a spreadsheet).
- **Overlapping polygons.** With [[vector.rasterize]], the last feature wins where polygons overlap; with
  [[vector.zonal]], every polygon gets all its cells, so overlapping areas are counted in both.
- **Different areas.** The area of a polygon (1,095 ha for South, computed from its outline) and of its cells on
  the grid (1,052 ha) differ: the cells follow the outline only to the nearest cell. Report which one you use.

## Going further

- [The vector calculator](../../../calculators/vector/): attributes computed from the others and from the
  geometry (areas, lengths, centroids) with [[vector.calc]], and features kept by a condition with [[vector.filter]].
- [Coordinate systems and reprojection](../coordinate-systems/): why the CRS of every layer matters.
- [One grid for all your maps](../one-grid/): the reference grid and the Prepare pane.
- The operators: [[vector.read]], [[vector.rasterize]], [[vector.zonal]], [[vector.extract]],
  [[vector.distance]], [[vector.buffer]], [[vector.clip]], [[vector.dissolve]], [[vector.join]],
  [[vector.exact_sum]], [[vector.write]].

## References

- GDAL/OGR contributors. *GDAL/OGR Geospatial Data Abstraction software Library*. Open Source Geospatial Foundation. https://doi.org/10.5281/zenodo.5884351
- Herring, J. R. (Ed.). (2011). *OpenGIS Implementation Standard for Geographic information – Simple feature access – Part 1: Common architecture* (OGC 06-103r4). Open Geospatial Consortium.
- Pebesma, E. (2018). Simple features for R: standardized support for spatial vector data. *The R Journal*, 10(1), 439–446. https://doi.org/10.32614/RJ-2018-009
