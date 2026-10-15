---
title: "Calibration and many runs: replicates, scenarios and settings chosen against the data"
short: Calibration and many runs
description: Run a land change model fifty times to see what is chance and what is the model, run scenarios side by side, and choose settings such as the isometry by scoring them against an observed map, from the command line and the Many runs pane.
order: 310
level: intermediate
time: 45 min
ops: [patches.observe, dinamica.step, validate.agreement, validate.similarity]
series: Validating and calibrating
part: 2
---

One run of a land change model is one draw among the many landscapes its random numbers could give. In this part
you will run Dinamica EGO's model of Sibuyan many times: fifty **replicates** to separate what the model says from
what chance says, three **scenarios** side by side, and a **calibration** that tries 25 combinations of two
settings, three seeds each, and scores every one against the observed map. You will do it with three commands
(`terraflux submit --replicates`, `terraflux submit --scenarios`, `terraflux calibrate`) and with the application's
**Many runs** pane, and learn to read the folders, tables and maps they write. Everything here runs on the cores of
one computer in seconds; the [guide to parallel computing](/parallel/) takes the same commands to the computers of
a lab, a cluster or the cloud.

## The idea

### Replicates: what is chance and what is the model

Dinamica EGO's allocation draws its seeds and grows its patches at random
([Expander and Patcher](/tutorials/land-change/expander-and-patcher/)), from the seed of the simulation. Run the
same model with seeds 1 to 50 and you get fifty landscapes with the same amounts of each class in partly different
places. What is common to all of them is what the model says: the consequence of the rates, the weights and the
shapes. What varies is chance. Two uses follow:

- a **map of the probability of change**: the share of the runs in which each cell changed (or became a class);
- the **distribution of any number** computed on the maps (a figure of merit, the area of forest left, a landscape
  metric, the habitat of a species): its mean and its spread over the runs, instead of one value.

!!! science "Key idea: one run is one sample"
    A simulated map \(M_s\) depends on the model \(\theta\) (its rates, weights and settings) and on the seed \(s\).
    A statistic \(T\) (a figure of merit, an area) computed on one run is a sample of a distribution \(T(\theta, s)\)
    over seeds. Comparing two models (or two settings) on one run each compares two samples: report the mean over
    \(n\) replicates, and its spread, and treat differences smaller than the spread as noise.

### Scenarios: the same model, other assumptions

A scenario changes an assumption and keeps the rest: other settings (compact or scattered patches), other inputs
(a drivers file with a new road, a start map with a new reserve), other rates. Run as a batch, the scenarios share
everything else, so their maps differ only by what the scenarios change (and by chance, which replicates measure).

### Calibration: settings chosen by the data

Some settings cannot be measured: the isometry of the patches cannot be read from two maps, and the measured
Expander's share may not give the best landscape. **Calibration** tries values, runs the model with each, and
scores the simulated map against the observed one. The ecological literature calls the general approach
*pattern-oriented modelling* (Grimm et al., 2005): choose the settings that reproduce the observed patterns,
several at once when possible (the location of change, its neighbourhood, the landscape's structure). Soares-Filho
et al. (2013) describe a method that calibrates Dinamica EGO's models by searching their settings automatically
against the observed change.

Two rules keep a calibration honest:

1. **Use the same seeds for every candidate** (common random numbers): then two candidates differ by their settings,
   not by their luck. `terraflux calibrate` runs every candidate with each of `--seeds`, and scores the mean.
2. **Calibrate and validate on different periods.** Settings tuned to reproduce 1997-2011 and then "validated"
   on 1997-2011 have only been fitted ([Validating a simulation](/tutorials/land-change/validating-a-simulation/#the-honest-test-a-period-the-model-did-not-see)).
   Sibuyan has two dates only, so the calibration below is a goodness of fit; with three dates, calibrate on the
   first period and validate on the second.

## When to use it

- **Replicates**, always, before reporting a simulated map or a number computed on it. Fifty is a common number;
  check that the mean of what you report no longer moves when more are added.
- **Scenarios**, to compare policies or assumptions (protection, a road, faster deforestation), each with its
  replicates.
- **Calibration**, for settings you cannot measure and that change the answer (the isometry, the Expander's share,
  CLUE-S's elasticities), with a metric that matches the use of the maps: the fuzzy similarity when near misses
  are acceptable, the figure of merit for exact cells.

A calibration is not a way to make a poor model good: a model whose drivers do not explain the change will find
its best settings at a low score. Check the drivers and the weights first.

## What you need

- A model whose randomness comes from a **seed** it reads: `terraflux new dinamica` writes an input `seed` that the
  allocation reads (`seed = "$seed"`). Replicates set it; `--seed-input` names another (a node's setting,
  `simulation/allocate.seed`, or several separated by commas).
- For a calibration, the **observed map** at the end of the simulated period (`file.tif:band`) and the **start**
  the simulation runs from (the input `t0` by default, `--start` another).
- Here, the model of [Dinamica EGO on Sibuyan](/tutorials/land-change/dinamica-on-sibuyan/), saved as
  `sibuyan.toml` next to `sibuyan_landuse.tif` and `sibuyan_drivers.tif`.

## Run them

### Replicates

```bash
terraflux submit sibuyan.toml --replicates 50 --out replicates --workers 4
```

`submit` starts, in its own process, a coordinator and `--workers` workers (a quarter of the cores by default) and
gives each run to a free worker; `--first-seed` sets the first seed (1). On a desktop computer of 20 threads the fifty
runs of Sibuyan took 3 to 4 seconds with 4 workers (about a third of a second each). It prints the progress
(`20 of 50 done, 4 running, 0 failed`) and writes:

```text
replicates/
  seed_1/maps.tif   seed_1/steps.csv   seed_1/weights.csv
  seed_2/...
  ...
  seed_50/...
  tasks.csv          # task, worker, seconds, ok, error
```

A folder per run holds the model's outputs (and the files its `raster.write` nodes write); `tasks.csv` says which
worker ran each task, in how many seconds, and any error. The full help of the command:

```tfcli
submit
```

### Scenarios

A scenarios file is TOML: a `[[scenario]]` per scenario, its `name` (the name of its folder) and `set`, the
values it changes:

```toml
[[scenario]]
name = "as_observed"

[[scenario]]
name = "compact"
set = { "patches.isometry" = 2.5, "patches.expansion" = 0.95 }

[[scenario]]
name = "scattered"
set = { "patches.isometry" = 1.0, "patches.expansion" = 0.1 }
```

A key `node.setting` sets a node's setting (here the `isometry` and `expansion` of the `patches` node) or a
literal input; `$name` sets a model input (`"$seed" = 2`, or `"$drivers" = "drivers_2030.tif"` for a file); a
node inside a loop is named through the loop, `simulation/allocate.prune_factor`. A scenario without `set` runs
the model as it is.

```bash
terraflux submit sibuyan.toml --scenarios scenarios.toml --out scenarios
```

writes `scenarios/as_observed/`, `scenarios/compact/` and `scenarios/scattered/`. Files a scenario names (a drivers
file of 2030) travel with the batch.

### Calibration

```bash
terraflux calibrate sibuyan.toml --param patches.isometry=0.5:2.5 --param patches.expansion=0.1:0.9 \
    --observed sibuyan_landuse.tif:2011 --metric similarity:5 --seeds 1,2,3 --out calibration --workers 4
```

- `--param key=from:to` is a setting to vary, in a range. With `--method grid` (the default) it takes `--n` values
  (5) from `from` to `to`, and every combination of the parameters' values is a **candidate**: here 5 × 5 = 25.
  `from:to:points` gives one range its own number of points; `--method random` draws `--n` candidates in a Latin
  hypercube (each range cut into `--n` strata, each drawn once), better when there are more than two or three
  settings.
- `--seeds 1,2,3`: every candidate runs with each seed; its score is the mean. 25 candidates × 3 seeds = 75 runs,
  which took about 6 seconds.
- `--observed`: the map the last simulated map is compared with, from the same start (`--start`, the input `t0`).
- `--metric`: what to maximise, computed on the workers:

| `--metric` | What it scores |
|---|---|
| `fom` | the figure of merit: hits / (hits + misses + false alarms + wrong hits) |
| `overall` | the share of cells of the same class (rarely a good choice: it rewards changing little) |
| `similarity:<window>` | Dinamica EGO's fuzzy similarity of the changes in a window of 1, 3, 5, 7, 9 or 11 cells |

It writes `calibration/calibration.csv` (every candidate, best first: `rank`, `candidate`, the settings, the score
and the runs), `calibration/best.json` (the best settings) and, next to the model, `sibuyan_calibrated.toml`: the
model with the best settings in it.

```tfcli
calibrate
```

!!! warning "Write ranges with decimals"
    For a setting the model file does not give (the Expander's share here, which [[patches.observe]] measures),
    the way the range is written decides: ends written as whole numbers make a range of **whole numbers**, and
    `--param patches.expansion=0:1` tries only 0 and 1 (the five values 0, 0.25... are rounded, and the repeats
    dropped). Write `patches.expansion=0.0:1.0` for a share. Whole-number ranges are meant for settings such as a
    number of steps or a window. Check the values tried in the first lines of `calibration.csv`.

### In the application: Many runs

The toolbar's **Many runs...** opens a pane with the three kinds of batch as tabs, run on workers of this computer
(**Runs at once on this computer**: each run gets an equal share of the threads):

- **Replicates**: the number of runs and the first seed. The pane finds the seeds the model reads (its input named
  `seed`, else the seeds its nodes are given) and lists them under *Seeds changed*; they can be ticked by hand.
- **Scenarios**: a table, a row per scenario (**+ Scenario**) and a column per setting it changes
  (**+ Setting to change**: any input of the model or setting of a node, picked from a list); an empty cell keeps
  the model's value.
- **Calibration**: the settings in ranges (**+ Setting to calibrate**: from, to and the number of values), *every
  combination* or *random draws*, the **Seeds of each try**, the **Simulated maps** (which output's last map is
  scored), **Starting from**, the **Observed map** and its band, and the **Score**: fuzzy similarity in a window,
  figure of merit, or same class.

Under every tab, **Each run keeps** lists what each run's folder receives (by default the maps of the nodes where
the model ends), and **Results in** the folder. **Run** starts the batch, with its progress. The results go to a
folder per run, as from the command line; **Show the runs of ... in the model** adds a [[raster.read]] of
`runs/*/file.tif`, the runs as one series to analyse in the model. A calibration plots its scores against each
setting, and **Apply the best settings to the model** writes them into the model. A batch goes on if the
application is closed: opened again, the pane offers to go on (the runs done are not run again) or to let it go.
**Run on** sends the batch to a cluster or a server saved in the Computers pane ([parallel guide](/parallel/)).

## Read the results

### Fifty replicates

![How often each cell changed in 50 replicates](/img/tutorials/land-change/calibration-and-many-runs/change_frequency.png "The share of the 50 runs in which each cell of Sibuyan changed between 1997 and 2011 (pale yellow: in none). Dark cells change in nearly every run: the model is sure of them. Light ones change in a few: chance decides.")

Of the 7,226 cells of the island, 69 changed in every one of the fifty runs, 1,618 in some, and 5,539 in none;
the observed change was 743 cells. The forest left in 2011 was 4,161 cells in every run (the rates fix the amounts:
4,160 observed), but its place was not:

```tfchart
type: bar
data: /data/tutorials/land-change/calibration-and-many-runs/frequency.csv
x: share of runs
y: [cells]
colors: { cells: "#3b8a5a" }
xlabel: Share of the 50 runs in which the cell changed
ylabel: Cells
height: 280
caption: The cells that changed in at least one run, by the share of runs in which they changed. A U shape: many cells change almost always (the model's core of change) or almost never (chance), fewer sit in between.
```

The U shape is typical: the large blocks of grassland and coconut are sure, their fringes uncertain. For an
ecologist this map is often more useful than any single run: a reserve placed where change is near-certain is a
different decision from one placed where it happens in one run of ten.

```tfgallery
columns: 2
caption: The figure of merit and the fuzzy similarity (5 x 5) of the 50 replicates against the observed map of 2011. The dashed line is the mean.
images:
  - { src: /img/tutorials/land-change/calibration-and-many-runs/fom_hist.png, caption: "Figure of merit: mean 0.461, sd 0.023, 0.414 to 0.512" }
  - { src: /img/tutorials/land-change/calibration-and-many-runs/similarity_hist.png, caption: "Fuzzy similarity: mean 0.759, sd 0.021, 0.715 to 0.801" }
```

Fifty seeds of the same model give figures of merit from 0.41 to 0.51. A single run could have reported either.

### Three scenarios

```tfgallery
columns: 3
caption: The change of 1997-2011 in the north-west of Sibuyan in three scenarios (seed 1). As observed, the patches as measured; compact, isometry 2.5 and 95% expansion; scattered, isometry 1 and 10% expansion.
images:
  - { src: /img/tutorials/land-change/calibration-and-many-runs/scenario_as_observed.png, caption: "As observed: 37 clumps of change" }
  - { src: /img/tutorials/land-change/calibration-and-many-runs/scenario_compact.png, caption: "Compact: 33 clumps" }
  - { src: /img/tutorials/land-change/calibration-and-many-runs/scenario_scattered.png, caption: "Scattered: 43 clumps" }
```

![Legend of the change](/img/tutorials/land-change/calibration-and-many-runs/change_legend.png)

The same amount of change, placed by the same probabilities, makes 33 to 43 separate clumps on the whole island:
the scattered scenario opens more small clearings in the forest. Report scenarios with replicates each (a batch
of scenarios × seeds: a scenarios file with a `"$seed"` per row, or one `--replicates` batch per scenario).

### The calibration

![The scores of the calibration](/img/tutorials/land-change/calibration-and-many-runs/calibration_grid.png "The fuzzy similarity (5 x 5, the mean of seeds 1, 2 and 3) of the 25 candidates of the calibration: the isometry along, the Expander's share up.")

`calibration.csv` ranks the candidates. The best, an isometry of 2.0 with an Expander's share of 0.7, scores 0.812;
the next ones 0.807 (2.5 and 0.5) and 0.806 (0.5 and 0.5). Calibrated by the figure of merit instead, the same
candidate wins (0.506). Read the grid before trusting the winner:

- the **Expander's share** matters: 0.5 to 0.7 score 0.80 to 0.81 whatever the isometry, 0.1 and 0.9 score 0.75 to
  0.78. The data say clearly that about two thirds of the change grows from existing patches;
- the **isometry** barely matters on Sibuyan: along a row the scores move by 0.01 to 0.03, no more than the
  spread of replicates (a standard deviation of 0.021 for the similarity). The single best cell (2.0, 0.7) is
  not reliably better than its neighbours. Three seeds are too few to separate them; with ten, or with the
  replicates' spread in mind, report the setting as "0.5 to 0.7 expansion; isometry not identifiable from these
  data".

That last sentence is a result, and an honest one: a calibration that cannot tell settings apart says that the
landscape's change does not constrain them, and the conclusions should not depend on them (check that they do
not, with scenarios over the plausible range).

`best.json` holds the winner (`{"patches.expansion": 0.7, "patches.isometry": 2.0}`; the values of a grid are
rounded to 12 significant digits, so 0.7 and not 0.7000000000000001), and `sibuyan_calibrated.toml` is the model with it, ready to run its replicates.

### How to report it

The settings calibrated, their ranges and the method (grid of how many points, or random draws); the metric and
its window; the seeds per candidate; the best settings with their score *and* the scores of the runners-up; the
period calibrated on and the period validated on. For replicates: their number, and every result as a mean with its
spread (or the map of the share of runs).

## Settings that change the answer

- **The metric.** The fuzzy similarity rewards near misses, the figure of merit only exact cells; on Sibuyan they
  agree on the best candidate, but they need not. Choose the one that matches how the maps will be used.
- **The window** of the similarity: a larger window rewards placing change in the right region rather than the
  right cells.
- **The number of seeds per candidate**: more seeds, smaller noise in the scores, more runs. Count: candidates ×
  seeds runs.
- **The number of workers** (`--workers`): more runs at once, until the computer's cores, memory or disk are
  saturated; on a desktop computer the gain stops between 4 and 8 workers for a model of this size (see
  [Many runs on your computer](/parallel/many-runs/)).

## Common pitfalls

- **Whole-number ranges** (`0:1`) for settings that are shares: see the warning above.
- **A model that reads no seed.** If no input or node reads the seed, every replicate is the same run. The Many
  runs pane says "the model has no seed"; on the command line, check that two replicates differ.
- **Calibrating on the period you then validate on.** Keep a period apart when you can.
- **Trusting the single best candidate** when its score is within the noise of the next ones.
- **Too many settings.** Every setting multiplies the candidates of a grid (5 values of 4 settings: 625
  candidates × seeds). Use `--method random`, or calibrate the settings that matter first.
- **Forgetting where the results went.** `submit` writes into `--out` (default `batch`), `calibrate` into `--out`
  (default `calibration`) and the calibrated model next to the original.

## Going further

- [Validating a simulation](/tutorials/land-change/validating-a-simulation/): the metrics used as scores here.
- [Maps larger than memory](/tutorials/land-change/maps-larger-than-memory/): when one run no longer fits on the
  computer.
- [Parallel and distributed computing](/parallel/): the same batches on the computers of a lab, a Slurm cluster or
  machines rented in the cloud.
- [Expander and Patcher](/tutorials/land-change/expander-and-patcher/): what the isometry and the Expander's share
  do.

## References

- Grimm, V., Revilla, E., Berger, U., Jeltsch, F., Mooij, W. M., Railsback, S. F., Thulke, H.-H., Weiner, J., Wiegand, T., & DeAngelis, D. L. (2005). Pattern-oriented modeling of agent-based complex systems: lessons from ecology. *Science*, 310(5750), 987–991. https://doi.org/10.1126/science.1116681
- Soares-Filho, B., Rodrigues, H., & Follador, M. (2013). A hybrid analytical-heuristic method for calibrating land-use change models. *Environmental Modelling & Software*, 43, 80–87. https://doi.org/10.1016/j.envsoft.2013.01.010
