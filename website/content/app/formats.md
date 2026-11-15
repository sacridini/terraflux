---
title: File formats
short: File formats
description: Every file format Terraflux reads and writes, by kind of data (rasters, vectors, tables, models, figures, plots, QGIS projects, animations, scripts and trees), and the addresses on the web it reads.
order: 90
---

A table for each kind of file. **Read** means a node (or **File > Open a raster or vectors...**) can use the file as
data. **Write** means Terraflux makes the file, from a node, from a menu or from the command line. The same formats
are used by the application and by `terraflux run`.

Terraflux reads most raster and vector formats through **GDAL**, the library most GIS software uses. GDAL is bundled
with the packages of Terraflux: you do not install it. **Settings > About & updates** says which GDAL this copy uses and how
many formats it holds (or *no GDAL: GeoTIFF only* when a build has none).

## Rasters

A raster is a grid of cells (a map, or a stack of bands).

| Format | Extension | Read | Write | Notes |
|---|---|---|---|---|
| GeoTIFF | `.tif`, `.tiff` | yes | yes | Read by Terraflux's own reader: strips or tiles, the usual compressions (LZW, Deflate, PackBits, none) and predictors, classic TIFF and **BigTIFF**. A file the own reader cannot read is tried with GDAL. Written by [[raster.write]]: Deflate compression (level 6), in the smallest cell type that holds the values unless you give one (`u8`, `i16`, `f32`...). A file that would pass about 4 GB is written as **BigTIFF** by itself |
| Erdas Imagine | `.img` | yes | with [[raster.convert]] | GDAL's `HFA` |
| GDAL virtual raster | `.vrt` | yes | | a small text file that points to other rasters |
| NetCDF | `.nc` | yes | with [[raster.convert]] | climate and ocean data, with several bands |
| JPEG 2000 | `.jp2` | yes | | |
| HDF | `.hdf`, `.h5` | yes | | |
| ASCII grid | `.asc` | yes | | text, as ArcGIS and many models write it |
| Other GDAL formats | `.grd`, `.rst`, `.sdat`, `.dem`, `.bil`, `.ecw`, `.kea` | yes | KEA with [[raster.convert]] | through GDAL |
| Cloud Optimized GeoTIFF | `.tif` | yes | with [[raster.convert]] (`COG`) | read from a file or an address on the web; only the parts the map needs are read |
| Any other format of GDAL | | yes, when GDAL opens it | with [[raster.convert]] | the extensions in this table are the ones Terraflux recognises in a folder; a file with another name is read if you give its path |

!!! note "Folders and patterns"
    [[raster.read]] takes a file, a folder or a pattern (`forest/*.tif`). A folder or a pattern is read as one stack, a
    layer for each band of each file, named from the file's name. Only the headers are read at first: a raster of any
    size is listed at once and read when a node needs it.

**Save output > As a file...** in the Viewer writes a map as a GeoTIFF. A map written by a model keeps its coordinate system
and a *no data* value. **Process a folder...** in the **File** menu runs a model on every file of a folder.

[[raster.convert]] copies a raster into another GDAL format or cell type (`GTiff`, `COG`, `HFA`, `netCDF`, `KEA`...).

## Vectors

Vector data is points, lines and polygons with attributes (a table of values for each feature).

| Format | Extension | Read | Write | Notes |
|---|---|---|---|---|
| GeoPackage | `.gpkg` | yes | yes | the default when a node writes features; a file can hold several layers (`layer` in [[vector.read]]) |
| Shapefile | `.shp` | yes | yes | with its `.dbf`, `.shx` and `.prj` beside it |
| GeoJSON | `.geojson`, `.json` | yes | yes | in a folder, a `.json` file is not taken for features (it is often something else): give its path |
| FlatGeobuf | `.fgb` | yes | yes | |
| KML | `.kml` | yes | yes | |
| GML | `.gml` | yes | yes | |
| GeoParquet | `.parquet` | yes, when this GDAL has the Parquet driver | | listed with the vector formats; reading depends on the GDAL |

[[vector.write]] chooses the format from the extension (`gpkg`, `shp`, `geojson`, `fgb`, `kml`, `gml`). With another
extension, or none, Terraflux writes a `.gpkg`. The layer's name is the file's name, or the setting `layer`.
Features on the web are read whole, unless a box is given (`extent` of [[vector.read]]: only the features that meet it).

A folder or a pattern of vector files (`roads/*.gpkg`) is read as one layer with a column `file` that says where each feature comes from.

## Tables

| Format | Extension | Read | Write | Notes |
|---|---|---|---|---|
| Comma-separated values | `.csv` | yes | yes | the default. A header names the columns; an empty value or `NA` is no value |
| Tab-separated | `.tsv`, `.txt` | yes | yes | |
| JSON | `.json` | yes | yes | a list of rows, each an object; a missing value is `null` |

[[table.read]] finds the separator (`,`, `;` or tab) and the decimal mark (`.` or `,`) by itself, so a file saved by a
spreadsheet set to a language that writes a comma for decimals is read as it should be. [[table.write]] chooses by the
extension; for a spreadsheet that reads the comma as a decimal mark, write with `separator` `;` and `decimal` `,`.
A table in the Viewer can be saved as CSV or JSON, and copied.

## Models

| Format | Extension | Read | Write | Notes |
|---|---|---|---|---|
| Model, TOML | `.toml` | yes | yes | the model's text format. Small, readable, kept in git |
| Model, JSON | any other (`.json`) | yes | yes | the same model as JSON |

A file is read as TOML when its extension is `.toml`; any other extension is read as JSON. **File > Save**, **Save as...**
and `terraflux convert` write them. Older models are brought up to date when they are read. The Model pane's **Text** view and the
**Document** tab show the same model.

## What a node writes

Besides rasters, vectors and tables, a node's output written to a file (a model's `outputs`, **Save output > As a file...**,
`terraflux run`) is written as:

| Kind of value | Extension |
|---|---|
| a map, a stack, a series of maps | `.tif` (GeoTIFF) |
| a table, a series of tables | `.csv` |
| vector features | `.gpkg` (or the format of the extension, see above) |
| a number, a yes or no, a fitted model, a Markov chain, a demand, a graph of habitat | `.json` |
| a text (a report, GuidosToolbox's text) | `.txt` |
| a phylogenetic tree | `.nwk` (Newick) |
| the agents of an agent-based model | `.csv` |

The extension of the value's kind replaces the one you give (a vector keeps its own when it is one of those above).

## Trees

| Format | Extension | Read | Write | Notes |
|---|---|---|---|---|
| Newick | `.nwk`, or the text itself | yes (with [[bio.tree]]) | yes | a tree written by a model is a Newick file |
| Nexus | | yes (with [[bio.tree]]) | | the Translate table is applied |

## Figures, pictures and animations

| What | Format | Extension | Made by | Notes |
|---|---|---|---|---|
| A figure of a map | PNG | `.png` | **Figure... > Save PNG...** | at 150, 300, 600 or 1200 DPI, the resolution recorded in the file |
| | SVG | `.svg` | **Figure... > Save SVG...** | text as text, to edit in Inkscape or Illustrator |
| | PDF | `.pdf` | **Figure... > Save PDF...** | the font embedded |
| The view as a picture, or a Plot | PNG | `.png` | **Export the view...**, `terraflux plot` | the map as the Viewer draws it, no legend |
| | GeoTIFF | `.tif` | same | four bands of bytes (red, green, blue, alpha) on the map's grid |
| An animation | GIF | `.gif` | **Save as GIF...** | for a talk or a class |
| | MP4 | `.mp4` | **Save as MP4...** | H.264, made by Terraflux itself |
| The model as a document | HTML, PDF | `.html`, `.pdf` | **Export report** | see [The model's document](/tutorials/building-models/the-model-document/) |
| The references | BibTeX, RIS | `.bib`, `.ris` | **Methods and citations...** | |

See [Figures for publication](/app/figures/) for what each one holds.

## QGIS

**File > Export to QGIS...** writes:

| File | Extension | Holds |
|---|---|---|
| Project | `.qgz` (zipped) or `.qgs` (text) | the layers, in the Viewer's colours |
| Maps | `.tif`, and a `.qml` style beside each | one file for each map (not when you choose one GeoPackage) |
| Vector layers | `.gpkg` | one file for each layer, or all of them in one |
| One GeoPackage | `.gpkg` | every vector layer as a table and every map as a raster table (16-bit or 32-bit float tiles) |

See [Export to QGIS](/app/qgis/).

## Python scripts

**File > Python script...** writes the model as a script of [terraflux-py](https://github.com/sacridini/terraflux-py)
(`.py`; `terraflux python model.toml` does the same on the command line). It uses `import terraflux as tf`.

## Data on the web

A path in [[raster.read]] or [[vector.read]], or a layer added with **A URL...** (in the menu that adds a layer and
in the Viewer's header), can be an address. Terraflux reads it through GDAL's network file systems, so only the parts that
are needed are fetched. Nothing is downloaded when no node or layer reads it.

| Address | Reads |
|---|---|
| `https://...`, `http://...`, `ftp://...` | a file on a web server (as `/vsicurl/...`); a Cloud Optimized GeoTIFF is read by windows |
| `s3://bucket/...` | Amazon S3 (as `/vsis3/...`) |
| `gs://bucket/...` | Google Cloud Storage (as `/vsigs/...`) |
| `/vsiaz/container/...`, `/vsiadls/...` | Azure Blob Storage and Data Lake, in GDAL's own form |
| `/vsicurl/...`, `/vsicurl_streaming/...`, `/vsis3/...`, `/vsigs/...` | GDAL's paths, used as they are |
| `WMTS:https://host/WMTSCapabilities.xml,layer=name` | map tiles, read as a raster |
| `WFS:https://host/wfs` | features from a Web Feature Service, read as a vector |
| `OAPIF:https://host/collections` | features from an OGC API, read as a vector |

Private buckets need keys: **Settings > Cloud storage** takes the keys of S3 (key id, secret, session token, region and endpoint), of
Google Cloud Storage (key id and secret, or a credentials file) and of Azure (account and key), kept in your computer's
keyring and never in the settings file. For a public bucket, tick *public buckets, requests not signed*, for `s3://` and for `gs://`.

An address is never made relative to the model's folder and is not checked to exist before the model runs. See
[Settings](/app/settings/).

## Going further

- [Figures for publication](/app/figures/) and [Export to QGIS](/app/qgis/).
- [Your first raster](/tutorials/getting-started/first-raster/), [Vector data](/tutorials/getting-started/vector-data/) and [Coordinate systems](/tutorials/getting-started/coordinate-systems/).
- [Settings](/app/settings/): the cloud keys and where Terraflux keeps its files.
