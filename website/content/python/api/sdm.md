---
title: Species distributions
description: "Clean records, choose predictors and background, spatial folds, fit, evaluate, project, disperse."
order: 180
---

Preparing occurrence records and environmental predictors for species
distribution models, fitting the models and projecting them: the `tf.sdm`
module. Every step reproduces the R package the field relies on for it, and
each function's notes name that package.

<!-- no-run -->
```python
import terraflux as tf

occ = tf.load_table(tf.sdm.example_data("bradypus"))
env = tf.load_raster(tf.sdm.example_data("bioclim"))
data = tf.sdm.prepare(occ, env)          # clean, one per cell, predictors, background, spatial folds
res = tf.sdm.fit(data, models)           # cross-validated scikit-learn models (or tf.sdm.MaxNet)
suit = tf.sdm.predict(res, env)          # ensemble and its uncertainty
```

## The whole preparation

::: terraflux.sdm.prepare

## Occurrence records

::: terraflux.sdm.download_gbif

::: terraflux.sdm.clean_occurrences

::: terraflux.sdm.reference_data

::: terraflux.sdm.one_per_cell

::: terraflux.sdm.thin

## Study area

::: terraflux.sdm.accessible_area

## Predictors

::: terraflux.sdm.select_predictors

::: terraflux.sdm.vif

## Background and data

::: terraflux.sdm.background

::: terraflux.sdm.bias_surface

::: terraflux.sdm.dataset

::: terraflux.sdm.SDMData

## Cross-validation folds

::: terraflux.sdm.spatial_blocks

::: terraflux.sdm.autocorrelation_range

::: terraflux.sdm.spatial_clusters

::: terraflux.sdm.buffer_folds

::: terraflux.sdm.quadrant_folds

::: terraflux.sdm.checkerboard_folds

::: terraflux.sdm.random_folds

::: terraflux.sdm.jackknife_folds

## Models

::: terraflux.sdm.MaxNet

## Evaluation

::: terraflux.sdm.evaluate

::: terraflux.sdm.Evaluation

::: terraflux.sdm.boyce

::: terraflux.sdm.threshold

::: terraflux.sdm.omission_rate

## Extrapolation

::: terraflux.sdm.mess

::: terraflux.sdm.mop

::: terraflux.sdm.exdet

## Models and projections

::: terraflux.sdm.fit

::: terraflux.sdm.SDMResult

::: terraflux.sdm.predict

::: terraflux.sdm.response_curves

::: terraflux.sdm.importance

## Dispersal in a changing climate

MigClim 1.6's cellular automaton: the colonisation of the suitable cells of successive periods within the
dispersal kernel's distance, barriers, long-distance dispersal and the cells' maturity; the same run as MigClim's
for a seed (its random numbers are Microsoft's C runtime `rand()`).

::: terraflux.sdm.migrate

::: terraflux.sdm.Migration

## Area of habitat

::: terraflux.sdm.area_of_habitat

::: terraflux.sdm.HabitatAreas

::: terraflux.sdm.cell_area_km2

## Example data

::: terraflux.sdm.example_data
