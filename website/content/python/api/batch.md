---
title: Large data & batch
description: "Folders of rasters, tiled MSPA, maps larger than memory."
order: 140
---

Many rasters at once, maps larger than memory, and tiled MSPA. Tutorial:
[large data and batch processing](/tutorials/large-data/large-data-in-python/).

Most tools also accept a [`RasterCollection`](/python/api/data/#terraflux.RasterCollection)
or a lazy stack directly (e.g. `tf.mspa(tf.load_raster("tiles/*.tif"),
output_dir="out")`), and [`Landscape`](/python/api/landscape-metrics/#terraflux.Landscape)
reads large maps in blocks (`block_rows`).

## Folders of rasters

::: terraflux.batch

::: terraflux.mspa_batch

::: terraflux.BatchResult

::: terraflux.batch_status

::: terraflux.batch.Stopped

### Tools of `batch`

`tf.BATCH_TOOLS` lists the names `batch` accepts; any function taking a raster
as its first argument works too.

`accounting`, `clustering_fac`, `conefor_inputs`, `contagion_pff`, `contortion`, `distance_to_resistance`, `euclidean_distance`, `fad`, `fad_multiscale`, `fos`, `gray_spatial_convolution`, `gtn_components`, `gtn_importance`, `influence_zones`, `landscape_mosaic`, `landscape_mosaic_heatmap`, `landscape_mosaic_stats`, `mspa`, `parcellation`, `proximity`, `reclassify`, `restoration_status`, `spa`, `spatial_entropy`, `spatial_entropy_map`, `landscape_metrics`, `window_metrics`.

## Several computers

`on=tf.helpers()` runs the tasks of `batch`, `mspa_batch`,
`simulate.scenarios`, `simulate.calibrate`, `sdm.fit` (its cross-validation),
`uncertainty.monte_carlo`, the trade-off calibrations and replacement importance
of `tf.prioritize`, `parallel.map` and `parallel.tiled` on this computer and on
the computers of its group running `terraflux-py worker`. Tutorial:
[several computers](/parallel/many-computers-in-python/).

::: terraflux.parallel.helpers

::: terraflux.parallel.Helpers

::: terraflux.parallel.map

::: terraflux.parallel.tiled

::: terraflux.parallel.call

::: terraflux.parallel.param

`tf.parallel.TOOLS` lists the tools `map`, `tiled` and `monte_carlo` can run by name:
the analyses of `tf` (`"fad"`, `"mspa"`, ...), `tf.surface` (`"surface.geomorphons"`),
`tf.hydro`, `tf.services`, `tf.nlm` and `tf.circuit`.

::: terraflux.parallel.run_worker

::: terraflux.parallel.invite_helpers

::: terraflux.parallel.list_helpers

::: terraflux.parallel.leave_group

::: terraflux.parallel.open_firewall

### The `terraflux` command

| Command | What it does |
|---|---|
| `terraflux-py helpers invite [--minutes 10]` | show a code with which other computers join this one's group |
| `terraflux-py worker --join CODE` | join a group (the first time) and lend this computer |
| `terraflux-py worker [--cores N] [--connect HOST[:PORT]] [--map SRC=DST] [--cache-gb 20]` | lend this computer until Ctrl+C |
| `terraflux-py helpers list` | the helpers waiting for work on the local network |
| `terraflux-py helpers firewall` | Windows: let helpers reach this computer (TCP 47811, UDP 47810, local subnet) |
| `terraflux-py helpers leave` | take this computer out of its group |
| `terraflux-py batch TOOL INPUTS OUTPUT [key=value ...] [--shard I/N\|array] [--requeue] [--workers N] [--threads-per-worker N] [--max-memory 16GB]` | `tf.batch` from the command line |
| `terraflux-py status OUTPUT` | the rasters done, failed and left, and the job array tasks to submit again (exit code 3 while rasters are left) |
| `terraflux-py slurm TOOL INPUTS OUTPUT --cpus 16 [--mem 32G] [--time 2:00:00] [--partition P] [--submit] [key=value ...]` | write (and submit) the Slurm job script that splits the batch among the tasks of a job array |
| `terraflux-py fetch [sdm\|examples\|all]` | download beforehand what Terraflux downloads on first use |
| `terraflux-py doctor [--no-network]` | what Terraflux finds on this computer: threads, job, cache, network, versions |

On a cluster, see [on a cluster (HPC, Slurm)](/parallel/hpc-slurm-in-python/).

## Maps larger than memory

::: terraflux.mspa_tiled

::: terraflux.tile_layout
