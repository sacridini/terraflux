---
title: The viewer
short: The viewer
description: "Layers, basemaps, Compare, the histogram, RGB, tables and selection, editing vertices, remote data, Go to, the coordinate grid and Plots in Terraflux's Viewer."
order: 30
---

The **Viewer** shows what the node selected gives: a map with its legend, the layers of a stack one at a time, the
maps of a loop's steps, vector features, a table, or a single value. It is refreshed at every run. If the node has
not run yet, it says *Not run yet: ctrl+enter runs it, and what it needs*.

This page lists its controls. [A tour of Terraflux](/tutorials/getting-started/tour/#the-viewer) introduces them and
[Plots and figures](/tutorials/building-models/plots-and-tables/) teaches them with a model.

## What is in the Viewer

- **The header**, over the map: the node's name and operator, a chooser for the node's outputs when it has several,
  a chooser for the step of a loop, and the buttons below.
- **Layers**, a panel at the map's left ([Layers](#layers)).
- **The map**, in the middle.
- **A side panel** at the map's right: the size in cells, **Figure...** and **Export the view...**, the animation
  controls for a map of several layers, the legend or the histogram, the area of the classes or statistics, a
  profile, **This cell in every map**, **Basemap**, **On the map**, **Compare** and **Draw on the map**.

### The header's buttons

| Button | Does |
|---|---|
| **Pop out** / **Back into the window** | Takes the Viewer to a window of its own, for another screen, and back. |
| **Metadata** | The coordinate system, cells, extent and bands of the map or features. |
| **Layers** | Hides or shows the panel of layers. |
| **Measure** (++m++) | Lengths, area and perimeter of a line clicked on the map ([Measure](#measure-and-profile)). |
| **A URL...** | Adds a layer from the web ([Remote data](#remote-data)). |
| **Go to...** (++ctrl+g++) | Finds a coordinate on the map ([Go to](#go-to)). |
| **›** and **‹** | The view after, and the view before (++alt+right++, ++alt+left++, the mouse's forward and back buttons). |
| **Places** | Views of the map kept by name in the model, to go back to. ++ctrl+1++ to ++ctrl+9++ keep the view in a slot, ++1++ to ++9++ go to it. |
| **Plots** | The model's Plots ([Plots](#plots)). |
| **Save output** | **As a file...**, **As a file, and a write node in the model...** (a [[raster.write]] node added, so every run writes it again) or **In a QGIS project...** ([Export to QGIS](/app/qgis/)). |
| **Trace this cell** | Shown when a cell is clicked: the chain of values that made it ([Trace](/app/canvas/#trace-why-a-cell-has-its-value)). |

At the right of the header, a line says how the node did: *running*, *run*, *nothing changed: from the cache*,
*from an earlier run* or *failed*.

## The map

| Gesture | Does |
|---|---|
| Wheel | Zooms around the pointer. |
| Drag | Pans. |
| Double click | Shows the whole map again. |
| Click | Picks a cell (outlined). The corner reads out the coordinates, the **row** and **column**, and the value under the pointer. |
| Right click on a cell | **Trace this cell**, **Ask the assistant why**, **Explain with an MCP assistant**. |
| ++b++ | Satellite imagery under the map, or none. |
| ++space++ | Hides the map, or shows it again. |
| ++m++ | Measure. |
| ++ctrl+g++ | Go to a coordinate. |

A map of any size is drawn from tiles made only for the part in view, so a map of billions of cells is shown and
moved as smoothly as a small one. Rasters on disk are read where and as finely as the screen needs.

A map of several layers (a stack, the steps of a loop, the bands of a raster) has **<**, **Play** and **>** and a
slider in the side panel, a **speed** in frames a second, **Save as GIF...** and **Save as MP4...**. The arrows
or the slider stop it. The steps of a series share one set of colours; the layers of a stack or a raster each keep
their own. Clicking a cell draws a chart of its value in every layer, and, for classes, the area of each class
along the steps. A click on the chart shows that layer.

## Layers

**Layers** keep outputs on the map whatever node you click. The panel lists them from the top; the map shown is drawn
at its place in the list, or over all of them if it is not one of them. A click on a layer's name shows that map
with its legend and settings.

- **Keep a map.** The pin (📌) beside the map's name keeps the map shown as a layer. A double click on a node does
  too.
- **Add a layer** (the **+** button) lists *the map shown, kept*, the maps and features **of the run**, **A file
  from disk...** (a node of the model reads it) and **A URL...**. Files dropped on the list are added too.
- **Remove the layer**, **Up** and **Down** move the layers selected (or drag a row).
- Each layer is **shown or hidden**, made more or less **opaque**, and, for vector features, **styled**.
- **Groups.** A group, as in a GIS, shows, hides and fades its layers together. **Group** and **Take out of its
  group** are in a layer's menu; a group has **Rename...** and **Ungroup**.

A layer's right-click menu holds:

| Item | Does |
|---|---|
| **Zoom to layer** | Fits the view to the layer. |
| **Its legend and settings at the right** | Shows that map in the side panel. |
| **Attribute table** | For features: the table of their attributes under the map ([Tables and selection](#tables-and-selection)). |
| **Edit the features** | For features read from a file: [Editing vertices](#editing-vertices). |
| **Colours** | A palette for the layer; colour-blind safe ones ([Colour vision](/app/colour-vision/)). |
| **Same colours for all similar layers** | Gives its palette and range to the other layers of the same kind, so a colour means the same value. |
| **Show its node** | Brings the canvas forward, opens the container the node is in, selects and frames it. |
| **Rename...**, **Group**, **Take out of its group** | The layer's name in the list, and groups. |
| **Show only this layer**, **Show every layer** | Shows or hides the others at once. |
| **To the top**, **Up**, **Down**, **To the bottom** | Its place in the list. |
| **Remove layer** | It is no longer a layer. The node stays. |

Under a layer's row are its settings:

- **Opacity** and **Blend**: *normal*, *multiply* (the colours multiplied: white leaves what is under it, so it
  darkens) or *screen* (black leaves what is under it, so it lightens).
- **Shade with this elevation**: the relief of a map of values, as a hillshade, darkens its colours. Settings: the sun's
  **azimuth** and **sun altitude**, the vertical **exaggeration** and the **strength** of the shade.
- **RGB**, for a layer of three maps or more (a stack, a raster of bands): shows three of them as one colour
  image. A box of **composites** offers the usual ones of a satellite, by the bands' names; **R**, **G** and **B**
  each choose a band and the range drawn black to full colour (**auto**: the 2nd to the 98th percentile of the
  band); **gamma** above 1 brightens the middle tones.

A map in another coordinate system than the map shown is reprojected as it is drawn, and its value under the pointer
read back through the same transformation.

## Basemaps

**Basemap**, in the side panel, puts a map of the world under your data. The tiles are downloaded, carried to your
map's coordinate system and kept in a cache. The map needs a coordinate system, and an Internet connection the first
time an area is shown.

| Choice | Service |
|---|---|
| **None** | No basemap. This is the default. |
| **Satellite (Esri)** | Esri World Imagery. |
| **Topographic (Esri)** | Esri World Topographic Map. |
| **OpenStreetMap** | OpenStreetMap's standard map. |
| **OpenTopoMap** | OpenTopoMap. |
| **Light (Carto)** | CARTO's light map. |
| **Custom...** | Tiles of your own, by an address with `{z}`, `{x}` and `{y}` in it (the XYZ tiles of OpenStreetMap, QGIS...), with an attribution and the deepest zoom. Up to five are kept, the last used first. |

With a basemap, **show the data** and an **opacity** slider let you see through your map to the imagery. The
attribution the service asks for is written on the map. ++b++ puts the satellite under the map and takes it away.
The tiles are kept in `web-tiles` under `%LOCALAPPDATA%\Terraflux` on Windows and `~/Library/Caches/Terraflux` on
macOS (on Linux, in the system's temporary folder), so they are not downloaded twice. Under **On the map**, **Scale
bar** is on at first.

## Compare

**Compare**, in the side panel, puts another map beside the one shown: another output of the run, or another
layer of the same stack or series. The list offers *nothing*, *another layer of this one* (with a slider) and each
map of the run; a stack or a series has a **band** choice.

| How | Shows |
|---|---|
| **Curtain** | The other map right of a line you drag across the map. |
| **Side by side** | Two views that zoom and pan together, the pointer marked in both. More views can be added, up to four. Each view draws the Viewer's layers or layers of its own. |
| **Difference** | For maps of values, the other map minus this one, in a ramp centred on 0, with the mean difference and the share of cells higher and lower. For maps of classes, the transitions ("forest -> pasture", the most common first, cells that kept their class in grey), with how many changed, and the matrix of every transition in a window. |

Both maps are drawn in the same colours, so that the same value is the same colour. **Keep as a layer** (📌) turns
a difference into a layer of its own, drawn over the map and kept in a Plot. The maps may be in different
coordinate systems: the other map is carried to the first one's cells. See
[Comparing two maps](/tutorials/building-models/plots-and-tables/#comparing-two-maps).

## The legend, the histogram and the numbers

- **Of classes**, the legend lists each class with its colour, its name and its area or number of cells in the part
  chosen. **Edit legend** changes the names and colours, and keeps them in the model (a
  [[raster.legend]] node).
- **Of values**, a **Histogram** draws the values in bars coloured as the map is, with the minimum, maximum and mean.
  The range of the colours is set on it. A large map is drawn from a sample of about four million cells.
- **Area of the classes**, or **Statistics** for values (count, mean, extremes, sum and spread), can be of the
  **map**, the **view**, the **polygon** of a line measured, or the polygons **selected** in a table of attributes.
- **Colours** chooses a palette. By default the colours fit the data: a colour for each class, a ramp for values.
  A **logarithmic** box in its menu puts the colours of values on a scale of logarithms.

## Tables and selection

A node that gives a **table** is shown as one: sortable by a column, with a *filter* box that takes words or a
condition such as `area > 1`, and drawn as **bars**, **lines** or **points** of one or more columns. See
[Tables in the Viewer](/tutorials/building-models/plots-and-tables/#tables-in-the-viewer).

Vector features have an **Attribute table** (in the layer's menu) under the map, one row per feature.

- A click on a row selects its feature, outlined on the map and zoomed to. A click on a feature on the map selects
  its row.
- ++ctrl+click++ toggles a row. ++shift+click++ selects the range from the last row clicked.
- Drag on the map with ++shift++ to select the features a rectangle meets; with ++shift+alt++, a lasso. ++ctrl++
  adds them to the selection. ++esc++ lets them go.
- The table's header has **Select all shown**, **Zoom to selection**, **Clear selection**, **Filter the selection**
  (a [[vector.filter]] node that keeps them), **Copy**, **Copy selected**, **Save as CSV** and **Save selected**.

## Editing vertices

**Edit the features**, in the menu of a layer of features read from a file, opens an editor in the side panel:

- drag a vertex to move it; the dot in the middle of an edge adds a vertex; a right click removes one;
- **Add a feature** draws a new one; the **×** beside a feature's row deletes it;
- the attributes are typed in a table (a number stays a number, a text a text);
- **Undo** and **Redo** (++ctrl+z++);
- **Save the edited features** writes them as a **new** GeoPackage, beside the file or where **Browse...** says, and
  a [[vector.read]] node reads it. The file read is never changed. **Stop** leaves the editor; edits not saved
  are lost.

To make features from nothing, **Draw on the map** (in the side panel) draws **polygons**, **rectangles**,
**points** or **lines**, with attributes. **Add to the model** writes them as a GeoPackage in a `drawn` folder beside
the model and adds a node that reads it; **Add, and data for it...** also opens the [data
catalog](/app/canvas/#the-data-catalog) cut to the area drawn.

## Remote data

**A URL...** (in the header and in **Add a layer**) adds a layer from an address. The window checks the address by
opening it and says what it found (size, bands, coordinate system, or why not), then **Add as a layer** adds a node
of the model that reads it. Terraflux reads a remote raster by ranges of bytes, only the window the map shows and
the overview that fits the screen, never the whole file. These are accepted:

| Address | Reads |
|---|---|
| `https://...` | A cloud-optimised GeoTIFF, or any raster GDAL opens over the web; GeoJSON or FlatGeobuf features. |
| `s3://...` | A raster or features in Amazon S3 (or a compatible storage). |
| `gs://...` | Google Cloud Storage. |
| `/vsicurl/...` | A raster by GDAL's web path. |
| `WMTS:https://host/WMTSCapabilities.xml,layer=name` | A WMTS service. |
| `WFS:https://...` or `OAPIF:https://...` | A web feature service, read for the view. |

The keys of S3, Google Cloud Storage and Azure are in **Cloud storage** of the [settings](/app/settings/#cloud-storage);
secrets go to the system's store of credentials, never to the model. The command line and the workers use the
environment's variables instead. See also [Maps on the web](/tutorials/building-models/plots-and-tables/#maps-on-the-web).

## Go to

**Go to...** (++ctrl+g++ over the map) opens a box that finds a place and marks it. It reads:

- `lat, lon`, with a comma, as Google Maps copies it;
- `lon lat`, with a space;
- degrees, minutes and seconds, such as `12°20'30"S 122°30'E`;
- the letters N, S, E and W to tell which is which;
- a zoom after an `@`: `-15.8, -47.9 @ 1:50000` or `@ 5 km`.

**degrees** (WGS 84) and **map units** choose how the numbers are read; a list sets the zoom after going (*keep
the zoom*, or a scale). ++esc++ lets the mark go. See [Finding your way on the
map](/tutorials/building-models/plots-and-tables/#finding-your-way-on-the-map).

## The coordinate grid

Under **On the map**, **Coordinate grid** draws lines at round values of the map's own coordinates, their values at
the edges. It is off at first. **Scale bar**, a round length in metres or kilometres (in cells, if the map has no
coordinate system), is on at first. Both are kept in the settings. The [figure](/app/figures/) has its own scale
bar, north arrow and coordinates.

## Measure and profile

**Measure** (++m++) clicks points on the map. It reports the length of each segment and the total and, from
three points, the area and perimeter of the polygon it closes. ++backspace++ takes the last point away, ++esc++
ends it. On a map in degrees, distances are geodesic on the WGS 84 ellipsoid; with a projected map they follow its
units. For a line, a **profile** under the legend charts the map's values along it; for classes, the classes crossed
and the length in each.

## Plots

A **Plot** is what the Viewer shows (its layers, the map shown, the comparison, a table's chart) kept with the
model. The Document, the report and the QGIS project show it as a figure. The **Plots** menu in the header:

| Item | Does |
|---|---|
| A list of the model's Plots | A click opens one. The header then says *Plot: its name*. |
| **title** | Names the Plot open. |
| **Update the plot** | Keeps in it what the Viewer shows now. |
| **Keep this view in the plot** / **Update the saved view** | Keeps the frame of the map now in view: the Plot opens on it, and its figure, export and Document picture show only that part. |
| **Go to the saved view** / **Forget the saved view** | Brings the map back to that frame, or shows the whole map again. |
| **Remove the plot** | The Plot goes; the layers stay in the Viewer. |
| **Save as a new plot** | Keeps what the Viewer shows as a new Plot. |

A `view.plot` node in the model makes a Plot by itself. See [Saving a Plot](/tutorials/building-models/plots-and-tables/#saving-a-plot).

## Figures, exports and accessibility

- **Figure...** makes a figure of the map for a paper, in PNG, SVG or PDF: [Figures for publication](/app/figures/).
- **Export the view...** writes the map as the Viewer shows it, with its layers, vectors and labels, as a PNG or a
  GeoTIFF placed on the map.
- **File > Export to QGIS...** writes the run's maps and vectors as a QGIS project: [Export to QGIS](/app/qgis/).
- **View > Colour vision** draws maps and legends as a colour-blind reader sees them, and a legend warns when two
  classes look alike: [Colour vision and accessibility](/app/colour-vision/).
- The file formats Terraflux reads and writes: [File formats](/app/formats/).
