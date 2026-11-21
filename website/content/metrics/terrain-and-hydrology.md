---
title: Terrain and hydrology
description: The indices of an elevation model, cell by cell, from slope and aspect to landforms, wetness, flow and the connectivity of rivers.
order: 60
---

::: metrics terrain-and-hydrology

## The example elevation {#example}

The values on the cards were computed on the SRTM elevation of south-western Oregon that ships with the library, put on a
30 m grid in UTM zone 10N (369 x 270 cells), the way the terrain and hydrology tutorials do. The hydrological indices use
the same elevation with its depressions filled, and the stream cells are those that drain at least 500 cells. `stats`
only summarises a raster as a line (mean, range, cells without a value); the call on each card is the one that makes the
raster:

```python
import numpy as np
import xarray as xr
import terraflux as tf

# SRTM elevation of south-western Oregon on a 30 m grid in UTM zone 10N (369 x 270 cells)
dem0 = tf.load_raster(tf.surface.example_data("orelevation"))
grid = xr.DataArray(np.zeros((369, 270)), dims=("y", "x"),
                    coords={"y": 4771920 - 15 - 30 * np.arange(369), "x": 500010 + 15 + 30 * np.arange(270)})
dem = tf.reproject(dem0, like=grid.rio.write_crs("EPSG:32610"), resampling="bilinear")

filled = tf.hydro.fill(dem)                           # depressions filled, for the hydrological indices
acc = tf.hydro.flow_accumulation(filled)              # D8 accumulation, in cells
rivers = tf.hydro.streams(acc, 500)                   # the cells that drain at least 500 cells
link_map, links = tf.hydro.stream_links(filled, rivers)

# the NDVI of the same area, as an illustrative cover weight (a USLE C factor) for the connectivity index
ndvi = tf.reproject(tf.load_raster(tf.surface.example_data("orforest")), like=grid.rio.write_crs("EPSG:32610"),
                    resampling="bilinear")
cover = xr.where(ndvi > 0.55, 0.003, 0.05)

def stats(r):
    """The mean, the range and the number of cells without a value of a raster."""
    v = np.asarray(r.values, float)
    ok = v[np.isfinite(v)]
    return f"mean {ok.mean():.3g}, range {ok.min():.3g} to {ok.max():.3g}, {v.size - ok.size} cells without a value"

print(stats(tf.surface.terrain(dem, "slope")))
print(stats(tf.surface.tpi(dem, 5)))
print(stats(tf.hydro.twi(filled)))
print(len(links), "links;", int(links.cells.sum()), "stream cells")
```

```text
mean 18.1, range 0.118 to 58.2, 1274 cells without a value
mean 0.00856, range -24 to 26.1, 2540 cells without a value
mean 6.83, range 3.84 to 19, 155 cells without a value
103 links; 2126 stream cells
```

The slope has no value on the edge cells (369 + 270 rows and columns around the raster), the topographic position none on
the two outer rings of a 5 x 5 window, and the wetness index none where the slope is exactly 0. The DEM is in metres and the
cells are 30 m, so slopes are in degrees on a surface that really is that steep. For your own data use the UTM zone of the
area, or any projected system: slopes, curvatures and the hydrological indices read the cell size from the raster.
