---
title: Deforestation allocation (VT0007)
description: "Verra's VT0007 as UDef-ARP: risk threshold, vulnerability, fit, allocation, assessment."
order: 220
---

Verra's VT0007, the Unplanned Deforestation Allocation (UDef-A) of a
jurisdiction's expected deforestation to its forest, as Clark Labs' UDef-ARP
implements it, the same numbers: the negligible risk threshold of the distance
to the forest's edge, the vulnerability in classes of geometric widths (the
benchmark from that distance, or an alternative map such as a transition
potential), the relative frequency of deforestation per class and subdivision,
the allocation of the activity data never above a cell, and the assessment of
the predicted against the actual deforestation in an assessment grid. The
`tf.redd` module.

<!-- no-run -->
```python
import terraflux as tf

forest = tf.load_raster("forest_2015.tif")            # 1 forest at the start of the calibration
cal = tf.load_raster("deforestation_2015_2020.tif")   # 1 deforested in the calibration period
juris = tf.load_raster("jurisdiction.tif")            # non-zero inside
states = tf.load_raster("states.tif")                 # subdivisions (ids 0-999)

d = tf.redd.edge_distance(forest)
t = tf.redd.nrt(d, cal, mask=juris)                   # negligible risk threshold (m)
classes = tf.redd.benchmark(d, t, mask=juris)
f = tf.redd.fit(classes, cal, subdivisions=states)

# confirm the model on a later period, then allocate the activity data
cnf = tf.redd.allocate(classes_2020, f, deforestation=cnf_2020_2023, subdivisions=states)
tf.redd.assess(cnf.density, cnf_2020_2023, grid_area=10_000, mask=juris).metrics
a = tf.redd.allocate(classes_2023, f, expected=12_000, subdivisions=states)
tf.write_raster(a.density, "expected_deforestation_ha.tif")
```

## Distance and threshold

::: terraflux.redd.edge_distance

::: terraflux.redd.nrt

## Vulnerability

::: terraflux.redd.benchmark

::: terraflux.redd.alternative

## Fit and allocation

::: terraflux.redd.fit

::: terraflux.redd.Fit

::: terraflux.redd.allocate

::: terraflux.redd.Allocation

## Assessment

::: terraflux.redd.assess

::: terraflux.redd.Assessment
