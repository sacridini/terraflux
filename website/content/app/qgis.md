---
title: Export to QGIS
short: Export to QGIS
description: Write the maps and vector layers of a run as a QGIS project, in the colours of the Viewer, as files in a folder or in one GeoPackage, and what does not carry over.
order: 50
---

**File > Export to QGIS...** writes the maps and the vector layers of a run to a folder, together with a **QGIS
project** that opens them all in their colours. You open the project in QGIS (a free GIS) to make a map there, to
add other data, or to share the result with someone who does not use Terraflux. The same window opens from the
Viewer's header: **Save output > In a QGIS project...**, with the output shown already ticked.

Run the model first. The window lists only what a run has produced: with nothing run it says *Nothing to export yet*.

## The window

The window **Export to QGIS** has a short note at the top and then these parts.

**The list of layers.** One check box for each map and vector layer the run made, in the model's order. At first these
are ticked:

- the outputs the model names (its `outputs`),
- the map shown in the Viewer,
- the layers drawn with it in the Viewer (maps and vector layers that are shown; a difference is not, because it has no file).

The buttons **all** and **none** tick or clear the whole list. A layer is named as the model's output that writes it,
else as the Viewer names it.

**Options of a vector layer.** Under a vector layer that is ticked:

- **labels**: an attribute to write on each feature in QGIS (*none* at first). A label chosen in the style of the layer in the Viewer is used as it is.
- **marker**, for points only: *circle*, *square*, *triangle*, *diamond*, *star*, *cross*, *pentagon* or *hexagon*.
- **as rules**: write the style as QGIS's *rule-based* renderer, a rule for each class or range and one for the features that
  have none, so that you can edit them rule by rule in QGIS.

**Project.** **.qgz** (the default) is zipped, the way QGIS saves its own projects. **.qgs** is the same project as plain
text (XML), useful to read, compare or keep in git.

**Layers.** **files in the folder** (the default) or **one GeoPackage** (see below).

**Export N layers...** asks for a folder and writes everything there, in the background. Then the window says *written:*
with the path of the project, and two buttons: **Open in QGIS** (opens the project with the program your computer uses for
`.qgz` and `.qgs` files) and **folder** (opens the folder). If something fails it says *not written:* and why.

The project is named after the model (its name, else its file's name, else `terraflux`). Its file names are made from the layers' names:
letters, digits, `-` and `_` are kept and other characters become `_`; two layers with the same name get `_2`, `_3`...

## What is written

### Layers as files in the folder

| Layer | File | Notes |
|---|---|---|
| A map | `<name>.tif` | a GeoTIFF, in the smallest cell type that holds its values |
| A map | `<name>.qml` | its style, beside it: QGIS uses it when you add the GeoTIFF to another project by hand |
| A vector layer | `<name>.gpkg` | a GeoPackage with one layer |
| The project | `<project>.qgz` or `<project>.qgs` | opens every layer, the vector layers on top |

A raster read from **one file** (a [[raster.read]] of a single GeoTIFF) is **not copied**: the project points to the file
where it is. Its colours are in the project.

A stack, a series or a raster with several bands goes in as **one layer**: the layer shown in the Viewer. When it is
not the one shown, a series goes in as its last step (where a simulation got to) and a raster as the band chosen.
To have every layer of a stack in a GeoTIFF, write it with a [[raster.write]] node.

### Layers in one GeoPackage

Choose **one GeoPackage** and every layer goes into a single file, `<project>.gpkg`:

- every vector layer is a **table** of the file;
- every map is a **raster table** (a *gridded coverage*), with its missing cells and its coordinate system kept;
- the project points into the file (`GPKG:./<project>.gpkg:<table>` for a map, `./<project>.gpkg|layername=<table>` for a vector layer), with the same colours and legends.

A GeoPackage raster holds 16-bit or 32-bit cells, so each map is written in the type that fits its values:

| The map's values | Tile type |
|---|---|
| whole numbers from 0 to 65,534 | 16-bit, unsigned |
| whole numbers from -32,767 to 32,766 | 16-bit, signed |
| anything else (decimals, or larger whole numbers) | 32-bit float (Float32) |

All the layers of the export are **copied** into the file, including a raster read from a file. Table names have no hyphens
(`-` becomes `_`) and avoid the names GeoPackage keeps for itself; names that differ only in case get `_2`... The
option needs GDAL with its GeoPackage driver. When that is missing, the choice is off and its tooltip tells why.

One file is easy to move and send. Files in the folder are easier to open one by one in other programs.

## Styles

The layers open in QGIS in the colours they have in Terraflux.

**Maps**

- A map of **classes** is a *paletted* raster: each class with its colour and its name (the names of the legend of
  the map, or as the Viewer names them).
- A map of **values** is a *singleband pseudocolor* raster: the Viewer's ramp from its least to its most value,
  in 11 steps that QGIS interpolates (on a scale of logarithms when the Viewer's is). The colours keep the range you
  set by hand in the Viewer.
- A colour composite (a map of three bands as colour) opens with the file's first three bands as red, green and blue.

**Vector layers**

- one colour, with its fill and its line width, as the Viewer draws it;
- or a colour for each class of an attribute (QGIS's *categorized* renderer), or ranges of its numbers (*graduated*);
- points with the marker and the size chosen, the size growing with an attribute when the Viewer sizes them so;
- **labels** by an attribute, as above;
- or **rules**, when *as rules* is ticked.

## Which QGIS

The project and the styles are written as **QGIS 3.34** (*Prizren*), the long-term release. They were checked in QGIS 3.34 with
PyQGIS (QGIS's Python interface): every layer opens as valid, the paletted classes have their colours and names, and
the labels and the rules are read. QGIS reads the files of older versions, so a newer QGIS should open the project
too; that was not tried here, nor was a QGIS older than 3.34.

## What does not carry over

- **The Viewer's way of drawing layers together.** Each layer comes with its own style. A layer's opacity and its
  blend mode are not written: the project holds every layer shown, at full opacity. The vector layers are put on top.
- **Basemaps** (the web imagery under a map) are not in the project. Add one in QGIS.
- **Colour vision.** The colours are written as they are, not as a reader with a colour vision deficiency sees them,
  whatever **View > Colour vision** says.
- **Differences and comparisons.** A difference map, a curtain and the views side by side are Viewer drawings with no layer
  of their own. Export the two maps and make the difference in QGIS, or write it with a node in the model.
- **Stacks and series**: one layer at a time (see above).
- **Plots.** A Plot is not a QGIS object; **Export the view...** writes its picture as a GeoTIFF (see
  [Figures for publication](/app/figures/#export-the-view)).
- **Edits you make in QGIS** are not read back by Terraflux.

## Going further

- [Opening the results in QGIS](/tutorials/getting-started/reclassify-calculate-save/#opening-the-results-in-qgis): a worked example.
- [File formats](/app/formats/): what each layer is written as.
- [Figures for publication](/app/figures/): a picture of the map with its legend and scale bar.
