---
title: Rasters larger than memory
short: Larger than memory
description: How Terraflux shows, measures and analyses rasters too large for your computer's memory - reading by strips of rows, strips with a halo, overlapping tiles and the tiled simulation engine - with the memory measured and the results compared to the whole map.
order: 10
level: intermediate
time: 40 min
ops: [raster.read, metrics.landscape, metrics.window, landscape.mspa_tiled, landscape.mspa, vector.zonal, vector.rasterize]
---

A land cover map of a country at 30 m has billions of cells; a continent at 10 m, tens of billions. Such maps do not
fit in a laptop's memory, and yet you may need their landscape metrics, their MSPA or a land change simulation over
them. In this tutorial you will learn how Terraflux deals with maps of any size: what happens when you open one, which
operators read them **by parts**, how each of them splits the map, and what that costs in exactness. You will measure
the landscape metrics of a map of 64 million cells with 1.3 GB of memory and with 90 MB, get the same numbers, and see
where MSPA by tiles differs from MSPA of the whole map and how to make it not differ.

## The idea: never hold the whole map

A raster's size in memory is its number of cells times the bytes per cell. A map of 40,000 x 40,000 cells (a region
of 1,200 km at 30 m) has 1.6 billion cells: 1.6 GB as bytes, but 6.4 GB as the 32-bit numbers most calculations use,
and an analysis often needs several copies (the map, a map of patches, the result). Most desktop software loads the
whole map and fails, or makes the computer swap to disk until it is useless.

The way around it is old: process the map **piece by piece**, keeping only one piece in memory at a time, and combine
the pieces' results. Geographers called it *tiling* or *out-of-core* computation; GIS software and remote-sensing
platforms rely on it (Gorelick et al., 2017). The difficulty is in the combining. Some results add up exactly over
pieces; others depend on cells far away, and need the pieces to overlap.

!!! science "Key idea: three kinds of calculations"
    - **Local** (each cell from itself: a reclassification, map algebra without neighbours): any split works.
    - **Neighbourhood of fixed size** (each cell from the cells within \(r\) cells: a moving window, a slope): split
      into strips, and read each with a **halo** of \(r\) rows above and below. The result is exact.
    - **Global** (a cell may depend on any other: a patch that crosses the map, MSPA's classes, the area of the
      largest patch): only sums of the pieces' parts are exact; the rest needs overlapping tiles and is exact only
      when the overlap is larger than the objects.

![Three ways to process a large map](/img/tutorials/large-data/larger-than-memory/strategies.png "Three strategies. 1: strips of rows, one in memory at a time, their sums added up (landscape metrics, zonal statistics). 2: strips with a halo of half a window above and below, so that every window sees all its cells (moving windows). 3: overlapping tiles, each computed alone and, in each overlap, the half nearer to each tile kept (GuidosToolbox's MSPA tiling).")

## When to use it

- **Maps of more than a few hundred million cells**, or any map that makes your computer run out of memory.
- **Many large maps on a computer with little memory**, or several analyses running at once.
- Not needed for maps that fit: everything below gives the same numbers in memory, and in memory is faster.

## What you need

- A large raster as a GeoTIFF on disk. Terraflux reads any format GDAL reads, but for large maps, a **tiled,
  compressed GeoTIFF** or a **cloud-optimised GeoTIFF** (COG) is best: it can be read a window at a time.
  [[raster.convert]] makes one (see [Reclassify, calculate and save](../../getting-started/reclassify-calculate-save/#rasterconvert-other-formats-cloud-optimised-geotiffs)).
- For this tutorial, `big.tif`: the CORINE Land Cover excerpt of the getting-started tutorials (2000 x 2000 cells
  of 25 m) repeated 4 x 4 times, mirrored every other time, into a map of 8000 x 8000 cells (64 million cells,
  200 x 200 km). Its land cover is not real beyond the first block, but its size is. Its figure script is
  `website/figures/large-data/larger-than-memory.py`.

## Opening a large raster

Opening a raster does not read its cells. [[raster.read]] reads only the file's **header** (its size, cell size,
coordinates, bands), whatever the file's size. The Viewer then reads only the tiles **in view**, at the screen's
resolution, in threads, showing a coarser level while it waits: a raster of billions of cells appears in under a
second and zooms smoothly. Nothing is loaded whole until a node needs it.

What a node needs depends on which of [[raster.read]]'s three outputs it is connected to:

| Output | What the next node gets | Memory |
|---|---|---|
| `raster` | the file on disk | none: the node reads what it needs, when it needs it |
| `map` | one band, loaded whole | the whole band |
| `stack` | every band, loaded whole | every band |

So the first rule for large maps: **connect `raster`**, not `map`, to the operators that can work on files. Those
that cannot take a file load it whole anyway (a `raster` connected to a `map` input is read when the node runs).

## Operators that work by parts

| Operator | How | Exact? |
|---|---|---|
| [[raster.reproject]], [[raster.resample]], [[raster.match]], [[raster.clip]], [[raster.convert]], [[raster.mosaic]], [[raster.terrain]], [[raster.fill]], [[distance.proximity]] | by GDAL, on the files, window by window | yes |
| [[metrics.landscape]], [[metrics.class]], [[metrics.patch]] | strips of rows (`block_rows`), the patches joined across strips | yes: the same metrics |
| [[metrics.window]] | strips with a halo of half the window's side; the maps written to a GeoTIFF, a band per metric | yes |
| [[metrics.sample]], [[metrics.series]], [[metrics.signatures]] | window by window (each sample, zone or tile read alone) | yes |
| [[vector.zonal]], [[vector.extract]], [[vector.exact_sum]] | strips of rows, or the windows under each polygon or point | yes |
| [[vector.rasterize]], [[vector.distance]] | burnt or measured strip by strip into a GeoTIFF | yes |
| [[landscape.mspa_tiled]] | GuidosToolbox's overlapping tiles | only where objects are smaller than the overlap |
| Dinamica EGO's model (`terraflux run --tiled`) | tiles with halos, on one computer or many | statistically equivalent |

For the operators with `block_rows`, there are two ways to work by parts:

1. **Give `block_rows`**, the number of rows per strip (a multiple of 64 for the landscape metrics).
2. **Do nothing** when the raster on disk has **more than 250 million cells**: those are always read by blocks.

## Landscape metrics by strips: the same numbers, a tenth of the memory

```tfgraph
#! caption: Landscape metrics of a large map read by strips of 512 rows: the raster on disk (output raster, not map) goes to metrics.landscape with block_rows.
#! highlight: metrics
#! numbered: true
format = "terraflux/1"
name = "Landscape metrics of a large map"

[[nodes]]
id = "landcover"
op = "raster.read"
params = { path = "big.tif" }

[[nodes]]
id = "metrics"
op = "metrics.landscape"
inputs = { map = "landcover.raster" }
params = { metrics = ["ta", "np", "lpi", "ed", "area_mn", "shdi", "contag", "ai"], block_rows = 512 }

[outputs.metrics]
from = "metrics.table"
```

The same model was run three times on `big.tif`: once with `landcover.raster` and no `block_rows` (the whole map in
memory), and twice by strips of 512 and of 128 rows. The figure script measured the memory the `terraflux` process
used at its peak:

```tfchart
type: bar
data: /data/tutorials/large-data/larger-than-memory/memory.csv
x: run
y: [peak_mib]
labels: { peak_mib: "Peak memory (MiB)" }
colors: { peak_mib: "#2f6f9f" }
ylabel: MiB
height: 260
caption: "The peak memory of terraflux run for the landscape metrics of 64 million cells: 1,261 MiB with the whole map in memory, 128 MiB by strips of 512 rows, 88 MiB by strips of 128 rows. The runs took 2.6, 3.7 and 3.9 seconds."
```

And the metrics were **identical to the last digit** in the three runs:

| ta (ha) | np | lpi (%) | ed (m/ha) | area_mn (ha) | shdi | contag (%) | ai (%) |
|---|---|---|---|---|---|---|---|
| 4,000,000 | 14,798 | 10.62965 | 23.326875 | 270.3068 | 1.8096 | 69.4497 | 97.1111 |

How can the number of patches be exact when a patch crosses the border of a strip? The patches of each strip are
labelled, and the labels of patches touching across a border are joined (*connected component labelling* by blocks),
so a patch split by strips is still counted once, with its whole area and edge. Memory grows with the width of the
map and the number of rows per strip, not with the number of rows of the map: a map twice as tall uses the same
memory, and takes twice as long.

The memory saved is roughly the map's size in the calculation's working form: here 64 million cells needed over a
gigabyte in memory. A map of 1.6 billion cells would need some 30 GB whole, and by strips, about what a strip needs.

!!! note "Class and patch metrics out of core"
    By strips, [[metrics.class]] and [[metrics.patch]] give their tables (the same values as in memory) but no
    maps: their `map` output is made only for a map in memory, with `show`. [[metrics.landscape]] gives the whole
    landscape's metrics the same way.

### Landscape metrics by strips in Python

The library reads by strips too, with the same `block_rows`: [`tf.Landscape`][terraflux.Landscape] (and
[`tf.landscape_metrics`][terraflux.landscape_metrics]) given a **file path** and `block_rows` never reads the whole
raster. The part's `big.tif` is made here in numpy, the same way as the figure script (the CORINE excerpt of the
example data repeated 4 x 4 times, mirrored every other time), and written once:

```python
import numpy as np
import xarray as xr
import terraflux as tf

clc = tf.load_raster(tf.example_data("clc"))          # 2000 x 2000 cells of 25 m
a = clc.values
rows = [np.hstack([a if (i + j) % 2 == 0 else a[:, ::-1] for j in range(4)]) for i in range(4)]
cells = np.vstack(rows)                                # 8000 x 8000
y = clc.y[0].item() - 25 * np.arange(cells.shape[0])   # the same corner and cell size, a bigger grid
x = clc.x[0].item() + 25 * np.arange(cells.shape[1])
big = xr.DataArray(cells, dims=("y", "x"), coords={"y": y, "x": x}).rio.write_crs(clc.rio.crs)
tf.write_raster(big, "big.tif")                        # 64 million cells, tiled and compressed
del big, rows, cells
```

```python
keys = ["ta", "np", "lpi", "ed", "area_mn", "shdi", "contag", "ai"]   # the graph's metrics
strips = tf.Landscape("big.tif", block_rows=512)       # a path and block_rows: read by strips of 512 rows
print(strips.out_of_core)
metrics512 = strips.landscape_metrics(keys)
metrics128 = tf.Landscape("big.tif", block_rows=128).landscape_metrics(keys)
print(metrics512.round(4).to_string(index=False))
print("identical with strips of 128 rows:", metrics512.equals(metrics128))
```

```text
True
       ta      np     lpi      ed  area_mn   shdi  contag      ai
4000000.0 14798.0 10.6296 23.3269 270.3068 1.8096 69.4497 97.1111
identical with strips of 128 rows: True
```

These are the part's numbers. Strips of 128 rows give the same values to the last digit, with less memory (the memory
is the figure script's measure of the `terraflux` program, not of this block). `block_rows` is a number of rows, a
multiple of 64 for the landscape metrics; `block_rows="auto"` reads blocks of 64 rows and works on one per thread, so
about 64 rows per thread are in memory. A file with more than 250 million cells is not read whole by the application
without `block_rows`; the library never guesses, so give it. A few features still need the whole raster (`patches`,
`centroids` and the FRAGSTATS `prox`, `simi`, `econ`, `connect`); the library refuses them with *not available for an
out-of-core landscape*, as the application does, and everything else works.

The maps of patch ids (`labels()`) and of metrics come out lazy, as dask arrays: each block is read and labelled again
when it is computed, and with `path=` the map is written to its file block by block, never whole in memory:

```python
ids = strips.labels()                                         # patch ids, lazy
print(type(ids.data).__name__, ids.shape)
print(strips.to_raster("area", path="patch_area.tif").name)   # the area of each cell's patch, written by blocks
```

```text
Array (8000, 8000)
patch_area.tif
```

For tables with millions of patches, write them in chunks instead of building them in memory (a `.parquet` file needs
pyarrow; `.csv` and `.csv.gz` also work):

```python
print(strips.write_patch_metrics("patches.csv", ["area", "perim", "shape"], chunk_rows=500_000).name)
```

```text
patches.csv
```

The library has no out-of-core moving window: [`tf.window_metrics`][terraflux.window_metrics] works in memory, so the
strips with a halo of the section below are an application feature.

## Moving windows by strips with a halo

[[metrics.window]] computes landscape metrics in a window around every cell (see the
[Landscape metrics](../../landscape-metrics/) tutorials). A window of side \(w\) centred on a cell of the first row of a
strip needs \( (w - 1)/2 \) rows of the strip above. Out of core, Terraflux reads each strip with that **halo** above
and below, computes the windows of the strip's own rows, and writes them to a GeoTIFF (its `path`, by default
`name_window5.tif` next to the input), a band per metric. Every window sees all its cells, so the maps are the same as
in memory, cell for cell.

## MSPA by overlapping tiles

MSPA's classes depend on whole objects: whether a cell is core, bridge or branch depends on the shape of the patch it
belongs to, which may cross the whole map. GuidosToolbox's answer for large maps is its *MSPA tiling* (Vogt & Riitters,
2017), which [[landscape.mspa_tiled]] reproduces: the map is cut into tiles of `tile_size` cells that **overlap** by
`overlap` cells; each tile is processed alone; in each overlap, the half nearer to each tile is kept. A raster on disk
is read and written tile by tile, to the GeoTIFF of `path`.

```tfgraph
#! caption: MSPA of a map on disk by tiles of 700 cells overlapping by 200: the raster output of raster.read goes to landscape.mspa_tiled, which writes its result tile by tile.
#! highlight: tiled
format = "terraflux/1"

[[nodes]]
id = "forest"
op = "raster.read"
params = { path = "forest.tif" }
note = "CORINE's forests as 2, the rest as 1."

[[nodes]]
id = "tiled"
op = "landscape.mspa_tiled"
inputs = { map = "forest.raster" }
params = { edge_width = 2, tile_size = 700, overlap = 200, path = "tiled_100.tif" }
```

It is an approximation, as GuidosToolbox itself warns: near the seams between tiles, the classes can differ from MSPA
of the whole map where an object is larger than the overlap. How much? The figure script ran
[[landscape.mspa_tiled]] on the forest of the CORINE excerpt (2000 x 2000 cells, edge width 2) with tiles whose
inner part is 500 cells and overlaps from 0 to 400 cells, and compared every result with [[landscape.mspa]] of the
whole map:

```tfslider
label: Overlap (cells)
caption: "Where MSPA by tiles differs from MSPA of the whole map (red), for overlaps of 0 to 400 cells between tiles of 500 cells; forest whose class is the same is pale green. Without overlap, 4,676 cells differ, mostly one perforated forest near a seam whose opening is classed otherwise; from 200 cells of overlap, none."
frames:
  - { value: 0, src: /img/tutorials/large-data/larger-than-memory/seams_0.png, caption: "no overlap", stats: { "Cells that differ": "4,676", "Share of the map": "0.117%" } }
  - { value: 50, src: /img/tutorials/large-data/larger-than-memory/seams_50.png, caption: "overlap 50", stats: { "Cells that differ": "4,137", "Share of the map": "0.103%" } }
  - { value: 100, src: /img/tutorials/large-data/larger-than-memory/seams_100.png, caption: "overlap 100", stats: { "Cells that differ": "794", "Share of the map": "0.020%" } }
  - { value: 200, src: /img/tutorials/large-data/larger-than-memory/seams_200.png, caption: "overlap 200", stats: { "Cells that differ": "0", "Share of the map": "0%" } }
  - { value: 400, src: /img/tutorials/large-data/larger-than-memory/seams_400.png, caption: "overlap 400", stats: { "Cells that differ": "0", "Share of the map": "0%" } }
```

The overlap needed is about the size of the largest objects whose classes span a seam: here, a forest of a few
kilometres with an opening inside it. GuidosToolbox's defaults, tiles of 6,500 cells overlapping by 2,000 (the
defaults of `tile_size` and `overlap`), are generous for most maps. Check on a part of your map that fits in memory,
as here: run both and count the cells that differ.

### MSPA by tiles in Python

[`tf.mspa_tiled`][terraflux.mspa_tiled] is the same operator: a GeoTIFF path in, a GeoTIFF written tile by tile, so
memory depends on the tile size, not on the map size. Here is the part's comparison, on the forest of the CORINE
excerpt (`forest.tif`, 2 for CORINE 23-25 and 1 for the rest), with tiles whose inner part is 500 cells
(`tile_size` includes the overlap, so `500 + overlap`) against MSPA of the whole map:

```python
forest = ((clc >= 23) & (clc <= 25)).astype("uint8") + 1
tf.write_raster(forest, "forest.tif")
whole = tf.mspa(forest, edge_width=2).values                 # the reference, in memory
for overlap in (0, 100, 200):
    path = tf.mspa_tiled("forest.tif", f"tiled_{overlap}.tif", edge_width=2,
                         tile_size=500 + overlap, overlap=overlap)
    differ = int((tf.load_raster(path).values != whole).sum())
    print(f"overlap {overlap:3d}: {differ:5d} cells differ ({100 * differ / whole.size:.3f}%)")
```

```text
overlap   0:  4676 cells differ (0.117%)
overlap 100:   794 cells differ (0.020%)
overlap 200:     0 cells differ (0.000%)
```

Those are the part's slider values for 0, 100 and 200 cells. Tiles are `tile_size` pixels wide including the
`overlap` shared with their neighbours; the defaults (6,500 and 2,000, GuidosToolbox's) suit most machines, and
`n_workers` processes several tiles at once. Features larger than the overlap can come out differently near the seams:
check on a part that fits in memory, as here. Given an array instead of a path, `tf.mspa_tiled` works in memory, tile
by tile, which is how to test a choice without a big file.

### Time stacks with dask

Rasters that share the same grid (the same region at several dates) form a lazy stack with one dask chunk per file, and
[`tf.mspa`][terraflux.mspa] runs on the slices in parallel when the result is computed. Two dates of the forest map:

```python
from pathlib import Path

Path("years").mkdir(exist_ok=True)
tf.write_raster(forest, "years/forest_2010.tif")
tf.write_raster(forest, "years/forest_2020.tif")
stack = tf.load_raster("years/*.tif", stack=True)            # (file, y, x), lazy
morph_stack = tf.mspa(stack, edge_width=2)                   # still lazy
print(stack.dims, morph_stack.shape)
first = morph_stack.isel(file=0).compute()                   # computing one slice reads and runs only that file
print(len(np.unique(first.values)), "MSPA codes in the first file")
```

```text
('file', 'y', 'x') (2, 2000, 2000)
13 MSPA codes in the first file
```

`morph_stack.compute()` runs every slice; `.to_netcdf(...)` writes the stack. The same applies to any function that
accepts a dask array. For a folder of unrelated maps, as in [Batch processing](../batch-processing/), a collection and
`tf.batch` are the better tool.

## Simulations by tiles: the tiled engine

Land change simulations are global too: Dinamica EGO's model (Soares-Filho et al., 2002) allocates a number of
changes over the whole map at each step, choosing cells by their probability. `terraflux run --tiled` runs it tile
by tile:

```bash
# tiles of 1024 cells, the layers in memory
terraflux run model.toml --tiled
# tiles of 2048 cells, the steps' layers kept on disk
terraflux run model.toml --tiled --tile 2048 --disk layers
# 8 threads
terraflux run model.toml --tiled --threads 8
```

- The **Markov chain** of transition rates and the **weights of evidence** are sums over cells, so they are fitted
  tile by tile and added up: the same, to the bit, as in memory.
- The **patches** the fit measures (sizes of the patches of change) are measured within tiles.
- The **allocation** draws its random numbers by counter (from the seed, the step and the cell, see
  [Randomness](../../building-models/randomness/#random-numbers-by-counter)) and finds the cells to change from
  histograms of probabilities summed over the tiles. Patches grow within `--radius` cells of their seed (32 by
  default), the halo a tile needs from its neighbours.

The result is not the exact engine's map, but a statistically equivalent one. On Sibuyan Island (the model made by
`terraflux new dinamica` from the land use of 1997 and 2011, 14 yearly steps):

| Cells in 2011 | Forest | Coconut | Grassland | Rice | Other |
|---|---|---|---|---|---|
| observed | 4,160 | 1,404 | 1,090 | 288 | 284 |
| exact engine | 4,161 | 1,403 | 1,093 | 287 | 282 |
| tiled, tiles of 32 cells | 4,163 | 1,402 | 1,092 | 287 | 282 |
| tiled, tiles of 64 cells | 4,165 | 1,402 | 1,093 | 284 | 282 |

```tfcompare
caption: "Drag the handle: Sibuyan's simulated land use after 14 steps by the exact engine (left) and by the tiled engine with tiles of 32 cells (right). The amounts of each class agree within a few cells; 523 of the 7,226 cells differ, where the allocation placed the same amount of change elsewhere."
before: { src: /img/tutorials/large-data/larger-than-memory/engine_exact.png, label: "Exact engine" }
after: { src: /img/tutorials/large-data/larger-than-memory/engine_tiled32.png, label: "Tiled, 32 cells" }
```

With the same tiles and seed, the tiled engine gives the same maps whatever the number of threads (with tiles of 32
cells, a run with one thread and one with all of them differ in no cell) and, with `--coordinator`, whatever the
number of computers: see the [parallel computing guide](/parallel/). Tiles of another size change the patches the
fit measures, and so the maps a little (222 cells between tiles of 32 and 64 here). Compare scenarios with the same
tiling.

## Common pitfalls

- **Connecting `map` instead of `raster`.** The `map` output loads the whole band before the next node runs, out of
  core or not. For large maps, connect `raster`.
- **One large node in the middle.** A chain is as large as its largest step: a [[raster.calc]] on two maps of a billion
  cells holds both. Do local calculations on files with GDAL's operators or after cutting the map with
  [[raster.clip]], or process tiles in a batch (see [Batch processing](../batch-processing/)).
- **An uncompressed, striped GeoTIFF.** Reading a window of a file stored in long strips reads the whole width; a
  tiled, compressed GeoTIFF or a COG reads less. Convert large inputs once with [[raster.convert]].
- **Too small an overlap for MSPA.** Check the seams on a part of the map, as above.
- **Expecting the tiled engine to reproduce the exact engine's map.** It reproduces its statistics. Report which
  engine you used.
- **Metrics that need the whole map.** `prox`, `simi`, `econ` and `connect` (and FRAGSTATS' `dcore`) compare
  patches across the map; out of core, Terraflux refuses them (*needs the whole map in memory*).
- **Running out of disk.** Out-of-core results are written to GeoTIFFs (`path`); a map of billions of cells needs
  gigabytes of disk for each.

## Going further

- [Batch processing](../batch-processing/): the same analysis on hundreds of files, in parallel.
- The [parallel computing guide](/parallel/): the tiled engine and batches on many computers.
- [Landscape metrics](../../landscape-metrics/) and [MSPA](../../habitat-patterns/mspa/).
- The operators [[metrics.landscape]], [[metrics.window]], [[landscape.mspa_tiled]], [[vector.zonal]].

## References

- Gorelick, N., Hancher, M., Dixon, M., Ilyushchenko, S., Thau, D., & Moore, R. (2017). Google Earth Engine: Planetary-scale geospatial analysis for everyone. *Remote Sensing of Environment*, 202, 18–27. https://doi.org/10.1016/j.rse.2017.06.031
- Soares-Filho, B. S., Coutinho Cerqueira, G., & Lopes Pennachin, C. (2002). DINAMICA: a stochastic cellular automata model designed to simulate the landscape dynamics in an Amazonian colonization frontier. *Ecological Modelling*, 154(3), 217–235. https://doi.org/10.1016/S0304-3800(02)00059-5
- Vogt, P., & Riitters, K. (2017). GuidosToolbox: universal digital image object analysis. *European Journal of Remote Sensing*, 50(1), 352–361. https://doi.org/10.1080/22797254.2017.1330650
