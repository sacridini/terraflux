---
title: Maps larger than memory
short: Maps larger than memory
description: When one map does not fit in the computer's memory - the tiled engine for land change on one machine or many, why its maps do not depend on the tiling, shared disks, and landscape metrics, vectors and MSPA computed by blocks or tiles.
order: 50
---

The other pages of this guide run many small models. This one is about one model, or one analysis, too large for
a computer: a land change simulation of a country at 30 m, the landscape metrics of a continent's forest cover, the
MSPA of a global map. Terraflux handles them by reading and computing the maps in pieces: **tiles** for Dinamica
EGO's land change model, **blocks of rows** for landscape metrics and vector statistics, GuidosToolbox's
**overlapping tiles** for MSPA. This page explains each, shows on real runs that the tiled land change model gives
the same maps whatever the tiling and the number of machines, and how to spread it over machines with or without a
shared disk. The land change side is told from the modeller's point of view in
[Maps larger than memory](/tutorials/land-change/maps-larger-than-memory/) of the land change tutorials.

## The idea

### How large is too large

A map of 30 m cells over 1 million km² has 1.1 billion cells. Stored on disk at one byte per cell (a land use
map), it is 1.1 GB; held in Terraflux's memory, where every cell is a number of 8 bytes, 9 GB per layer. A model
holds several layers at once (the land use, a dozen drivers, a probability per transition, the map of the previous
step), so a land change model of that area needs well over 100 GB of memory, more than most computers have. An analysis that holds one map (landscape metrics of
a forest map) needs only a few GB, but a continent at 30 m is still beyond a laptop.

### Pieces, and what crosses their edges

Cutting a map is easy; what is hard is everything that crosses the cuts. Each tool here answers it its own way:

![Tiles and a halo](/img/guide/parallel/tiles.png "A map simulated by the tiled engine, with its tiles of 256 cells. A tile (orange) is computed with a halo (dashed) twice the radius a patch can grow from its seed, so a patch that crosses a tile's edge grows the same way in every tile it touches.")

- **Landscape metrics by blocks of rows**: the patches that cross from one block to the next are followed across
  the blocks, so the metrics are **the same values** as on the whole map. A moving window reads strips of rows with the window's half side above and below each strip.
- **Dinamica EGO's model on tiles**: the fits are sums over tiles (the same to the bit), random numbers are drawn
  by counter, the seeds of the patches come from thresholds found on histograms summed over all tiles, and each
  tile is computed with a **halo** wide enough for every patch that can reach it. The maps are **the same whatever
  the tiling**, and statistically equivalent to the exact engine's.
- **MSPA by overlapping tiles** (GuidosToolbox's MSPA Tiling): each tile is processed with an overlap, and of each
  overlap the half nearest to each tile is kept. It is an **approximation**, as GuidosToolbox warns: near the seams
  the classes can differ from those of the whole map where objects are larger than the overlap.

!!! science "Key idea: same maps whatever the tiling"
    A tiled computation gives the same result as the whole map when every piece computes from the same inputs and
    the parts are merged by operations whose order does not matter (sums, counts, maxima). The tiled engine is
    built on this: its random draws are a function of (seed, step, cell, purpose), not of the order cells are
    visited; its global decisions come from sums of histograms; its patches are grown within a fixed radius from
    keys every tile computes alike. So 1 tile or 100, 1 machine or 10, give the same map. (The one exception: patch
    sizes measured during the run by [[patches.observe]] are measured within tiles, so the tile size, not the
    number of threads or machines, changes those maps a little.)

### Estimating the memory a map needs

A quick estimate tells whether a map fits: cells × bytes per cell × layers held at once. A forest map of
40,000 × 50,000 cells (2 billion cells, a large country at 30 m) stored as one byte per cell is 2 GB on disk;
loaded as a map for an analysis, Terraflux holds each cell as a number of 8 bytes, so 16 GB for one layer, and
an analysis that makes two or three intermediate layers of the same size needs two or three times that. Compare
with the memory of the computer, minus what the system and other programs use. If it does not fit with room to
spare, use the tools of this page, or work at a coarser resolution where the question allows it (a 90 m map has a
ninth of the cells of a 30 m one).

## When to use it

| Analysis | Too large? | Use |
|---|---|---|
| Dinamica EGO's land change model | maps that do not fit, or one run to spread over machines | `terraflux run --tiled` |
| Other land change models (CLUE-S, PLUS, FUTURES...) | no tiled version | a coarser resolution, or regions run separately ([[demand.regional]] does not split memory) |
| Landscape metrics: patch, class, landscape, window, samples, series, signatures | rasters on disk of more than 250 million cells, or any with `block_rows` | the same operators, by blocks |
| Zonal statistics, extraction, rasterization of vectors | the same | [[vector.zonal]], [[vector.extract]], [[vector.rasterize]] by strips |
| MSPA | a map larger than memory | [[landscape.mspa_tiled]] |
| Distances, reprojection, resampling | any size | GDAL's operators work on the file ([[distance.proximity]], [[raster.reproject]]...) |

## What you need

The maps as **files** (GeoTIFF; tiled and compressed GeoTIFFs read fastest, a Cloud-Optimised GeoTIFF from
[[raster.convert]] is ideal), on a fast local disk, with room for the outputs (and for the tiled engine's layers
with `--disk`).

## Run it

### Dinamica EGO's model on tiles, one machine

```bash
terraflux run model.toml --tiled                              # tiles of 1024 cells, the steps' layers in memory
terraflux run model.toml --tiled --tile 2048 --disk layers    # the layers on disk, a file per tile
terraflux run model.toml --tiled --threads 8 --radius 32      # threads; the farthest a patch grows from its seed
```

The model is the one `terraflux new dinamica` writes (a Markov chain, weights of evidence, the patches' settings,
a loop of [[woe.probability]] and [[dinamica.step]] with its 3 × 3 window). Its maps are written per step as tiled
GeoTIFFs (`out/maps/step_001.tif`...). With the patches' settings given by [[patches.settings]] the maps do not
depend on the tiling at all; [[patches.observe]] measures the observed patches within tiles, which can shift them
slightly with the tile size.

The test of the land change tutorial, on Sibuyan's maps enlarged four times (432 × 512 cells): every tiled run
(tiles of 128, 256, 1,024 cells; layers in memory or on disk; on one machine or on a coordinator's two workers)
gave **the same map to the cell**; against the exact engine, 4% of the cells differed, all in the zones of change,
with the same areas within 11 cells and the same figure of merit within 0.002 (0.513 against 0.515).

And how it scales with threads, on a map twice as large on each side (864 × 1,024 cells, tiles of 256):

```tfchart
type: line
data: /data/guide/parallel/timings.csv
x: cores
y: [tiled]
labels: { tiled: "Tiled engine, one run" }
colors: { tiled: "#2f6f9f" }
xlabel: Threads
ylabel: Seconds
height: 260
caption: One run of the tiled engine (14 steps, 864 x 1,024 cells, tiles of 256) with 1 to 19 threads on a desktop computer of 20 logical processors.
```

35 seconds with one thread, 12 with four, 11 with eight; more threads do not help on this computer.

### On many machines

```bash
terraflux serve --listen 0.0.0.0:7700 --code                        # a coordinator
terraflux worker --code K7QM-2XPA-9DTE                              # on every machine that works
terraflux run model.toml --tiled --disk run-1/layers --coordinator http://coordinator-host:7700 --code K7QM-2XPA-9DTE
```

![The tiled engine on a coordinator's workers](/img/guide/parallel/tiles_flow.png "Your machine runs the model: it fits the pieces, keeps each step's layers on its disk, and merges the parts of every pass. Each pass over the tiles is a job on the coordinator; its tasks are runs of neighbouring tiles; workers fetch their tiles and halos and send back the tiles they wrote.")

- **No storage in common** (the default): the model's maps go to each worker once, by hash, as a batch's files. The
  steps' layers stay on the disk of the machine that runs the model (`--disk` is required); each pass lists each
  layer's tiles and their hashes, a worker fetches the tiles of its tasks and their halos (each tile once per
  worker), and sends back by hash the tiles it wrote.
- **A shared disk** (`--shared-disk`: NFS, SMB, a cluster's parallel file system): the workers read the model's maps
  and read and write the layers themselves, at the same paths as the machine that runs the model; only the passes
  and their sums travel through the coordinator.
- **Passes**: every pass over the tiles (a histogram, a count, the growth of the patches) is a job whose tasks are
  runs of contiguous tiles (4 per worker); the machine that runs the model merges their parts and decides the next
  pass. A worker that vanishes loses its task to another; a task writes whole tiles under their final names only
  when done, so a task run twice writes the same tiles.

On the test map, two workers through a coordinator took 25 to 30 seconds against 4 on one machine: each step is
several passes, each a round trip through the coordinator. Many machines pay off when each tile's work is large
(maps of billions of cells), not for maps that fit on one machine.

### Landscape metrics, vectors and MSPA

```tfgraph
#! caption: Out-of-core analyses of a large forest map: class metrics read by blocks of 1,024 rows (node 2), a moving window by strips written to a GeoTIFF (node 3), and MSPA by GuidosToolbox's overlapping tiles (node 4).
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "forest"
op = "raster.read"
params = { path = "forest_30m.tif" }
note = "A map of forest (2) and non-forest (1) of several billion cells: only its header is read."

[[nodes]]
id = "metrics"
op = "metrics.class"
inputs = { map = "forest.raster" }
params = { metrics = ["ca", "np", "area_mn", "ed", "ai"], block_rows = 1024 }
note = "The same values as on the whole map, read 1,024 rows at a time."

[[nodes]]
id = "window"
op = "metrics.window"
inputs = { map = "forest.raster" }
params = { metrics = ["ed"], window = 33, path = "forest_ed33.tif" }
note = "The edge density in 33 x 33 windows (1 km), by strips, written to a GeoTIFF."

[[nodes]]
id = "mspa"
op = "landscape.mspa_tiled"
inputs = { map = "forest.raster" }
params = { foreground = 2, edge_width = 3, tile_size = 6500, overlap = 2000 }
note = "MSPA by overlapping tiles; near the seams, objects larger than the overlap can differ."
```

- [[metrics.class]], [[metrics.patch]], [[metrics.landscape]], [[metrics.window]], [[metrics.sample]],
  [[metrics.series]] and [[metrics.signatures]] take `block_rows` (a multiple of 64): the map is read that many rows
  at a time, with the same results. Rasters on disk of more than 250 million cells are always read so.
- [[vector.zonal]] reads the map by strips of rows, [[vector.extract]] only the strips that hold points,
  [[vector.rasterize]] burns into a GeoTIFF strip by strip.
- [[landscape.mspa_tiled]]: `tile_size` (with its overlap, 6,500 cells by default) and `overlap` (2,000): choose an
  overlap larger than the largest objects whose class matters (a core of a large forest can span more), and check
  the seams.

## Read the results

The outputs are those of the same analyses in memory: tables of metrics, GeoTIFFs written to `path`, the steps'
maps of the simulation. Two checks are worth the time: compare, on a part of the map small enough for memory, the
out-of-core result with the in-memory one (the same values for metrics; the same statistics for the tiled engine);
and for MSPA, look at the seams of the tiles.

## Settings that change the answer

- `--tile` and `block_rows` change time and memory, not the results (MSPA's `overlap` excepted).
- `--radius` of the tiled engine: patches cannot reach farther from their seed; set it above the largest patch's
  reach.
- MSPA's `overlap`: too small, and large objects are classified differently near the seams.

## Common pitfalls

- **A model the tiled engine does not run** (another family, dynamic drivers in the loop, a window other than
  3 × 3): it says what it lacks.
- **Expecting a map in memory where none fits**: an operator that needs the whole map in memory (most of them) still
  needs it; out-of-core work is limited to the operators above.
- **Slow disks**: out-of-core work reads and writes much; a network disk or a USB drive makes it crawl. Use a local
  SSD, or `--shared-disk` on a cluster's parallel file system.
- **Too little disk** for `--disk`: the layers of a step (a probability per transition, the Expander's and the
  Patcher's maps), tile by tile, while the step and the next need them; they are removed after.

## Going further

- [Maps larger than memory](/tutorials/land-change/maps-larger-than-memory/), the land change tutorial.
- [Troubleshooting](/parallel/troubleshooting/) and the table of which tool for which job.

```tfcli
run
```
