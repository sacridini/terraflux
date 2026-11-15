---
title: Data on demand
description: "WorldClim, CHELSA, Copernicus DEM, SoilGrids, MapBiomas, Hydrography90m, STAC, onto a map's grid."
order: 20
---

Rasters downloaded onto the grid of a map: the climate of WorldClim 2.1 and
CHELSA 2.1 (now and under the CMIP6 scenarios), the Copernicus DEM (30 or 90
m), SoilGrids 2.0, MapBiomas' land cover of Brazil, Hydrography90m's streams,
sub-catchments and terrain, the scenes of any STAC catalogue (a median of the
items; a function to sign the links where they need a token) or any raster a URL or a path
names; and the environment of Hydrography90m's sub-catchments (the climate,
land cover and soil Environment90m sums up for each) as a table or a map. GDAL
reads only the part of each file the grid covers, warps it onto the grid of
`like` (or of an `extent` in longitudes and latitudes and a `cellsize`), and
the result is kept in the cache (`TERRAFLUX_CACHE`): a second call reads the
file it wrote. Classes are resampled by their most common cell, values
bilinearly. The `tf.download` module.

<!-- no-run -->
```python
import terraflux as tf

forest = tf.load_raster("forest.tif")
clim = tf.download.worldclim(forest, layers=[1, 12])       # bio01 and bio12 on the forest's grid
future = tf.download.chelsa(forest, gcm="MPI-ESM1-2-HR", ssp=370, period="2041-2070")
dem = tf.download.copernicus_dem(forest, resolution=30)
soc = tf.download.soilgrids(forest, property="soc", depth=["0-5cm", "5-15cm"])
cover = tf.download.mapbiomas(forest, years=[1985, 2023])
basins = tf.download.hydrography90m(forest, variable="sub_catchment")

s2 = tf.download.stac("https://earth-search.aws.element84.com/v1", "sentinel-2-l2a", forest,
                      assets=["red", "nir"], datetime="2024-07-01/2024-08-31",
                      query={"eo:cloud_cover": {"lt": 10}})
ndvi = (s2.sel(band="nir") - s2.sel(band="red")) / (s2.sel(band="nir") + s2.sel(band="red"))

env = tf.download.environment90m(basins, ["bio01_1981-2010_observed", "c10_2020"], stats=["mean", "sd"])
env.table                                                   # a row per sub-catchment
```

!!! note "Online"
    These functions need the internet the first time they fetch a layer; the
    examples that download are not run as tests.

## Climate

::: terraflux.download.worldclim

::: terraflux.download.chelsa

## Terrain, soils and land cover

::: terraflux.download.copernicus_dem

::: terraflux.download.soilgrids

::: terraflux.download.mapbiomas

## Hydrography90m and Environment90m

::: terraflux.download.hydrography90m

::: terraflux.download.environment90m

::: terraflux.download.Environment90m

## Any raster, any STAC catalogue

::: terraflux.download.raster

::: terraflux.download.stac
