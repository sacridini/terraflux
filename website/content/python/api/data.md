---
title: Data & configuration
description: "Read, write and reclassify rasters, collections, colour tables, threads."
order: 10
---

Reading, preparing and writing rasters, vectors and tables, saving and
plotting results, colour tables, collections of files and the thread settings shared by every
tool. The page [Loading and saving data](/python/getting-started/data/) shows
them together.

!!! example "Every example runs"
    The examples on these pages use the maps that ship with Terraflux
    (`tf.example_data`) and are run as tests, outputs included.

## Rasters

::: terraflux.load_raster

::: terraflux.write_raster

::: terraflux.match_grid

::: terraflux.reproject

::: terraflux.clip

::: terraflux.reclassify

## Vectors

::: terraflux.load_vector

::: terraflux.write_vector

::: terraflux.rasterize

## Values by zone and at points

::: terraflux.zonal_stats

::: terraflux.zonal_map

::: terraflux.tabulate_area

::: terraflux.extract

## Tables

::: terraflux.load_table

::: terraflux.write_table

## Saving results

::: terraflux.save

::: terraflux.load_result

Every result object (of `fos`, `restoration_status`, `zonation`...) also has
a `save(folder)` method that calls `save`.

## Plotting

::: terraflux.plot

::: terraflux.Plot
    options:
      members: [maps, charts, figure, show, save, to_html]

The page [Plotting](/python/getting-started/plotting/) shows the plots of every
kind of data, and how to choose their colours.

## Example data

::: terraflux.example_data

## Example data

`tf.EXAMPLE_DATA` describes the habitat and land-cover rasters; the
examples of the other modules are listed under `example_data` above:

| Name | Content |
|---|---|
| `"input"` | habitat map, 1000 × 1000 cells, 0 missing, 1 background, 2 forest (EPSG:4326) |
| `"clc"` | CORINE land cover excerpt, 2000 × 2000 cells of 25 m (EPSG:3035), classes 1–44 |
| `"gscinput"` | percentage map, 1990 × 1289 cells of 2430 m (EPSG:5070), 0–100, 255 missing |

## Collections of rasters

::: terraflux.RasterCollection
    options:
      members: [load, to_stack]

::: terraflux.RasterInfo
    options:
      members: false

## Checking inputs

::: terraflux.validate_mspa_input

::: terraflux.validate_fad_input

::: terraflux.InvalidInputError
    options:
      members: false

## Colour tables

Every class map has a colour table: a `{value: (red, green, blue, alpha)}`
dictionary with 256 entries, embedded by `write_raster`. Each is also
available as a constant (`tf.MSPA_CLASSES` names the MSPA codes;
`tf.FAD_COLORMAP`, `tf.LM_COLORMAP`... are the tables themselves).

::: terraflux.mspa_colormap

::: terraflux.fad_colormap

::: terraflux.fad5_colormap

::: terraflux.lm_colormap

::: terraflux.accounting_colormap

::: terraflux.change_colormap

::: terraflux.foschange_colormap

::: terraflux.dist_colormap

::: terraflux.entropy_colormap

::: terraflux.contortion_colormap

::: terraflux.resistance_colormap

::: terraflux.gtn_components_colormap

::: terraflux.gtn_importance_colormap

## Threads

::: terraflux.default_threads

::: terraflux.set_default_threads

::: terraflux.max_threads

::: terraflux.doctor
