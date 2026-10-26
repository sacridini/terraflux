---
title: "MOLUSCE: transition potentials and a Monte Carlo cellular automaton"
short: MOLUSCE
description: Rebuild the simulation of QGIS's MOLUSCE plugin on Sibuyan Island, with its weights of evidence or a logistic regression for the potential and its simulator that changes the most confident cells, and learn why its simulated quantities can fall short of the observed change.
order: 630
level: intermediate
time: 45 min
ops: [molusce.total, molusce.step, rng.numpy, change.crosstab, validate.three_map, validate.kappa, glm.fit, glm.predict]
series: Other land change models
part: 4
---

MOLUSCE is the land change plugin of QGIS, and for many ecologists the first land change model they meet: it
takes two maps and a few drivers and, in a handful of dialogue boxes, gives a map of the future. In this part you
rebuild its simulation in Terraflux on Sibuyan Island: the cross table of the observed change, the transition
potential as MOLUSCE's weights of evidence (or a logistic regression), and its simulator, run from 2011 for two
periods (2025 and 2039). You will check it on the calibration period, see where its logic differs from Dinamica
EGO's and CLUE-S's, and learn to read the one behaviour that surprises its users most: the simulated change can be
much smaller than the change it was asked for.

## The idea

### The plugin

MOLUSCE (Modules for Land Use Change Simulations) was written by NextGIS for Asia Air Survey and released as a
QGIS plugin in 2013. It is organised as its modules: the **inputs** (the maps of two dates and the drivers, which
it calls factors), the **area change** (the cross table of the two maps), the **transition potential modelling**
(an artificial neural network, a logistic regression, weights of evidence, or multi-criteria evaluation), the
**cellular automaton simulation** (a Monte Carlo simulator) and the **validation** (kappa statistics and an error
budget). Its ease of use made it common in studies of land use change in Asia and Africa; Muhammad et al. (2022),
for example, used it to project the land use of Linyi, China. Terraflux reproduces its potential (weights of
evidence) and its simulator through landschaft, and offers any of its own potentials (a logistic regression here)
in their place.

### The quantities: the cross table

MOLUSCE's quantity is the observed **cross table** of the period: the cells of each class at the first date that
hold each class at the second. On Sibuyan from 1997 to 2011:

```tftable
data: /data/tutorials/land-change/molusce/cross.csv
labels: { from: "From 1997, to 2011:", "1": "Forest", "2": "Coconut", "3": "Grassland", "4": "Rice", "5": "Other" }
digits: 0
heat: true
caption: "The cross table of Sibuyan, 1997 to 2011: the cells of each class of 1997 (rows: 1 forest, 2 coconut, 3 grassland, 4 rice, 5 other) that hold each class in 2011 (columns)."
```

(made by [[change.crosstab]]). Each iteration of the simulator applies these counts once: an iteration is the
whole period (14 years), not a year. From 2011, one iteration gives 2025, two give 2039.

### The potential: total weights per transition

MOLUSCE's weights of evidence cut each driver into **categories** by bounds you give (elevation at 50, 150, 300 and
600 m, slope at 4, 10, 16 and 22°, the distance to roads at 250 m to 4 km) and give each category a weight per
transition, **persistence included** (forest to forest has weights too):

!!! science "Key idea: MOLUSCE's total weight and its prediction"
    For a cell \(x\) of class \(i\) and a transition \(i \to j\) (\(j\) may be \(i\)), the **total weight** is the
    sum, over the drivers \(k\), of the weight of the category \(c_k(x)\) the cell falls in:

    \[ T_{ij}(x) = \sum_k W_{ij,k}\big(c_k(x)\big) \]

    The cell is **predicted** the transition of largest total weight, with a **confidence** of 100 times the gap
    between the largest and the second largest (truncated to a whole number). A category where the transition
    never happened has a weight of about −33 (the log of a tiny number): it rules the transition out.

```tfgallery
columns: 2
caption: The total weight of two transitions from forest in every cell of Sibuyan (molusce.total; clipped at -5 and 5, blue against, red for). Forest stays forest in the mountains; it becomes grassland on the lower slopes near roads.
images:
  - { src: /img/tutorials/land-change/molusce/total_1_to_1.png, caption: "Forest stays forest" }
  - { src: /img/tutorials/land-change/molusce/total_1_to_3.png, caption: "Forest to grassland" }
```

The weights of the distance to roads tell a familiar story: forest to grassland is most likely 500 m to 2 km
from a road (weights of 0.85 and 0.72) and ruled out beyond 4 km (−33), while forest to forest becomes certain far
from roads (4.9 beyond 4 km):

```tfchart
type: bar
data: /data/tutorials/land-change/molusce/weights_dist_roads_chart.csv
x: range
y: [Forest_to_Forest, Forest_to_Grassland]
labels: { Forest_to_Forest: "Forest stays forest", Forest_to_Grassland: "Forest to grassland" }
colors: { Forest_to_Forest: "#3b8a5a", Forest_to_Grassland: "#a7c957" }
xlabel: Distance to roads (m)
ylabel: Weight
height: 280
caption: MOLUSCE's weights of the distance to roads for two transitions from forest, up to 4 km. Beyond 4 km (not drawn), forest stays forest with a weight of 4.9 and forest to grassland has -33.2, no transition at all.
```

### The simulator

!!! science "Key idea: MOLUSCE's Monte Carlo simulator"
    At each iteration:

    1. every cell is predicted a transition and a confidence (above);
    2. for each transition \(i \to j\) with \(i \neq j\), the cells of class \(i\) **predicted to make it** are
       ranked by their confidence plus a small random number (\(U(0,1)/1000\), to break ties);
    3. the first \(n_{ij}\) of them change, \(n_{ij}\) being the count of the cross table.

    A transition can only take cells predicted to make it. If fewer cells are predicted forest to grassland than
    the cross table's 320, fewer than 320 change.

That last line is the essential difference from Dinamica EGO (whose rates always change their cells, placed by
probability) and from CLUE-S (whose allocation iterates until the areas are met). In MOLUSCE the counts are a
**ceiling**: the potential decides how much of it is reached.

### MOLUSCE's other potentials

The plugin offers four ways to make the potential. Its **weights of evidence** are reproduced exactly by
[[molusce.total]]. A **logistic regression** has its equivalent in [[glm.fit]] and [[glm.predict]] (below). Its
**artificial neural network** (a multi-layer perceptron) and **multi-criteria evaluation** (weighted overlays of
standardised factors) have no node of their own: a neural network's potential computed elsewhere can be read as a
stack of a layer per class, and a weighted overlay is a [[raster.calc]] expression per class put together by
[[stack.of]]. Whatever its origin, a potential enters [[molusce.step]] the same way, and the simulator then behaves
as described above.

## When to use it

Use this model to reproduce or extend a MOLUSCE study, or as a quick first model of a landscape with two maps and a
few categorical drivers. Its potentials are easy to explain (a weight per category), and its quantities are the
observed transitions. Its limits: one iteration is a whole period (no yearly steps), the quantities are a ceiling
that the potential may not reach, the allocation is cell by cell (no patches), and its validation in the plugin
rests on kappa. For yearly steps, patches and the quantities always met, use
[Dinamica EGO](/tutorials/land-change/dinamica-on-sibuyan/); for scenarios of areas, [CLUE-S](/tutorials/land-change/clue-s-ideas/).

## What you need

- The two maps of the calibration period, and the map to start from (Sibuyan's of 1997 and 2011).
- Drivers, and the bounds of their categories (as in the plugin's *Weights of evidence* dialogue).
- The weights: fitted by MOLUSCE (the plugin's table, or landschaft's `MolusceWoE` in Python, which gives the same
  values) and written into [[molusce.total]]'s settings; or another potential (a logistic regression).

## Build the model

```tfgraph
#! caption: MOLUSCE's simulation of Sibuyan from 2011: the total weights of every transition (node 3), numpy's generator (node 4) and the loop (node 5) of two iterations of molusce.step (node 6), each applying the cross table of 1997-2011 once.
#! numbered: true
#! highlight: weights, allocate
#! params: false
format = "terraflux/1"

[[nodes]]
id = "start"
op = "raster.read"
params = { path = "sibuyan_landuse.tif", band = "2011" }

[[nodes]]
id = "drivers"
op = "raster.read"
params = { path = "sibuyan_drivers.tif" }

[[nodes]]
id = "weights"
op = "molusce.total"
inputs = { drivers = "drivers.raster" }
params = { classes = [1, 2, 3, 4, 5], drivers = ["elevation", "slope", "dist_roads"], bins = { elevation = [50, 150, 300, 600], slope = [4, 10, 16, 22], dist_roads = [250, 500, 1000, 2000, 4000] }, weights = [{ from = 1, to = 1, driver = "dist_roads", categories = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0], weights = [-3.203498, -2.363637, -1.334145, -0.114809, 1.482993, 4.906252] }, { from = 1, to = 3, driver = "dist_roads", categories = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0], weights = [-0.155531, 0.439672, 0.845118, 0.715742, -1.307703, -33.191493] }] }

[[nodes]]
id = "rng"
op = "rng.numpy"
inputs = { seed = 1 }

[[nodes]]
id = "simulation"
op = "control.repeat"
inputs = { times = 2, landscape = "start.raster" }

[nodes.body]
carry = { landscape = "allocate.landscape" }
collect = { maps = "allocate.landscape", info = "allocate.info" }

[[nodes.body.nodes]]
id = "allocate"
op = "molusce.step"
inputs = { landscape = "@landscape", total = "weights.total", rng = "rng.rng", step = "@step" }
params = { classes = [1, 2, 3, 4, 5], counts = [{ from = 1, to = 1, cells = 4160 }, { from = 1, to = 2, cells = 126 }, { from = 1, to = 3, cells = 320 }, { from = 1, to = 4, cells = 116 }, { from = 1, to = 5, cells = 1 }, { from = 2, to = 2, cells = 1140 }, { from = 2, to = 3, cells = 18 }, { from = 3, to = 2, cells = 77 }, { from = 3, to = 3, cells = 745 }, { from = 3, to = 4, cells = 16 }, { from = 3, to = 5, cells = 1 }, { from = 4, to = 2, cells = 61 }, { from = 4, to = 3, cells = 7 }, { from = 4, to = 4, cells = 156 }, { from = 5, to = 5, cells = 282 }] }

[outputs.maps]
from = "simulation.maps"

[outputs.info]
from = "simulation.info"
```

The graph shows two transitions' weights of one driver only, to stay readable; the model run for the figures has
every transition and driver ([model.toml](/data/tutorials/land-change/molusce/model.toml)).

<div class="steps" markdown>

1. **The start and the drivers** (nodes 1 and 2): the map of 2011 and the drivers' stack.
2. **The total weights** (node 3, [[molusce.total]]): `classes`, the `drivers` used in order, their `bins` (the
   bounds of the categories: \(n\) bounds make \(n + 1\) categories, numbered 1, 2...), and `weights`, a list with
   an entry per transition and driver: `from`, `to`, `driver`, the `categories` and their `weights`. Its output is
   a stack with a layer per transition, persistence included (`1_to_1`, `1_to_2`...); a cell missing a driver gets
   no value.
3. **The generator** (node 4, [[rng.numpy]]): numpy's legacy generator after `numpy.random.seed(1)`, the one the
   plugin draws its tie-breaking numbers from.
4. **The loop** (node 5): `times = 2` iterations, carrying the map (`@landscape`) from one to the next.
5. **The simulator** (node 6, [[molusce.step]]): `landscape` from `@landscape`, `total` from node 3 (or
   `potential`: a stack of a layer per class, below), `rng` and `step`. Its settings: the `classes` and the
   `counts` of the cross table (`{ from, to, cells }` for every pair). Its `info` table says how many cells each
   transition changed.

</div>

The counts come from [[change.crosstab]] of the two observed maps; copy its table into `counts` (the persistence
entries, such as `{ from = 1, to = 1 }`, are ignored by the allocation but harmless).

### A logistic regression instead

The `potential` input takes a stack with a layer per class, such as [[glm.predict]]'s suitability: each cell is
then predicted the class of largest potential, and the rest is the same. Replace node 3 by a [[glm.fit]] of every
class on the three drivers and a [[glm.predict]], and connect its `suitability` to `potential`.

## Read the results

### On the calibration period

Run one iteration from 1997 and compare it with 2011 ([[validate.three_map]], blocks of 1 to 8 cells):

```tfchart
type: line
data: /data/tutorials/land-change/molusce/fom.csv
x: factor
y: [woe, glm]
labels: { woe: "Weights of evidence", glm: "Logistic regression" }
colors: { woe: "#3b8a5a", glm: "#e8901f" }
xlabel: Block side (cells of 250 m)
ylabel: Figure of merit
xlog: true
height: 280
caption: The figure of merit of one MOLUSCE iteration from 1997 against the observed map of 2011, with the weights of evidence and with a logistic regression as the potential.
```

With the weights of evidence the figure of merit is 0.14 at 250 m and 0.29 at 2 km; with the logistic regression
half that. Dinamica EGO's model on the same period reaches 0.47 at 250 m
([Validating a simulation](/tutorials/land-change/validating-a-simulation/)), with thirteen drivers instead of
three and patches instead of single cells; the comparison is about the whole model, not only the simulator.

```tfcompare
caption: Drag the handle between the observed map of 2011 (left) and one MOLUSCE iteration from 1997 with the weights of evidence (right). Forest green, coconut ochre, grassland light green, rice blue, other purple.
before: { src: /img/tutorials/land-change/molusce/observed_2011.png, label: Observed 2011 }
after: { src: /img/tutorials/land-change/molusce/check_woe.png, label: Simulated 2011 }
```

### The quantities that were not reached

The `info` table of the projection from 2011 says how many cells each transition changed. With the weights of
evidence, the first iteration (2011 to 2025) changed forest to coconut 126 times, forest to grassland 320 and
grassland to coconut 77, as asked, but forest to rice only 61 times of 116: only 61 forest cells were predicted to
become rice. In the second iteration forest to rice changed no cell, and forest to coconut 70 of 126. With the
logistic regression it is starker: forest to grassland changed **9 cells of 320** in the first iteration, none in
the second, because the class of largest potential of almost every forest cell is forest.

This is MOLUSCE's design, not an error, but it changes the meaning of the result: the simulated map of 2039 is not
"the trend of 1997-2011 continued", it is the part of that trend the potential agrees with. Always read `info`:

```tfgallery
columns: 2
caption: Sibuyan in 2039 after two MOLUSCE iterations from 2011, with each potential. With the logistic regression, forest loses much less (54.0% of the island left against 45.3%) because few forest cells are predicted to change.
images:
  - { src: /img/tutorials/land-change/molusce/woe_2039.png, caption: "Weights of evidence" }
  - { src: /img/tutorials/land-change/molusce/glm_2039.png, caption: "Logistic regression" }
```

### What to report

The drivers and their categories; the weights (a table per transition, as the plugin shows them); the cross table
used as counts; the number of iterations and the period each stands for; the cells each transition actually
changed (`info`), against the counts; and the validation, with the figure of merit and the quantity and allocation
disagreement rather than kappa alone ([[validate.kappa]] gives MOLUSCE's kappas; [[validate.error_budget]] its
error budget).

## Settings that change the answer

- **The categories' bounds.** Coarse categories smooth the weights; fine ones fit noise, and categories without
  transitions get the weight of −33, which forbids the transition outright. Choose bounds with enough cells in
  each.
- **The potential.** As above, it decides how much of the counts is reached.
- **The counts.** They can be edited for a scenario (half the forest loss), as a ceiling.
- **The number of iterations.** Each is a whole period; there is no yearly step.

## Common pitfalls

- **Reading the counts as the result.** Compare `info` with the counts.
- **Iterations taken for years.** One iteration of a 14-year cross table is 14 years.
- **Drivers with missing values.** A cell missing a driver gets no total weight and never changes.
- **Validating with kappa only.** Use the figure of merit and the disagreement components
  ([Validating a simulation](/tutorials/land-change/validating-a-simulation/)).

## Going further

- [Weights of evidence](/tutorials/land-change/weights-of-evidence/): Dinamica EGO's weights, with generalised
  ranges and significance.
- [A demand per region](/tutorials/land-change/regional-demand/), the next part.
- [FUTURES](/tutorials/land-change/futures/) and [PLUS](/tutorials/land-change/plus-cars/).
- The operators: [[molusce.total]], [[molusce.step]], [[rng.numpy]], [[change.crosstab]].

## References

- Muhammad, R., Zhang, W., Abbas, Z., Guo, F., & Gwiazdzinski, L. (2022). Spatiotemporal change analysis and prediction of future land use and land cover changes using QGIS MOLUSCE plugin and remote sensing big data: a case study of Linyi, China. *Land*, 11(3), 419. https://doi.org/10.3390/land11030419
