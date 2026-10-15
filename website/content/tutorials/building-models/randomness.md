---
title: Randomness and reproducibility
short: Randomness
description: Where chance enters a model (neutral landscapes, simulations, sampling, Monte Carlo), what a seed is, why the same seed gives the same bits, how to run replicates and scenarios with terraflux submit, and how many replicates are enough.
order: 50
level: intermediate
time: 35 min
ops: [nlm.random_cluster, rng.r, rng.numpy, landscape.mspa, metrics.class]
---

Many models in landscape ecology draw random numbers: a neutral landscape is drawn by chance, a land change
simulation chooses which cells change by lot, a species distribution model samples background points, a Monte
Carlo analysis draws its parameters. In this tutorial you will learn what a **seed** is and how Terraflux uses it so
that a random model gives **exactly the same result** every time you run it, and how to run a model many times
with different seeds (**replicates**) to see how much of a result is due to chance. You will run 100 replicates of
a forest's core area on random landscapes, and find how many replicates were needed.

## The idea: pseudo-random numbers and seeds

Computers do not draw truly random numbers. They use a **pseudo-random number generator**: a formula that, from a
starting number called the **seed**, produces a long sequence of numbers that look random and pass every
statistical test of randomness, but are entirely determined by the seed. Give it the same seed and it gives the
same sequence; give it another seed and it gives a different one. The generator most scientific software uses,
R's and numpy's legacy one, is the *Mersenne Twister* (Matsumoto & Nishimura, 1998).

This is a feature, not a flaw. It lets you **reproduce** a stochastic result exactly: a reviewer who runs your
model with your seed gets your maps, to the last cell. And it lets you **vary** the chance on purpose: run with
seeds 1, 2, ..., 100 and you get 100 independent draws of what chance can do, from which you can measure the
variability of a result.

!!! science "Key idea"
    A stochastic model is a function \( y = f(x, s) \) of its data and settings \(x\) and a seed \(s\). With \(x\)
    and \(s\) fixed, \(y\) is fixed: the run is **reproducible**. With \(x\) fixed and \(s\) varied, the spread of
    \(y\) over seeds is the model's **stochastic variability**: the part of the result due to chance alone. A
    result smaller than that spread (a difference between two scenarios, say) cannot be told apart from chance.

Reporting stochastic models well means reporting both: the seed(s), so others can reproduce, and the spread over
replicates, so others know how much to trust a single run (Grimm et al., 2006, 2020).

## Where chance enters a Terraflux model

| Where | Operators | How the seed is given |
|---|---|---|
| Neutral landscape models | [[nlm.random_cluster]], [[nlm.gaussian_field]], [[nlm.fbm]] and the other `nlm.*` | the setting `seed` (R's generator: NLMR's map for that seed) |
| Land change simulations | [[dinamica.step]], `cars.step`, [[crafty.allocate]], `futures.simulation` | the input `seed`, usually a model input `$seed` |
| CLUE-S and MOLUSCE | `lulcc.step`, `molusce.step` | a generator made by [[rng.r]] or [[rng.numpy]] from a seed, shared by every step |
| Species distributions | [[sdm.background]], `sdm.thin`, the folds of cross-validation, [[sdm.prepare]] | the setting `seed` |
| Prioritization | [[prioritize.marxan]], [[prioritize.zonation]] | the setting `seed` |
| Monte Carlo | [[uncertainty.monte_carlo]] | the setting `seed` (numpy's generator) |
| Clusters and biodiversity | `metrics.cluster`, `bio.randomise`... | the setting `seed` |

Every one of them has a default seed, so a model that never mentions a seed is still reproducible: it always uses
the same one.

### Generators carried through a loop

Most operators make their own generator from their seed each time they run. Two kinds of simulation instead
reproduce software that draws from **one** generator for the whole run: lulcc's CLUE-S in R (`set.seed()` once,
then every step draws after the previous one) and MOLUSCE in Python. For them, [[rng.r]] (R's generator after
`set.seed(seed)`) or [[rng.numpy]] (numpy's after `numpy.random.seed(seed)`) makes the generator once, outside the
loop, and every step of the loop draws from it in turn:

```tfgraph
#! caption: How the models of terraflux new clue draw: one R generator made outside the loop (node rng), used by every step of the allocation. A fragment: the regression, the demand and the history are left out.
#! fragment: true
#! highlight: rng
format = "terraflux/1"

[inputs.seed]
kind = "value"
value = 1

[[nodes]]
id = "rng"
op = "rng.r"
inputs = { seed = "$seed" }
note = "R's generator after set.seed(seed), as lulcc draws."

[[nodes]]
id = "simulation"
op = "control.repeat"
inputs = { times = 14, landscape = "$t0" }

[nodes.body]
carry = { landscape = "allocate.landscape" }
collect = { maps = "allocate.landscape" }

[[nodes.body.nodes]]
id = "allocate"
op = "lulcc.step"
inputs = { landscape = "@landscape", step = "@step", rng = "rng.rng" }
```

This is how Terraflux reproduces lulcc's simulations bit for bit. A node that gives a generator is never taken
from the cache (its state changes as it is used), so it is made again at every run.

### Random numbers by counter

Dinamica EGO's model in Terraflux, CRAFTY's competition for land and the tiled engine draw differently: each
random number is computed from the seed, the step and the cell (a *counter-based* generator). The number a cell
gets does not depend on the order in which cells are visited, nor on how many threads or computers share the work.
This is what makes the [tiled engine](../../large-data/larger-than-memory/) give the same maps whatever the tiling.

## When to use it

- **Always: fix and report the seed** of every stochastic node, so a run can be reproduced.
- **Replicates** (same data and settings, different seeds) to measure the stochastic variability of a simulation,
  a neutral landscape, a Monte Carlo analysis or a heuristic optimisation (Marxan's runs, for instance).
- **Scenarios** (different settings, or data) to compare policies, climates or parameters; with replicates of
  each, to tell whether their difference is larger than chance.
- **Neutral landscapes as a null model**: replicates of random landscapes with a known structure show what a
  landscape metric would be by chance alone (With & King, 1997); see [Neutral landscape models](../../neutral-landscapes/).

## What you need

Only Terraflux. The model draws its landscapes: a 300 x 300 cell map of three classes (1 farming, 2 water,
3 forest, 55% of it) from [[nlm.random_cluster]], MSPA of its forest, and the share of each MSPA class
([[metrics.class]], the metric `pland`). The question: **how much core forest does a landscape with 55% forest,
clustered as this one, have?** Every seed gives a different landscape with the same structure, so the answer is a
distribution, not a number.

```tfgraph
#! caption: The model: a random landscape (its seed a setting of node 1), MSPA of its forest, and the share of each MSPA class. Its outputs are written by terraflux run and terraflux submit.
#! numbered: true
#! highlight: landscape
format = "terraflux/1"
name = "Core of a random forest"

[[nodes]]
id = "landscape"
op = "nlm.random_cluster"
params = { rows = 300, columns = 300, p = 0.55, weights = [0.3, 0.15, 0.55], seed = 7 }
note = "Three classes drawn by chance: 1 farming, 2 water, 3 forest (55 %)."

[[nodes]]
id = "mspa"
op = "landscape.mspa"
inputs = { map = "landscape.map" }
params = { foreground = [3], edge_width = 1 }
note = "MSPA of the forest (class 3)."

[[nodes]]
id = "classes"
op = "metrics.class"
inputs = { map = "mspa.map" }
params = { metrics = ["pland"], show = "pland" }
note = "The share of the map of every MSPA class (pland, %)."

[outputs.mspa]
from = "mspa.map"

[outputs.classes]
from = "classes.table"

[outputs.landscape]
from = "landscape.map"
```

## One seed, one landscape

Change the `seed` of node 1 in the Node pane and run: a new landscape, with the same share of forest on average
and the same kind of clusters, but its forest elsewhere. Seeds 1 to 8:

```tfslider
label: Seed
caption: "Left, the landscape drawn by nlm.random_cluster with seeds 1 to 8 (forest green, farming yellow, water blue); right, MSPA of its forest (core bright green, edge black, background grey). The structure is the same; the place of each patch, and the share of core, change with the seed."
frames:
  - { value: 1, src: /img/tutorials/building-models/randomness/seed_1.png, caption: "seed 1", stats: { "Forest": "53.1%", "Core": "36.0%" } }
  - { value: 2, src: /img/tutorials/building-models/randomness/seed_2.png, caption: "seed 2", stats: { "Forest": "49.7%", "Core": "31.7%" } }
  - { value: 3, src: /img/tutorials/building-models/randomness/seed_3.png, caption: "seed 3", stats: { "Forest": "53.0%", "Core": "36.3%" } }
  - { value: 4, src: /img/tutorials/building-models/randomness/seed_4.png, caption: "seed 4", stats: { "Forest": "60.2%", "Core": "42.2%" } }
  - { value: 5, src: /img/tutorials/building-models/randomness/seed_5.png, caption: "seed 5", stats: { "Forest": "51.0%", "Core": "34.0%" } }
  - { value: 6, src: /img/tutorials/building-models/randomness/seed_6.png, caption: "seed 6", stats: { "Forest": "56.2%", "Core": "38.1%" } }
  - { value: 7, src: /img/tutorials/building-models/randomness/seed_7.png, caption: "seed 7", stats: { "Forest": "53.9%", "Core": "36.9%" } }
  - { value: 8, src: /img/tutorials/building-models/randomness/seed_8.png, caption: "seed 8", stats: { "Forest": "54.7%", "Core": "37.2%" } }
```

Run seed 7 again, tomorrow, on another computer, with 1 thread or with 16: the result is the same to the bit. The
figure script of this page checks it on MSPA's map, comparing a fingerprint (a SHA-256 hash) of the map from a run
with all the cores and a run with one:

```text
mspa.tif of seed 7, all threads: 4c6420c9005fbb4c; one thread: 4c6420c9005fbb4c
```

## Many seeds: replicates

Running a model 100 times by hand is no fun. `terraflux submit` runs a **batch**: the model many times, each run a
*task*, spread over the cores of your computer (or over many computers; see the
[parallel computing guide](/parallel/)). `--replicates N` runs it with the seeds 1 to N (or from `--first-seed`),
and `--seed-input` says where the seed goes: a model input (`seed`, the default) or a node's setting
(`landscape.seed`):

```text
$ terraflux submit model.toml --replicates 100 --seed-input landscape.seed --out replicates
4 workers on this machine
job job-5: 100 tasks
  0 of 100 done, 4 running, 0 failed
  24 of 100 done, 4 running, 0 failed
  62 of 100 done, 4 running, 0 failed
  100 of 100 done, 0 running, 0 failed
wrote replicates
```

The folder `replicates` holds a folder per task, `seed_1` to `seed_100`, each with the model's outputs
(`landscape.tif`, `mspa.tif`, `classes.csv`), and `tasks.csv`, a row per task with the worker that ran it, its
time, and whether it succeeded. In the application, the **Many runs** pane does the same without the command
line.

!!! tip "Give the seed a name"
    For models you will run in batches, make the seed a **model input** (`[inputs.seed] kind = "value"`,
    `value = 1`) and connect it where it is needed (`seed = "$seed"`): then `--replicates` needs no
    `--seed-input`, `terraflux run --set seed=7` runs any seed, and several stochastic nodes can share it. A
    setting such as `nlm.random_cluster`'s `seed` cannot be connected to an input; reach it with
    `--seed-input node.seed` instead. Several places can be given at once, separated by commas.

## Read the results

The share of the map that is core forest, over the 100 replicates:

```tfchart
type: bar
data: /data/tutorials/building-models/randomness/histogram.csv
x: core
y: [replicates]
labels: { replicates: "Replicates" }
colors: { replicates: "#3b8a5a" }
xlabel: Core forest (% of the map)
ylabel: Replicates
height: 280
caption: "The core forest of 100 random landscapes with the same structure: mean 37.8% of the map, standard deviation 3.7, from 27.8% to 46.7%. 95% of the replicates fall between 31.3% and 45.4%."
```

One run (seed 7) said 36.9%. The replicates say that a landscape like this has **37.8% ± 3.7% core** (mean ±
standard deviation), and that any single landscape may have from about 31% to 45%. If you measured 25% of core in a
real landscape with 55% forest and this structure, the replicates tell you it is lower than chance alone would
give: something (roads, the shape of the fields, the history of clearing) has cut the forest more than random
clustering does. This is the logic of neutral landscape models as null models (Gardner et al., 1987; With &
King, 1997).

### How many replicates are enough?

The mean of \(n\) replicates is itself uncertain: its standard error is \( s / \sqrt{n} \), where \(s\) is the
standard deviation of the replicates. The chart shows the mean as replicates accumulate, with its 95% interval
(\( \pm 1.96\, s/\sqrt{n} \)):

```tfchart
type: line
data: /data/tutorials/building-models/randomness/running.csv
x: replicates
y: [mean, low, high]
labels: { mean: "Mean core (%)", low: "95% interval, low", high: "95% interval, high" }
colors: { mean: "#3b8a5a", low: "#9aa59d", high: "#9aa59d" }
xlabel: Replicates
ylabel: Core forest (% of the map)
height: 300
caption: "The mean of the first n replicates and its 95% confidence interval. After 10 replicates the mean is known to about ±2.5 points; after 100, to about ±0.7. Halving the interval takes four times as many replicates."
```

Choose the number of replicates for what you need to know:

- to report **the mean** to a given precision \(e\), you need about \( n = (1.96\, s / e)^2 \) replicates: with
  \(s = 3.7\) and \(e = 0.5\) points, about 210;
- to describe **the spread** (the 2.5% and 97.5% quantiles), you need many more, a few hundred at least, since
  the extreme quantiles rest on few replicates each;
- to compare **two scenarios**, enough that their intervals do not overlap by chance; or test the difference
  with the replicates of both.

Note the converse too: since you can always run more replicates, any difference between two scenarios, however
small, becomes "statistically significant" with enough of them. White et al. (2014) argue that significance tests
are therefore meaningless for simulation results; report the size of a difference and the spread over replicates
instead, and say how the number of replicates was chosen. A plot like the one above is the simplest way.

## Scenarios: settings that change

A **scenario** is a run with some settings or inputs changed. A TOML file lists them; each `set` changes model
inputs (`"$seed" = 2`) or node settings (`"mspa.edge_width" = 2`):

```toml
[[scenario]]
name = "edge1"
set = { "mspa.edge_width" = 1 }

[[scenario]]
name = "edge2"
set = { "mspa.edge_width" = 2 }

[[scenario]]
name = "edge3_seed2"
set = { "mspa.edge_width" = 3, "landscape.seed" = 2 }
```

```text
$ terraflux submit model.toml --scenarios scenarios.toml --out scenarios
4 workers on this machine
job job-5: 3 tasks
  3 of 3 done, 0 running, 0 failed
wrote scenarios
```

Each scenario's outputs go to a folder of its name. Here the core forest is 36.9% of the map with an edge of 1
cell, 22.9% with 2, and 10.2% with 3 on the landscape of seed 2. The last scenario changes two things at once,
the edge and the landscape, so its result cannot be attributed to either: in a real study, cross every setting
with the same set of seeds.

## Settings that change the answer

The seed is meant to change the answer, and by how much is what replicates measure. What matters is the number of
replicates (above) and **which seeds**: use consecutive seeds (1, 2, 3...) or any fixed list, and report it; never
choose seeds after seeing their results. In a comparison of scenarios, give every scenario the **same** seeds
(*common random numbers*): the scenarios then differ only by their settings, and their difference is measured
more precisely than with independent seeds.

## Common pitfalls

- **Changing a seed until the result looks good.** This is fishing. Fix the seeds before running, and report all
  replicates.
- **Several stochastic nodes with the same seed.** Two [[nlm.gaussian_field]] nodes with the same seed and size
  give the same field: two "independent" drivers that are identical. Give each its own seed.
- **One run reported as the result.** A single simulation of land change or a single Marxan run is one draw. Report
  the mean and the spread over replicates, or the selection frequency of Marxan's runs.
- **Replicates that are not independent.** Replicates that share data drawn once (a neutral landscape drawn
  outside the part that varies) measure only the variability of the part that varies. Decide what the replicates
  are replicates *of*.
- **Comparing with software that draws differently.** Terraflux reproduces R's and numpy's generators where it
  ports their software, so a CLUE-S run matches lulcc's for the same seed. Two different programs with "the same
  seed" otherwise give different draws; compare their distributions over replicates, not single runs.

## Going further

- [Monte Carlo](../../uncertainty/monte-carlo/): propagate the uncertainty of parameters by drawing them.
- [Loops](../loops/): simulations step by step.
- [Neutral landscape models](../../neutral-landscapes/): random landscapes as null models.
- The [parallel computing guide](/parallel/): replicates and scenarios on many computers, and calibration.

## References

- Gardner, R. H., Milne, B. T., Turner, M. G., & O'Neill, R. V. (1987). Neutral models for the analysis of broad-scale landscape pattern. *Landscape Ecology*, 1(1), 19–28. https://doi.org/10.1007/BF02275262
- Grimm, V., Berger, U., Bastiansen, F., Eliassen, S., Ginot, V., Giske, J., Goss-Custard, J., Grand, T., Heinz, S. K., Huse, G., Huth, A., Jepsen, J. U., Jørgensen, C., Mooij, W. M., Müller, B., Pe'er, G., Piou, C., Railsback, S. F., Robbins, A. M., ... DeAngelis, D. L. (2006). A standard protocol for describing individual-based and agent-based models. *Ecological Modelling*, 198(1–2), 115–126. https://doi.org/10.1016/j.ecolmodel.2006.04.023
- Grimm, V., Railsback, S. F., Vincenot, C. E., Berger, U., Gallagher, C., DeAngelis, D. L., Edmonds, B., Ge, J., Giske, J., Groeneveld, J., Johnston, A. S. A., Milles, A., Nabe-Nielsen, J., Polhill, J. G., Radchuk, V., Rohwäder, M.-S., Stillman, R. A., Thiele, J. C., & Ayllón, D. (2020). The ODD protocol for describing agent-based and other simulation models: a second update to improve clarity, replication, and structural realism. *Journal of Artificial Societies and Social Simulation*, 23(2), 7. https://doi.org/10.18564/jasss.4259
- Matsumoto, M., & Nishimura, T. (1998). Mersenne twister: a 623-dimensionally equidistributed uniform pseudo-random number generator. *ACM Transactions on Modeling and Computer Simulation*, 8(1), 3–30. https://doi.org/10.1145/272991.272995
- White, J. W., Rassweiler, A., Samhouri, J. F., Stier, A. C., & White, C. (2014). Ecologists should not use statistical significance tests to interpret simulation model results. *Oikos*, 123(4), 385–388. https://doi.org/10.1111/j.1600-0706.2013.01073.x
- With, K. A., & King, A. W. (1997). The use and misuse of neutral landscape models in ecology. *Oikos*, 79(2), 219–229. https://doi.org/10.2307/3546007
