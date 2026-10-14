# Many runs on many machines

A batch is a model run many times with something changed each time: the seed (replicates), a set of inputs and
settings (scenarios), the settings a calibration tries. Terraflux runs batches on the cores of one machine or on
the workers of a coordinator, with the same commands. Every run is the exact engine's, so a task gives the same
maps on any worker as on the machine that submitted it.

## From the application

The toolbar's **Many runs...** opens a pane with the three kinds of batches, run on workers of this computer
(as many at once as chosen; each gets an equal share of the threads):

- **Replicates:** the number of runs and the first seed. The seeds the runs change are found in the model (its
  inputs named `seed`, else the seeds its nodes are given), or ticked by hand.
- **Scenarios:** a table, a row per scenario and a column per setting it changes, any of the model's inputs and
  nodes' settings picked by name (an empty cell keeps the model's value).
- **Calibration:** settings in ranges (every combination, or random draws of a Latin hypercube), the seeds of
  each try, the simulated maps, the map they start from (an input or a `raster.read`), the observed map and its
  band, and the score.

Each run keeps the model's outputs, the files its nodes write (in the run's folder) and the outputs ticked
under "Each run keeps" (by default, the maps of the nodes where the model ends). The results go to a folder per
run (`<model> runs/replicates 1/seed_1/...`) with `tasks.csv`; "Show the runs of ... in the model" adds a
`raster.read` of `runs/*/file.tif`, the runs as one series. A calibration writes `calibration.csv` and
`best.json`, plots its scores against each setting, and "Apply the best settings to the model" puts them in it.

## On one machine

Without `--coordinator`, `submit` and `calibrate` start a coordinator and `--workers` workers in their own
process (a quarter of the cores by default):

```bash
terraflux submit model.toml --replicates 20 --out replicates --workers 8
terraflux submit model.toml --scenarios scenarios.toml --out scenarios
terraflux calibrate model.toml --param patches.isometry=0.5:2.5 --param patches.expansion=0.2:0.8 \
    --observed landuse.tif:2011 --metric similarity:5 --seeds 1,2,3 --workers 8
```

`submit` writes a folder per task with the model's outputs and `tasks.csv` (worker, seconds, errors).
`calibrate` writes `calibration.csv` (every candidate, its settings and score, best first), `best.json`, and the
model with the best settings next to the original (`model_calibrated.toml`).

### Scenarios

```toml
[[scenario]]
name = "business_as_usual"
set = { "$seed" = 1 }

[[scenario]]
name = "compact_growth"
set = { "$seed" = 1, "patches.isometry" = 2.0, "patches.expansion" = 0.8 }

[[scenario]]
name = "other_drivers"
set = { "$drivers" = "drivers_2030.tif" }
```

`$name` sets an input (a value, or the path of a file input); `node.setting` sets a node's setting or literal
input (`simulation/allocate.prune_factor` for a node inside the loop). `--seed-input` names what holds the seed
of replicates and calibrations: an input (`seed`), a node's (`simulation.seed`), or several, separated by commas.

The files the nodes write (`raster.write`, `vector.write`) go to each task's folder, by their names.

### Calibration

Each `--param key=from:to` is a range: `--method grid` tries `--n` values of each (every combination;
`from:to:points` gives a range its own number), `--method random` draws `--n` candidates (a Latin hypercube).
Every candidate runs with each of `--seeds`; its score is the mean. The metric is computed on the workers, the
last simulated map against the observed one from the same start (`--start`, the input `t0` by default):

| `--metric` | |
|---|---|
| `fom` | the figure of merit: hits / (hits + misses + false alarms + wrong hits) |
| `overall` | the share of cells of the same class |
| `similarity:<window>` | Dinamica EGO's fuzzy similarity of the changes in a window (1, 3, 5, 7, 9 or 11) |

## On many machines

```bash
# a machine the others reach
terraflux serve --listen 0.0.0.0:7700

# every machine that works (any number, started and stopped at any time)
terraflux worker --coordinator http://coordinator-host:7700

# anywhere that reaches the coordinator
terraflux submit model.toml --replicates 200 --coordinator http://coordinator-host:7700 --out results
terraflux workers --coordinator http://coordinator-host:7700
```

- **Connections:** workers only connect to the coordinator, so they need no open port and can run behind
  firewalls or NAT.
- **Data:** the data travels once. The client sends each file of the model once (by its SHA-256), and each
  worker fetches it once into its cache (`--cache`).
- **Workers that vanish:** a worker sends heartbeats while it runs a task. If it is not heard of for `--timeout`
  seconds (60 by default), the task goes to another worker; a task is tried three times.
- **Slurm:** `tools/slurm/terraflux-workers.sbatch` starts workers on the nodes of a Slurm allocation.

**Security:** the coordinator has no authentication or encryption yet. Anyone who reaches its port can submit
tasks and read their data. It listens on 127.0.0.1 unless given another address. Expose it only on a trusted
network, or reach it through an SSH tunnel (`ssh -L 7700:localhost:7700 coordinator-host`).

## What travels

| Message | From | To |
|---|---|---|
| the model (JSON), the overrides of each task, the evaluation | client | coordinator, then each worker |
| the input files (once, by hash) | client | coordinator, then each worker once |
| a task's output files (maps as GeoTIFF, tables as CSV), or its metrics | worker | coordinator, then client |

| a pass of the tiled engine (where the maps are, what to compute) and the tiles of each task | client | coordinator, then each worker |
| a pass's part of some tiles (histograms, counts, keys) | worker | coordinator, then client |

## One large map on many machines

Batches run many small models; the tiled engine runs one model too large for a machine (a continent at
30 m). Its tiles are spread over the workers:

```bash
terraflux worker --coordinator http://coordinator-host:7700          # on every machine that works
terraflux run model.toml --tiled --disk /shared/run-1/layers --coordinator http://coordinator-host:7700
```

- **Shared storage:** the workers read the model's maps and read and write the steps' layers themselves, at
  the same paths as the machine that runs the model (NFS, SMB, a cluster's parallel file system). Only the
  passes and their sums travel through the coordinator.
- **Passes:** every pass over the tiles (a histogram, a count, the growth of the patches) is a job whose
  tasks are runs of contiguous tiles (`4` per worker); the machine that runs the model merges their parts
  and decides the next pass.
- **The same maps:** the parts merge in any order into the same sums, so the maps are those of the same model
  run with `--tiled` on one machine, however many workers and whichever computed which tiles.
- **Workers that vanish:** as with batches, their tasks go to other workers; a task writes whole tiles under
  their final names only when done, so a task run twice writes the same tiles.

