---
title: Configuration
description: "Threads, other computers, downloads and cache, memory, output files, environment variables and reproducibility: the settings of the Python library."
order: 40
---

Terraflux has no configuration file: a handful of settings, passed to the
functions that use them or set once for the Python session.

## Threads

Terraflux **never uses all the cores**: every thread count (the default, the
one you set with `set_default_threads` and an `n_threads` you pass) is held to
the **processors the process may use, minus a quarter of them** (at least one
stays free): 20 processors give at most 15 threads, 8 give 6, 4 give 3, 2 and 1
give 1. The computer stays responsive while it works. Inside a batch job of a
cluster (Slurm, PBS, LSF), or when the process may run on fewer processors than
the computer has, the rule applies to the processors of the job or of the
process. It is the same rule as the desktop app's. You can change the count per
call or for the whole session (up to that limit; a larger request is lowered,
with one warning per process):

```python
import terraflux as tf

tf.max_threads()          # processors available, e.g. 20
tf.default_threads()      # threads used when you do not say: 15

forest = tf.load_raster(tf.example_data("input"))
tf.mspa(forest, n_threads=4)       # this call only: 4 threads
tf.set_default_threads(8)          # from now on, every call: 8 threads
tf.set_default_threads(None)       # back to the automatic count
```

`n_threads=None` or `n_threads=0` always means the default. The results never
depend on the number of threads: the same input gives the same bits with 1 or
64 threads.

Functions that process many rasters at once (`tf.batch`, `tf.mspa` on a
collection, `tf.mspa_tiled`) also take `n_workers`, the number of rasters or
tiles in flight (`tf.batch` also `threads_per_worker`); by default the workers
share the same cores (at most the limit above).

Without `n_threads` or `tf.set_default_threads`, the count comes from the
environment, from the first that applies:

| | |
|---|---|
| `TERRAFLUX_THREADS` | this process and the processes it starts (`LANDSCHAFT_NUM_THREADS`, its name until 1.58, is still read) |
| a dask.distributed worker | the worker's threads |
| `OMP_NUM_THREADS` | as for any OpenMP program |
| a batch job (Slurm, PBS, LSF) | the job's processors (`SLURM_CPUS_PER_TASK`...) |
| the computer | the processors this process may run on (its affinity and CPU quota) |

`terraflux-py doctor` (or `tf.doctor()`) prints the count and where it came
from; see [on a cluster](/parallel/hpc-slurm-in-python/).

!!! app "From the command line"
    The application and its command line read the same `TERRAFLUX_THREADS`: see
    [environment variables](/cli/environment/).

!!! note "Jobs that run side by side"
    If you start several Python processes at once on one computer, give each a
    share of the cores (`TERRAFLUX_THREADS=4 python script.py`, or
    `tf.set_default_threads(4)` at the top of each script), so that together
    they do not use more cores than the machine has.

## Other computers

Functions that run many independent tasks (`batch`, `mspa_batch`,
`simulate.scenarios`, `simulate.calibrate`) take `on=tf.helpers()` and share
the tasks with the computers of a group running `terraflux-py worker`, with the
same results; see [several computers](/parallel/many-computers-in-python/). The
group's secret is kept in `%APPDATA%\Terraflux\python` (Windows) or
`~/.config/terraflux/python` (`TERRAFLUX_CONFIG_DIR` overrides it); the `landschaft`
folder of an older version is moved there the first time.

## Downloads and the cache

A few tools download reference data the first time they need it (each file
pinned by its checksum) into `%LOCALAPPDATA%\Terraflux\python\cache` (Windows),
`~/Library/Caches/terraflux/python` (macOS) or `~/.cache/terraflux/python` (Linux;
`XDG_CACHE_HOME` moves it); the `landschaft` folder of an older version is
moved there the first time. `TERRAFLUX_CACHE` puts it elsewhere, and
`terraflux-py fetch` downloads everything beforehand, for computers without
internet access.

## Memory

| Setting | Where | What it does |
|---|---|---|
| `max_memory="16GB"` | `tf.batch`, `tf.mspa` on a collection | rasters are started only while their estimated memory fits the budget |
| `block_rows=...` | `tf.Landscape`, `tf.landscape_metrics` | landscape metrics of a file read in blocks of rows, for rasters larger than memory |
| `tile_size=...` | `tf.mspa_tiled` | MSPA of rasters larger than memory, tile by tile |
| `memory_limit=...` | `tf.omniscape` | caps the memory of the moving windows (fewer threads for very large windows) |

## Output files

`tf.write_raster(data, "out.tif")` writes a compressed, tiled GeoTIFF with
the georeferencing, NoData and band names of `data` and, for class maps, their
colour table (`colormap="auto"`); pass `colormap=None` to leave it out or a
`{value: (r, g, b, a)}` dictionary for your own colours. It chooses what the
data need: NaN as the NoData of floats (or `nodata=-9999`), `uint8` for
`True`/`False` maps, the smallest integer type for labels, the format from the
extension (`.tif`, `.asc`, `.img`...; `driver="COG"` for a Cloud-Optimised
GeoTIFF). Dask arrays are written block by block, and missing folders are
created. Tables are pandas DataFrames; large metric tables can be streamed to
Parquet or CSV (see the [landscape metrics tutorial](/tutorials/landscape-metrics/large-maps/#in-python)).

## Environment variables

Terraflux was called landschaft until 1.58, and its variables then began with
`LANDSCHAFT_`: those names are still read (the `TERRAFLUX_` one wins when both
are set).

| Variable | Effect |
|---|---|
| `TERRAFLUX_CIRCUIT_AVX2=0` | use the generic build of the circuit solver instead of the AVX2 one (for comparisons; the default picks AVX2 when the processor has it) |
| `GDAL_*` | GDAL's own settings (cache size, number of threads for decoding...), honoured by all file reading |

## Reproducibility

Terraflux has no random numbers: the same input and options give the same
output on the same platform, whatever the number of threads. Record the
version (`tf.__version__`) with your results.
