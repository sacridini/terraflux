---
title: Brazil's Forest Code
description: "APPs of rivers, lakes, springs, slopes, hilltops; the Reserva Legal's balance per property."
order: 230
---

The Áreas de Preservação Permanente (APP) of Brazil's Forest Code (Lei
12.651/2012, art. 4) and the balance of every rural property's Reserva Legal:
the margins of rivers by their width and of natural lakes by their area, the
springs, the veredas, the slopes above 45°, the hilltops
([`tf.surface.hilltop`](/python/api/surface/#terraflux.surface.hilltop)), the land above
1,800 m and protected classes; art. 61-A's narrower margins in the areas
consolidated by 22 July 2008 by the property's fiscal modules; art. 12's
shares of the Reserva Legal, art. 15's APP counted and art. 67's small
properties. The `tf.forest_code` module.

<!-- no-run -->
```python
import terraflux as tf

dem = tf.load_raster("dem.tif")
a = tf.forest_code.app(
    rivers=tf.load_raster("rivers.tif"), river_width=tf.load_raster("river_width.tif"),
    lakes=tf.load_raster("lakes.tif"), springs=tf.load_vector("springs.gpkg"),
    slope=tf.surface.terrain(dem, "slope"), elevation=dem, hilltops=tf.surface.hilltop(dem).app,
    properties=tf.load_raster("car.tif"), consolidated=tf.load_raster("consolidated.tif"), fiscal_module=70)
a.table                                               # hectares of each kind of APP

r = tf.forest_code.reserve(tf.load_raster("car.tif"), tf.load_raster("native_vegetation.tif"),
                           share=80, app=a.app, fiscal_module=70,
                           vegetation_2008=tf.load_raster("native_2008.tif"))
r.table[["property", "required_ha", "counted_ha", "deficit_ha", "surplus_ha"]]
```

## Áreas de Preservação Permanente

::: terraflux.forest_code.app

::: terraflux.forest_code.APP

::: terraflux.forest_code.river_margin

::: terraflux.forest_code.consolidated_river

::: terraflux.forest_code.consolidated_lake

## Reserva Legal

::: terraflux.forest_code.reserve

::: terraflux.forest_code.ReservaLegal
