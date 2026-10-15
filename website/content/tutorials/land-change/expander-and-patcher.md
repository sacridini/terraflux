---
title: "Expander and Patcher: where the change goes, and in what shape"
short: Expander and Patcher
description: How Dinamica EGO turns transition rates and probability maps into patches of change, by growing existing patches and seeding new ones, and how the share, size and isometry of the patches shape the simulated landscape.
order: 110
level: intermediate
time: 45 min
template: Dinamica EGO
ops: [dinamica.step, patches.observe, patches.settings, woe.probability, markov.fit, markov.matrix]
series: Dinamica EGO in Terraflux
part: 2
---

The [first part](/tutorials/land-change/weights-of-evidence/) gave every cell a probability of each transition;
the [Markov chain](/tutorials/land-change/transition-rates-markov-chains/) says how many cells make each
transition per step. One question is left: which cells, exactly, change, and in what shape? A clearing in a
forest is rarely a scatter of single cells: it is a patch, often next to an older clearing. Dinamica EGO answers
with two functions, the **Expander**, which grows patches that already exist, and the **Patcher**, which starts
new ones. In this part you will run them on a small landscape drawn by chance, where each setting has a visible
effect, then on Sibuyan Island, and learn to measure the settings on observed maps with [[patches.observe]], to
give them by hand with [[patches.settings]], and to read what [[dinamica.step]] did at every step. At the end you
will know what each of the five numbers of a transition (the Expander's share, the mean and variance of the patch
sizes, the isometry) does to the simulated landscape, and how to choose them.

## The idea

### Allocation: the third piece of the model

A land change model answers three questions: how much changes, where, and in what shape
([What land change models are](/tutorials/land-change/what-land-change-models-are/)). In Dinamica EGO's classic
model, the *how much* is a Markov chain of transition rates, the *where* is a map of probability per transition
(weights of evidence), and the *shape* is the job of the **allocation**. Soares-Filho et al. (2002) designed it for
DINAMICA's first application, the deforestation of a colonisation frontier in the Brazilian Amazon, where new
clearings appear as patches beside the roads and older clearings grow into the forest around them. A cell-by-cell
allocation (take the cells of highest probability) would put the change in a fringe of single cells around every
high-probability area: the right amount, in roughly the right places, with the wrong landscape. Since the
simulated maps are then used for habitat, fragmentation or connectivity, the shape is not a detail: two
simulations with the same amount of deforestation can leave very different forests.

The allocation works one transition at a time (forest to grassland, forest to coconut...), and splits the cells
each transition must change between two functions:

![The Expander and the Patcher on a toy grid](/img/tutorials/land-change/expander-and-patcher/functions.png "Left, a patch of grassland (light green) in a forest. Middle, the Expander adds cells at the edge of the existing patch (pale). Right, the Patcher starts new patches elsewhere in the forest (orange), in cells chosen by their probability.")

- The **Expander** changes cells *next to* cells that already have the target class: grassland grows at the edge
  of grassland. It produces the slow advance of a frontier.
- The **Patcher** seeds *new patches* in cells chosen by their probability, wherever they are: a new clearing far
  from the old ones.

Both build patches, not cells: from a seed cell, a patch grows to a size drawn at random, cell by cell, into the
neighbouring cells of the origin class. How large the patches are and how compact is set per transition.

### How many cells, and by which function

!!! science "Key idea: the cells of a transition at a step"
    For a transition from class \(i\) to class \(j\), with \(r_{ij}\) its rate per step (from the Markov chain)
    and \(N_i\) the cells of class \(i\) on the map at the start of the step, the step changes

    \[ n_{ij} = \operatorname{round}(r_{ij} \, N_i) \]

    cells, of which a share \(e_{ij}\) (the **Expander's share**, `expansion`) by the Expander and the rest by the
    Patcher. On Sibuyan, forest to grassland has a rate of 0.0053 per year and 4,723 cells of forest at the start:
    25 cells in the first step, 24 of them by the Expander (\(e = 0.947\)).

Because \(N_i\) shrinks as the class loses cells, the number of cells per step falls slowly over a simulation (25
cells of forest to grassland in the first year, 23 in the fourteenth), exactly as the Markov chain projects.

### The Expander

The Expander must place its cells beside the target class. It changes each cell's probability \(P\) of the
transition into a probability of *expansion*:

!!! science "Key idea: the Expander's probability"
    \[ P' = \begin{cases} P & \text{if } n_j \ge 5 \\ P \, n_j / 5 & \text{otherwise} \end{cases} \]

    where \(n_j\) is the number of the cell's eight neighbours (the 3 × 3 window, the `window` setting of
    [[dinamica.step]]) that already hold the target class \(j\), and 5 is half of the window's nine cells, rounded
    up. A cell with no neighbour of the target class has \(P' = 0\): the Expander never seeds a patch away from
    the class. A cell half surrounded keeps its full probability.

The Expander ranks the cells of the origin class by \(P'\), keeps the first `prune_factor` × \(n\) of them (10
times the cells it must change, by default), draws a candidate among them in proportion to \(P'\) and accepts it as
a seed with probability \(P'\). Each accepted seed grows into a patch. The draws go on until the Expander has made
its share or run out of candidates; what it could not place is handed to the Patcher.

### The Patcher

The Patcher draws its seeds among the cells of the origin class in proportion to their probability \(P\) (again
only among the first `prune_factor` × \(n\) cells, ranked by \(P\)), and grows a patch from each, until its share
is made. Its seeds can fall anywhere the probability is high, so most of its patches are new ones.

### Patch size and isometry

!!! science "Key idea: the size and the shape of a patch"
    The size \(s\) of a patch (in cells) is drawn from a **lognormal** distribution with the mean \(m\) and the
    variance \(v\) given for the transition and function: with \(\sigma^2 = \ln(1 + v/m^2)\) and
    \(\mu = \ln m - \sigma^2/2\), \(s = \operatorname{round}(e^{\mu + \sigma z})\), \(z\) a standard normal draw
    (at least 1 cell, and never more than the cells left to change). Most patches are small and a few are
    large, as in real landscapes.

    The patch then grows from its seed one cell at a time. Each candidate cell (a cell of the origin class next to
    the patch) is drawn with a weight

    \[ w = P \times \text{isometry}^{\,k} \]

    where \(k\) is the number of the candidate's neighbours already in the patch. With an **isometry** above 1,
    cells surrounded by the patch are preferred and patches are compact; with 1, growth follows the probabilities
    alone; below 1, cells that touch the patch by one side are preferred and patches are thin and lacy.

Two more rules complete the step. Every cell changed is **corroded**: its probabilities of every transition are set
to 0, so no other transition can take it in the same step. And the transitions run in order: first every
transition's Expander, then every transition's Patcher (with what its Expander left over). Random numbers come from
the seed of the simulation, so the same seed gives the same maps.

## When to use it

Use the Expander and Patcher when the **shape** of the change matters: deforestation that advances as a front or
opens clearings, the spread of plantations, agricultural frontiers, regrowth of abandoned land around old
fallows, urban growth at the edge of towns. Because it is per transition, each kind of change gets its own
shapes: on Sibuyan, grassland grew almost only at the edge of grassland (95% expansion), while rice appeared
mostly in new patches (24% expansion).

It is not the tool when every class competes for every cell under a fixed demand of areas; that is the domain of
CLUE-S ([Demand-driven allocation](/tutorials/land-change/clue-s-ideas/)), whose allocation is cell by cell. PLUS's
CARS ([PLUS](/tutorials/land-change/plus-cars/)) and FUTURES ([FUTURES](/tutorials/land-change/futures/)) are the
other patch-based allocations in Terraflux: CARS seeds random patches under a demand of areas, FUTURES grows
patches of development from a library of observed patch shapes. Dinamica's allocation has been used in many
studies of tropical deforestation since Soares-Filho et al. (2002), including the basin-wide scenarios of
Soares-Filho et al. (2006), and in the comparison of land change software by Mas et al. (2014).

## What you need

- A **map** of land use at the start of the step (a class per cell).
- A **probability** stack with a layer per transition, named `1_to_2`, `1_to_3`...: the output of
  [[woe.probability]] (part 1), computed on the same map.
- The **rates** per step: a Markov chain from [[markov.fit]] (or [[markov.matrix]] for rates written by hand).
- The **patch settings** of each transition: measured on the observed change by [[patches.observe]], or given by
  [[patches.settings]].
- The **step** number and a **seed** (two whole numbers; inside a loop, `@step` and the model's seed).

The application's template **Dinamica EGO** (Templates, Land change) has all of them on maps drawn by chance; the
model below is smaller and isolates the allocation.

## Build the model

### A step on a landscape drawn by chance

The model below draws a landscape of forest (1) and farmland (2) with [[nlm.random_cluster]], a smooth driver
with [[nlm.gaussian_field]], makes an "observed" change to learn from (forest cleared where the driver is high),
fits the chain and the weights on it, and lets [[dinamica.step]] place the same amount of change again, with
patches. It runs as it is: paste it into the application (or save it as `toy.toml` and run
`terraflux run toy.toml`).

```tfgraph
#! caption: One step of the Expander and Patcher on a landscape drawn by chance. Nodes 1 to 4 make the data, 5 and 6 the chain and the weights, 7 the probability map; node 8 gives the patches' settings and node 9 allocates.
#! numbered: true
#! highlight: patches, allocate
#! params: false
format = "terraflux/1"

[[nodes]]
id = "land"
op = "nlm.random_cluster"
params = { rows = 150, columns = 150, p = 0.5, weights = [0.8, 0.2], seed = 4 }
note = "The map at the start: 1 forest, 2 farmland."

[[nodes]]
id = "suitability"
op = "nlm.gaussian_field"
params = { rows = 150, columns = 150, range = 30, seed = 5 }
note = "A driver: how well each cell suits farming."

[[nodes]]
id = "drivers"
op = "stack.of"
inputs = { suitability = "suitability.map" }

[[nodes]]
id = "observed"
op = "map.calc"
inputs = { land = "land.map", s = "suitability.map" }
params = { expression = "if land == 1 and s > 0.68 then 2 else land" }
note = "A change to learn from: forest cleared where farming suits it best."

[[nodes]]
id = "chain"
op = "markov.fit"
inputs = { t0 = "land.map", t1 = "observed.map", steps = 1 }

[[nodes]]
id = "weights"
op = "woe.fit"
inputs = { t0 = "land.map", t1 = "observed.map", drivers = "drivers.stack", markov = "chain.markov" }
params = { ranges = { suitability = 0.05 } }

[[nodes]]
id = "probability"
op = "woe.probability"
inputs = { model = "weights.model", landscape = "land.map", drivers = "drivers.stack" }

[[nodes]]
id = "patches"
op = "patches.settings"
params = { transitions = [{ from = 1, to = 2, expansion = 0.5, expander = [20, 200, 1.5], patcher = [20, 200, 1.5] }] }

[[nodes]]
id = "allocate"
op = "dinamica.step"
inputs = { landscape = "land.map", probability = "probability.probability", rates = "chain.markov", patches = "patches.patches", step = 1, seed = 1 }
```

<div class="steps" markdown>

1. **The data** (nodes 1 to 4). `land` is the start; `suitability`, put into a stack by [[stack.of]] (its layer is
   named after the input, `suitability`), is the only driver; `observed` is what "happened": every forest cell
   with a suitability above 0.68 became farmland (2,410 cells, 13% of the forest). In a real model these nodes are
   two [[raster.read]] of the land use maps and one of the drivers.
2. **The chain** (node 5, [[markov.fit]], `steps = 1`): the rate of forest to farmland is the share of the forest
   that changed. Connect `t0` to `land.map` and `t1` to `observed.map`.
3. **The weights** (node 6) and **the probability** (node 7): as in [part 1](/tutorials/land-change/weights-of-evidence/).
   [[woe.probability]] gives a stack with one layer, `1_to_2`, the probability of forest to farmland in every
   forest cell. This is the stack [[dinamica.step]] reads: its layers must be named by transition, and only
   [[woe.probability]] (or a file whose bands are named `1_to_2`...) names them so.
4. **The patch settings** (node 8, [[patches.settings]], subject *Dinamica EGO* in the library). Its one setting,
   `transitions`, is a list with an entry per transition: `from`, `to`, `expansion` (the Expander's share, 0 to 1)
   and, for each function, three numbers: `[mean, variance, isometry]` of its patches. Here half of the change by
   each, patches of 20 cells on average (variance 200), isometry 1.5.
5. **The allocation** (node 9, [[dinamica.step]]). Connect `landscape` to the map the step starts from,
   `probability` to the probability stack (computed on that same map), `rates` to the chain, `patches` to the
   settings, and type `step = 1` and `seed = 1` in the Node pane. Its outputs: `landscape`, the map after the
   step, and `info`, a table of what each function did.

</div>

![The probability of forest to farmland](/img/tutorials/land-change/expander-and-patcher/toy_probability.png "The probability of the transition in every forest cell (node 7), from the weights of the driver: the darker the cell, the less likely; farmland has no value (white).")

Run it (++f5++, or ++ctrl+enter++ on node 9) and select node 9: the map shows the new farmland. Its `info` table
has a row per step and four columns per transition: `requested_1_2` (\(n_{ij}\), here 2,410), `expanded_1_2`
(1,205, by the Expander), `patched_1_2` (1,205, by the Patcher) and `patches_1_2` (116 patches made).

![The landscape after one step](/img/tutorials/land-change/expander-and-patcher/toy_base.png "The landscape after the step: forest (green), farmland at the start (ochre) and the 2,410 cells of new farmland (purple), half grown from old farmland, half in new patches; mean size 20 cells, isometry 1.5.")

![Legend](/img/tutorials/land-change/expander-and-patcher/toy_legend.png)

### On Sibuyan: settings measured on the observed change

On real data the five numbers of each transition can be measured: [[patches.observe]] compares the two maps of the
calibration period and, for each transition with a rate, finds the patches of change (cells that made the
transition, joined by their eight neighbours), counts those that touch the target class of the first map
(expansions) and those that do not (new patches), and measures the mean and variance of their sizes. The isometry
cannot be measured from two maps; it is given (1.5 by default, Dinamica EGO's usual value).

```tfgraph
#! caption: The patch settings of Sibuyan, measured on the change from 1997 to 2011: the chain (node 1) says which transitions have a rate, patches.observe (node 2) measures their patches.
#! numbered: true
#! highlight: patches
format = "terraflux/1"

[inputs.t0]
kind = "map"
path = "sibuyan_landuse.tif"
band = "1997"

[inputs.t1]
kind = "map"
path = "sibuyan_landuse.tif"
band = "2011"

[[nodes]]
id = "markov"
op = "markov.fit"
inputs = { t0 = "$t0", t1 = "$t1", steps = 14 }

[[nodes]]
id = "patches"
op = "patches.observe"
inputs = { t0 = "$t0", t1 = "$t1", markov = "markov.markov" }
params = { isometry = 1.5 }
note = "The Expander's share and the patch sizes measured on the observed change; the isometry given."
```

The settings it finds for the main transitions of Sibuyan (sizes in cells of 250 × 250 m, 6.25 ha):

| Transition | Expander's share | Expander: mean (variance) | Patcher: mean (variance) |
|---|---:|---:|---:|
| Forest to coconut | 0.77 | 16.2 (210) | 3.6 (9.7) |
| Forest to grassland | 0.95 | 20.2 (1,212) | 1.9 (1.4) |
| Forest to rice | 0.24 | 4.7 (17.6) | 5.9 (74) |
| Grassland to coconut | 0.94 | 12.0 (188) | 1 (0) |
| Rice to coconut | 0.92 | 4.7 (13.4) | 5.0 (0) |

Read the table as a description of how Sibuyan changed between 1997 and 2011: grassland and coconut spread
from the patches they already had (large expansions of 16 to 20 cells on average, with a few very large ones: the
variance of 1,212 means sizes up to well over 100 cells), while rice was mostly opened in new fields of about six
cells. A setting of 1 with a variance of 0 means that there was no such patch (or one patch of one cell) to
measure.

This is the node `terraflux new dinamica` writes into the model (part 3). Its `expansion` setting replaces every
measured share by one value, and `isometry` sets the isometry of all; for anything finer, write the settings
with [[patches.settings]] (in the application, the measured values can be read from the node's output and copied).

## Read the results

### What each function did

[[dinamica.step]]'s `info` table, collected by the simulation's loop, tells, for every step and transition, how
many cells were requested, made by each function, and in how many patches. On Sibuyan (the full model of
[part 3](/tutorials/land-change/dinamica-on-sibuyan/)):

```tfchart
type: bar
data: /data/tutorials/land-change/expander-and-patcher/steps.csv
x: step
y: ["expanded 1-3", "patched 1-3", "expanded 1-2", "patched 1-2"]
labels: { "expanded 1-3": "Forest to grassland, Expander", "patched 1-3": "Forest to grassland, Patcher", "expanded 1-2": "Forest to coconut, Expander", "patched 1-2": "Forest to coconut, Patcher" }
colors: { "expanded 1-3": "#a7c957", "patched 1-3": "#5f7f2a", "expanded 1-2": "#d9a441", "patched 1-2": "#8a5a12" }
xlabel: Step (year)
ylabel: Cells changed
height: 300
caption: The cells of forest to grassland and forest to coconut made by each function at every step of Sibuyan's simulation. Grassland comes almost entirely from the Expander (24 of 25 cells in the first step); the number of cells falls slowly as the forest shrinks.
```

A good sign: each function makes what it was asked (`expanded + patched = requested`). When the Expander cannot
find enough cells beside the target class (a class whose patches are surrounded by other classes, or whose edge
cells have no probability), its leftover goes to the Patcher and `patched` is larger than its share: the change
then has more new patches than you asked for. When even the Patcher cannot make its cells (too few cells of
positive probability), `expanded + patched` is smaller than `requested`, and the simulated areas fall behind the
Markov chain: raise the `prune_factor`, or check the probabilities.

### The simulated change

```tfcompare
caption: Drag the handle to compare the observed change of the north-west of Sibuyan from 1997 to 2011 (left) with the change simulated by the full model (right). Grey, unchanged; ochre, to coconut; light green, to grassland; blue, to rice.
before: { src: /img/tutorials/land-change/expander-and-patcher/observed_change.png, label: Observed }
after: { src: /img/tutorials/land-change/expander-and-patcher/simulated_change.png, label: Simulated }
```

![Legend of the change](/img/tutorials/land-change/expander-and-patcher/legend.png)

The simulated grassland grows in the same blocks as the observed one, from the same old patches; coconut forms
rings around older coconut. The exact cells differ, and should: the allocation is stochastic, and a simulation is
one of many possible landscapes with the same amount and kind of change. How close the two are is measured in
[Validating a simulation](/tutorials/land-change/validating-a-simulation/).

### What to report

The patch settings of every transition (a table like the one above), how they were obtained (measured on the
calibration period, given, or calibrated), the isometry, the `prune_factor` and the window. If the settings were
calibrated, the metric and the range tried ([Calibration and many runs](/tutorials/land-change/calibration-and-many-runs/)).

## Settings that change the answer

The dashboards below are real runs of the model above, each with one setting changed and everything else as in
the model (the same seed, the same 2,410 cells of new farmland).

### The Expander's share

```tfslider
label: Expander's share (expansion)
caption: The same 2,410 cells of new farmland (purple) with all of them made by the Patcher (0), half and half (0.5), or all by the Expander (1). With the Expander alone, every new patch touches old farmland (ochre); with the Patcher alone, many stand on their own in the forest.
frames:
  - { value: 0.0, src: /img/tutorials/land-change/expander-and-patcher/toy_exp_0.0.png, stats: { "By the Expander": "0", "By the Patcher": "2,410", "Patches made": "119" } }
  - { value: 0.5, src: /img/tutorials/land-change/expander-and-patcher/toy_exp_0.5.png, stats: { "By the Expander": "1,205", "By the Patcher": "1,205", "Patches made": "116" } }
  - { value: 1.0, src: /img/tutorials/land-change/expander-and-patcher/toy_exp_1.0.png, stats: { "By the Expander": "2,410", "By the Patcher": "0", "Patches made": "120" } }
start: 1
```

Ecologically, the Expander's share decides whether the new land use **consolidates** (fewer, larger patches
of farmland, a forest edge that retreats) or **fragments** the forest (new openings in its interior). For forest
species, the second is usually worse at the same amount of loss: more edge, smaller cores.

### The size of the patches

```tfslider
label: Mean patch size (cells)
caption: The same change with patches of 1, 4, 20 and 80 cells on average (variance half the mean squared). Patches of one cell scatter the change as salt and pepper; patches of 80 cells open a few large clearings.
frames:
  - { value: 1, src: /img/tutorials/land-change/expander-and-patcher/toy_size_1.png, stats: { "Patches made": "1,972", "Clumps of new farmland": "1,088", "Mean clump (cells)": "2.2" } }
  - { value: 4, src: /img/tutorials/land-change/expander-and-patcher/toy_size_4.png, stats: { "Patches made": "600", "Clumps of new farmland": "346", "Mean clump (cells)": "7.0" } }
  - { value: 20, src: /img/tutorials/land-change/expander-and-patcher/toy_size_20.png, stats: { "Patches made": "116", "Clumps of new farmland": "74", "Mean clump (cells)": "32.6" } }
  - { value: 80, src: /img/tutorials/land-change/expander-and-patcher/toy_size_80.png, stats: { "Patches made": "30", "Clumps of new farmland": "21", "Mean clump (cells)": "114.8" } }
start: 2
```

*Clumps* are the groups of new cells that touch each other (by any of their eight neighbours): fewer than the
patches made, since patches grown side by side merge. Notice the first frame: with a mean of 1 cell, only 405 of
the Expander's 1,205 cells could be placed beside old farmland (single cells, each needing its own accepted seed),
and the Patcher made the rest.

### The isometry

```tfslider
label: Isometry
caption: The same change with isometries from 0.5 to 3. Below 1 the patches grow as thin, branching strands that merge into lacy networks; above 1 they are compact blobs.
frames:
  - { value: 0.5, src: /img/tutorials/land-change/expander-and-patcher/toy_iso_0.5.png, stats: { "Clumps of new farmland": "49", "Largest clump (cells)": "288" } }
  - { value: 1.0, src: /img/tutorials/land-change/expander-and-patcher/toy_iso_1.0.png, stats: { "Clumps of new farmland": "54", "Largest clump (cells)": "321" } }
  - { value: 1.5, src: /img/tutorials/land-change/expander-and-patcher/toy_iso_1.5.png, stats: { "Clumps of new farmland": "74", "Largest clump (cells)": "302" } }
  - { value: 2.0, src: /img/tutorials/land-change/expander-and-patcher/toy_iso_2.0.png, stats: { "Clumps of new farmland": "73", "Largest clump (cells)": "120" } }
  - { value: 3.0, src: /img/tutorials/land-change/expander-and-patcher/toy_iso_3.0.png, stats: { "Clumps of new farmland": "76", "Largest clump (cells)": "186" } }
start: 2
```

The isometry changes the shape more than the number of patches. Lacy patches (isometry below 1) have much more
edge per cell: for a forest species they cut more of the forest into edge than compact ones of the same area. The
isometry cannot be measured from two maps, so it is a setting to calibrate: try values against the observed
landscape's metrics (edge density, the shape index of the new patches) or against the fuzzy similarity of part 3.
Soares-Filho et al. (2013) describe how Dinamica EGO's users search such settings automatically, scoring each try
against the observed map; Terraflux's `terraflux calibrate` does the same.

### On Sibuyan

On a real island the per-step change is small (about 57 cells of 11,000 a year on Sibuyan), so the settings act
on a few hundred cells over fourteen years; their effect is still visible:

```tfslider
label: Expander's share on Sibuyan (expansion)
caption: The change simulated from 1997 to 2011 in the north-west of Sibuyan with every transition's Expander share set to one value (the patch sizes as measured). With 0, coconut and grassland appear in many small patches across the forest; with 1, they grow only from the old patches.
frames:
  - { value: 0.0, src: /img/tutorials/land-change/expander-and-patcher/expansion_0.0.png, stats: { "Coconut patches": "23", "Grassland patches": "20", "Rice patches": "31" } }
  - { value: 0.25, src: /img/tutorials/land-change/expander-and-patcher/expansion_0.25.png, stats: { "Coconut patches": "20", "Grassland patches": "24", "Rice patches": "37" } }
  - { value: 0.5, src: /img/tutorials/land-change/expander-and-patcher/expansion_0.5.png, stats: { "Coconut patches": "18", "Grassland patches": "22", "Rice patches": "26" } }
  - { value: 0.75, src: /img/tutorials/land-change/expander-and-patcher/expansion_0.75.png, stats: { "Coconut patches": "14", "Grassland patches": "17", "Rice patches": "22" } }
  - { value: 1.0, src: /img/tutorials/land-change/expander-and-patcher/expansion_1.0.png, stats: { "Coconut patches": "8", "Grassland patches": "15", "Rice patches": "17" } }
start: 4
```

The numbers are the patches of each class on the whole island in the simulated 2011 (by [[metrics.class]], `np`):
the observed map of 2011 has 16 of coconut, 17 of grassland and 34 of rice. Here the share of 0.5 to 0.75 comes
closest for coconut and grassland, and the measured shares (0.77 and 0.95) slightly overconsolidate them, a hint
that a calibration could do better. The isometry and the patch sizes, on the other hand, move these numbers by only
one to four patches on Sibuyan: with so few cells per step, the patches are short of cells to grow before they
reach their drawn size.

### `prune_factor` and `window`

`prune_factor` (10) sets how many of the most probable cells each function draws from: 10 × the cells it must
change. A small factor (2 or 3) makes the allocation nearly deterministic, the change concentrated on the very
highest probabilities; a large one (100) lets lower probabilities take part, spreading the change. `window`
(`[3, 3]`) is the neighbourhood of the Expander's \(n_j\) and of the patches' growth: a larger window (`[5, 5]`)
lets the Expander work a little further from the existing patches and makes growth smoother.

## Common pitfalls

- **Probabilities of another map.** The probability stack must be computed on the map the step starts from. In a
  simulation this means [[woe.probability]] *inside* the loop, on `@landscape` (part 3); computed once on the
  start map, the probabilities of the cells that changed would not follow them.
- **Layers without transition names.** [[dinamica.step]] looks for the layer `1_to_3` of transition 1 to 3. A
  stack read from a file with unnamed bands has no such layers, and the step stops with an error naming the
  missing transition. Declare the file's layers with [[stack.classes]]; for probabilities made by map algebra, put
  each map in a [[stack.of]] whose input is named after its transition (`1_to_3 = "rule.map"`).
- **Settings for transitions without a rate.** Settings of [[patches.settings]] for a transition the chain does not
  have are ignored; a transition with a rate but no settings stops the step. Measure with [[patches.observe]] or
  list every transition of the chain.
- **Sizes in hectares.** The mean and variance are in **cells**. On a map of 30 m cells (0.09 ha), a clearing of
  5 ha is about 56 cells.
- **Reading one run.** One run is one draw. Compare settings on several seeds ([Calibration and many
  runs](/tutorials/land-change/calibration-and-many-runs/)), never on one map each.
- **Expecting the Patcher to avoid old patches.** In Terraflux the Patcher draws its seeds by probability alone;
  where the probability is highest next to the old patches (a weight of evidence of the distance to the class, a
  dynamic driver), its patches will often land beside them too.

## Going further

- [Part 3: Dinamica EGO on Sibuyan](/tutorials/land-change/dinamica-on-sibuyan/): the whole model, with the
  allocation inside the loop.
- [Validating a simulation](/tutorials/land-change/validating-a-simulation/): how near the simulated patches are
  to the observed ones.
- [Calibration and many runs](/tutorials/land-change/calibration-and-many-runs/): choosing the isometry and the
  Expander's share against an observed map.
- [PLUS](/tutorials/land-change/plus-cars/) and [FUTURES](/tutorials/land-change/futures/): other patch-based
  allocations.
- The operators: [[dinamica.step]], [[patches.observe]], [[patches.settings]].

## References

- Mas, J.-F., Kolb, M., Paegelow, M., Camacho Olmedo, M. T., & Houet, T. (2014). Inductive pattern-based land use/cover change models: a comparison of four software packages. *Environmental Modelling & Software*, 51, 94–111. https://doi.org/10.1016/j.envsoft.2013.09.010
- Soares-Filho, B. S., Coutinho Cerqueira, G., & Lopes Pennachin, C. (2002). DINAMICA: a stochastic cellular automata model designed to simulate the landscape dynamics in an Amazonian colonization frontier. *Ecological Modelling*, 154(3), 217–235. https://doi.org/10.1016/S0304-3800(02)00059-5
- Soares-Filho, B. S., Nepstad, D. C., Curran, L. M., Cerqueira, G. C., Garcia, R. A., Ramos, C. A., et al. (2006). Modelling conservation in the Amazon basin. *Nature*, 440(7083), 520–523. https://doi.org/10.1038/nature04389
- Soares-Filho, B., Rodrigues, H., & Follador, M. (2013). A hybrid analytical-heuristic method for calibrating land-use change models. *Environmental Modelling & Software*, 43, 80–87. https://doi.org/10.1016/j.envsoft.2013.01.010
