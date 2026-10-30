---
title: Landscape metrics of large maps
short: Large maps
description: "Landscape metrics of maps larger than memory, read by blocks of rows with exactly the same values: the landscape level, signatures, sampling and moving windows by strips, and what cannot be computed this way."
order: 70
level: advanced
time: 25 min
ops: [metrics.landscape, metrics.window, metrics.signatures, metrics.sample, raster.read]
series: Landscape metrics
part: 7
---

A land cover of a country at 30 m has hundreds of millions of cells: Germany, about 400 million; Brazil, nearly ten
billion. Held in memory as numbers of 8 bytes, 250 million cells already take 2 GB, and the patches, their edges and
their adjacencies take more. This last part of the series shows how Terraflux computes landscape metrics of such maps
by reading them in **blocks of rows**, with only a few blocks in memory at a time, and with the same values as if the
map had been read whole. At the end you will know which operators and metrics work this way, how to set them, and
what to expect of the time they take.

## The idea

Most landscape metrics are sums: of cells, of edges, of adjacencies, of the areas and perimeters of patches. Sums can
be made piece by piece. The difficulty is the patches: a forest that runs across several blocks is one patch, and
its area, perimeter and core must be those of the whole forest, not of its pieces.

![Blocks of rows and strips](/img/tutorials/landscape-metrics/large-maps/blocks.png "Left: a map read in three blocks of rows. The large forest crosses all three; it is still counted as one patch, its pieces joined across the blocks' boundaries. Right: a moving window by strips. To compute the windows of the cells of strip 2 (the red square is one window), the strip is read with half a window of rows above and below (the orange halo).")

Terraflux uses landschaft's *out-of-core landscape*: the map is read block by block (blocks of a multiple of 64 rows,
the strips in which the patches are labelled), a first time to label the patches and join the pieces that touch
across blocks, a second time to sum the metrics of every patch, class and the landscape (a third or fourth time for
core areas, the radius of gyration, the circumscribing circle and the nearest-neighbour distances). The sums are made
in the same order as in memory, so the values are the same to the last digit, not approximations. A moving window
needs no labelling across strips: each strip is read with half a window of rows above and below, and its windows are
complete.

Four metrics need the whole map at once and cannot be computed by blocks: the proximity and similarity indices
(`prox`, `simi`, which search for patches within a radius), the contrast-weighted edge (`econ`) and the connectance
(`connect`); with `convention = "fragstats"`, also the disjunct core metrics (`dcore`). Asking for them by blocks
stops the model with a message saying so.

## When to use it

- A **map larger than your memory**: a country, a continent, a fine resolution over a large area.
- **Rasters on disk of more than 250 million cells**: Terraflux always reads them by blocks for their metrics, even
  without `block_rows`.
- **Many maps in a batch** on a machine with little memory, where several runs share it.
- Not needed for maps of a few tens of millions of cells: reading them whole is as fast.

## What you need

A categorical raster on disk (a GeoTIFF, ideally tiled and compressed), read with [[raster.read]] and connected by
its **`raster`** output, not `map`: `raster` passes the file on disk, which the metrics operators read by blocks;
`map` would load the whole map into memory first. We use [`clc.tif`](/data/tutorials/habitat-patterns/mspa/clc.tif)
(4 million cells), and the same map tiled 3 × 3 into one of 36 million cells, to time it.

## The landscape level by blocks

```tfgraph
#! caption: The landscape metrics of a land cover read by blocks of 512 rows.
#! numbered: true
#! highlight: land
format = "terraflux/1"

[[nodes]]
id = "cover"
op = "raster.read"
params = { path = "clc.tif" }
note = "Connect raster, not map."

[[nodes]]
id = "land"
op = "metrics.landscape"
inputs = { map = "cover.raster" }
params = { metrics = ["ta", "np", "lpi", "ed", "area_mn", "shape_mn", "ai", "cohesion", "core_mn", "enn_mn", "contag", "shdi"], block_rows = 512 }
note = "The same values as the whole map."
```

`block_rows` is the number of rows of a block, rounded up to a multiple of 64; without it, a raster on disk under
250 million cells is read whole, and one above is read by blocks of 64 rows (more for a map narrower than 512
cells). Smaller blocks use less
memory; very small ones read the file more times. A block of 512 rows of a map 100,000 cells wide is 51 million cells:
a few hundred MB.

We ran the twelve metrics above on `clc.tif` whole and by blocks of 512 and 128 rows, and compared:

| Read | Time |
|---|---|
| Whole map (4 million cells) | 0.50 s |
| Blocks of 512 rows | 0.57 s |
| Blocks of 128 rows | 0.59 s |
| Blocks of 512 rows, the map tiled 3 × 3 (36 million cells, 4 metrics) | 2.2 s |

The values were identical: the largest relative difference over the twelve metrics was 0. Reading by blocks cost
about 15% more time here, and time grows roughly in proportion to the number of cells: 2.2 seconds for 36 million
cells, nine times the map, against 0.5 for 4 million. The 36-million-cell map gave 8,499 patches, an edge
density of 23.4 m/ha, an aggregation of 97.1% and a Shannon diversity of 1.81.

!!! note "Class and patch metrics by blocks give tables"
    Read by blocks (a raster on disk with `block_rows`, or above 250 million cells), [[metrics.class]] and
    [[metrics.patch]] give their `table` but no `map`: a map of a metric per class or patch would be as large as the
    raster. Paint one afterwards if you need it, from the table, with [[raster.reclassify]] (a rule per class).
    [[metrics.signatures]] gives class metrics by blocks too, as one row with a column per metric and class
    (`pland_23`, `np_23`...), the form to compare many maps or zones.

## Moving windows by strips

```tfgraph
#! caption: Shannon's diversity and edge density in a window of 9 cells around every cell, computed by strips of 256 rows and written to a GeoTIFF.
#! highlight: window
format = "terraflux/1"

[[nodes]]
id = "cover"
op = "raster.read"
params = { path = "clc.tif" }

[[nodes]]
id = "window"
op = "metrics.window"
inputs = { map = "cover.raster" }
params = { metrics = ["shdi", "ed"], window = 9, block_rows = 256, path = "clc_window9.tif" }
note = "A band per metric, written strip by strip."
```

With rasters on disk, [[metrics.window]] writes its maps to the GeoTIFF of `path` (default: next to the input, its
name with `_window9`), a band per metric named after it, strip by strip; its `maps` output is that file, which other
nodes read as rasters on disk in turn. On the 4 million cells of CORINE at 25 m with all 44 classes, the two metrics
in windows of 81 cells took 33 seconds.

![Diversity in a window of 9 cells, computed by strips](/img/tutorials/landscape-metrics/large-maps/window9.png "Shannon's diversity of CORINE's 44 classes within 9 × 9 cells of 25 m (225 m), computed by strips of 256 rows. Each strip is read with its halo, so every window is complete.")

The time of a moving window grows with the number of cells *times* the window's area: doubling the window's side
quadruples the time. For large maps, choose the window from the ecology (part 4), then the grain: the same window in
metres on cells twice as large is sixteen times faster.

!!! app "In the application: how far a long run is, and Stop"
    Read by blocks or strips, the metrics tell how far they are while they run, in the status bar and in the bar
    over the canvas: `land: block 3 of 12` for the landscape level (counted again at each of its passes over the
    file), `window: rows 1024 of 2000` for a moving window, whose bar fills and whose box fills along its bottom
    too. **Stop**, in the status bar, stops the node between two blocks or two strips: the one being read is
    finished, and the run ends there, not at the end of the map.

## Signatures, sampling and series

[[metrics.signatures]], [[metrics.sample]] and [[metrics.series]] take `block_rows` too. Their zones (squares of a
grid, polygons, or buffers around points) are read window by window from the raster on disk, so even a grid of
thousands of squares over a continent needs memory for one square at a time; whole maps of a series are read by
blocks. [[metrics.patch]] can write the map of its patches block by block to a GeoTIFF (`patches_path`) instead of
keeping it in memory.

## Many maps: a batch

A large study area is often delivered as many tiles, or a study compares many landscapes, each a file. Instead of a
model per file, give the folder to [[raster.read]]: [[metrics.landscape]] runs once per raster, several at the same
time, and its tables come out as one, a row per raster (see [Batch processing](../../large-data/batch-processing/)):

```tfgraph
#! caption: The landscape metrics of every raster of a folder, each read by blocks: one table, a row per raster.
format = "terraflux/1"

[[nodes]]
id = "tiles"
op = "raster.read"
params = { path = "tiles/*.tif", workers = 4 }
note = "Every raster of the folder; four at the same time."

[[nodes]]
id = "metrics"
op = "metrics.landscape"
inputs = { map = "tiles.raster" }
params = { metrics = ["np", "ed", "ai", "shdi"], block_rows = 512 }

[[nodes]]
id = "write"
op = "table.write"
inputs = { table = "metrics.table" }
params = { path = "metrics.csv" }
```

The table's first column, `layer`, is the raster's name. A raster that fails
does not stop the others (the node warns which failed and why). Choose `workers` and `block_rows` together: four
workers reading blocks of 512 rows of a map 100,000 cells wide hold about four times 51 million cells.

For batches across several machines, the [parallel computing](/parallel/) guide shows `terraflux submit` and a
cluster of workers.

## How much memory?

A rough guide for deciding whether a map needs blocks: in memory, Terraflux holds a map of classes as numbers of 8
bytes for the metrics, plus the patches' labels (4 bytes a cell) and the work of the metrics. Count about 16 bytes a
cell:

| Map | Cells | Whole, about | By blocks of 512 rows |
|---|---|---|---|
| CORINE excerpt of this page | 4 million | 64 MB | about 16 MB |
| A region at 30 m (100 × 100 km) | 11 million | 180 MB | about 30 MB |
| Germany at 30 m | 400 million | 6 GB | some hundred MB |
| Brazil at 30 m | 9.5 billion | 150 GB | about a GB |

The figures are orders of magnitude, not promises: the patch metrics of a map with millions of patches need more.
When in doubt, run by blocks: the values are the same, and the cost in time is small.

## Settings that change the answer

None, by design: the values by blocks are the values of the whole map. What changes is time and memory:

- **`block_rows`**: memory against the number of reads.
- **The file's layout**: a GeoTIFF tiled in blocks of 256 or 512 cells, or in strips, reads faster than one stored
  as one strip per row with heavy compression. Convert very large files once (with GDAL's `gdal_translate -co
  TILED=YES -co COMPRESS=DEFLATE`).
- **The cell type**: classes in a byte (`u8`) take an eighth of the disk and the reading of 64-bit numbers.

## Common pitfalls

- **Connecting `map` instead of `raster`.** The whole map is then loaded in memory before the metrics; with a large
  map, memory runs out before they start.
- **Metrics that need the whole map** (`prox`, `simi`, `econ`, `connect`) in a list run by blocks: the model stops.
  Compute them on a smaller extent, or drop them.
- **A large map in degrees.** Over a continent, cells of a geographic grid differ in area by latitude. Project to an
  equal-area system first (and expect the projected map to be larger).
- **Comparing with a run of another program on a resampled copy.** If you checked a value against another program,
  check that both read the same grid.

## Going further

- [Large data and batches](/tutorials/large-data/): the same analysis over hundreds of files, and other operators
  that read by blocks.
- [Parallel computing](/parallel/): running batches of maps on several machines.
- [Habitat patterns](/tutorials/habitat-patterns/networks-and-change/): MSPA of large maps by tiles.
- The operator reference: [[metrics.landscape]], [[metrics.window]], [[metrics.signatures]], [[metrics.sample]],
  [[metrics.series]].

## References

- Bosch, M. (2019). PyLandStats: an open-source Pythonic library to compute landscape metrics. *PLOS ONE*, 14(12), e0225734. https://doi.org/10.1371/journal.pone.0225734
- Hesselbarth, M. H. K., Sciaini, M., With, K. A., Wiegand, K., & Nowosad, J. (2019). landscapemetrics: an open-source R tool to calculate landscape metrics. *Ecography*, 42(10), 1648–1657. https://doi.org/10.1111/ecog.04617
