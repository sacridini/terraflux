---
title: Batch processing
description: Run one operator on every raster of a folder with batch.run, as GuidosToolbox's Batch Process does - the files it writes, its table and report, runs that resume where they stopped - and when to use a batch of model runs or tiles of a large map instead.
order: 20
level: beginner
time: 25 min
ops: [batch.run, landscape.mspa, metrics.class]
---

Ecological studies often apply one analysis to many maps: the MSPA of a forest map for each of 200 watersheds, the
landscape metrics of every year of a time series, the fragmentation of every tile of a national land cover map. In
this tutorial you will use [[batch.run]] to run one operator on every raster of a folder, in parallel, with its
outputs named after each input; read the table and the report it gives; run it again and see it skip what is done;
and combine the results of all the files into one chart. At the end you will know when a batch of files is the
right tool, and when a batch of model runs or a large map read by parts is.

## The idea

A **batch** is the same operation repeated over a list of inputs, each run independent of the others. Because they
are independent, they can run at the same time, one per core of the computer: the work of 16 files on 8 cores takes
about the time of 2. GuidosToolbox has offered this for a long time as *Batch Process* (Vogt & Riitters, 2017), and
landschaft's `batch` module does the same in Python; [[batch.run]] is its port.

!!! science "Key idea"
    A batch is the simplest form of parallel computing: *embarrassingly parallel* work, where the pieces never need
    each other's results. It scales with the number of cores (and, for model runs, of computers) almost perfectly,
    and it fails gracefully: a file that fails does not stop the others.

There are three kinds of batch in Terraflux, for three needs:

| Need | Tool | Each task is |
|---|---|---|
| one operator on many **files** | [[batch.run]] (this tutorial) | one raster |
| one **model** many times: seeds, scenarios, calibration | `terraflux submit`, `calibrate`, the **Many runs** pane | one run of the model |
| one **large map** in parts | out-of-core operators, `terraflux run --tiled` | a strip or a tile of the map |

The second is in [Randomness and reproducibility](../../building-models/randomness/) and the
[parallel computing guide](/parallel/); the third in [Rasters larger than memory](../larger-than-memory/).

## When to use it

- Many maps of the same kind: watersheds, municipalities, protected areas, years, tiles, species.
- An operator that takes one raster and gives maps and tables, run with the same settings on all of them.
- Results that must be kept per file (one MSPA map per watershed, named after it), with a record of how each was
  made.

When *not* to: when the maps should be analysed **together** (a time series as a stack, see
[Stacks and layers](../../building-models/stacks-and-layers/)); when one file is too large for memory (see
[Rasters larger than memory](../larger-than-memory/)); when each file needs a chain of several operators (then make a
model with a model input, and run it once per file: see [Your first model](../../getting-started/first-model/#from-the-command-line)
and the scenarios of `terraflux submit`).

## What you need

A folder of rasters. Here the CORINE Land Cover excerpt around Hanover of the getting-started tutorials, cut into
**16 tiles** of 500 x 500 cells (12.5 x 12.5 km), `tiles/clc_r0c0.tif` to `tiles/clc_r3c3.tif` (row and column of
the tile). In a real study the files would be your watersheds or your years; they need not have the same size or
grid.

```text
study/
├── batch.toml
└── tiles/
    ├── clc_r0c0.tif
    ├── clc_r0c1.tif
    ...
    └── clc_r3c3.tif
```

## Build the model

The whole batch is one node:

```tfgraph
#! caption: batch.run: MSPA of the forests of every GeoTIFF in the folder tiles, written to the folder mspa. Its table and report are the model's outputs.
#! highlight: batch
format = "terraflux/1"
name = "MSPA of every tile"

[[nodes]]
id = "batch"
op = "batch.run"
params = { operator = "landscape.mspa", inputs = "tiles", output_dir = "mspa", params = { foreground = [23, 24, 25], edge_width = 2 } }
note = "MSPA of the forests (CORINE 23, 24, 25) of every GeoTIFF of the folder tiles, written to the folder mspa."

[outputs.table]
from = "batch.table"

[outputs.report]
from = "batch.report"
```

Its settings:

| Setting | Here | Meaning |
|---|---|---|
| `operator` | `landscape.mspa` | the operator to run on each raster: any operator of the library that takes a map |
| `inputs` | `tiles` | a folder (its `.tif` files, in alphabetical order), a pattern such as `tiles/*.tif`, or a list of files |
| `output_dir` | `mspa` | the folder of the results (made if needed) |
| `params` | `{ foreground = [23, 24, 25], edge_width = 2 }` | the operator's settings, the same for every file |
| `input` | (not given) | which input of the operator takes the raster; by default its first map |
| `workers` | (not given) | how many rasters at the same time; by default as many as the threads |
| `overwrite` | (not given: false) | compute again the inputs already done |
| `recursive` | (not given: false) | the rasters of the folder's sub-folders too |

The operator's settings are those of its own node: [[landscape.mspa]]'s `foreground` and `edge_width` here. Paths
are relative to the model's folder.

!!! app "In the application"
    [[batch.run]] is in the library's subject **Large data & batch**. Type its settings in the Node pane;
    `params` is written as above, between braces. Select the node and press ++ctrl+enter++: the Viewer shows its
    table, a row per input.

## Run it

```text
$ terraflux run batch.toml --out run1
MSPA of every tile: inputs read (0.0 s)
wrote run1\report.txt
wrote run1\table.csv
done in 0.1 s
```

The 16 tiles took 0.1 second together: each took 0.05 to 0.08 seconds, and they ran side by side.

## Read the results

### The files

For each input, [[batch.run]] writes the operator's outputs named after the input and the operator:

- **maps** as `<input>_<operator>[_<output>].tif`, except MSPA, which uses GuidosToolbox's own naming: the input's
  name followed by the connectivity, the edge width, and the transition and internal flags, here
  `clc_r0c0_8_2_1_1.tif` (8-connectivity, edge width 2);
- **tables** as `.csv`, and texts as `.txt`;
- a **record** `<input>_<operator>.json` with the input, the files written, the settings, the numbers the operator
  gave, the time and Terraflux's version.

```text
$ ls mspa
clc_r0c0_8_2_1_1.tif
clc_r0c0_landscape_mspa.json
clc_r0c1_8_2_1_1.tif
clc_r0c1_landscape_mspa.json
...  (32 files)
```

```json
{
  "input": "tiles\\clc_r0c0.tif",
  "outputs": ["mspa\\clc_r0c0_8_2_1_1.tif"],
  "params": { "edge_width": 2, "foreground": [23, 24, 25] },
  "seconds": 0.054,
  "terraflux": "0.79.0",
  "tool": "landscape.mspa",
  "values": {}
}
```

The record is what you need for a methods section, or to know a year later with which settings each file was made.

### The table and the report

The node's `table` has a row per input: its number, `ok` (1, or 0 when it failed), `skipped` (1 when it was done
before, see below), its `seconds`, and the number of `files` written. Its `report` is a text with a line per input:
*tiles\clc_r0c0.tif: ok, 2 files*, or the error of an input that failed. Made an output of the model, the report
is written as plain text (`report.txt`), the table as CSV. A failure does not stop the batch: the other inputs run, and the report says which failed
and why.

![The 16 tiles' MSPA put together](/img/tutorials/large-data/batch-processing/mosaic.png "The 16 MSPA maps of the batch, put back in their places (the lines are the tiles' borders). Each tile was analysed alone, blind to what lies across its borders.")

The figure shows the maps put back together. Each tile was analysed alone, so it cannot see across its borders: a
forest cell next to a field just across a border is core in its tile and edge in the whole map, and an opening of
the forest is classed by what each tile sees of it. Compared cell by cell with MSPA of the whole map, 4,676 cells
(0.12%) differ, most of them along the borders. **A batch of tiles is not the same as an analysis of the whole
map.** That is fine when each file is a unit of its own (a watershed, a reserve, a year); for one large map cut into
tiles, use the out-of-core operators of [Rasters larger than memory](../larger-than-memory/), whose tiles overlap.

### Combining the results

A batch of tables is most useful combined. The same batch with [[metrics.class]] (`operator = "metrics.class"`,
`params = { metrics = ["pland", "np", "ed"], show = "pland" }`, `inputs = "tiles/*.tif"`) writes for every tile a
table `clc_r0c0_metrics_class_table.csv`, a map of `pland` and a record. Read them with any tool (a spreadsheet, R,
Python) and put them together; here, the share of forest, arable land and urban land of each tile:

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
caption: "The share of forest, arable land and urban land in each of the 16 tiles, from the 16 tables of a batch of metrics.class. Tile r2c3 is Hanover (59% urban); the southern tiles (r3c0 to r3c2) are the most forested."
```

## Resume where it stopped

Run the same model again:

```text
$ terraflux run batch.toml --out run2
MSPA of every tile: inputs read (0.0 s)
wrote run2\report.txt
wrote run2\table.csv
done in 0.0 s
```

Every input took less than a thousandth of a second: an input whose record (`.json`) exists in `output_dir` is
**skipped**. The table says so in its column `skipped` (1 for all 16 inputs here), and the report too:

```text
tiles\clc_r0c0.tif: skipped (done before; overwrite runs it again), 2 files
tiles\clc_r0c1.tif: skipped (done before; overwrite runs it again), 2 files
...
```

 A
batch of 500 watersheds that stopped at the 300th (a power cut, a closed laptop) continues from the 301st when you
run it again. To compute everything again after changing a setting, set `overwrite = true`, or use another
`output_dir`, which also keeps the two sets of results apart.

!!! warning "Skipped inputs keep their old results"
    A skipped input is not computed again, whatever has changed: if you change `params` and run again without
    `overwrite`, the old results stay, with their old records, and the table says `skipped` = 1 for every input.
    Look at that column (or at the report) after a run, and read the records' `params` when in doubt.

## Settings that change the answer

The answer is the operator's, so its settings are what matter: see its tutorial. Two of [[batch.run]]'s own
settings change **how** it runs, not the result:

- `workers`: rasters at the same time. More workers finish sooner, until the cores, the disk or the memory run out:
  each worker holds one raster and its results in memory. For large rasters, fewer workers.
- `overwrite`: compute again or not (above).

## Common pitfalls

- **Files of different classes.** The same `params` go to every file. If the forest is class 23 in some files and
  class 3 in others, reclassify them first, or run two batches.
- **Files of different resolutions.** An edge width of 2 cells is 50 m on 25 m cells and 60 m on 30 m cells.
  Resample the files to one cell size first ([One grid](../../getting-started/one-grid/)).
- **A forgotten `overwrite`.** Results of old settings are kept (see the warning above).
- **Output in the input folder.** With `inputs = "tiles"` and `output_dir = "tiles"`, the results become inputs of
  the next run. Keep them apart.
- **Tiles of one map.** Results near the borders of tiles differ from the whole map's for analyses of objects or
  windows (above). Use overlapping tiles or the out-of-core operators.

## Going further

- [Rasters larger than memory](../larger-than-memory/): one large map by strips and tiles.
- [Randomness and reproducibility](../../building-models/randomness/): a model run many times with `terraflux submit`.
- The [parallel computing guide](/parallel/): batches of model runs on many computers.
- The operators [[batch.run]], [[landscape.mspa]], [[metrics.class]].

## References

- Vogt, P., & Riitters, K. (2017). GuidosToolbox: universal digital image object analysis. *European Journal of Remote Sensing*, 50(1), 352–361. https://doi.org/10.1080/22797254.2017.1330650
