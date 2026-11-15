---
title: Surface metrics
description: "Texture of continuous rasters (ISO 25178): roughness, shape, trend surfaces."
order: 80
---

Measures of the texture of a continuous raster (NDVI, elevation, canopy height,
any gradient) that need no classification into patches: how much it varies,
how its heights are distributed, how its trend can be removed. They are the
areal surface texture parameters of ISO 25178, brought to landscape ecology as
*gradient surface metrics*, and live in the `tf.surface` module.

The metrics follow the R package geodiv, bit for bit wherever geodiv is
right; where it errs (Sq and the moments built on it, the seven-point slopes,
the valleys, the summit curvature, the areas of the bearing area curve, the
centre of its Fourier images, the fractal dimension's prisms, the choice of
trend surface), Terraflux computes what the method intends, and each
function's notes say what differs.

<!-- no-run -->
```python
import terraflux as tf

ndvi = tf.surface.example_data("orforest")
z = tf.surface.remove_plane(ndvi)       # residuals from the best trend surface
tf.surface.metrics(z)                   # every metric in one table
```

## All metrics at once

::: terraflux.surface.metrics

::: terraflux.surface.list_metrics

## Amplitude

::: terraflux.surface.sa

::: terraflux.surface.sq

::: terraflux.surface.ssk

::: terraflux.surface.sku

::: terraflux.surface.sv

::: terraflux.surface.sph

::: terraflux.surface.smean

## Slopes and developed area

::: terraflux.surface.sdq

::: terraflux.surface.sdq6

::: terraflux.surface.sdr

::: terraflux.surface.surface_area

::: terraflux.surface.flat_area

## Peaks and valleys

::: terraflux.surface.find_peaks

::: terraflux.surface.find_valleys

::: terraflux.surface.sds

::: terraflux.surface.ssc

::: terraflux.surface.s10z

## Bearing area curve

::: terraflux.surface.bearing_area

::: terraflux.surface.height_ba

::: terraflux.surface.sdc

::: terraflux.surface.sbi

::: terraflux.surface.svi

::: terraflux.surface.sci

::: terraflux.surface.sk

::: terraflux.surface.spk

::: terraflux.surface.svk

## Autocorrelation, orientation and wavelength

::: terraflux.surface.aacf

::: terraflux.surface.scl

::: terraflux.surface.stxr

::: terraflux.surface.std

::: terraflux.surface.srw

::: terraflux.surface.sfd

## Texture images

::: terraflux.surface.window_metrics

## Terrain

Indices of a DEM's shape as the R package spatialEco computes them (over
terra's slope and aspect), corrected where it errs.

::: terraflux.surface.terrain

::: terraflux.surface.tpi

::: terraflux.surface.tri

::: terraflux.surface.vrm

::: terraflux.surface.srr

::: terraflux.surface.dissection

::: terraflux.surface.curvature

::: terraflux.surface.spherical_sd

::: terraflux.surface.hsp

::: terraflux.surface.trasp

::: terraflux.surface.hli

## Landforms (geomorphons)

The 10 landforms and 498 ternary patterns of Jasiewicz & Stepinski (2013),
from the lines of sight of every cell: GRASS GIS's `r.geomorphon`, bit for bit
(four of its errors corrected).

::: terraflux.surface.geomorphons

::: terraflux.surface.Geomorphons

`tf.surface.GEOMORPHON_FORMS` maps the codes to the names: 1 flat, 2 peak,
3 ridge, 4 shoulder, 5 spur, 6 slope, 7 hollow, 8 footslope, 9 valley, 10 pit.

## Hilltops (APP de topo de morro)

::: terraflux.surface.hilltop

::: terraflux.surface.Hilltop

## Viewshed

::: terraflux.surface.viewshed

## Spatial interaction

::: terraflux.surface.gravity

## Trend surfaces

::: terraflux.surface.fit_plane

::: terraflux.surface.best_fit_plane

::: terraflux.surface.remove_plane

## Example data

::: terraflux.surface.example_data
