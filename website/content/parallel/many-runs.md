---
title: Many runs on your computer
short: Many runs on your computer
description: Replicates, scenarios and calibrations on the cores of one computer, from the command line and the Many runs pane - the scenarios file, the folders of results, how many runs at once, and what it costs in time.
order: 10
---

The most useful kind of parallel computing for an ecologist needs nothing but the computer on the desk: running a
model many times at once. Replicates show what is chance and what is the model; scenarios compare assumptions;
a calibration chooses settings against an observed map. This page shows how Terraflux runs all three on the cores
of one computer, from the command line (`terraflux submit`, `terraflux calibrate`) and from the application's
**Many runs** pane, how to write a scenarios file, where the results go, how many runs to put at once and how long
they take. What the runs are *for* in a land change study is in [Calibration and many
runs](/tutorials/land-change/calibration-and-many-runs/); here, the machinery.

## The idea

A **batch** is a model run many times with something changed each time: the seed (replicates), a set of settings
or inputs (scenarios), or the settings a calibration tries. Each run is a **task**; tasks do not depend on each
other, so they can run at the same time, each on its own share of the cores.

On one computer, `submit` and `calibrate` start, inside their own process, everything the [design of this
guide](/parallel/#how-it-works-in-one-picture) needs: a coordinator that holds the queue of tasks, and `--workers`
workers that take tasks from it, run them, and hand back their files. The same commands with `--coordinator`,
`--code` or `--on` send the same batch to other computers, unchanged.

!!! science "Key idea: how many at once"
    With \(W\) workers and \(N\) runs of \(t\) seconds each, a batch takes about \(\lceil N / W \rceil \times t\)
    when nothing else limits it. Each worker gets \(T / W\) threads of the \(T\) Terraflux may use. Since one run of
    a patch-based model hardly uses more than one thread, many workers of few threads use a computer better than
    few workers of many threads, until the memory, the disk or the slower cores limit them.

## When to use it

- **Replicates**: before reporting any simulated map or number, run the model with tens of seeds.
- **Scenarios**: a handful to a few dozen variants of the model, each with its replicates.
- **Calibration**: a grid or random draws of a few settings, each tried with a few seeds.

When the batch would take more than a night on your computer, see [the lab's computers](/parallel/lab-computers/)
or [a cluster](/parallel/slurm-cluster/).

## What you need

- A model file that runs on its own (`terraflux run model.toml` works), next to its data.
- A **seed** the model reads, for replicates and calibrations: `terraflux new` writes an input `seed`. A model with
  its seed in a node's setting is fine too: name it with `--seed-input` (`simulation/allocate.seed`, a node inside
  a loop, or several separated by commas).

## Run it

### Replicates

```bash
terraflux submit model.toml --replicates 50 --out replicates
terraflux submit model.toml --replicates 50 --first-seed 101 --workers 8 --out more
```

The runs use the seeds `--first-seed`, `--first-seed + 1`... (1 to 50 by default), each given to every key of
`--seed-input` (the input `seed` by default). `--workers` sets how many run at once on this computer (a quarter of
the cores by default, at least 1). While it runs, `submit` prints the progress:

```text
4 workers on this machine
job job-5: 50 tasks
  20 of 50 done, 4 running, 0 failed
  ...
  50 of 50 done, 0 running, 0 failed
wrote replicates
```

### Scenarios

A scenarios file is a TOML file with one `[[scenario]]` table per scenario: its `name` (a folder of the results)
and `set`, the values it changes:

```toml
[[scenario]]
name = "trend"                                     # the model as it is

[[scenario]]
name = "compact_growth"
set = { "patches.isometry" = 2.0, "patches.expansion" = 0.8 }

[[scenario]]
name = "new_road"
set = { "$drivers" = "drivers_with_road.tif", "$seed" = 7 }

[[scenario]]
name = "slower_pruning"
set = { "simulation/allocate.prune_factor" = 3 }
```

The keys of `set`:

| Key | What it sets | Example |
|---|---|---|
| `$name` | an input of the model: a value, or the path of a file input | `"$seed" = 2`, `"$drivers" = "drivers_2030.tif"` |
| `node.setting` | a node's setting, or a literal input of the node | `"patches.isometry" = 2.0` |
| `loop/node.setting` | a node inside a loop (or group): the path of containers, then the node | `"simulation/allocate.prune_factor" = 3` |

Two scenarios cannot share a name; a scenario without `set` runs the model unchanged. A file a scenario names
(`drivers_with_road.tif`) travels with the batch like the model's own files, so the same file works on other
computers.

```bash
terraflux submit model.toml --scenarios scenarios.toml --out scenarios
```

To run every scenario with replicates, write a scenario per seed (`trend_1`, `trend_2`... with `"$seed"`), or one
`--replicates` batch per scenario model.

A model can also keep its scenarios in its own file, made and compared in the application's **Parameters** pane
([below](#in-the-application-parameters-and-scenarios)).

### Calibration

```bash
terraflux calibrate model.toml --param patches.isometry=0.5:2.5 --param patches.expansion=0.1:0.9 \
    --observed landuse.tif:2011 --metric similarity:5 --seeds 1,2,3 --out calibration
```

- `--param key=from:to` varies a setting (keys as in scenarios). `--method grid` (the default) tries `--n` values of
  each (5), every combination; `key=from:to:points` gives one setting its own number of values. `--method random`
  draws `--n` candidates as a **Latin hypercube** (each range cut into `--n` strata, each stratum drawn once,
  reproducible), better with three settings or more.
- **Write the ends as the setting is**: `3:7` for a whole number (a window, a number of iterations), `0.0:1.0`
  for a share. The model decides: whole-number values when its value of the setting is a whole number, decimals when it is
  not (`expansion=0:1` of a share tries 0, 0.25, 0.5...); the written ends decide only for a setting it does not
  give.
  The values are rounded to 12 significant digits (0.7, not 0.7000000000000001).
- `--seeds 1,2,3`: every candidate runs with each seed; its score is the mean (common random numbers).
- `--observed file.tif:band` and `--start` (the input the simulation starts from, `t0` by default); `--metric fom`,
  `overall` or `similarity:<window>`, computed by the workers.

It writes `calibration.csv` (every candidate, best first), `best.json`, and the model with the best settings next
to the original (`model_calibrated.toml`). On Sibuyan, 25 candidates × 3 seeds (75 runs of a third of a second)
took 5 seconds with 4 workers.

### What a batch writes

```text
replicates/
  seed_1/        the model's outputs (maps.tif, steps.csv...) and the files its nodes write
  seed_2/
  ...
  tasks.csv      a row per task: task, worker, seconds, ok, error
```

Each task's folder has the model's outputs and the files its [[raster.write]] and [[vector.write]] nodes write,
by their names. `tasks.csv` is the first thing to read: `ok = false` and its `error` for a task that failed; the
`seconds` per task to plan the next batch. A batch stopped half way keeps the folders it fetched.

### In the application: Many runs

The toolbar's **Many runs...** opens the pane. Its three tabs are the three kinds of batch (**Replicates**,
**Scenarios** as a table of a row per scenario and a column per setting, **Calibration** with ranges, seeds, the
observed map and the score); the settings and inputs are picked from lists, so no key has to be typed. Under every
tab:

- **Each run keeps**: the outputs and files each run's folder receives (by default the maps of the nodes where the
  model ends);
- **Results in**: the folder (a new one per batch by default, `<model> runs/replicates 1/...`);
- **Run on**: this computer (and the computers helping it, see [the lab's computers](/parallel/lab-computers/)), or
  a cluster or server saved in the Computers pane;
- **Runs at once on this computer**: by default half the threads Terraflux may use, the application keeping the
  rest; each run gets an equal share of the threads.

**Run** starts the batch and shows its progress; **Stop** stops it. The runs go to the application's own
coordinator, whose jobs are kept in the user's application data (`pool`): close the application during a batch,
open it again, and the pane offers to **go on** (the runs done are not run again, their files not fetched again)
or to let it go. While a batch runs, the computer does not go to sleep. At the end, **Open the folder**, **Show the
runs of ... in the model** (a [[raster.read]] of `runs/*/file.tif`, every run as one series to analyse), and, for a
calibration, its scores against each setting and **Apply the best settings to the model**.

### In the application: parameters and scenarios

A model's **parameters** are the values someone using it is meant to change: an input of a value (`$name`) or a
node's setting (`node.setting`, `loop/node.setting` inside a container), each with a label, a unit, a range, a
step, a few choices or a line of help. The **Parameters** pane (**View > Parameters**) shows them as a form, so a
colleague can try another assumption without touching the graph: a slider for a number with a least and a most
value, a list for a few choices, a box for yes or no, a field for the rest. **+ Parameter** adds one from the
model's settings and inputs, ✏ edits its label, unit, range, choices and help, and **x** takes it out of the form
(the model keeps its value). Each change is an edit of the model (++ctrl+z++ undoes it), and the model runs again.

**Scenarios** are sets of those values kept by name. Type a name and press **Save the values** to keep the values
now as one (a scenario of the same name is replaced). Each scenario's row has a box to check it for a
comparison, its values, **Apply** (its values in the model) and **x** (forget it). The model file keeps both:

```toml
[[parameters]]
key = "patches.isometry"
label = "Isometry of the new patches"
min = 0.5
max = 2.5
step = 0.5
help = "The isometry of the new patches."

[[scenarios]]
name = "compact_growth"
note = "The trend, with an isometry of 2."
values = { "patches.isometry" = 2.0 }
```

These are not the `[[scenario]]` tables of a scenarios file for `submit`: in the model they are `[[scenarios]]`
with `values`. `terraflux run model.toml --scenario compact_growth` runs one from the command line.

To compare scenarios, check them, choose the output to compare (the model's outputs, the values its plots show,
else each node's first output) and press one of:

- **Compare**: each scenario checked runs here, one after the other, up to that output;
- **Compare in Many runs**: the scenarios checked run at once as a batch of the Many runs pane, wherever its **Run
  on** says (this computer and those helping it, or a cluster), each run keeping only the output compared. The
  pane says *running in Many runs* until the batch ends.

Either way, the results come back to the Parameters pane side by side, a column per scenario in the pane's order.
The first is the reference: under each of the others, its difference from the first. For a series (the maps of a
simulation), the last step is compared. Continuous maps, tables of the same shape and numbers are subtracted
("the others less this one"). **Maps of classes** (land use, MSPA's classes: whole numbers that the viewer draws as
classes) are not subtracted, since class 3 minus class 2 means nothing: the difference is a map of the **changes**,
the class each cell changed to and nothing where it stayed the same. Under it are how many cells changed class
(*N cells changed class (S%)*, the share of the cells that have a value in both maps) and the five most common
changes, from the first scenario's class to this one's, with their cells (*2 → 3: 8517*). This is what to read
when two scenarios of a land change model are compared: not only how much forest each leaves, but where they
differ and into what.

**To Many runs** puts all the scenarios in the Scenarios tab of Many runs (a row each, a column per value), for a
batch with every option of that pane; **Calibrate** puts the parameters that have a range in its Calibration tab,
each tried from its least to its most value (five values, or one per step when they are fewer).

## Read the results

### How long it takes

The batch of 24 replicates of Sibuyan's enlarged model (432 × 512 cells, 3.3 seconds a run) on the desktop
computer of the [overview](/parallel/#measured-how-much-faster-on-one-desktop-computer) (20 logical processors):

```tfchart
type: bar
data: /data/guide/parallel/timings.csv
x: cores
y: [batch]
labels: { batch: "24 replicates" }
colors: { batch: "#3b8a5a" }
xlabel: Workers
ylabel: Seconds
height: 280
caption: The time of a batch of 24 replicates with 1 to 19 workers (terraflux submit --replicates 24 --workers N; the shorter of two runs).
```

From 79 seconds with one worker to 44 with two and 23 with eight; more workers did not help. The first workers
pay off fully, the next ones less: they share the memory, the disk and, on this processor, slower efficiency
cores. A reasonable rule: start with a quarter to half of the cores (the defaults), time a small batch, and only
then decide whether to add workers here or computers elsewhere.

### How to estimate a batch

Time one run (`terraflux run`), multiply by the runs, divide by the workers, and add a margin: 500 replicates of a
model of 20 seconds on 8 workers take about 500 × 20 / 8 ≈ 21 minutes, if 8 runs fit in memory at once. When the
estimate is in days, move to more computers.

### A worked plan: a sensitivity analysis

Suppose a land change model of 20 seconds a run, and the question "how much do the isometry and the Expander's
share change the forest left in 2040?". Five values of each (25 combinations), each with 10 seeds: 250 runs.

<div class="steps" markdown>

1. **Time one run**: `terraflux run model.toml` takes 20 s and, in the task manager, about 1.5 GB.
2. **Choose the workers**: a computer of 16 GB, 8 cores: 6 workers leave memory for the system (6 × 1.5 = 9 GB).
3. **Estimate**: 250 × 20 / 6 ≈ 14 minutes, with some margin for the workers sharing the memory: say 20.
4. **Write the scenarios**: one `[[scenario]]` per combination and seed (250 of them, named `iso1.5_exp0.5_s3`,
   with `set = { "patches.isometry" = 1.5, "patches.expansion" = 0.5, "$seed" = 3 }`), written by a few lines of
   any scripting language. Each scenario's folder then holds the maps to measure the forest on.
5. **Run a tenth first** (3 combinations × 10 seeds), check the folders and the time, then the rest.

</div>

### Reproducible batches

Every run of a batch is reproducible: the model, the settings of its task and its seed decide its maps, whichever
worker runs it and in whatever order. To make a whole batch reproducible by someone else, keep with the results:
the model file, the scenarios file or the command line (with `--first-seed` and `--seeds`), the version of
Terraflux (`terraflux --version`), and `tasks.csv`. A run can then be repeated alone with
`terraflux run model.toml --set seed=17`, which gives the maps of `seed_17`.

## Settings that change the answer

The machinery does not change the maps: every run is the exact engine's, and a run gives the same maps whichever
worker ran it and however many ran at once (the seed decides the random numbers, not the order of the runs). What
changes the scientific answer is the batch itself: the number of replicates, the seeds per candidate, the ranges
and the metric of a calibration.

## Common pitfalls

- **A model that reads no seed**: every replicate is the same run. The pane warns ("the model has no seed").
- **Too many workers for the memory**: the system starts swapping and everything slows down; watch the memory in
  the task manager during a first small batch.
- **Results overwritten**: a second batch with the same `--out` writes into the same folders; give each batch its
  own.
- **Paths outside the model's folder** in a scenario: a file must exist where the scenario says, relative to the
  model's folder.
- **A share the model does not give**, in a calibration: its written ends decide (above), so write `0.0:1.0`, not
  `0:1`.

## Going further

- [The lab's computers](/parallel/lab-computers/): the same batches on the computers of the network.
- [Calibration and many runs](/tutorials/land-change/calibration-and-many-runs/): replicates, scenarios and
  calibration in a land change study.

```tfcli
submit
```

```tfcli
calibrate
```
