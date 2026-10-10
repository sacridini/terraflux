# Many runs on many machines

A batch is a model run many times with something changed each time: the seed (replicates), a set of inputs and
settings (scenarios), the settings a calibration tries. Terraflux runs batches on the cores of one machine or on
the workers of a coordinator, with the same commands. Every run is the exact engine's, so a task gives the same
maps on any worker as on the machine that submitted it.

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
input (`simulation/allocate.prune_factor` for a node inside the loop).

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

Batches of large maps (a continent at 30 m) are the work of the tiled engine (next): its tasks are parts of one
run, its data read from shared storage rather than sent.
