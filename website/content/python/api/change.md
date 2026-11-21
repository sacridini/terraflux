---
title: Change detection
description: "Habitat, fragmentation and landscape-mosaic change between dates."
order: 130
---

Differences between two dates of a habitat map: pixel transitions,
structural change without edge noise, and the change of fragmentation and of
landscape mosaics. Tutorial:
[change detection](/tutorials/change-detection/).

All functions take the two dates on the same grid. The change of
connectivity indices over time is in
[`tf.conn.eca_change`](/python/api/networks/#terraflux.conn.eca_change).

## Habitat change

::: terraflux.simple_change

::: terraflux.SimpleChangeResult

::: terraflux.morphological_change

::: terraflux.MorphologicalChangeResult

## Fragmentation change

::: terraflux.fos_change

::: terraflux.FOSChangeResult

::: terraflux.fad_multiscale_change

::: terraflux.FADMultiscaleChangeResult

::: terraflux.FADChangeScale

## Landscape mosaic change

::: terraflux.landscape_mosaic_change

::: terraflux.LMHeatmapChange
