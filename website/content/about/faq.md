---
title: Frequently asked questions
short: FAQ
description: Short answers to the questions people ask before and after installing Terraflux.
order: 20
---

## Before installing

Do I need to know how to program?
:   No. Everything can be done in the application: open a template or your maps, add operators from the library,
    connect them, and look at the results. The command line and the model files are there when you want to
    repeat an analysis exactly or run many of them.

Do I need GDAL, Python or R?
:   No. The installers bring GDAL and every library Terraflux needs. Python and R are only needed if you want to
    use [landschaft](https://github.com/sacridini/landschaft) or compare with your own scripts.

Which computer do I need?
:   Any recent Windows, macOS (Apple silicon) or Linux computer. Most analyses of a study area of a few million
    cells run in seconds on a laptop. Maps larger than memory run in tiles; very large batches can go to a
    cluster.

Is it really free?
:   Yes, for any purpose, under the GPL-3.0-or-later license. The source code is public.

## Results

Will I get the same numbers as FRAGSTATS, GuidosToolbox, Zonation or Dinamica EGO?
:   The methods are re-implemented from their publications and code, and Terraflux's tests compare each one with
    the reference implementation; most agree to the last bit. Where a result can only be statistically
    equivalent (the tiled engine's simulations, for example), the documentation says so.

How do I report what I did in a paper?
:   Give the version of Terraflux, cite the papers of the methods (see [Cite Terraflux](/about/citing/)), report
    the settings that matter (each tutorial says which), and publish the model file (`.toml`) as supplementary
    material: anyone can run it again.

Why do two runs of my model give different maps?
:   The model is stochastic: it draws random numbers. The seed fixes them; the same seed gives the same map. Run
    several seeds and report the spread (see [Batches](/cli/batches/)).

## Data

Which formats can I open?
:   Every raster format GDAL reads (GeoTIFF, COG, NetCDF, HDF, JPEG 2000, ERDAS IMG...) and every vector format
    (GeoPackage, Shapefile, GeoJSON, FlatGeobuf, KML...). Results are written as GeoTIFF, GeoPackage and CSV.

My maps do not line up. What do I do?
:   They have different coordinate systems, cell sizes or cell edges. The Prepare pane shows how each differs from
    the reference grid and puts it on the grid in one click; the tutorials
    [Coordinate systems](/tutorials/getting-started/coordinate-systems/) and
    [One grid for all your maps](/tutorials/getting-started/one-grid/) explain why and how.

Can I use maps in latitude and longitude?
:   You can open and show them, but areas, distances and most landscape metrics need a projected coordinate
    system in metres. Reproject first (`raster.reproject`), to an equal-area projection if areas matter.

## Getting help

Something does not work. Where do I ask?
:   Open an issue in the [repository](https://github.com/sacridini/terraflux/issues) with what you did, what you
    expected, what happened, and the model file if you can share it.
