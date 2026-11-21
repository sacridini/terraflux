---
title: Wetness, stream power and sediment connectivity
short: Wetness and connectivity
description: Combine contributing area and slope into the topographic wetness, stream power and sediment transport indices, and map how strongly every hillslope is connected to the streams with the index of connectivity.
order: 30
level: intermediate
time: 45 min
ops: [hydro.twi, hydro.spi, hydro.sti, hydro.connectivity_index, hydro.flow_accumulation, hydro.hand, raster.calc]
series: Terrain hydrology
part: 3
---

In this part you compute the terrain indices that ecologists use most as proxies of soil water and erosion:
the topographic wetness index, the stream power index and the sediment transport index, all three built from
the contributing area of [part 1](../depressions-and-flow/) and the local slope. Then you map the *index of
connectivity*, which asks how easily sediment from each hillslope reaches a stream, give it a cover weight
from a vegetation map, and see what clearing the riparian forest would do to it. At the end you will know what
each index measures, which flow method to use for each, and how to report them so that others can reproduce
your numbers.

## The idea

Two numbers describe the water arriving at a point of a hillslope: how much land drains to it, and how steep
it is there. Much land and a gentle slope: water collects faster than it drains away, and the soil stays wet.
Much land and a steep slope: the water runs fast and has power to erode. Little land: the soil drains and dries.
The indices of this page are combinations of these two numbers.

The land is measured by the **specific contributing area** \( a \): the area that drains through a cell, divided
by the width of the contour it crosses (the width of a cell), in m²/m (part 1, `units = sca`). Dividing by the
width makes it independent of the resolution: it is the area that drains through each metre of contour. The
slope \( \beta \) is the local slope of the cell, from Horn's (1981) method.

!!! science "Key idea: the topographic wetness index"
    Beven & Kirkby (1979) built the hydrological model TOPMODEL on the idea that the depth of the water table at
    a point depends on

    \[ \mathrm{TWI} = \ln \frac{a}{\tan \beta} \]

    where \( a \) is the specific contributing area (m²/m) and \( \tan \beta \) the slope as a gradient
    (metres of fall per metre). Places with the same TWI respond alike: they become saturated at the same
    level of the water table. TWI is high (10 to 15) in valley floors and hollows, where much area drains through
    a gentle slope, and low (4 to 6) on ridges and steep convex slopes. It is a logarithm, so a difference of
    1 is a factor of 2.7 in the ratio of area to slope.

Moore, Grayson & Ladson (1991) reviewed the family of such *compound terrain indices* and their uses in
hydrology, geomorphology and biology. Two others, for erosion:

!!! science "Key idea: stream power and sediment transport"
    The **stream power index** is proportional to the power of the overland flow, its ability to erode:

    \[ \mathrm{SPI} = a \tan \beta \]

    The **sediment transport index** of Moore & Burch (1986) is the length–slope factor *LS* of the Universal
    Soil Loss Equation rewritten with the contributing area instead of the length of a plot:

    \[ \mathrm{STI} = (m + 1) \left( \frac{a}{22.13} \right)^{m} \left( \frac{\sin \beta}{0.0896} \right)^{n} \]

    with \( m = 0.4 \) and \( n = 1.3 \) by default. 22.13 m and 0.0896 (a slope of 9 %, \( \sin \beta \)) are
    the length and slope of the standard plot of the USLE, so an STI of 1 means "as erosive as the standard
    plot", 10 means ten times as much.

The last index is about connections rather than amounts. Borselli, Cassi & Torri (2008) asked how likely
sediment eroded on a cell is to reach a stream, and answered with the **index of connectivity** (IC); Cavalli et
al. (2013) adapted it to high-resolution DEMs of Alpine catchments, and InVEST's sediment delivery ratio model
uses it to decide what share of each cell's soil loss reaches the streams (Hamel et al., 2015).

!!! science "Key idea: the index of connectivity"
    \[ \mathrm{IC} = \log_{10} \frac{D_{up}}{D_{dn}}, \qquad D_{up} = \bar{W} \, \bar{S} \sqrt{A}, \qquad
       D_{dn} = \sum_i \frac{d_i}{W_i \, S_i} \]

    The **upslope component** \( D_{up} \) is the potential for sediment to come down to the cell: \( A \) the
    contributing area (m²), \( \bar{W} \) the mean *weight* and \( \bar{S} \) the mean slope gradient of the
    area upslope. The **downslope component** \( D_{dn} \) is the resistance of the path from the cell to the
    nearest stream: the sum, over the cells \( i \) of that path, of the length \( d_i \) of each step divided
    by the weight and slope of the cell. The weight \( W \) is a cover factor between 0 and 1 that slows
    sediment, in InVEST the USLE's C of each land use: 0.001 to 0.01 under dense forest, much higher on bare or
    cropped land. IC rises (towards 0 and above) where a large, steep, bare area lies close to a stream along a
    steep, bare path, and falls (large negative numbers) where the path to the stream is long, gentle and
    covered.

## When to use it

- **Soil moisture and wetland plants.** TWI is one of the most common covariates of species distribution
  models of plants, bryophytes and amphibians, and of maps of wet soils and peat. In temperate forest plots,
  Kopecký, Macek & Wild (2021) found that a TWI computed with FD8 explained more than twice the variation in
  measured soil moisture and in the composition of vascular plants and bryophytes than one computed with D8.
- **Where gullies and erosion start.** High SPI marks the hollows and channel heads where concentrated flow can
  erode; STI is a map of the LS factor for soil loss studies.
- **Where to put buffers and restoration.** IC shows the hillslopes whose sediment reaches the streams most
  easily: the first candidates for riparian buffers, reforestation or erosion control, and the places where a
  clearing would hurt the streams most.
- **InVEST.** [[hydro.connectivity_index]] is the IC of InVEST's sediment delivery ratio model, computed the
  same way; use it to understand that model or to build variants of it ([Ecosystem services](../../ecosystem-services/)).

What they are *not*: TWI assumes a water table parallel to the surface and soils of uniform conductivity, so it
works best in humid landscapes with shallow soils over impermeable rock and worst on deep, permeable or flat
land. SPI and STI rank places by erosion potential; they are not soil loss in tonnes (for that, see the RUSLE of
InVEST's [[services.sdr]] or SWAT+ in [part 4](../swat-plus/)). IC is a relative index: compare it within a map
and between scenarios on the same map, not as an absolute measure between regions.

## What you need

- **The filled DEM of [part 1](../depressions-and-flow/)**: `filled.tif`, the Oregon SRTM elevation reprojected
  to UTM 10N in cells of 30 m and filled, as part 1's `save_filled` node writes it.
- **A projected DEM**, for the index of connectivity: it takes the size of its cells from the map's coordinates
  as they are, so a map in degrees would give areas in square degrees. TWI, SPI and STI take cells in metres
  either way.
- **A cover weight** for IC: here made from the summer NDVI of the same area (`ndvi.tif`, as in part 1), forest
  above 0.55, open land below.

## Build the model

### Wetness, stream power and sediment transport

<div class="steps" markdown>

1. Start a new model with a [[raster.read]] (subject **IO**) of `filled.tif`, renamed `filled`.
2. Select `filled` and, in the library's subject **Terrain hydrology**, click [[hydro.twi]], [[hydro.spi]] and
   [[hydro.sti]]. Each comes connected to `filled`'s raster; rename them `twi`, `spi` and `sti`.
3. Run to each (++ctrl+enter++).

</div>

```tfgraph
#! caption: The wetness, stream power and sediment transport indices of the filled DEM.
#! highlight: twi, spi, sti
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "filled"
op = "raster.read"
params = { path = "filled.tif" }

[[nodes]]
id = "twi"
op = "hydro.twi"
inputs = { dem = "filled.raster" }
params = { method = "fd8" }
note = "ln(a / tan slope)"

[[nodes]]
id = "spi"
op = "hydro.spi"
inputs = { dem = "filled.raster" }
note = "a x tan slope"

[[nodes]]
id = "sti"
op = "hydro.sti"
inputs = { dem = "filled.raster" }
params = { sca_exponent = 0.4, slope_exponent = 1.3 }
note = "The LS factor from the area."
```

Each of the three operators computes its two parts itself from the DEM: the specific contributing area by the
accumulation **method** (`fd8` by default, also `quinn`, `dinf`, `d8` and `mfd`, as in part 1) and the slope
by Horn's method. The results are WhiteboxTools' `WetnessIndex`, `StreamPowerIndex` and
`SedimentTransportIndex` cell for cell. The other settings:

- **min_slope** ([[hydro.twi]] only): a floor for \( \tan \beta \). On perfectly flat cells \( \tan \beta = 0 \)
  and the TWI is infinite, so they get no value (here 155 cells, the flat floors of filled depressions). A floor
  such as `0.001` gives them a high, finite value.
- **sca_exponent** and **slope_exponent** ([[hydro.sti]] only): \( m \) and \( n \). Moore & Burch's 0.4 and
  1.3 are the usual values.
- **flats**: drain flat areas (on by default; leave it on with a filled DEM).

Two optional inputs replace the parts: `sca`, a specific contributing area made your own way, and `slope`, a
slope in degrees. That is how to follow a recipe exactly. Kopecký et al. (2021) recommend FD8 with a flow
dispersion (the **exponent** of [[hydro.flow_accumulation]]) close to 1.0, the flow width equal to the cell
size, and the local slope; Terraflux's default exponent is Freeman's 1.1. For their recipe:

```tfgraph
#! caption: TWI by the recipe of Kopecký et al. (2021): FD8 with a dispersion of 1.0, given as the specific contributing area.
#! highlight: sca, twi
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "filled"
op = "raster.read"
params = { path = "filled.tif" }

[[nodes]]
id = "sca"
op = "hydro.flow_accumulation"
inputs = { dem = "filled.raster" }
params = { method = "fd8", exponent = 1.0, units = "sca" }
note = "m²/m, the area per metre of contour."

[[nodes]]
id = "twi"
op = "hydro.twi"
inputs = { dem = "filled.raster", sca = "sca.map" }
```

On this map the difference is small (the two TWIs correlate at 0.9999, the recipe's is higher by 0.02 on
average), but stating it makes your index reproducible.

### Wetness, stream power and sediment transport in Python

The part starts from the filled DEM of part 1. In Python it is its three steps (the Oregon SRTM, projected to UTM 10N in
cells of 30 m), and the HAND of part 2 for the valley floors:

```python
import numpy as np
import xarray as xr
import terraflux as tf

raw = tf.load_raster(tf.surface.example_data("orelevation"))
x = 500010 + 15 + 30 * np.arange(270)
y = 4771920 - 15 - 30 * np.arange(369)
grid = xr.DataArray(np.zeros((369, 270), "float32"), coords={"y": y, "x": x}, dims=("y", "x")).rio.write_crs("EPSG:32610")
filled = tf.hydro.fill(tf.reproject(raw, like=grid, resampling="bilinear"))     # part 1
height = tf.hydro.hand(filled, threshold=500)                                   # part 2

wet = tf.hydro.twi(filled)                          # method="fd8", node `twi`
spi = tf.hydro.spi(filled)                          # node `spi`
sti = tf.hydro.sti(filled)                          # sca_exponent=0.4, slope_exponent=1.3: node `sti`
print(np.nanpercentile(wet.values, [1, 50, 99]).round(2), int(np.isnan(wet.values).sum()))
print(round(float(np.nanmedian(spi.values))), round(float(np.nanmedian(sti.values)), 1),
      round(float(np.nanpercentile(sti.values, 90)), 1))
```

```text
[ 4.77  6.36 13.23] 155
69 17.8 39.4
```

[`tf.hydro.twi`][terraflux.hydro.twi], [`tf.hydro.spi`][terraflux.hydro.spi] and [`tf.hydro.sti`][terraflux.hydro.sti] each
compute their two parts from the DEM, by the accumulation `method` (`"fd8"` by default, `"quinn"`, `"dinf"`, `"d8"`,
`"mfd"`) and Horn's slope. TWI goes from 4.8 to 13.2 (1st and 99th percentiles) with a median of 6.4, and the 155 cells
with no value are the flat floors of the filled depressions: `min_slope=0.001` gives them a high, finite value (no cell is left empty). The median of SPI is 69 and those of STI 17.8 and its 90th percentile 39, the
part's numbers. Two optional inputs replace the parts, `sca=` (a specific contributing area) and `slope=` (in degrees),
which is how to follow a recipe: the one of Kopecký et al. (2021) is FD8 with an exponent of 1.0:

```python
sca = tf.hydro.flow_accumulation(filled, method="fd8", exponent=1.0, units="sca")
recipe = tf.hydro.twi(filled, sca=sca)
ok = ~np.isnan(wet.values) & ~np.isnan(recipe.values)
print(round(float(np.corrcoef(wet.values[ok], recipe.values[ok])[0, 1]), 4), round(float(np.mean(recipe.values[ok] - wet.values[ok])), 3))
```

```text
0.9999 0.018
```

The two TWIs correlate at 0.9999 and the recipe's is higher by 0.02 on average, as the part says. The wetness of the
valley floors against the rest, by the five methods (the slider's numbers: the mean, the mean on the cells less than 5 m
above their stream and elsewhere, and the correlation with FD8), and by class of HAND:

```python
h = height.values
low = h < 5
fd8 = tf.hydro.twi(filled).values
for method in ("d8", "dinf", "fd8", "quinn", "mfd"):
    w = tf.hydro.twi(filled, method=method).values
    ok = ~np.isnan(w) & ~np.isnan(fd8)
    print(f"{method:6s} mean {np.nanmean(w):.2f}  valley floors {np.nanmean(w[low]):.2f}  elsewhere {np.nanmean(w[~low & ~np.isnan(h)]):.2f}  "
          f"corr with fd8 {np.corrcoef(w[ok], fd8[ok])[0, 1]:.3f}")
d8 = tf.hydro.twi(filled, method="d8").values
for lo, hi in ((0, 5), (5, 10), (10, 20), (20, 50), (50, 100), (100, 1e9)):
    c = (h >= lo) & (h < hi)
    print(f"HAND {lo:3d}-{min(hi, 999):3.0f} m: FD8 {np.nanmean(fd8[c]):.2f}  D8 {np.nanmean(d8[c]):.2f}")
ok = ~np.isnan(fd8) & ~np.isnan(h)
print(round(float(np.corrcoef(fd8[ok], h[ok])[0, 1]), 2))
```

```text
d8     mean 6.20  valley floors 10.48  elsewhere 6.02  corr with fd8 0.827
dinf   mean 6.44  valley floors 10.71  elsewhere 6.25  corr with fd8 0.927
fd8    mean 6.83  valley floors 11.19  elsewhere 6.65  corr with fd8 1.000
quinn  mean 6.87  valley floors 11.21  elsewhere 6.69  corr with fd8 0.999
mfd    mean 6.87  valley floors 11.65  elsewhere 6.67  corr with fd8 0.982
HAND   0-  5 m: FD8 11.19  D8 10.48
HAND   5- 10 m: FD8 8.63  D8 6.81
HAND  10- 20 m: FD8 7.83  D8 6.52
HAND  20- 50 m: FD8 7.11  D8 6.26
HAND  50-100 m: FD8 6.79  D8 6.14
HAND 100-999 m: FD8 6.29  D8 5.82
-0.43
```

The mean TWI of the valley floors is 11.2 and the others' 6.7, falling steadily as HAND rises, and the correlation of TWI
with HAND is -0.43 (the part says -0.42): related, not the same. The D8 TWI correlates with FD8's at only 0.83; the
resolution of the DEM and the method change the index, so say both when you report it.

### The index of connectivity

<div class="steps" markdown>

1. Add a [[raster.read]] of `ndvi.tif` (`ndvi_read`) and a [[raster.reproject]] after it (`ndvi`) with the
   settings of part 1: **crs** `EPSG:32610`, **resolution** `30`, **resampling** `bilinear`, **extent**
   `[500010, 4760850, 508110, 4771920]`, so that it lies on the grid of `filled.tif`.
2. Add a [[raster.calc]] named `cover` with input `n` from `ndvi` and the expression
   `0.003 if n > 0.55 else 0.05`: the weight \( W \), a cover factor of 0.003 under forest and 0.05 on open
   land.
3. Add [[hydro.connectivity_index]] named `ic`, its `dem` from `filled` and its `weight` from `cover`, and set
   **threshold** to `500`.

</div>

```tfgraph
#! caption: The index of connectivity with a cover weight from the NDVI.
#! highlight: cover, ic
#! numbered: true
#! params: false
format = "terraflux/1"

[[nodes]]
id = "filled"
op = "raster.read"
params = { path = "filled.tif" }

[[nodes]]
id = "ndvi_read"
op = "raster.read"
params = { path = "ndvi.tif" }

[[nodes]]
id = "ndvi"
op = "raster.reproject"
inputs = { raster = "ndvi_read.raster" }
params = { crs = "EPSG:32610", resolution = 30, extent = [500010, 4760850, 508110, 4771920], resampling = "bilinear" }

[[nodes]]
id = "cover"
op = "raster.calc"
inputs = { n = "ndvi.raster" }
params = { expression = "0.003 if n > 0.55 else 0.05" }
note = "W: a USLE C per cover (illustrative)."

[[nodes]]
id = "ic"
op = "hydro.connectivity_index"
inputs = { dem = "filled.raster", weight = "cover.map" }
params = { threshold = 500, method = "mfd" }
note = "log10(D_up / D_dn), as InVEST's SDR."
```

The two weights are illustrative values of the order of a closed forest's and a grassland's C; in a real study,
take them from the biophysical table of your land uses (the same table InVEST's sediment model reads) and say
where they come from.

[[hydro.connectivity_index]] reproduces what InVEST's sediment delivery ratio model does inside: it fills the
DEM, routes water by InVEST's multiple flow directions (**method** `mfd`, the default, or `d8`), computes the
slope gradient as InVEST does and bounds it between 0.005 and 1, raises weights below 0.001 to 0.001, and finds
the streams either from the `streams` input (1 on the streams) or as the cells whose accumulation reaches the
**threshold**, in cells. The streams have no value: IC is about the way *to* them.

### The index of connectivity in Python

[`tf.hydro.connectivity_index`][terraflux.hydro.connectivity_index] takes the filled DEM, the weight raster on its grid
and the streams, as a raster (`streams=`, 1 on the streams) or as the `threshold` in cells, traced as InVEST does. The
weight is the part's: 0.003 under forest (NDVI above 0.55), 0.05 on open land:

```python
ndvi = tf.reproject(tf.load_raster(tf.surface.example_data("orforest")), like=grid, resampling="bilinear")
cover = xr.where(ndvi > 0.55, 0.003, 0.05)                       # W, a USLE C per cover (illustrative)
ic = tf.hydro.connectivity_index(filled, cover, threshold=500)   # method="mfd", the default
print(np.nanpercentile(ic.values, [1, 99]).round(2), round(float((cover.values == 0.05).mean()) * 100, 1))
```

```text
[-7.15 -3.61] 3.8
```

IC goes from -7.15 to -3.61 (1st and 99th percentiles; the part has -7.1 and -3.6), 3.8 % of the map is open land, and the
streams have no value (the result is NaN there and wherever the flow reaches no stream). The routing (`method="d8"`)
and the stream threshold are the settings that matter; the weights decide the contrast between land uses:

```python
for cells in (100, 500, 2500):
    print(cells, round(float(np.nanmean(tf.hydro.connectivity_index(filled, cover, threshold=cells).values)), 2))
ic_d8 = tf.hydro.connectivity_index(filled, cover, threshold=500, method="d8")
ok = ~np.isnan(ic.values) & ~np.isnan(ic_d8.values)
print(round(float(np.corrcoef(ic.values[ok], ic_d8.values[ok])[0, 1]), 2))
```

```text
100 -5.12
500 -5.48
2500 -5.88
0.86
```

The mean IC is -5.12 with streams from 9 ha, -5.48 from 45 ha and -5.88 from 225 ha, and IC by D8 correlates with IC by
`mfd` at 0.86: the pattern changes little, the values shift. The function fills the DEM itself, takes slopes as InVEST
does (kept between 0.005 and 1) and raises weights below 0.001 to 0.001; the DEM must be projected, in metres.

### A scenario: the riparian forest cleared

What if the forest along the streams were cleared? Use the HAND of [part 2](../streams-watersheds-and-hand/)
to find the valley floors and give them the weight of open land:

<div class="steps" markdown>

1. Add [[hydro.hand]] named `hand`, its `dem` from `filled`, with **threshold** `500` (it makes its own streams
   from the D8 accumulation).
2. Add a [[raster.calc]] named `cleared` with inputs `c` from `cover` and `h` from `hand`, and the expression
   `0.05 if h < 20 else c`: every cell less than 20 m above its stream becomes open land.
3. Add a second [[hydro.connectivity_index]] named `ic_cleared`, the same as `ic` but with its `weight` from
   `cleared`.

</div>

```tfgraph
#! caption: The index of connectivity with the forest of the valley floors (HAND under 20 m) cleared.
#! highlight: hand, cleared, ic_cleared
#! numbered: true
#! params: false
format = "terraflux/1"

[[nodes]]
id = "filled"
op = "raster.read"
params = { path = "filled.tif" }

[[nodes]]
id = "ndvi_read"
op = "raster.read"
params = { path = "ndvi.tif" }

[[nodes]]
id = "ndvi"
op = "raster.reproject"
inputs = { raster = "ndvi_read.raster" }
params = { crs = "EPSG:32610", resolution = 30, extent = [500010, 4760850, 508110, 4771920], resampling = "bilinear" }

[[nodes]]
id = "cover"
op = "raster.calc"
inputs = { n = "ndvi.raster" }
params = { expression = "0.003 if n > 0.55 else 0.05" }

[[nodes]]
id = "hand"
op = "hydro.hand"
inputs = { dem = "filled.raster" }
params = { threshold = 500 }

[[nodes]]
id = "cleared"
op = "raster.calc"
inputs = { c = "cover.map", h = "hand.map" }
params = { expression = "0.05 if h < 20 else c" }

[[nodes]]
id = "ic"
op = "hydro.connectivity_index"
inputs = { dem = "filled.raster", weight = "cover.map" }
params = { threshold = 500 }

[[nodes]]
id = "ic_cleared"
op = "hydro.connectivity_index"
inputs = { dem = "filled.raster", weight = "cleared.map" }
params = { threshold = 500 }
```

```tfgallery
columns: 2
caption: "The cover weight W today (forest where the summer NDVI exceeds 0.55, 96 % of the map) and with the forest of the valley floors cleared (open land on 15 % of the map)."
images:
  - { src: /img/tutorials/terrain-hydrology/wetness-and-connectivity/cover.png, caption: "cover: today" }
  - { src: /img/tutorials/terrain-hydrology/wetness-and-connectivity/cleared.png, caption: "cleared: HAND under 20 m open" }
```

=== "Application"

    Run to a node with ++ctrl+enter++ to see its map in the Viewer; a double click keeps it there as a layer. To
    compare `ic` and `ic_cleared`, choose the other under **Compare** in the Viewer (a curtain, side by side or
    their difference), or subtract them with a [[raster.calc]] (`b - a`).

=== "Command line"

    Save the model as `indices.toml` beside `elevation.tif` and `ndvi.tif`, add a [[raster.write]] after each
    map you want on disk, and run:

    ```bash
    terraflux check indices.toml
    terraflux run indices.toml --out output
    ```

### The riparian forest cleared in Python

The scenario is the same raster arithmetic: every cell less than 20 m above its stream becomes open land, and the
index is computed again with the new weight (nodes `cleared` and `ic_cleared`):

```python
cleared = xr.where(height < 20, 0.05, cover)
ic_cleared = tf.hydro.connectivity_index(filled, cleared, threshold=500)
valley = height.values < 20
print(round(float((cleared.values == 0.05).mean()) * 100, 1))
for name, cells in (("valley floors", valley), ("slopes above", ~valley)):
    print(name, round(float(np.nanmean(ic.values[cells])), 2), round(float(np.nanmean(ic_cleared.values[cells])), 2))
```

```text
14.3
valley floors -4.7 -3.18
slopes above -5.54 -5.21
```

Open land goes from 3.8 % of the map to 14.3 %, all of it on the valley floors, and the mean IC of the valley floors
rises from -4.70 to -3.18 (the part has -4.71 and -3.19), about 1.5 units: sediment reaches the streams some 30 times
more easily. The slopes above, untouched, rise by 0.33 (a factor of 2), because their path to the stream now crosses
cleared land.

## Read the results

**Wetness.** The TWI (FD8) of this map goes from about 5 on the ridges to 13 in the main valleys (1st and 99th
percentiles); its median is 6.4. The streams draw continuous dark lines, and the hollows at their heads fan out in lighter blue: the zones of
the hillslopes where water converges. Valley floors are much wetter than the rest: the cells less than 5 m
above their stream (part 2) have a mean TWI of 11.2, the others 6.7, and the mean falls steadily as HAND rises.

```tfchart
type: bar
data: /data/tutorials/terrain-hydrology/wetness-and-connectivity/twi-by-hand.csv
x: HAND
y: ["Mean TWI (FD8)", "Mean TWI (D8)"]
colors: { "Mean TWI (FD8)": "#2f6f9f", "Mean TWI (D8)": "#9ecae1" }
xlabel: Height above the nearest stream
ylabel: Mean TWI
height: 280
caption: "The mean wetness index of the cells in six classes of HAND, by FD8 and by D8. Both fall from the valley floors to the ridges; D8 gives lower values on the hillslopes, where it does not spread water."
```

TWI and HAND are related but not interchangeable: their correlation across the map is −0.42. HAND says how
high a cell is above the drainage; TWI how much water converges on it. A hollow high on a slope can have a high
TWI and a large HAND.

```tfgallery
columns: 3
caption: "The three indices of the filled DEM. TWI (FD8): high in valleys and hollows. SPI, on a logarithmic scale: highest along the streams and in the steep hollows above them. STI, the LS factor: highest where steep slopes collect area, the hillslope hollows and valley sides."
images:
  - { src: /img/tutorials/terrain-hydrology/wetness-and-connectivity/twi-small.png, caption: "Wetness (TWI)" }
  - { src: /img/tutorials/terrain-hydrology/wetness-and-connectivity/spi.png, caption: "Stream power (SPI)" }
  - { src: /img/tutorials/terrain-hydrology/wetness-and-connectivity/sti.png, caption: "Sediment transport (STI)" }
```

**Stream power and sediment transport.** SPI spans five orders of magnitude (median 69, the main river
several hundred thousand), so map and analyse its logarithm. Its highest values are on the streams themselves,
where the index has little meaning for hillslope erosion: mask the streams, or read SPI on the hillslopes and
channel heads. STI has a median of 17.8 and 90 % of the map below 39: this steep terrain is, by its shape alone,
an erosive one; the cover (forest) is what protects it.

**Connectivity.** IC ranges from about −7.1 to −3.6 here (1st to 99th percentiles); only the differences
mean something. Red cells (high IC) are hillslopes close to a stream along steep paths, and the open patches,
whose larger weight (a less protective cover) raises the upslope component and shortens the downslope one; blue cells (low IC) are ridges far from any stream, behind long
forested slopes.

![The index of connectivity](/img/tutorials/terrain-hydrology/wetness-and-connectivity/ic.png "The index of connectivity with a cover weight of 0.003 under forest and 0.05 on open land (3.8 % of the map). The streams (white) have no value. The patches of open land stand out in red: their sediment reaches the streams most easily.")

Clearing the riparian forest turns 11 % more of the map into open land, all of it on the valley floors. Drag
the handle:

```tfcompare
caption: "Drag the handle: the index of connectivity today (left) and with the forest less than 20 m above the streams cleared (right), the same colours. The valley floors turn red: the cleared cells themselves, and every cell whose path to the stream crosses them."
before: { src: /img/tutorials/terrain-hydrology/wetness-and-connectivity/ic_now.png, label: "Today" }
after: { src: /img/tutorials/terrain-hydrology/wetness-and-connectivity/ic_cleared.png, label: "Riparian forest cleared" }
```

On the valley floors (HAND under 20 m) the mean IC rises from −4.71 to −3.19, 1.5 units: sediment reaches the
streams some 30 times more easily (\( 10^{1.5} \)). On the slopes above, untouched, it still rises by 0.34 (a
factor of 2), because their path to the streams now crosses cleared land. That is the case for riparian
buffers in one figure: a strip of forest along the streams protects them from the sediment of the whole
hillslope above.

!!! paper "In the literature"
    For TWI, SPI and STI, report the DEM and its conditioning, the accumulation method (with its exponent), how
    the specific contributing area was computed (flow width) and how the slope was computed, and what was done
    with flat cells: Kopecký et al. (2021) found that most studies do not, which makes them impossible to
    reproduce. For IC, report the weight of every land use and its source, the routing (mfd or d8) and the
    stream threshold, and compare scenarios on the same DEM.

## Settings that change the answer

**The flow method** matters for TWI more than any other setting. Move the slider:

```tfslider
label: Accumulation method
caption: "TWI by the five methods of the specific contributing area, the same colours. D8 leaves hillslopes in thin parallel lines and lowers their values; the multiple flow methods spread the water and give smooth, higher values. The numbers: the mean TWI, and the mean on the valley floors (HAND under 5 m) and elsewhere."
frames:
  - { value: "D8", src: /img/tutorials/terrain-hydrology/wetness-and-connectivity/twi_d8.png, stats: { "Mean": "6.20", "Valley floors": "10.47", "Elsewhere": "6.02", "Correlation with FD8": "0.83" } }
  - { value: "D-infinity", src: /img/tutorials/terrain-hydrology/wetness-and-connectivity/twi_dinf.png, stats: { "Mean": "6.44", "Valley floors": "10.70", "Elsewhere": "6.25", "Correlation with FD8": "0.93" } }
  - { value: "FD8", src: /img/tutorials/terrain-hydrology/wetness-and-connectivity/twi_fd8.png, stats: { "Mean": "6.83", "Valley floors": "11.19", "Elsewhere": "6.65", "Correlation with FD8": "1" } }
  - { value: "Quinn", src: /img/tutorials/terrain-hydrology/wetness-and-connectivity/twi_quinn.png, stats: { "Mean": "6.87", "Valley floors": "11.21", "Elsewhere": "6.69", "Correlation with FD8": "0.999" } }
  - { value: "MFD (InVEST)", src: /img/tutorials/terrain-hydrology/wetness-and-connectivity/twi_mfd.png, stats: { "Mean": "6.87", "Valley floors": "11.66", "Elsewhere": "6.67", "Correlation with FD8": "0.98" } }
start: 2
```

The TWI by D8 correlates with the TWI by FD8 at only 0.83 across the map. In a species distribution model, that
is a different covariate. Prefer FD8 (Kopecký et al., 2021) unless you have a reason, such as consistency with
an earlier study, and test the choice against field measurements when you have them; Sørensen, Zinko & Seibert
(2006) found that the best method varied with the variable (species richness, soil pH, groundwater, soil
moisture) and the site.

**The resolution** changes everything: \( a \) grows and slopes flatten as cells grow, and a TWI from a 30 m DEM
cannot see the microtopography that a lidar DEM of 1 m sees. Compare TWI only between maps of the same
resolution and method.

**The stream threshold of IC** moves where the paths end. With streams from 9 ha, the paths are shorter and
the mean IC is −5.12; from 45 ha, −5.48; from 225 ha, −5.88. The pattern changes little; the values shift. The
**routing** matters less: IC by D8 routing correlates with IC by `mfd` at 0.86. And the **weights** decide
the contrast between land uses: they are the part to justify best.

## Common pitfalls

- **An unprojected DEM for IC.** [[hydro.connectivity_index]] takes the cell size from the map's coordinates;
  in degrees, the areas and distances are wrong. Reproject first.
- **Flat cells.** Without **min_slope**, filled flats have no TWI. A model fitted on the cells with a value
  silently leaves out the wettest places (filled valley bottoms). Set a small floor, or fill the holes.
- **SPI on its natural scale.** It is so skewed that a regression on it is driven by a few stream cells. Take
  its logarithm (with a small constant if some cells are 0).
- **Comparing with other software.** SAGA's "SAGA wetness index", ArcGIS recipes and WhiteboxTools' tools give
  different numbers for the "same" TWI: different accumulation methods, flow widths and slopes. Report your
  method, and compare like with like.
- **IC between regions.** IC depends on the units, the resolution and the weights. Compare scenarios on one map,
  not absolute values between studies.
- **Weights in the wrong range.** \( W \) is a cover factor between 0 and 1 (InVEST raises values below 0.001 to
  0.001). A weight of 1 for forest and 10 for crops is not a C factor.

## Going further

- [Part 1: Depressions, flow directions and flow accumulation](../depressions-and-flow/) and
  [part 2: Streams, watersheds and HAND](../streams-watersheds-and-hand/).
- [Part 4: Watershed simulation with SWAT+](../swat-plus/): erosion and water in tonnes and millimetres, day by
  day.
- [Terrain from an elevation model](../../surface-metrics/terrain/): slope, curvature and topographic position,
  the other terrain covariates.
- [Species distributions](../../species-distributions/): TWI and HAND as covariates.
- [Ecosystem services](../../ecosystem-services/): InVEST's sediment delivery ratio model, which uses this IC.
- The operators' reference: [[hydro.twi]], [[hydro.spi]], [[hydro.sti]], [[hydro.connectivity_index]],
  [[services.sdr]].

## References

- Beven, K. J., & Kirkby, M. J. (1979). A physically based, variable contributing area model of basin hydrology. *Hydrological Sciences Bulletin*, 24(1), 43–69. https://doi.org/10.1080/02626667909491834
- Borselli, L., Cassi, P., & Torri, D. (2008). Prolegomena to sediment and flow connectivity in the landscape: A GIS and field numerical assessment. *Catena*, 75(3), 268–277. https://doi.org/10.1016/j.catena.2008.07.006
- Cavalli, M., Trevisani, S., Comiti, F., & Marchi, L. (2013). Geomorphometric assessment of spatial sediment connectivity in small Alpine catchments. *Geomorphology*, 188, 31–41. https://doi.org/10.1016/j.geomorph.2012.05.007
- Hamel, P., Chaplin-Kramer, R., Sim, S., & Mueller, C. (2015). A new approach to modeling the sediment retention service (InVEST 3.0): Case study of the Cape Fear catchment, North Carolina, USA. *Science of the Total Environment*, 524–525, 166–177. https://doi.org/10.1016/j.scitotenv.2015.04.027
- Horn, B. K. P. (1981). Hill shading and the reflectance map. *Proceedings of the IEEE*, 69(1), 14–47. https://doi.org/10.1109/PROC.1981.11918
- Kopecký, M., Macek, M., & Wild, J. (2021). Topographic Wetness Index calculation guidelines based on measured soil moisture and plant species composition. *Science of the Total Environment*, 757, 143785. https://doi.org/10.1016/j.scitotenv.2020.143785
- Moore, I. D., & Burch, G. J. (1986). Physical basis of the length-slope factor in the Universal Soil Loss Equation. *Soil Science Society of America Journal*, 50(5), 1294–1298. https://doi.org/10.2136/sssaj1986.03615995005000050042x
- Moore, I. D., Grayson, R. B., & Ladson, A. R. (1991). Digital terrain modelling: A review of hydrological, geomorphological, and biological applications. *Hydrological Processes*, 5(1), 3–30. https://doi.org/10.1002/hyp.3360050103
- Sørensen, R., Zinko, U., & Seibert, J. (2006). On the calculation of the topographic wetness index: Evaluation of different methods based on field observations. *Hydrology and Earth System Sciences*, 10(1), 101–112. https://doi.org/10.5194/hess-10-101-2006
