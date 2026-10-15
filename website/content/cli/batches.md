---
title: "Batches: replicates, scenarios, calibration"
short: Batches
description: Run a model many times at once, with different seeds, scenarios or settings, and find the settings that best reproduce an observed map.
order: 20
---

A single run of a stochastic model is one possible future among many. To report a result you usually need
**many runs**: replicates with different random seeds, a set of scenarios, or a sweep of a setting to calibrate
it. The commands of this page run those batches on all the cores of your computer, and with one more option on
other machines, without changing the model.

The examples continue the Dinamica EGO model of Sibuyan from [Running models](/cli/models/); the outputs are
real, from a computer with 20 cores.

## terraflux submit

Runs a batch of a model: **replicates** (the same model with seeds 1, 2, 3...) or **scenarios** (the same model
with some inputs or settings changed). Each run is a **task**; the tasks run side by side on **workers**, and each
writes its outputs to a folder of its own.

```tfcli
submit
```

### Replicates

```text
> terraflux submit model.toml --replicates 4 --out replicates
4 workers on this machine
job job-5: 4 tasks
  0 of 4 done, 4 running, 0 failed
  4 of 4 done, 0 running, 0 failed
wrote replicates
```

```text
replicates/
├── seed_1/   maps.tif  steps.csv  weights.csv
├── seed_2/   ...
├── seed_3/
├── seed_4/
└── tasks.csv
```

`tasks.csv` says which worker ran each task, how long it took and whether it failed:

```text
task,worker,seconds,ok,error
seed_1,worker-2,0.27,true,""
seed_2,worker-1,0.26,true,""
seed_3,worker-3,0.26,true,""
seed_4,worker-4,0.27,true,""
```

How the seed reaches the model: by default, the model's input named `seed` (`--seed-input seed`). A model whose
seed is a node's setting says so: `--seed-input simulation.seed`, or several separated by commas.

How many workers: without a coordinator, `submit` starts one here with `--workers` workers, a quarter of the
cores by default (each worker's tasks use threads too, so a quarter keeps the machine busy without
overloading it). A model that is itself parallel (most operators are) gains little from more workers; many small
models gain a lot.

### Scenarios

A scenarios file lists the runs by name, and what each one changes: an input (`$name`) or a node's setting
(`node.setting`):

```toml
# scenarios.toml
[[scenario]]
name = "compact"
set = { "patches.isometry" = 2.0 }

[[scenario]]
name = "scattered"
set = { "patches.isometry" = 1.0, "$seed" = 2 }
```

```text
> terraflux submit model.toml --scenarios scenarios.toml --out scenarios
4 workers on this machine
job job-5: 2 tasks
  0 of 2 done, 2 running, 0 failed
  2 of 2 done, 0 running, 0 failed
wrote scenarios
```

Each scenario's outputs go to a folder of its name: `scenarios/compact/`, `scenarios/scattered/`. Node ids and
setting names are those of the model file (open it in a text editor, or click the node in the application: its
id and settings are in the Node pane).

### Long batches

`--detach` only submits the batch to a coordinator and prints its job; you can close the terminal, the batch goes
on, and `terraflux results <job>` fetches it later. This needs a coordinator that keeps running: one on another
machine (`--coordinator`, `--code`) or a cluster (`--on`); see [Many machines](/cli/distributed/).

## terraflux calibrate

Finds the settings that best reproduce an observed map. Terraflux runs the model with each candidate value of the
settings you name, compares each simulated map with the observed one, and ranks the candidates.

```tfcli
calibrate
```

```text
> terraflux calibrate model.toml --param patches.isometry=0.5:2.5 --n 5 \
      --observed landuse.tif:2011 --metric similarity:5
calibration: 5 candidates x 1 seeds
4 workers on this machine
job job-5: 5 tasks
  0 of 5 done, 4 running, 0 failed
  4 of 5 done, 1 running, 0 failed
  5 of 5 done, 0 running, 0 failed
best: {"patches.isometry":0.5} (similarity:5 = 0.760995)
wrote calibration\calibration.csv and model_calibrated.toml
```

`calibration/calibration.csv` ranks every candidate:

```text
rank,candidate,patches.isometry,similarity:5,runs
1,0,0.5,0.7609952554648208,1
2,4,2.5,0.743976662377797,1
3,3,2.0,0.7408510327820502,1
4,2,1.5,0.7393553877903667,1
5,1,1.0,0.7041361514443825,1
```

`model_calibrated.toml`, next to the model, is the model with the best settings applied, and
`calibration/best.json` holds them.

What to choose:

`--param node.setting=from:to`
:   A setting and its range; `from:to:points` gives the number of values of this one. Write the ends as the
    setting is: `3:7` for a whole number (a window of cells: 3, 4, 5, 6, 7), `0.0:1.0` for a share (0, 0.25, 0.5,
    0.75, 1); the model decides: the values are whole numbers when the model's value of the setting is one, decimals when it is
    not (so `0:1` of a share also tries 0.25, 0.5, 0.75); the written ends decide only for a setting the model
    does not give. The values are
    rounded to 12 significant digits (0.7, not 0.7000000000000001). Repeat `--param` for several settings: the
    grid then holds every combination, so their number grows fast (5 values of 3 settings are 125 runs).

`--method grid` or `random`
:   `grid` tries `--n` evenly spaced values of each range; `random` draws `--n` candidates by a Latin hypercube,
    better for several settings at once.

`--seeds`
:   How many seeds each candidate runs with; its score is the mean. With one seed, the ranking of close
    candidates may be chance: use 5 to 10 seeds before trusting a small difference (in the run above, one seed
    each, the four best candidates are within 0.02 of each other).

`--metric`
:   What to maximise: `fom` (the figure of merit of the changes), `overall` (the agreement of all cells), or
    `similarity:<window>` (Dinamica EGO's fuzzy similarity of the changes in a window of that many cells).
    Which one fits your question is explained in the tutorial on
    [validating and calibrating](/tutorials/#land-change).

`--observed`, `--start`
:   The observed map at the end of the simulated period, and the input the simulation starts from (`t0` by
    default).

!!! warning "Calibrate on one period, validate on another"
    A model calibrated and validated against the same observed map will look better than it is. Keep a later
    map aside to validate the calibrated model.

## terraflux jobs and terraflux results

A coordinator keeps its jobs (done or not) until they are fetched. `jobs` lists them; `results` follows one to its
end and writes its files, keeping those already written, so it can be run again after an interruption.

```tfcli
jobs
```

```tfcli
results
```

```bash
terraflux submit model.toml --replicates 500 --on hpc --detach   # prints job-12, and how to fetch it
terraflux jobs --coordinator http://server:7700
terraflux results job-12 --on hpc --out replicates
```

## In the application

The **Many runs** pane does all of this without a terminal: replicates, scenarios in a table, and calibration
against an observed map, with their progress and results, and the best settings applied to the model in a
click. The [parallel computing guide](/parallel/) shows both ways side by side.
