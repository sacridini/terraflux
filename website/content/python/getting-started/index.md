---
title: Getting started
description: Install the Python library, run a first analysis, learn the few ideas every tool shares and set it up.
order: 10
---

New to the Python library? These pages take you from nothing installed to your first analysis, and explain the few
ideas that every tool shares.

- **[Installation](/python/getting-started/installation/)**: set up Python, a compiler if you need one, and
  Terraflux on Windows, macOS or Linux, and check that everything works.
- **[Quickstart](/python/getting-started/quickstart/)**: ten minutes, one example map: load it, look at it, analyse
  it and save the result as a GeoTIFF you can open in QGIS.
- **[Configuration](/python/getting-started/configuration/)**: threads, memory limits, output formats and colours:
  the settings that apply to every tool.
- **[Core concepts](/python/getting-started/concepts/)**: rasters and georeferencing, how habitat maps are coded,
  NoData, cell size and units, and how results are returned.
- **[Loading and saving data](/python/getting-started/data/)**: rasters, vectors and tables in and out, values by
  zone and at points.
- **[Plotting](/python/getting-started/plotting/)**: `tf.plot` draws any map, vector layer, table or result on an
  interactive map over satellite imagery, or as a chart, in one call.

## Suggested path

1. [Install Terraflux](/python/getting-started/installation/) and run the check at the end of the page.
2. Follow the [quickstart](/python/getting-started/quickstart/) with the example map that ships with the package.
3. Read [core concepts](/python/getting-started/concepts/) before you use your own data: most surprises come from
   how the input map is coded.
4. Pick the tutorial of the topic you need; each one starts from the basics and ends with the advanced options.
   They are listed in the [tutorials](/python/tutorials/).

!!! app "In the application"
    The application has its own [quickstart](/quickstart/) and [installers](/quickstart/install/); it needs no
    Python. Use the library when you prefer to write code, or to call the same analyses from a notebook.
