---
title: Batch processing
description: Run nodes on every raster of a folder - a folder given to any node that takes a map, the files written by their names, one table of all of them, files run side by side and runs that resume where they stopped, folders paired by name - and when to use a batch of model runs or tiles of a large map instead.
order: 20
level: beginner
time: 25 min
ops: [raster.read, landscape.mspa, raster.write, metrics.class, table.write]
---

Ecological studies often apply one analysis to many maps: the MSPA of a forest map for each of 200 watersheds, the
landscape metrics of every year of a time series, the fragmentation of every tile of a national land cover map. In
this tutorial you will give a folder of rasters to a model as you would give it one raster: every node that takes a
map runs once per file, file by file and several files at the same time, each result written under the file's name;
the tables of all the files come out as one. You will run it again and see it skip what is done, pair two folders
file by file, and see when a batch of files is the right tool, and when a batch of model runs or a large map read
by parts is.

## The idea

A **batch** is the same operation repeated over a list of inputs, each run independent of the others. Because they
are independent, they can run at the same time, one per core of the computer: the work of 16 files on 8 cores takes
about the time of 2. GuidosToolbox has offered this for a long time as *Batch Process* (Vogt & Riitters, 2017), and
landschaft's `batch` module does the same in Python. In Terraflux there is no node for it: **a folder is a value
like a raster**, and any node given one runs once per file.

!!! science "Key idea"
    A batch is the simplest form of parallel computing: *embarrassingly parallel* work, where the pieces never need
    each other's results. It scales with the number of cores (and, for model runs, of computers) almost perfectly,
    and it fails gracefully: a file that fails does not stop the others.

There are three kinds of batch in Terraflux, for three needs:

| Need | Tool | Each task is |
|---|---|---|
| nodes on many **files** | a folder given to [[raster.read]] (this tutorial) | one raster |
| one **model** many times: seeds, scenarios, calibration | `terraflux submit`, `calibrate`, the **Many runs** pane | one run of the model |
| one **large map** in parts | out-of-core operators, `terraflux run --tiled` | a strip or a tile of the map |

The second is in [Randomness and reproducibility](../../building-models/randomness/) and the
[parallel computing guide](/parallel/); the third in [Rasters larger than memory](../larger-than-memory/).

## When to use it

- Many maps of the same kind: watersheds, municipalities, protected areas, years, tiles, species.
- One or several operators run with the same settings on each of them: MSPA, then its metrics; a reclassification,
  then a distance.
- Results that must be kept per file (one MSPA map per watershed, named after it), with a record of how each was
  made.

When *not* to: when the maps should be analysed **together** (a time series as a stack, see
[Stacks and layers](../../building-models/stacks-and-layers/)); when one file is too large for memory (see
[Rasters larger than memory](../larger-than-memory/)).

## What you need

A folder of rasters. Here the CORINE Land Cover excerpt around Hanover of the getting-started tutorials, cut into
**16 tiles** of 500 x 500 cells (12.5 x 12.5 km), `tiles/clc_r0c0.tif` to `tiles/clc_r3c3.tif` (row and column of
the tile). In a real study the files would be your watersheds or your years; they need not have the same size or
grid.

```text
study/
├── folder.toml
└── tiles/
    ├── clc_r0c0.tif
    ├── clc_r0c1.tif
    ...
    └── clc_r3c3.tif
```

## Build the model

Three nodes, as for one raster: the folder read, MSPA, and the files written.

```tfgraph
#! caption: A folder given to raster.read: MSPA runs once per GeoTIFF of the folder tiles, and raster.write writes each result under its file's name, in the folder mspa.
#! highlight: tiles
format = "terraflux/1"
name = "MSPA of every tile"

[[nodes]]
id = "tiles"
op = "raster.read"
params = { path = "tiles" }
note = "every GeoTIFF of the folder tiles: each node after it that takes a map runs once per file"

[[nodes]]
id = "mspa"
op = "landscape.mspa"
inputs = { map = "tiles.raster" }
params = { foreground = [23, 24, 25], edge_width = 2 }
note = "MSPA of the forests (CORINE 23, 24, 25)"

[[nodes]]
id = "write"
op = "raster.write"
inputs = { value = "mspa.map" }
params = { path = "mspa/{name}_mspa.tif" }
note = "a file per tile, named after it"
```

- [[raster.read]]'s `path` is a folder (its rasters, in alphabetical order) or a pattern such as `tiles/*.tif`.
  Only the files' headers are read here.
- [[landscape.mspa]] takes one map. Given the folder, it runs once per file. So does every node after it that takes
  its maps: here [[raster.write]]. The nodes run **file by file**: a tile goes through MSPA and is written before
  the next, so only a few tiles are in memory at a time, however many the folder has.
- [[raster.write]]'s `{name}` is the file's name: `mspa/clc_r0c0_mspa.tif`, `mspa/clc_r0c1_mspa.tif`... Without
  `{name}`, files of different grids are each written with their name before the file's (`clc_r0c0_mspa.tif` for
  `mspa.tif`), and files of one grid as one file of a band per file.

Two settings of [[raster.read]] are for folders:

| Setting | Meaning |
|---|---|
| `first` | only the first files (`first = 2`): try the model on a few before running it on all |
| `workers` | files at the same time; by default as many as the threads |

!!! app "In the application"
    **File > Process a folder...** (also on the empty canvas and in the command palette, ++ctrl+k++) asks for the
    folder, then for the operator: it builds the three nodes, the results written beside the folder
    (`tiles_mspa/{name}_mspa.tif`), and selects the operator to choose its settings. For a node already in a model,
    right-click it and choose **Run on a folder...**. While it runs, the node tells which file it is on.

## Run it

```text
$ terraflux run folder.toml --out run1
MSPA of every tile: inputs read (0.0 s)
wrote run1\manifest.json (terraflux reproduce runs it again)
done in 0.2 s
```

The 16 tiles took 0.2 second together: they ran side by side.

## Read the results

### The files

For each file, the folder `mspa` holds the map written and a **record** of how it was made, `.json` beside it:

```text
$ ls mspa
clc_r0c0_mspa.json
clc_r0c0_mspa.tif
clc_r0c1_mspa.json
clc_r0c1_mspa.tif
...  (32 files)
```

```json
{
  "input": "tiles\\clc_r0c0.tif",
  "name": "clc_r0c0",
  "nodes": [
    { "id": "mspa", "op": "landscape.mspa", "params": { "edge_width": 2, "foreground": [23, 24, 25] } },
    { "id": "write", "op": "raster.write", "params": { "path": "mspa/{name}_mspa.tif" } }
  ],
  "outputs": { "write": "mspa/clc_r0c0_mspa.tif" },
  "seconds": 0.118,
  "terraflux": "1.24.2"
}
```

The record is what you need for a methods section, or to know a year later with which settings each file was made.
It also lets a run resume where it stopped (below).

### Files that fail

A file that fails does not stop the others: they run, and the node warns which failed and why
(*1 of the 16 files failed, the others went on: clc_r2c1 (mspa: ...)*). Only when every file fails is it an error.
A warning that some files gave is told once, with their names.

![The 16 tiles' MSPA put together](/img/tutorials/large-data/batch-processing/mosaic.png "The 16 MSPA maps of the batch, put back in their places (the lines are the tiles' borders). Each tile was analysed alone, blind to what lies across its borders.")

The figure shows the maps put back together. Each tile was analysed alone, so it cannot see across its borders: a
forest cell next to a field just across a border is core in its tile and edge in the whole map, and an opening of
the forest is classed by what each tile sees of it. Compared cell by cell with MSPA of the whole map, 4,676 cells
(0.12%) differ, most of them along the borders. **A batch of tiles is not the same as an analysis of the whole
map.** That is fine when each file is a unit of its own (a watershed, a reserve, a year); for one large map cut into
tiles, use the out-of-core operators of [Rasters larger than memory](../larger-than-memory/), whose tiles overlap.

### One table of all the files

A node that gives a table, given a folder, gives **one** table: the tables of all the files, a column `layer` first
with the file's number (in the folder's alphabetical order: 1 is `clc_r0c0`, 16 is `clc_r3c3`).

```tfgraph
#! caption: metrics.class of every tile: one table, a row per class of each tile, written by table.write.
#! highlight: metrics
format = "terraflux/1"
name = "Metrics of every tile"

[[nodes]]
id = "tiles"
op = "raster.read"
params = { path = "tiles" }

[[nodes]]
id = "metrics"
op = "metrics.class"
inputs = { map = "tiles.raster" }
params = { metrics = ["pland", "np", "ed"] }

[[nodes]]
id = "write"
op = "table.write"
inputs = { table = "metrics.table" }
params = { path = "metrics.csv" }
```

```text
layer,class,pland,np,ed
1,1,0.1916,1,0.176
1,2,7.1476,9,4.8448
1,3,0.1744,1,0.32
...  (195 rows: a row per class of each tile)
```

Read in a spreadsheet, R or Python, or in the Viewer, it gives at once the share of forest, arable land and urban
land of each tile:

```tfchart
type: bar
data: /data/tutorials/large-data/batch-processing/tiles.csv
x: tile
y: [forest, arable, urban]
labels: { forest: "Forest (23-25)", arable: "Arable land (12)", urban: "Urban (1-4)" }
colors: { forest: "#3b8a5a", arable: "#d9a441", urban: "#b4508a" }
xlabel: Tile (row, column)
ylabel: "% of the tile"
height: 320
caption: "The share of forest, arable land and urban land in each of the 16 tiles, from the one table of metrics.class given the folder. Tile r2c3 is Hanover (59% urban); the southern tiles (r3c0 to r3c2) are the most forested."
```

The two models can be one: [[metrics.class]] given the MSPA maps (`mspa.map`) gives the metrics of MSPA's classes
of every tile, run in the same pass, file by file.

## Resume where it stopped

Run the MSPA model again:

```text
$ terraflux run folder.toml --out run2
MSPA of every tile: inputs read (0.0 s)
  warning: mspa: 16 of the 16 files were done before with the same nodes and settings (their records): not run again (delete a file's record, .json, to run it again)
wrote run2\manifest.json (terraflux reproduce runs it again)
done in 0.0 s
```

A file whose record says it was done **with the same nodes and settings, from the same file** (its size and date),
its map still there, is not run again. A batch of 500 watersheds that stopped at the 300th (a power cut, a closed
laptop) continues from the 301st when you run it again. Change a setting (`edge_width = 3`) and every file runs
again; replace one input file, and only it runs. To run one file again, delete its record.

Files are skipped only when nothing but the files written reads what the nodes give: in the metrics model above,
the table needs every file's metrics, and every file runs.

## Two folders, file by file

A node that takes two maps can be given two folders: their files are **paired by name**. With the land use of each
watershed in `uso/` and its roads in `estradas/`:

```text
uso/uso_bacia_01.tif        estradas/estradas_bacia_01.tif
uso/uso_bacia_02.tif        estradas/estradas_bacia_02.tif
uso/uso_bacia_03.tif
```

The files are paired by the same name, or, when the names differ, by what is left once each folder's common start
and end are cut (`uso_bacia_` and `estradas_bacia_` cut, `01` with `01`). The node runs on each pair, named after the first folder's file
(`uso_bacia_01`); a file with no pair (`uso_bacia_03` here) is left out, with a warning.

## The older way: batch.run

Models made before this version may hold a [[batch.run]] node, which runs one operator on a folder and writes its
results by GuidosToolbox's naming. It still runs. In the application, select it and click **Make it a chain of
nodes** in the Node pane: it becomes the three nodes above, with its folder, operator, settings and output folder,
its maps seen in the Viewer as any node's.

## Settings that change the answer

The answer is the operator's, so its settings are what matter: see its tutorial. [[raster.read]]'s `workers` and
`first` change **how** it runs, not the result:

- `workers`: files at the same time. More workers finish sooner, until the cores, the disk or the memory run out:
  each worker holds one file and its results in memory. For large rasters, fewer workers.
- `first`: only the first files, to try the model.

When the maps of all the files would take more than 2 GB of memory, only the files written are kept, not the maps
in memory: see them in the files.

## Common pitfalls

- **Files of different classes.** The same settings go to every file. If the forest is class 23 in some files and
  class 3 in others, reclassify them first, or run two folders.
- **Files of different resolutions.** An edge width of 2 cells is 50 m on 25 m cells and 60 m on 30 m cells.
  Resample the files to one cell size first ([One grid](../../getting-started/one-grid/)).
- **Output in the input folder.** With the folder `tiles` read and `tiles/{name}_mspa.tif` written, the results
  become inputs of the next run. Keep them apart (Process a folder... writes beside the folder).
- **Tiles of one map.** Results near the borders of tiles differ from the whole map's for analyses of objects or
  windows (above). Use overlapping tiles or the out-of-core operators.

## Going further

- [Rasters larger than memory](../larger-than-memory/): one large map by strips and tiles.
- [Randomness and reproducibility](../../building-models/randomness/): a model run many times with `terraflux submit`.
- The [parallel computing guide](/parallel/): batches of model runs on many computers.
- The operators [[raster.read]], [[landscape.mspa]], [[raster.write]], [[metrics.class]], [[table.write]].

## References

- Vogt, P., & Riitters, K. (2017). GuidosToolbox: universal digital image object analysis. *European Journal of Remote Sensing*, 50(1), 352–361. https://doi.org/10.1080/22797254.2017.1330650
