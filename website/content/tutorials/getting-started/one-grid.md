---
title: One grid for all your maps
short: One grid
description: Resolution and alignment, putting rasters on one reference grid with raster.match, coarsening and refining them with raster.resample, cutting and joining them, and the rules of aggregation for classes, values and counts.
order: 50
level: beginner
time: 40 min
ops: [raster.resample, raster.match, raster.clip, raster.mosaic, raster.reproject, raster.convert]
---

A land change model, a species distribution model or a prioritization reads several maps at once and combines
them **cell by cell**: the land use of a cell, its slope, its distance to a road. That only works if every map has
the same cells: the same coordinate system, the same cell size, the same extent and cells that start at the same
place. In this tutorial you will put maps on one grid, coarsen a land cover map from 25 m to 500 m and watch what
happens to its classes, learn the three rules of aggregation (majority, mean and sum), cut a map and join tiles,
and meet the **Prepare** pane, which does most of this in a click.

## The idea: a reference grid

Two rasters are on the **same grid** when four things are equal:

1. the **coordinate reference system** (see [Coordinate systems and reprojection](../coordinate-systems/));
2. the **cell size** (resolution);
3. the **alignment**: the corners of the cells fall on the same coordinates (the origin is a whole number of cells
   away);
4. the **extent**: the same rows and columns.

![Two grids that do not line up](/img/tutorials/getting-started/one-grid/misaligned.png "Two grids over the same place: a land cover map of 30 m cells (green) and a driver of 37.5 m cells, shifted (orange, dashed). No cell of one corresponds to a cell of the other: before they can be combined, one must be resampled onto the other.")

In practice, a model chooses one map as its **reference grid**, usually the land use or land cover map (the map
the model predicts or analyses), and puts every other map on it. Moving a raster onto another grid is
**resampling**: each new cell takes its value from the old cells under it, by a rule that must suit the data.

Changing the grid also changes what the map says. Ecologists have known since the 1980s that landscape pattern
depends on the **grain** (the cell size) and the **extent** of the map: as cells grow, rare and narrow classes
disappear, dominant ones grow, and patch metrics change in ways that can be predicted for some metrics and not
for others (Turner et al., 1989; Wu, 2004). Choosing a resolution is therefore an ecological decision: the grain
should match the scale at which the organism or the process responds, not the resolution of the first file you
downloaded.

!!! science "Key idea: grain and extent"
    The **grain** is the size of the smallest unit of a map (its cell); the **extent** is the area it covers. A
    map can only show patterns between the two: nothing smaller than a cell, nothing larger than the map.
    Resampling to larger cells (coarsening, or *aggregating*) loses detail for good; resampling to smaller cells
    (refining, or *disaggregating*) adds cells but no information.

## When to use it

- **Before combining maps**: drivers of land change, predictors of a species distribution model, features of a
  prioritization. Always.
- **To match the scale of the question**: a land cover of 10 m for a beetle, of 1 km for a wolf.
- **To make a problem smaller**: a prioritization or a circuit analysis on 100 million cells may need coarser
  cells (but see what coarsening does, below).
- **To cut** a study area out of a large map, or **join** tiles into one.

## What you need

The maps, and a decision: which grid is the reference? Use the map whose classes or values matter most for the
question, at the finest resolution you can afford. This tutorial uses the 15 x 15 km window of CORINE Land Cover
around Hanover (600 x 600 cells of 25 m, EPSG:3035) from [Your first raster](../first-raster/).

## Coarsen a map: raster.resample

[[raster.resample]] changes the cell size of rasters **in their own CRS** (with GDAL's `gdalwarp`). Its settings
are `resolution` (the new cell side in the map's units, or `[x, y]`), `resampling` and `path`. With
`resampling = "auto"` (the default), the method follows the data: for classes the nearest cell, or the **most
common** one (`mode`) when the cells grow; for values bilinear, or the **mean** (`average`) when they grow.

```tfgraph
#! caption: Coarsen the land cover from 25 m to 250 m: the most common class of each block of 10 x 10 cells.
#! highlight: coarse
format = "terraflux/1"

[[nodes]]
id = "landcover"
op = "raster.read"
params = { path = "window.tif" }
note = "CORINE around Hanover: 600 x 600 cells of 25 m."

[[nodes]]
id = "coarse"
op = "raster.resample"
inputs = { raster = "landcover.raster" }
params = { resolution = 250, resampling = "mode", path = "landcover_250m.tif" }
note = "Classes on larger cells: the majority (mode). auto would choose the same."
```

Move the slider to coarsen the map from 25 m to 500 m. Each step was a run of [[raster.resample]] with `mode`;
the chart shows each class's share **relative to its share at 25 m** (100 = unchanged):

```tfslider
label: Cell size
caption: "The land cover coarsened by majority. Arable land, the dominant class, grows (50.2% to 53.0%); broad-leaved forest, in small patches, loses a quarter of its area; the river, two cells wide, keeps only a fifth of its cells at 500 m. At 500 m one class has vanished altogether."
frames:
  - { value: "25 m", src: /img/tutorials/getting-started/one-grid/mode_25.png, caption: "25 m: 600 x 600 cells (the original)", stats: { "Arable": "50.2%", "Coniferous forest": "19.2%", "Broad-leaved forest": "2.75%", "River": "0.58%", "Classes": "15" } }
  - { value: "50 m", src: /img/tutorials/getting-started/one-grid/mode_50.png, caption: "50 m: 300 x 300 cells", stats: { "Arable": "50.2%", "Coniferous forest": "19.2%", "Broad-leaved forest": "2.73%", "River": "0.59%", "Classes": "15" } }
  - { value: "100 m", src: /img/tutorials/getting-started/one-grid/mode_100.png, caption: "100 m: 150 x 150 cells", stats: { "Arable": "50.3%", "Coniferous forest": "19.2%", "Broad-leaved forest": "2.71%", "River": "0.57%", "Classes": "15" } }
  - { value: "250 m", src: /img/tutorials/getting-started/one-grid/mode_250.png, caption: "250 m: 60 x 60 cells", stats: { "Arable": "51.4%", "Coniferous forest": "19.3%", "Broad-leaved forest": "2.72%", "River": "0.28%", "Classes": "15" } }
  - { value: "500 m", src: /img/tutorials/getting-started/one-grid/mode_500.png, caption: "500 m: 30 x 30 cells", stats: { "Arable": "53.0%", "Coniferous forest": "19.8%", "Broad-leaved forest": "2.00%", "River": "0.11%", "Classes": "14" } }
chart:
  type: line
  data: /data/tutorials/getting-started/one-grid/shares_relative.csv
  x: cell
  y: [arable, coniferous, pastures, urban, broadleaved, river]
  labels: { arable: "Arable land", coniferous: "Coniferous forest", pastures: "Pastures", urban: "Discontinuous urban", broadleaved: "Broad-leaved forest", river: "Water courses" }
  colors: { arable: "#d9a441", coniferous: "#3b8a5a", pastures: "#a7c957", urban: "#b4508a", broadleaved: "#80c040", river: "#2f6f9f" }
  xlabel: Cell size (m)
  ylabel: Share, % of its share at 25 m
```

Two things stand out. Up to 100 m (blocks of 4 x 4 cells), the shares barely move: most patches here are much
larger than 100 m. Beyond that, the **majority rule** rewards what is common and wide and punishes what is rare
and narrow: the river, a line two or three cells wide, is outvoted in almost every block. The total of the shares
is still 100%, so what the river and the small woods lose, arable land gains.

!!! tip "Majority keeps the map readable, nearest keeps the shares"
    `near` takes one cell (the one at the centre of each block) and ignores the rest. Its shares are unbiased on
    average: at 500 m the river still covers 0.56% of the cells, but as scattered dots where the sample happened to
    hit it. `mode` gives a cleaner map with biased shares. For the area of each class, compute it on the fine
    map; for a coarse map to model with, use `mode`, and report the shares at both resolutions.

## The three rules of aggregation

When cells grow, every new cell covers many old ones and must summarise them. The right summary depends on what
the values are, and Terraflux's **Prepare** pane asks exactly this: are the values *classes*, *values* or
*counts*? Here is a forest map (1 forest, 0 not, at 25 m) aggregated to 100 m, blocks of 4 x 4 = 16 cells, by
the three rules:

```tfgallery
columns: 3
caption: "The same forest map at 100 m by three rules. Majority: forest where at least half the block is forest, 24.7% of the cells (24.8% at 25 m). Mean: the share of forest in each block, 0 to 1. Sum: the number of forest cells in each block, 0 to 16; the total, 89,231 cells, is exactly the total at 25 m."
images:
  - { src: /img/tutorials/getting-started/one-grid/agg_mode.png, caption: "Classes: mode (majority)" }
  - { src: /img/tutorials/getting-started/one-grid/agg_average.png, caption: "Values: average (mean)" }
  - { src: /img/tutorials/getting-started/one-grid/agg_sum.png, caption: "Counts: sum" }
```

| The values are | Finer or equal cells | Larger cells | Why |
|---|---|---|---|
| **classes** (land cover, soil types) | `near` | `mode` | codes cannot be averaged; each cell keeps a class that is there |
| **values** (elevation, temperature, a share, a probability) | `bilinear` | `average` | measurements vary smoothly; the mean of a block is its best summary |
| **counts** (people, individuals, cells of forest) | `sum` (of the parts) | `sum` | the total must be kept: 10 people in a block are 10 people in its cell |

!!! warning "Average a map of whole numbers? Make it decimal first"
    The mean of the forest map above was computed on a copy written as `float32`. GDAL writes the result in the
    **type of the input**, so averaging the original `uint8` map gave 0s and 1s again (the mean rounded to the
    nearest whole number), not shares. Before `average` or `bilinear` on a map stored as whole numbers, write it
    as decimals ([[raster.write]] with `type = "f32"`, or [[raster.convert]] with `type = "Float32"`).
    The same rule decides what `auto` does: Terraflux's `auto` treats a raster stored as **whole numbers** as
    classes and one stored as **decimals** as values. An elevation model stored as `int16`, as SRTM often is,
    would be resampled as classes (nearest or mode); give `resampling = "bilinear"` (or `average`) yourself.

## Put a map on another's grid: raster.match

[[raster.resample]] keeps the CRS and the origin; it only changes the cell size. To put a map **on the grid of
another**, with its CRS, cell size, alignment and extent in one step, use [[raster.match]] (landschaft's
`match_grid`, by `gdalwarp`). Its inputs are `raster` (the rasters to move) and `like` (the raster whose grid to
take: only its header is read). Its `resampling` follows the same `auto` rule.

As an example, here is a coarse "product" of the kind you might download: the share of forest in cells of
0.006° (about 400 x 670 m), in longitude and latitude (EPSG:4326). [[raster.match]] puts it on the land cover's
grid, EPSG:3035 with 25 m cells, by bilinear interpolation:

```tfgraph
#! caption: Put a coarse map in degrees on the land cover's grid: its CRS, cells and extent.
#! highlight: matched
format = "terraflux/1"

[[nodes]]
id = "landcover"
op = "raster.read"
params = { path = "window.tif" }
note = "The reference grid: EPSG:3035, 600 x 600 cells of 25 m."

[[nodes]]
id = "product"
op = "raster.read"
params = { path = "forest_share_degrees.tif" }
note = "A share of forest (0 to 1, float32) in EPSG:4326: 37 x 23 cells of 0.006 degrees."

[[nodes]]
id = "matched"
op = "raster.match"
inputs = { raster = "product.raster", like = "landcover.raster" }
params = { path = "forest_share_matched.tif" }
note = "Decimal values on finer cells: bilinear (auto)."

[[nodes]]
id = "drivers"
op = "stack.of"
inputs = { landcover = "landcover.raster", forest_share = "matched.raster" }
note = "Now on one grid, the two maps can form a stack."
```

```tfgallery
columns: 2
caption: "Left: the product in its own grid, 37 x 23 cells of 0.006 degrees in EPSG:4326. Right: the same values on the reference grid, 600 x 600 cells of 25 m in EPSG:3035, ready to be combined with the land cover cell by cell. Matching to finer cells adds cells, not detail."
images:
  - { src: /img/tutorials/getting-started/one-grid/product_degrees.png, caption: "Before: EPSG:4326, 0.006°" }
  - { src: /img/tutorials/getting-started/one-grid/product_matched.png, caption: "After raster.match: EPSG:3035, 25 m" }
```

The last node, [[stack.of]], gathers maps of one grid into a stack (the drivers of a land change model, the
features of a prioritization); it would refuse maps of different grids.

!!! app "In the application: Prepare and the reference grid"
    The **Prepare** pane (a tab beside the Node pane, on the left) does this for you.

    1. **Reference grid**: when the model reads a map of classes, Prepare suggests it (**Use landcover
       (classes)**); **Use the node selected** makes any raster the reference. It then shows its grid in one line:
       coordinate system, cell size, and columns x rows.
    2. Select any other node that gives rasters. Prepare shows its grid and what differs from the reference
       (*another coordinate system*, *larger cells* or *smaller cells*, *cells shifted from the grid's*, *another
       extent*), or *on the reference grid*.
    3. Check **Its values are**: *classes*, *values* or *counts*. Prepare guesses; the choice sets the resampling
       (near or mode, bilinear or average, sum).
    4. **Put on the reference grid** adds a [[raster.match]] node with that resampling and a note saying why.
       **Every raster > Put the N other rasters off the grid on it** does it for all of them at once.

## Cut and join: raster.clip and raster.mosaic

[[raster.clip]] cuts rasters to an `extent`, `[xmin, ymin, xmax, ymax]` in their own coordinates, keeping their
cells as they are (GDAL's `gdal_translate`). The window used throughout these tutorials was cut this way from the
50 x 50 km CORINE excerpt:

```tfgraph
#! caption: Cut a 15 x 15 km window out of the CORINE excerpt.
#! highlight: window
format = "terraflux/1"

[[nodes]]
id = "landcover"
op = "raster.read"
params = { path = "clc.tif" }

[[nodes]]
id = "window"
op = "raster.clip"
inputs = { raster = "landcover.raster" }
params = { extent = [4261250, 3267500, 4276250, 3282500], path = "window.tif" }
note = "x from 4,261,250 to 4,276,250 and y from 3,267,500 to 3,282,500, in EPSG:3035 metres."
```

![The window on the whole map](/img/tutorials/getting-started/one-grid/clip_box.png "The 15 x 15 km window (black box) in the north-west corner of the 50 x 50 km CORINE excerpt.")

Large datasets often come as **tiles**: a file per square of the map. [[raster.mosaic]] joins the files of a series
into one raster (GDAL's `gdalwarp`), in the CRS of the first file unless `crs` says otherwise, with an optional
`resolution` and `resampling`; where tiles overlap, the cells of the last win. Give [[raster.read]] the folder or a
pattern (`tiles/*.tif`) and connect its `raster` output:

```tfgraph
#! caption: Every file of a folder joined into one raster.
#! highlight: mosaic
format = "terraflux/1"

[[nodes]]
id = "tiles"
op = "raster.read"
params = { path = "tiles/*.tif" }
note = "Four tiles of 300 x 300 cells: nw, ne, sw, se."

[[nodes]]
id = "mosaic"
op = "raster.mosaic"
inputs = { raster = "tiles.raster" }
params = { path = "mosaic.tif" }
```

```tfgallery
columns: 2
caption: "Left: four tiles cut from the window, four files in a folder. Right: their mosaic, 600 x 600 cells, identical cell for cell to the window they were cut from."
images:
  - { src: /img/tutorials/getting-started/one-grid/tiles.png, caption: "tiles/*.tif" }
  - { src: /img/tutorials/getting-started/one-grid/mosaic.png, caption: "raster.mosaic" }
```

## From the command line: terraflux resample

For a quick job outside a model, the command `terraflux resample` puts a raster on the grid of another **of the
same CRS** (for another CRS, use [[raster.match]] or [[raster.reproject]] in a model):

```bash
terraflux resample forest_average.tif --like window.tif -o forest_share_25m.tif
# wrote forest_share_25m.tif ([Bilinear])
```

Its `--method` is `auto` (classes: nearest or mode; continuous: bilinear or average), `nearest`, `mode`,
`bilinear` or `average`. It tells you which it used. Given rasters of two CRSs it stops with
`the grids have different coordinate reference systems ... reproject first`.

```tfcli
resample
```

## Common pitfalls

- **Combining maps of different grids.** Terraflux checks it: [[stack.of]] refuses them and Prepare shows
  *next to the grid*. Put every map on the reference grid first.
- **Classes resampled as values.** Bilinear on a land cover invents classes along every edge (see
  [Coordinate systems](../coordinate-systems/)). Check **Its values are** in Prepare.
- **Values stored as whole numbers.** `auto` treats them as classes, and `average` returns whole numbers. Convert
  to `Float32` first.
- **Counts averaged.** A population map averaged to larger cells loses people; use `sum`.
- **Refining and believing.** Resampling 1 km cells to 25 m makes 1,600 identical cells per original cell, not
  25 m information. Report the original resolution of every input.
- **Edges of the extent.** A cell size that does not divide the extent leaves a partial row and column at the
  edges, with no value or with 0. Choose cell sizes that divide the extent (here 25, 50, 100, 250 and 500 m divide
  15 km).
- **Reprojecting and resampling twice.** Each step degrades the map a little. Use [[raster.match]] once, from the
  original file, instead of a reprojection followed by a resampling.

## Going further

- [Coordinate systems and reprojection](../coordinate-systems/) and [Your first raster](../first-raster/).
- [Reclassify, calculate and save](../reclassify-calculate-save/): rules, map algebra and file types.
- The operators: [[raster.resample]], [[raster.match]], [[raster.clip]], [[raster.mosaic]], [[raster.reproject]],
  [[stack.of]].

## References

- Turner, M. G., O'Neill, R. V., Gardner, R. H., & Milne, B. T. (1989). Effects of changing spatial scale on the analysis of landscape pattern. *Landscape Ecology*, 3(3–4), 153–162. https://doi.org/10.1007/BF00131534
- Wu, J. (2004). Effects of changing scale on landscape pattern analysis: scaling relations. *Landscape Ecology*, 19(2), 125–138. https://doi.org/10.1023/B:LAND.0000021711.40074.ae
