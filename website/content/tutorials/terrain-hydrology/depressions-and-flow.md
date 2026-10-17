---
title: Depressions, flow directions and flow accumulation
short: Depressions and flow
description: Make an elevation model drain, give every cell the direction its water takes, and add up the area that drains through each cell, by single and multiple flow methods.
order: 10
level: intermediate
time: 45 min
ops: [raster.read, raster.reproject, hydro.fill, hydro.breach, hydro.flow_direction, hydro.flow_accumulation, map.calc]
series: Terrain hydrology
part: 1
---

In this tutorial you take a real elevation model of a forested, mountainous catchment in south-western Oregon
and ask the oldest question of terrain hydrology: where does the water go? You will remove the small
depressions that trap water in every elevation model, by filling them and by breaching them, give every cell
the direction its water flows to, and add up, for every cell, the area of the hillslopes that drain through it.
At the end you will have a map of *flow accumulation* in which the valleys and streams appear on their own, you
will know how the five flow methods of Terraflux differ and when to choose each, and you will have weighted the
accumulation to ask how much cleared land lies upstream of every reach of the streams. Parts 2 and 3 build
streams, watersheds, the height above the nearest drainage and the wetness indices on top of this model.

## The idea

A **digital elevation model** (*DEM*) is a *raster*: a grid of square cells, each holding the height of the
ground. Water on the ground runs downhill, so the heights alone say where rain that falls on a cell will go: to
a lower neighbour, then to its lower neighbour, and so on, until it leaves the map or stops in a hollow. Following
those paths cell by cell is what terrain hydrology does. O'Callaghan & Mark (1984) showed how to extract a whole
drainage network from a DEM this way, and the method is now the first step of almost every analysis of
catchments, streams, soil moisture and erosion (Moore et al., 1991).

It takes three steps:

1. **Condition the DEM.** Real elevation models contain *depressions*: cells, or groups of cells, lower than
   all their neighbours, from which water cannot leave. A few are real (a lake, a sinkhole, a quarry), but most
   are errors: noise of the sensor, a bridge or a road embankment across a valley, trees that hide the valley
   floor. Each one breaks the flow paths, so they are removed first.
2. **Give each cell a flow direction**: which neighbour (or neighbours) its water goes to.
3. **Accumulate**: for each cell, count the cells whose water passes through it, itself included. That count,
   times the area of a cell, is the cell's *contributing area* (or *catchment area*): the size of the hillslope
   and valley above it.

The figure shows the three steps on a toy valley of 5 × 5 cells that drains south, out of the bottom edge. The
cell in orange is a pit, 9 m high, lower than all its neighbours.

![A toy valley of 5 x 5 cells: elevations, D8 directions and accumulation, before and after filling](/img/tutorials/terrain-hydrology/depressions-and-flow/toy.png "Computed by Terraflux on a toy elevation model. Top: on the raw model, the pit (orange) has no lower neighbour (code 0) and swallows the water of 20 cells, even the cell below it, which flows back uphill into it; only 5 cells reach the outlet. Bottom: hydro.fill raises the pit to 11 m, the height where it spills, its water now goes south, and all 25 cells drain to the outlet at the bottom edge.")

!!! science "Key idea: D8, one direction per cell"
    In the **D8** method (O'Callaghan & Mark, 1984) each cell sends all its water to the one of its eight
    neighbours with the steepest downhill slope,

    \[ \text{slope}_k = \frac{z_0 - z_k}{d_k} \]

    where \( z_0 \) is the elevation of the cell, \( z_k \) that of neighbour \( k \), and \( d_k \) the distance
    between their centres (one cell side, or \( \sqrt{2} \) times it for a diagonal). Terraflux writes the
    direction with ESRI's codes, one power of two per neighbour: **1** east, **2** south-east, **4** south,
    **8** south-west, **16** west, **32** north-west, **64** north, **128** north-east, and **0** for a cell with
    no lower neighbour (a pit, or a cell at the edge whose water leaves the map). The *flow accumulation* of a
    cell is then

    \[ A_i = 1 + \sum_{j \to i} A_j \]

    one for the cell itself plus the accumulation of every neighbour \( j \) that drains into it: in the toy,
    the outlet's 25 is its own 1 plus the 20 of the cell above and the 2 of each of its two side neighbours.

On the toy, the pit (code 0) collects the water of 20 cells and the outlet receives only 5. One false pit can
cut a river in two. That is why every analysis starts by removing depressions, and there are two ways to do it.

**Filling** raises each depression to the height at which it would overflow, its *spill point*, as if it filled
with water. Barnes, Lehman & Mulla (2014a) gave the fastest exact method, **Priority-Flood**: it starts from the
cells of the map's edge (where water can leave), keeps the cells still to visit in a queue ordered by height, and
always takes the lowest one; any neighbour lower than the cell it is reached from is raised to that cell's
height. Every cell is visited once. Filling leaves *flat* areas where the depressions were, and a flat area has
no downhill direction; Barnes, Lehman & Mulla (2014b) also gave the method Terraflux uses to drain flats,
towards the lower terrain at their outlet and away from the higher terrain around them.

**Breaching** cuts a channel through the rim of a depression instead, down to a cell lower than its bottom, as
a river cuts through a road embankment through its culvert. Lindsay & Dhun (2015) and Lindsay (2016) chose, for
each depression, the channel of *least cost*, the one that changes the elevation least, and showed that
breaching alters the DEM less than filling and keeps the flow paths closer to the real ones in landscapes full
of roads and embankments.

The profile below follows the main stream of the study area for 3.8 km. The elevation model (black) is full of
dips: where the stream crosses them, filling (blue) draws flat lakes over them, while breaching (orange) cuts
the rims and keeps the stream close to the original ground.

![The longitudinal profile of a stream, raw, filled and breached](/img/tutorials/terrain-hydrology/depressions-and-flow/depression-profile.png "Elevation along the main stream, from 3.8 km upstream to the edge of the map, as the elevation model has it (black), filled (blue dashes, the raised water in light blue) and breached (orange dots). The largest depression, 1.8 km long, is filled up to 21.5 m. The last cells show a pitfall: near the edge of the map, breaching cut 40 m through the valley side to reach the edge.")

## When to use it

Flow directions and accumulation are rarely the end of an ecological analysis, but almost everything about
water in a landscape is built on them:

- **Streams and catchments.** Cells with a large accumulation are streams; the cells draining to a point are its
  catchment (part 2). Where no good map of small streams exists, these are often the best one available.
- **What happens upstream.** Stream ecologists relate the fish, invertebrates and water quality of a reach to
  the land use of its whole catchment, not only of its banks (Allan, 2004). A *weighted* accumulation sums any
  quantity over the catchment of every cell: the area of cleared land, of a soil type, of farms.
- **Soil moisture, wetness and erosion.** The wetness, stream power and sediment transport indices (part 3)
  combine the contributing area of each cell with its slope.
- **The routing of models.** InVEST's sediment and water models, and SWAT+ (part 4), route water and sediment
  along the same directions.

What it is *not*: a hydrological model. Flow accumulation is a property of the terrain only. It assumes that
every cell produces the same water, that water follows the surface (not the bedrock, not pipes or ditches), and
it says nothing about how much water flows or when. For volumes and timing, use a watershed model such as
SWAT+ ([part 4](../swat-plus/)).

The alternatives: for the shape of the terrain itself (slope, curvature, topographic position), see
[Terrain from an elevation model](../../surface-metrics/terrain/). For a mapped stream network you trust, you
may not need to extract one; you will still need the directions to delineate catchments.

## What you need

- **An elevation model**, as a GeoTIFF or any format GDAL reads. Here: `orelevation.tif`, the SRTM elevation
  (1 arc-second, about 30 m) of a forested area in the Oregon Coast Range foothills, the example data of
  landschaft (in Terraflux's repository, `extern/landschaft/src/landschaft/surface/data/`). Save it as
  `elevation.tif` next to your model. It is the same file as in
  [Terrain from an elevation model](../../surface-metrics/terrain/).
- **Projected coordinates**, ideally. The `hydro.*` operators take the size of the cells of a map in longitude
  and latitude in metres at its centre latitude, so they run on it, but streams snapped to outlets, the index of
  connectivity of part 3 and every distance read from the results are simpler in metres. The model therefore
  starts by reprojecting the DEM to UTM zone 10 north (EPSG:32610) in square cells of 30 m, 369 rows by 270
  columns, from 440 m to 1,388 m above sea level.
- **Optionally a land cover map** for the weighted accumulation. Here: the summer NDVI (a vegetation index,
  near 1 over dense green canopy) of the same area, `orforest.tif` in the same folder, saved as `ndvi.tif`.
- **A margin.** A catchment cut by the edge of the map is incomplete: its accumulation is too small. Read a DEM
  that covers the whole catchment of the streams you care about, with a margin.

![The elevation model of the tutorial](/img/tutorials/terrain-hydrology/depressions-and-flow/elevation.png "The elevation of the study area, 8.1 km by 11.1 km in cells of 30 m, over its hillshade. The main river leaves the map at its south-east corner.")

## Build the model

### Read, reproject and fill

<div class="steps" markdown>

1. In the operators' library, open the subject **Data preparation** (or press `/` and type *read*) and click
   [[raster.read]]. In the Node pane, set **path** to `elevation.tif` and rename the node `dem`
   (++ctrl+r++).
2. With `dem` selected, click [[raster.reproject]]: it comes connected to `dem`'s `raster` output. Set **crs**
   to `EPSG:32610`, **resolution** to `30`, **resampling** to `bilinear` and **extent** to
   `[500010, 4760850, 508110, 4771920]`. Rename it `elevation`.
3. Open the subject **Terrain hydrology** and, with `elevation` selected, click [[hydro.fill]]. Rename it
   `filled`. If its `dem` input is not connected, drag from `elevation`'s `raster` port to it.
4. Select `filled` and press ++ctrl+enter++ to run the model up to it.
5. Parts 2 and 3 start from this filled DEM. Add a [[raster.write]] (subject **IO**) after `filled`, rename it
   `save_filled`, and set **path** to `filled.tif` and **type** to `f64` (64-bit numbers, the values exactly as
   computed). Run it once.

</div>

```tfgraph
#! caption: The elevation model read, reprojected to UTM zone 10N in cells of 30 m, filled, and the filled DEM saved for parts 2 and 3.
#! highlight: filled, save_filled
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "dem"
op = "raster.read"
params = { path = "elevation.tif" }
note = "SRTM elevation, 1 arc-second."

[[nodes]]
id = "elevation"
op = "raster.reproject"
inputs = { raster = "dem.raster" }
params = { crs = "EPSG:32610", resolution = 30, extent = [500010, 4760850, 508110, 4771920], resampling = "bilinear" }
note = "Square cells of 30 m."

[[nodes]]
id = "filled"
op = "hydro.fill"
inputs = { dem = "elevation.raster" }
note = "Every depression raised to its spill point."

[[nodes]]
id = "save_filled"
op = "raster.write"
inputs = { value = "filled.map" }
params = { path = "filled.tif", type = "f64" }
```

The reprojection uses *bilinear* resampling, a weighted mean of the four nearest cells, because elevation is
continuous: the nearest-cell method would create small terraces, and each terrace edge would become a false
step (see [Terrain from an elevation model](../../surface-metrics/terrain/) for the extent and the reasons).

[[hydro.fill]] is Priority-Flood. It has one setting, **drain_flats**. Left false, a filled depression is
perfectly flat, and the directions on it are worked out later by the flat-drainage method of Barnes et al.
(2014b), which every direction and accumulation operator applies by default (their **flats** setting). Set to
true, filling raises each cell by the smallest step a number can take above the cell it drains to (the
*Priority-Flood+ε* of Barnes et al., 2014a), so the filled DEM itself slopes, imperceptibly, towards each
depression's outlet. Use it when another program, one without a flat-drainage method, will read the filled DEM.
Both give every cell a way out: on this map, directions computed from the plain filled DEM with **flats** set to
false leave 1,457 cells without a direction; with **flats** left on, or with **drain_flats** on, none.

Filling here raised 1,456 cells (1.5 % of the map), by up to 21.5 m: 224 separate depressions, 66 of them a
single cell, the largest 241 cells. On the raw DEM, 251 cells away from the edge had no lower neighbour.

??? note "Breaching instead of filling"
    Replace `filled` by [[hydro.breach]], connected to `elevation` in the same way. Its settings:

    - **max_length**: the longest channel it may cut, in cells (100 by default).
    - **max_cost**: the costliest channel it may cut, the cost being how much elevation the channel removes
      (default: any). Use it to keep breaching from cutting deep trenches through real ridges.
    - **min_dist**: count the channel's length in its cost, so that shorter channels are preferred (default
      true).
    - **flat_increment**: the drop per cell along a channel (default: from the elevations, as WhiteboxTools).
    - **fill_rest**: fill the depressions no channel within these limits could solve (default true), so that
      the result always drains.

    On this map, breaching changed 683 cells (443 lowered, 240 raised by at most 4.2 m) and moved the ground by
    1,827 m in total, against 1,456 cells and 6,183 m for filling: a much smaller change, as Lindsay (2016)
    found. Which to prefer? Breach where depressions are mostly embankments, roads and dams across valleys
    (lidar and urban DEMs, as Lindsay & Dhun, 2015); fill where they are noise in flat terrain, or when you need
    the InVEST or WhiteboxTools results that start from a filled DEM. Many analysts breach with a short
    **max_length** and fill the rest.

```tfgallery
columns: 2
caption: "What each method changed in the south-east of the map (3.9 km wide, 4.2 km tall, where the largest depressions are), over the hillshade. Filling raises whole basins along the valley floors, some by more than 10 m (darker blue); breaching lowers short channels through their rims (blue is lowered, red raised) and changes far fewer cells. The counts are for the whole map."
images:
  - { src: /img/tutorials/terrain-hydrology/depressions-and-flow/fill-depth.png, caption: "hydro.fill: 1,456 cells raised, 6,183 m in all" }
  - { src: /img/tutorials/terrain-hydrology/depressions-and-flow/breach-change.png, caption: "hydro.breach: 683 cells changed, 1,827 m in all" }
```

### Flow directions and accumulation

<div class="steps" markdown>

1. Select `filled` and click [[hydro.flow_direction]] in **Terrain hydrology**. Rename it `directions`.
2. Select `filled` again and click [[hydro.flow_accumulation]]. Rename it `accumulation`.
3. Run to `accumulation` (++ctrl+enter++). In the map under the canvas, the valleys appear as bright lines.

</div>

```tfgraph
#! caption: D8 flow directions and accumulation of the filled DEM.
#! highlight: directions, accumulation
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "dem"
op = "raster.read"
params = { path = "elevation.tif" }

[[nodes]]
id = "elevation"
op = "raster.reproject"
inputs = { raster = "dem.raster" }
params = { crs = "EPSG:32610", resolution = 30, extent = [500010, 4760850, 508110, 4771920], resampling = "bilinear" }

[[nodes]]
id = "filled"
op = "hydro.fill"
inputs = { dem = "elevation.raster" }

[[nodes]]
id = "directions"
op = "hydro.flow_direction"
inputs = { dem = "filled.map" }
params = { method = "d8" }
note = "ESRI codes: 1 E, 2 SE, 4 S ... 128 NE."

[[nodes]]
id = "accumulation"
op = "hydro.flow_accumulation"
inputs = { dem = "filled.map" }
params = { method = "d8", units = "cells" }
note = "Cells draining through each cell."
```

Notice that both operators take the **DEM**, not each other's output: [[hydro.flow_accumulation]] computes the
directions it needs itself, by its own **method**. The `directions` node is there to see and save the
directions; you need it only if another program reads them.

[[hydro.flow_direction]] has three **method**s:

- `d8` (the default): ESRI's codes, as in the key idea above.
- `dinf`: the **D-infinity** direction of Tarboton (1997), an angle in degrees clockwise from north (0 north,
  90 east), any angle, not only multiples of 45°; −1 where the cell has no downhill direction. Tarboton fitted
  planes to the eight triangles formed by a cell and each pair of adjacent neighbours and took the steepest
  downhill direction among them.
- `mfd`: the multiple flow directions of InVEST's routing (its library pygeoprocessing), each cell's share
  towards each of its eight neighbours packed into one whole number of 32 bits, 4 bits per neighbour. It is a
  format for other programs, not a map to read.

![D8 flow directions near a stream](/img/tutorials/terrain-hydrology/depressions-and-flow/d8-zoom.png "The D8 directions of 22 x 22 cells (660 m) near the middle of the map, over the hillshade. On the hillslopes, the arrows point straight downhill and converge into the valley floor; the arrows in blue drain at least 50 cells, the beginning of a stream.")

[[hydro.flow_accumulation]] has five **method**s, the core of this tutorial:

| method | water of a cell goes to | from |
|---|---|---|
| `d8` | the steepest lower neighbour, all of it | O'Callaghan & Mark (1984) |
| `dinf` | the two neighbours on either side of the D-infinity angle, split by how close the angle is to each | Tarboton (1997) |
| `fd8` | every lower neighbour, in proportion to slope to the power **exponent** (1.1) | Freeman (1991) |
| `quinn` | every lower neighbour, in proportion to slope times the length of contour it crosses (½ cell side across a side, 0.354 across a corner), slope to the power **exponent** (1) | Quinn et al. (1991) |
| `mfd` | every lower neighbour, as InVEST's sediment and water models route it | pygeoprocessing |

Single-direction D8 concentrates water in lines one cell wide, which is right for a channel but wrong on an open
hillslope, where water spreads as a sheet; on planar slopes it draws artificial parallel lines. The multiple
flow methods (`fd8`, `quinn`, `mfd`) spread water on hillslopes, and D-infinity sits between them. The
results of `d8`, `dinf` and `fd8` are WhiteboxTools' (`D8FlowAccumulation`, `DInfFlowAccumulation`,
`FD8FlowAccumulation`) cell for cell, and `mfd` is pygeoprocessing's, so that numbers can be compared with those
programs.

Two more settings:

- **units**: `cells` (the default), `area` (the cells times the area of a cell, in square map units: m² here)
  or `sca`, the *specific contributing area*, the area divided by the width of a cell (m²/m), the area that
  drains through each metre of contour. The specific contributing area is what the wetness and stream power
  indices of part 3 use. At the outlet of the study area's largest catchment: 37,974 cells, 34.18 km², and
  1,139,220 m²/m.
- **weights**: a map of what each cell contributes instead of 1 (an input port; missing cells count as 0).
  With weights the accumulation is the sum of the weights upslope, as the next step shows.

### What lies upstream: a weighted accumulation

Which reaches of the streams drain cleared land? Treat each cell of open land as contributing 1 and each forest
cell 0: the weighted accumulation is then the number of open cells upslope, and dividing it by the plain
accumulation gives the share of the catchment that is open.

<div class="steps" markdown>

1. Add a [[raster.read]] of `ndvi.tif` (rename it `ndvi_read`) and a [[raster.reproject]] after it with the same
   settings as `elevation` (rename it `ndvi`), so both maps share one grid.
2. Add a [[map.calc]] (subject **Map algebra**) named `open`; connect `ndvi`'s raster to an input named `n` and
   write `if n <= 0.55 then 1 else 0`: open land where the summer NDVI is at most 0.55, the threshold landschaft's
   watershed example uses to separate forest from pasture.
3. Add a second [[hydro.flow_accumulation]] named `open_acc`, its `dem` from `filled` and its `weights` from
   `open`.
4. Add a [[map.calc]] named `open_share` with inputs `o` (from `open_acc`) and `a` (from `accumulation`) and the
   expression `if a >= 500 then 100 * o / a else null`: the percentage of the upslope area that is open, on
   the cells draining at least 500 cells (45 ha), roughly the streams (part 2 explains that threshold).

</div>

```tfgraph
#! caption: The share of each stream's catchment that is open land, from an accumulation weighted by an open-land map.
#! highlight: open, open_acc, open_share
#! numbered: true
#! params: false
format = "terraflux/1"

[[nodes]]
id = "dem"
op = "raster.read"
params = { path = "elevation.tif" }

[[nodes]]
id = "elevation"
op = "raster.reproject"
inputs = { raster = "dem.raster" }
params = { crs = "EPSG:32610", resolution = 30, extent = [500010, 4760850, 508110, 4771920], resampling = "bilinear" }

[[nodes]]
id = "filled"
op = "hydro.fill"
inputs = { dem = "elevation.raster" }

[[nodes]]
id = "accumulation"
op = "hydro.flow_accumulation"
inputs = { dem = "filled.map" }

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
id = "open"
op = "map.calc"
inputs = { n = "ndvi.raster" }
params = { expression = "if n <= 0.55 then 1 else 0" }
note = "1 open land, 0 forest."

[[nodes]]
id = "open_acc"
op = "hydro.flow_accumulation"
inputs = { dem = "filled.map", weights = "open.map" }
note = "Open cells upslope."

[[nodes]]
id = "open_share"
op = "map.calc"
inputs = { o = "open_acc.map", a = "accumulation.map" }
params = { expression = "if a >= 500 then 100 * o / a else null" }
note = "% of the catchment that is open, on the streams."
```

The weights can be anything that adds up: the area of a land use, a runoff coefficient, the number of houses or
farms, an estimate of nitrogen applied per cell. The result is always "how much of it lies upstream".

=== "Application"

    Click a node and press ++ctrl+enter++ to run to it; its map shows under the canvas. A double click opens it
    in a tab of its own. To keep a map, use *Save output* in its tab, or add a [[raster.write]] node after it.

=== "Command line"

    Save the last model as `flow.toml` beside `elevation.tif` and `ndvi.tif`, add a [[raster.write]] node after
    each map you want on disk, and run:

    ```bash
    terraflux check flow.toml
    terraflux run flow.toml --out output
    ```

## Read the results

**The accumulation map.** Flow accumulation ranges over four orders of magnitude, from 1 on ridges to 37,974
cells at the outlet, so it is always drawn on a logarithmic scale. Drag the handle to compare it on the raw DEM
and on the filled one, with the same colours:

```tfcompare
caption: "Drag the handle: D8 flow accumulation on the raw elevation model (left) and on the filled one (right), in cells, the same logarithmic colours (dark 1, yellow 38,000). On the raw model, the depressions stop the flow: the largest accumulation is 2,847 cells, and the main valleys break into pieces. Filled, they join into one network."
before: { src: /img/tutorials/terrain-hydrology/depressions-and-flow/compare_raw.png, label: "Raw DEM" }
after: { src: /img/tutorials/terrain-hydrology/depressions-and-flow/compare_filled.png, label: "Filled DEM" }
```

What to look for:

- **A connected network.** After conditioning, every valley should carry a continuous bright line down to the
  edge of the map. A bright line that stops suddenly means a depression that was not removed, or a hole in the
  DEM (see the pitfalls).
- **Straight parallel lines on hillslopes** are an artefact of D8 on even slopes, not features of the land.
- **Where water leaves the map.** With D8, 47 cells have code 0, all on the edge: the water of the whole map
  leaves through them. The largest outlet, in the south-east corner, drains 34.2 km², 38 % of the map.

**The weighted accumulation.** Open land covers 3.8 % of this forested map, and the streams reflect it: half
the stream cells have less than 3.1 % of open land upstream, and 90 % less than 7.8 %. A few short reaches
stand out, with up to 39 % open land upstream: small streams that drain clear-cuts. In a study of stream
invertebrates or fish, this is the variable you would relate to each sampling site, and the map tells you
which reaches to sample to cover the gradient.

![The share of open land upstream of each stream cell](/img/tutorials/terrain-hydrology/depressions-and-flow/open-share.png "The percentage of the upslope area that is open land (NDVI at most 0.55), on the cells draining at least 500 cells, drawn three cells wide to be seen at this size. Most of the network drains forest; the dark reaches drain clear-cuts.")

!!! paper "In the literature"
    Report the source and resolution of the DEM, how it was conditioned (filling or breaching, with the
    settings), the flow method (and its exponent for FD8 or Quinn) and the units of the accumulation. For a
    weighted accumulation, say what the weights are and how they were made. A figure of the accumulation on a
    logarithmic scale over a hillshade lets readers judge the network.

## Settings that change the answer

The flow method changes the accumulation of every hillslope cell, and little on the large streams. Move the
slider: the five methods drawn with the same colours.

```tfslider
label: Flow method
caption: "Flow accumulation of the filled DEM by the five methods, in cells, the same logarithmic colours. D8 draws single lines everywhere; D-infinity splits water between two neighbours at most; FD8, Quinn and MFD spread it over the hillslopes and draw them smooth. The numbers: the largest catchment, the share of cells that receive less than one other cell's water (ridges and hillslope tops), and the median accumulation."
frames:
  - { value: "D8", src: /img/tutorials/terrain-hydrology/depressions-and-flow/acc_d8.png, stats: { "Largest catchment": "34.18 km²", "Receive less than a cell": "22.5%", "Median": "3.0 cells" } }
  - { value: "D-infinity", src: /img/tutorials/terrain-hydrology/depressions-and-flow/acc_dinf.png, stats: { "Largest catchment": "34.18 km²", "Receive less than a cell": "19.8%", "Median": "4.2 cells" } }
  - { value: "FD8", src: /img/tutorials/terrain-hydrology/depressions-and-flow/acc_fd8.png, stats: { "Largest catchment": "34.26 km²", "Receive less than a cell": "9.4%", "Median": "6.2 cells" } }
  - { value: "Quinn", src: /img/tutorials/terrain-hydrology/depressions-and-flow/acc_quinn.png, stats: { "Largest catchment": "34.25 km²", "Receive less than a cell": "8.7%", "Median": "6.5 cells" } }
  - { value: "MFD (InVEST)", src: /img/tutorials/terrain-hydrology/depressions-and-flow/acc_mfd.png, stats: { "Largest catchment": "34.26 km²", "Receive less than a cell": "9.1%", "Median": "6.3 cells" } }
```

The catchment of the outlet is the same within 0.3 %, whatever the method: on the large streams all methods
agree. The differences are on the hillslopes. The chart shows, for each method, the share of the map's cells
with at least a given accumulation:

```tfchart
type: line
data: /data/tutorials/terrain-hydrology/depressions-and-flow/exceedance.csv
x: cells
y: ["D8", "D-infinity", "FD8", "Quinn", "MFD (InVEST)"]
colors: { "D8": "#1c2420", "D-infinity": "#e8901f", "FD8": "#3b8a5a", "Quinn": "#a7c957", "MFD (InVEST)": "#2f6f9f" }
xlog: true
xlabel: Accumulation (cells, logarithmic)
ylabel: "% of the cells with at least this accumulation"
height: 320
caption: "With D8, 22 % of the cells have at least 10 cells upslope; with the multiple flow methods, 35 %. Beyond about 5,000 cells (the main rivers) the curves meet."
```

A close look at one hillslope makes the difference plain:

```tfgallery
columns: 2
caption: "The same hillslopes and valley, 3 km by 2.4 km, by four methods (the same colours as above). D8 draws parallel lines down every slope; D-infinity fewer; FD8 and MFD spread the water and leave single lines only in the valley floors."
images:
  - { src: /img/tutorials/terrain-hydrology/depressions-and-flow/zoom_d8.png, caption: "D8" }
  - { src: /img/tutorials/terrain-hydrology/depressions-and-flow/zoom_dinf.png, caption: "D-infinity" }
  - { src: /img/tutorials/terrain-hydrology/depressions-and-flow/zoom_fd8.png, caption: "FD8" }
  - { src: /img/tutorials/terrain-hydrology/depressions-and-flow/zoom_mfd.png, caption: "MFD (InVEST)" }
```

Which method to choose:

- **D8** for anything that needs one path per cell: streams, stream links, watersheds and HAND (part 2) all use
  D8 directions in Terraflux. It is also the most common method in the literature, which helps comparison.
- **FD8, Quinn or MFD** for indices of hillslope wetness and erosion (part 3), where the spreading of water on
  slopes matters. In temperate forest plots, a wetness index computed with D8 explained less than half of the
  variation in measured soil moisture and plant species composition that the same index computed with FD8
  explained (Kopecký et al., 2021); Sørensen et al. (2006) found that the best method varied with the variable
  and the site. Part 3 returns to this.
- **D-infinity** as a compromise: it spreads water, but only between two neighbours, so it disperses less than
  FD8.
- **MFD** when your results must match InVEST's sediment and water models.

The **exponent** of FD8 (1.1 by default, Freeman's value) controls the spreading: larger values send more water
to the steepest neighbour and, as they grow, approach D8.

## Common pitfalls

- **Skipping the conditioning.** Directions and accumulation on a raw DEM stop at every depression (the
  comparison above). Always fill or breach first; every `hydro.*` operator says "best with depressions filled"
  in its description of the DEM.
- **Holes in the DEM.** Cells without a value are treated as places where water leaves the map, as the edge is:
  water flows into a hole and disappears. Fill small holes first ([[raster.fill]]), or mask large water bodies
  on purpose.
- **Catchments cut by the edge.** If the DEM ends before the watershed does, the accumulation of every cell
  below the cut is too small, and breaching may cut channels to the edge: here a depression near the east edge
  was breached by a 40 m deep cut through the valley side, straight to the edge. Read a DEM larger than the
  catchments you study, and cut the results afterwards.
- **Real depressions.** Lakes, sinkholes, karst and prairie potholes really do hold water. Filling them all
  connects them to streams that do not exist. Mask them or treat them apart.
- **Roads and dams.** An embankment across a valley is a dam in the DEM. Filling makes a lake behind it;
  breaching cuts through it, which is usually what the culvert does in reality (Lindsay & Dhun, 2015).
- **Surface models.** In forests, radar DEMs such as SRTM partly measure the top of the canopy. Clearings
  become pits, forest edges become walls, and streams under the canopy are hard to see. Prefer a bare-earth DEM
  (lidar) for small streams.
- **Units.** `cells` and `area` differ by the area of a cell (900 m² here); `sca` divides by its width as well.
  Say which you report, and do not compare counts of cells between DEMs of different resolutions.
- **Resolution.** A DEM of 30 m cannot see a ditch 2 m wide, and one of 1 m sees every wheel rut. Choose the
  resolution for the flow paths you care about.

## Going further

- [Part 2: Streams, watersheds and HAND](../streams-watersheds-and-hand/): turn the accumulation into a stream
  network, split it into links with their Strahler order, delineate the watershed of a gauge and measure the
  height of every cell above the nearest stream.
- [Part 3: Wetness, stream power and sediment connectivity](../wetness-and-connectivity/): the topographic
  wetness index and its relatives, and the index of connectivity of InVEST's sediment model.
- [Part 4: Watershed simulation with SWAT+](../swat-plus/): from the terrain to daily water and sediment
  budgets.
- [Terrain from an elevation model](../../surface-metrics/terrain/): slope, aspect, curvature and topographic
  position.
- The operators' reference: [[hydro.fill]], [[hydro.breach]], [[hydro.flow_direction]],
  [[hydro.flow_accumulation]].

## References

- Allan, J. D. (2004). Landscapes and riverscapes: The influence of land use on stream ecosystems. *Annual Review of Ecology, Evolution, and Systematics*, 35, 257–284. https://doi.org/10.1146/annurev.ecolsys.35.120202.110122
- Barnes, R., Lehman, C., & Mulla, D. (2014a). Priority-flood: An optimal depression-filling and watershed-labeling algorithm for digital elevation models. *Computers & Geosciences*, 62, 117–127. https://doi.org/10.1016/j.cageo.2013.04.024
- Barnes, R., Lehman, C., & Mulla, D. (2014b). An efficient assignment of drainage direction over flat surfaces in raster digital elevation models. *Computers & Geosciences*, 62, 128–135. https://doi.org/10.1016/j.cageo.2013.01.009
- Freeman, T. G. (1991). Calculating catchment area with divergent flow based on a regular grid. *Computers & Geosciences*, 17(3), 413–422. https://doi.org/10.1016/0098-3004(91)90048-I
- Kopecký, M., Macek, M., & Wild, J. (2021). Topographic Wetness Index calculation guidelines based on measured soil moisture and plant species composition. *Science of the Total Environment*, 757, 143785. https://doi.org/10.1016/j.scitotenv.2020.143785
- Lindsay, J. B. (2016). Efficient hybrid breaching-filling sink removal methods for flow path enforcement in digital elevation models. *Hydrological Processes*, 30(6), 846–857. https://doi.org/10.1002/hyp.10648
- Lindsay, J. B., & Dhun, K. (2015). Modelling surface drainage patterns in altered landscapes using LiDAR. *International Journal of Geographical Information Science*, 29(3), 397–411. https://doi.org/10.1080/13658816.2014.975715
- Moore, I. D., Grayson, R. B., & Ladson, A. R. (1991). Digital terrain modelling: A review of hydrological, geomorphological, and biological applications. *Hydrological Processes*, 5(1), 3–30. https://doi.org/10.1002/hyp.3360050103
- O'Callaghan, J. F., & Mark, D. M. (1984). The extraction of drainage networks from digital elevation data. *Computer Vision, Graphics, and Image Processing*, 28(3), 323–344. https://doi.org/10.1016/S0734-189X(84)80011-0
- Quinn, P., Beven, K., Chevallier, P., & Planchon, O. (1991). The prediction of hillslope flow paths for distributed hydrological modelling using digital terrain models. *Hydrological Processes*, 5(1), 59–79. https://doi.org/10.1002/hyp.3360050106
- Sørensen, R., Zinko, U., & Seibert, J. (2006). On the calculation of the topographic wetness index: Evaluation of different methods based on field observations. *Hydrology and Earth System Sciences*, 10(1), 101–112. https://doi.org/10.5194/hess-10-101-2006
- Tarboton, D. G. (1997). A new method for the determination of flow directions and upslope areas in grid digital elevation models. *Water Resources Research*, 33(2), 309–319. https://doi.org/10.1029/96WR03137
