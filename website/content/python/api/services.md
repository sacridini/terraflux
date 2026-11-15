---
title: Ecosystem services
description: "InVEST's models: carbon, habitat quality, water yield and hydropower, sediment delivery."
order: 240
---

Models of [InVEST](https://naturalcapitalproject.stanford.edu/software/invest)
(the Natural Capital Project's natcap.invest, Apache-2.0), reimplemented in C++
from land use maps and parameter tables: their maps, bit for bit where the
arithmetic permits. A simulated land use map (`tf.simulate`) or a series of
scenarios goes straight in.

The input rasters may come on different grids (cell sizes, origins, extents,
CRS): each model aligns them first as InVEST does, on the grid of its reference
raster (the DEM, or the land use map), and the models with watersheds or an AOI
clip the maps to the polygons' bounds. Inputs already on that grid are used as
they are, and the results come on the grid they were computed on.

## Carbon storage and sequestration

::: terraflux.services.carbon

::: terraflux.services.Carbon

::: terraflux.services.valuation_constant

## Emissions and removals of land change

The carbon of a series of land use maps by the IPCC's stock-difference method:
a stock per class, or a curve of regrowth from the date a class appeared in a
cell (Chapman-Richards or linear, for secondary forest); emissions and removals
by period, by transition and by cell.

::: terraflux.services.emissions

::: terraflux.services.Emissions

## Annual water yield and hydropower

::: terraflux.services.water_yield

::: terraflux.services.WaterYield

## Seasonal water yield

::: terraflux.services.seasonal_water_yield

::: terraflux.services.SeasonalWaterYield

## Sediment delivery ratio

::: terraflux.services.sdr

::: terraflux.services.SDR

## Nutrient delivery ratio

::: terraflux.services.ndr

::: terraflux.services.NDR

## Habitat quality and rarity

::: terraflux.services.habitat_quality

::: terraflux.services.HabitatQuality

## Crop pollination

::: terraflux.services.pollination

::: terraflux.services.Pollination

## Coastal vulnerability

::: terraflux.services.coastal_vulnerability

::: terraflux.services.CoastalVulnerability
