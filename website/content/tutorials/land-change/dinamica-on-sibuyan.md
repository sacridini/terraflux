---
title: "Dinamica EGO on Sibuyan: the whole model"
short: Dinamica EGO on Sibuyan
description: Write Dinamica EGO's classic model for Sibuyan Island with one command, understand every node and connection, the loop that runs fourteen years, and read the simulated maps step by step.
order: 120
level: intermediate
time: 50 min
template: Dinamica EGO
ops: [markov.fit, woe.fit, woe.probability, patches.observe, dinamica.step, control.repeat, validate.agreement, validate.similarity]
series: Dinamica EGO in Terraflux
part: 3
---

The first two parts of this series built the pieces of Dinamica EGO's classic land change model one by one: the
[weights of evidence](/tutorials/land-change/weights-of-evidence/) that say where each transition is likely, and
the [Expander and Patcher](/tutorials/land-change/expander-and-patcher/) that place the change in patches. In this
part you put them together into the whole model of Sibuyan Island, from the maps of 1997 and 2011, and run it
for fourteen yearly steps. You will write the model with one command (`terraflux new dinamica`), then read it
node by node: what each node does, which output feeds which input and why, and above all how the **loop**
carries the map from one year to the next, recomputing the probabilities on the map of each year. At the end you
will have fourteen simulated maps, know how to read them and the tables beside them, how to make a projection
beyond the last observed date, and what to check before trusting the result.

## The idea

### A model in three pieces, run in a loop

Dinamica EGO's classic model (Soares-Filho et al., 2002) answers the three questions of every land change model
with three pieces fitted on two maps of the past:

| Question | Piece | Operator | Fitted from |
|---|---|---|---|
| How much changes per step? | the transition rates, a Markov chain | [[markov.fit]] | the two maps |
| Where is each transition likely? | the weights of evidence of the drivers (Bonham-Carter, 1994) | [[woe.fit]] | the two maps and the drivers |
| In what shape? | the Expander's share and the patches' sizes | [[patches.observe]] | the two maps |

These three are fitted **once**, before the simulation. The simulation is then a **loop**: at every step, the
probabilities of the transitions are computed on the current map ([[woe.probability]]) and the change is
allocated on it ([[dinamica.step]]); the new map is the start of the next step. Fourteen steps take the map of
1997 to a simulated map of 2011, which can be compared with the map that was really observed in 2011.

!!! science "Key idea: a cellular automaton"
    The model is a **cellular automaton**: the state of every cell at step \(t+1\) depends on its state and its
    neighbourhood at step \(t\). The neighbourhood enters in two places: the Expander (a cell beside the target
    class can expand it) and the growth of the patches. The drivers that do not change (slope, soils, distances to
    roads that are not built during the simulation) enter through the weights. The loop is what makes it a
    simulation: each year's map is the outcome of the previous year's, so patches grow over the years, and a
    patch opened in year 3 can be expanded in year 4.

### Why the probabilities are recomputed inside the loop

The probability of a transition exists only in cells of its origin class: forest to grassland in forest cells.
After a step, some forest is grassland, so the map of the probabilities computed on 1997 is wrong for 1998: it
gives a probability of becoming grassland to cells that already are. Computing them inside the loop, on the map
of the step, keeps them right. The weights themselves (the effect of each range of each driver) do not change:
the model assumes that the processes of 1997 to 2011 go on. When a driver itself changes with the map (the
distance to the nearest grassland, the share of farmland around a cell), it must be recomputed in the loop too:
that is the subject of [Dynamic drivers](/tutorials/land-change/dynamic-drivers/).

### Sibuyan Island

Sibuyan is a mountainous island of the Philippines, mostly forested, with coconut plantations, grassland and rice
on its coastal plains. Its land use maps of 1997 and 2011 on cells of 250 m (108 × 128 cells, 7,226 of them on
the island) and thirteen drivers are the classic data of CLUE-S (Verburg et al., 2002) and come with Terraflux
(see [What land change models are](/tutorials/land-change/what-land-change-models-are/) for the maps and their
change). Between the two dates the forest fell from 65% to 58% of the island, mostly to grassland and coconut.

## When to use it

The whole model suits a landscape where change is a set of **transitions with steady rates** and **patch-shaped
outcomes**: a deforestation frontier, the expansion of plantations or pastures, regrowth of abandoned land. It
is the model of hundreds of studies of tropical deforestation since Soares-Filho et al. (2002), from the
Amazon-wide scenarios of Soares-Filho et al. (2006) to studies of single municipalities.

Its assumptions are worth keeping in mind:

- the **rates** of the calibration period go on (a Markov chain: stationary, each step like the last);
- the **drivers'** effects go on, and each driver brings evidence the others do not;
- the **shape** of the change goes on (the same share of expansions, the same patch sizes).

When the amount of each class comes from elsewhere (a scenario of areas, an economic model), a demand-driven model
fits better ([CLUE-S](/tutorials/land-change/clue-s-ideas/)); when it comes from what people decide,
[agents](/tutorials/land-change/agents/). The rates can also be given by hand ([[markov.matrix]]) for scenarios
of faster or slower change, keeping the rest of the model.

## What you need

- **Two maps of land use** of the same area on the same grid: the start and the end of the calibration period,
  with whole class codes and a no-data value outside the area. Here `sibuyan_landuse.tif`, two bands named
  `1997` and `2011`.
- **The drivers**: a raster with a band per driver, every band named, on the same grid
  (`sibuyan_drivers.tif`, 13 bands: distances to the sea, roads, towns and streams in metres, elevation, slope,
  aspect, population density, and five soil and erosion maps of 0 and 1).
- **The number of time steps** between the maps: 14 for yearly steps from 1997 to 2011.

Both files are in Terraflux's test data (`tests/data/inputs`) and can be downloaded from the
[first part](/tutorials/land-change/weights-of-evidence/)'s data. The application's template **Dinamica EGO**
builds the same model on maps drawn by chance, to try it without data.

## Build the model

### With one command

```bash
terraflux new dinamica --t0 sibuyan_landuse.tif:1997 --t1 sibuyan_landuse.tif:2011 \
    --drivers sibuyan_drivers.tif --steps 14 -o sibuyan.toml
terraflux check sibuyan.toml
terraflux run sibuyan.toml --out sibuyan
```

```tfcli
new dinamica
```

`--t0` and `--t1` name a file and a band (by name, or by its 0-based index). The model written is a plain text
file in which every choice is a node with a note; the run takes a third of a second on Sibuyan and writes
`sibuyan/maps.tif` (a band per step), `sibuyan/steps.csv` (what [[dinamica.step]] did at every step) and
`sibuyan/weights.csv` (the weights report of part 1).

!!! app "In the application"
    The toolbar's **Wizard...** writes the same model in three steps. *The model*: choose Dinamica EGO's classic
    model. *The data*: the land use map (the two dates are chosen from its bands) and the drivers; the data is
    checked as soon as it is chosen (grids, classes, missing values, constant or collinear drivers), and a driver
    on another grid can be resampled onto the land use map's grid with a click. *The settings*: the time steps
    between the two maps, the steps to simulate, the weights' buffer increment (automatic or a number), the
    Expander's share (as observed, or a number) and the isometry. **Create the model...** saves it and opens it in
    the builder, where every node can be clicked and changed.

### The inputs

The model starts with four **inputs**, the values a user is expected to change (in the application they are the
column at the left of the canvas; from the command line, `--set name=value` replaces one):

| Input | Kind | What it is |
|---|---|---|
| `t0` | map | the land use at the start of the calibration period (band `1997`) |
| `t1` | map | the land use at its end (band `2011`) |
| `drivers` | stack | every band of the drivers' file, by its name |
| `seed` | value | the seed of the random numbers of the allocation (1) |

Inside the model they are referred to as `$t0`, `$t1`, `$drivers` and `$seed`.

### Step 1: the three fitted pieces

```tfgraph
#! caption: The pieces fitted before the simulation: the Markov chain (node 1) gives the rates and says which transitions exist; the weights (node 2) and the patch settings (node 3) are fitted for those transitions.
#! numbered: true
#! highlight: markov, woe, patches
format = "terraflux/1"

[inputs.t0]
kind = "map"
path = "sibuyan_landuse.tif"
band = "1997"

[inputs.t1]
kind = "map"
path = "sibuyan_landuse.tif"
band = "2011"

[inputs.drivers]
kind = "stack"
path = "sibuyan_drivers.tif"

[[nodes]]
id = "markov"
op = "markov.fit"
inputs = { t0 = "$t0", t1 = "$t1", steps = 14 }
note = "Transition rates per step: the 14th root of the matrix of the calibration period."

[[nodes]]
id = "woe"
op = "woe.fit"
inputs = { t0 = "$t0", t1 = "$t1", drivers = "$drivers", markov = "markov.markov" }
params = { ranges = { dist_sea = 250, dist_roads = 250, dist_urban = 250, dist_streams = 250, elevation = 50, slope = 2, aspect = 30, pop_density = 50 }, categorical = ["diorite", "ultramafic", "sediments", "no_erosion", "moderate_erosion"] }
note = "Weights of evidence of every transition with a rate."

[[nodes]]
id = "patches"
op = "patches.observe"
inputs = { t0 = "$t0", t1 = "$t1", markov = "markov.markov" }
params = { isometry = 1.5 }
note = "The Expander's share and the patch sizes measured on the observed change."
```

1. **`markov`** ([[markov.fit]]) reads `t0` and `t1` and `steps = 14`. It cross-tabulates the two maps (how many
   cells of each class in 1997 hold each class in 2011), turns the counts into the probabilities of the period,
   and takes the 14th root of that matrix: the rates of one year whose fourteen repetitions give the period's
   change (see [Markov chains](/tutorials/land-change/transition-rates-markov-chains/)). Its output `markov`
   carries both. On Sibuyan it has ten transitions with a positive rate.
2. **`woe`** ([[woe.fit]]) reads the two maps, the drivers and the chain. The connection from `markov.markov` to
   its `markov` input is what restricts the weights to the transitions the chain will ask for; without it every
   observed transition would be fitted. Its settings, `ranges` and `categorical`, are those chosen in
   [part 1](/tutorials/land-change/weights-of-evidence/#the-increment). `terraflux new dinamica` writes the node
   without them (every continuous driver cut into buffers of a hundredth of its range, `--increment` another
   width); add them in the Node pane or the file, as here. Its outputs: `model` (the weights, used by the loop)
   and `table` (the report).
3. **`patches`** ([[patches.observe]]) measures, for every transition of the chain, the Expander's share and the
   mean and variance of the patches' sizes on the observed change ([part 2](/tutorials/land-change/expander-and-patcher/#on-sibuyan-settings-measured-on-the-observed-change)),
   and gives them with the isometry set (1.5). Its `markov` input, again from the chain, says which transitions to
   measure.

Why connect the chain to both? Because the three pieces must agree on the list of transitions: the allocation
asks, for every transition with a rate, for its probability layer and its patch settings, and stops if one is
missing. Feeding the same chain to all three guarantees they match.

### Step 2: the loop

```tfgraph
#! caption: The loop alone (the fitted pieces of step 1 feed it: woe.model, markov.markov, patches.patches). The simulation: a control.repeat of 14 steps carries the map (landscape). Its body computes the probabilities on the map of the step (@landscape) and allocates the change; carry passes the new map to the next step, collect keeps the map and the table of every step.
#! highlight: probability, allocate
#! params: false
#! fragment: true
format = "terraflux/1"

[inputs.t0]
kind = "map"
path = "sibuyan_landuse.tif"
band = "1997"

[inputs.t1]
kind = "map"
path = "sibuyan_landuse.tif"
band = "2011"

[inputs.drivers]
kind = "stack"
path = "sibuyan_drivers.tif"

[inputs.seed]
kind = "value"
value = 1

[[nodes]]
id = "simulation"
op = "control.repeat"
inputs = { times = 14, landscape = "$t0" }
note = "One iteration per time step, the map carried from step to step."

[nodes.body]
carry = { landscape = "allocate.landscape" }
collect = { maps = "allocate.landscape", info = "allocate.info" }

[[nodes.body.nodes]]
id = "probability"
op = "woe.probability"
inputs = { model = "woe.model", landscape = "@landscape", drivers = "$drivers" }
note = "The transition probabilities of the current map."

[[nodes.body.nodes]]
id = "allocate"
op = "dinamica.step"
inputs = { landscape = "@landscape", probability = "probability.probability", rates = "markov.markov", patches = "patches.patches", step = "@step", seed = "$seed" }
params = { prune_factor = 10.0, window = [3, 3] }
note = "Expander and Patcher: each transition changes its rate times the cells of its origin class."
```

The whole model, as written and run for the figures, is [dinamica.toml](/data/tutorials/land-change/dinamica-on-sibuyan/dinamica.toml).
The loop node, `simulation` ([[control.repeat]]), has two inputs:

- `times = 14`: how many times the body runs;
- `landscape = "$t0"`: a **carried value**, the map of 1997 at the first step. Any input of a loop other than
  `times` is carried: inside the body it is read as `@landscape`.

And two tables that define what moves between steps and what is kept:

- `carry = { landscape = "allocate.landscape" }`: at the end of every step, the carried value `landscape` takes
  the map the allocation gives; the next step's `@landscape` is that map. This is the line that makes the years
  follow each other.
- `collect = { maps = "allocate.landscape", info = "allocate.info" }`: the values to keep from every step. After
  the loop, `simulation.maps` is the series of the fourteen maps and `simulation.info` the fourteen tables (one
  CSV of fourteen rows when written). `simulation.landscape`, the carried value after the last step, is the map
  of 2011.

The body has two nodes:

1. **`probability`** ([[woe.probability]]) reads the weights from *outside* the loop (`woe.model`: fitted once),
   the map from *inside* (`@landscape`: this step's map) and the drivers (`$drivers`). Its output is the stack of
   a probability layer per transition, on this step's map.
2. **`allocate`** ([[dinamica.step]]) reads the same `@landscape`, the probabilities just computed, the rates
   (`markov.markov`) and the patch settings (`patches.patches`) from outside, the step number (`@step`, 1 to 14,
   a value every loop gives its body) and the seed (`$seed`). It gives the map after the step (`landscape`) and
   the table of what it did (`info`).

!!! warning "Inside the loop, read the carried map"
    Both body nodes must read `@landscape`. If `probability` read `$t0` instead, every step would compute the
    probabilities of 1997 and allow cells that already changed to change again; if `allocate` read `$t0`, every
    step would start again from 1997 and the loop would give fourteen one-year simulations instead of one
    fourteen-year simulation. The application refuses a cycle but not this mistake: check the connections of a
    loop's body.

Why does `step` matter? The allocation's random numbers are drawn from the seed *and* the step, so every year
gets its own draws while the whole run stays reproducible: the same seed gives the same fourteen maps on any
computer.

!!! app "In the application"
    Double-click the `simulation` node (or **Edit the body**) to see its body; the column at its left lists what
    the body can read: `@landscape`, `@step` and the outer nodes it uses. To make another value carried (a second
    map, a population of agents), add an input to the loop and set what it takes at the end of each step in the
    loop's **carry**. The trail at the top of the canvas brings you back to the model.

### Step 3: the outputs

```toml
[outputs.maps]
from = "simulation.maps"      # maps.tif, a band per step

[outputs.steps]
from = "simulation.info"      # steps.csv, a row per step

[outputs.weights]
from = "woe.table"            # weights.csv, the weights report
```

The model's outputs say what `terraflux run` writes into the output folder. Any port can be one: add
`[outputs.patches] from = "patches.patches"` to keep the patch settings as JSON, or a [[raster.write]] node for a
map at a path of your choice. In the application, the maps of a loop show in the **Map** pane as they are
computed, and the **Charts** pane draws the area of each class along the steps.

## Read the results

### Fourteen years

```tfslider
label: Year
caption: Sibuyan simulated from 1997 to 2011 by Dinamica EGO's model (seed 1). The chart follows the share of each class; the marker follows the slider. Forest in green, coconut in ochre, grassland in light green, rice in blue, other in purple.
frames:
  - { value: 1997, src: /img/tutorials/land-change/dinamica-on-sibuyan/step_0.png, caption: "1997, the start", stats: { Forest: "65.4%", Coconut: "16.0%", Grassland: "11.6%", Rice: "3.1%", "Cells changed": "0" } }
  - { value: 1998, src: /img/tutorials/land-change/dinamica-on-sibuyan/step_1.png, caption: "Step 1", stats: { Forest: "64.8%", Coconut: "16.3%", Grassland: "11.9%", Rice: "3.2%", "Cells changed": "56" } }
  - { value: 1999, src: /img/tutorials/land-change/dinamica-on-sibuyan/step_2.png, caption: "Step 2", stats: { Forest: "64.2%", Coconut: "16.5%", Grassland: "12.2%", Rice: "3.2%", "Cells changed": "56" } }
  - { value: 2000, src: /img/tutorials/land-change/dinamica-on-sibuyan/step_3.png, caption: "Step 3", stats: { Forest: "63.6%", Coconut: "16.7%", Grassland: "12.4%", Rice: "3.3%", "Cells changed": "56" } }
  - { value: 2001, src: /img/tutorials/land-change/dinamica-on-sibuyan/step_4.png, caption: "Step 4", stats: { Forest: "63.0%", Coconut: "17.0%", Grassland: "12.7%", Rice: "3.4%", "Cells changed": "57" } }
  - { value: 2002, src: /img/tutorials/land-change/dinamica-on-sibuyan/step_5.png, caption: "Step 5", stats: { Forest: "62.5%", Coconut: "17.2%", Grassland: "12.9%", Rice: "3.5%", "Cells changed": "57" } }
  - { value: 2003, src: /img/tutorials/land-change/dinamica-on-sibuyan/step_6.png, caption: "Step 6", stats: { Forest: "61.9%", Coconut: "17.5%", Grassland: "13.2%", Rice: "3.5%", "Cells changed": "57" } }
  - { value: 2004, src: /img/tutorials/land-change/dinamica-on-sibuyan/step_7.png, caption: "Step 7", stats: { Forest: "61.3%", Coconut: "17.7%", Grassland: "13.4%", Rice: "3.6%", "Cells changed": "57" } }
  - { value: 2005, src: /img/tutorials/land-change/dinamica-on-sibuyan/step_8.png, caption: "Step 8", stats: { Forest: "60.8%", Coconut: "18.0%", Grassland: "13.7%", Rice: "3.6%", "Cells changed": "58" } }
  - { value: 2006, src: /img/tutorials/land-change/dinamica-on-sibuyan/step_9.png, caption: "Step 9", stats: { Forest: "60.2%", Coconut: "18.2%", Grassland: "13.9%", Rice: "3.7%", "Cells changed": "58" } }
  - { value: 2007, src: /img/tutorials/land-change/dinamica-on-sibuyan/step_10.png, caption: "Step 10", stats: { Forest: "59.7%", Coconut: "18.5%", Grassland: "14.2%", Rice: "3.8%", "Cells changed": "57" } }
  - { value: 2008, src: /img/tutorials/land-change/dinamica-on-sibuyan/step_11.png, caption: "Step 11", stats: { Forest: "59.2%", Coconut: "18.7%", Grassland: "14.4%", Rice: "3.8%", "Cells changed": "56" } }
  - { value: 2009, src: /img/tutorials/land-change/dinamica-on-sibuyan/step_12.png, caption: "Step 12", stats: { Forest: "58.6%", Coconut: "18.9%", Grassland: "14.7%", Rice: "3.9%", "Cells changed": "56" } }
  - { value: 2010, src: /img/tutorials/land-change/dinamica-on-sibuyan/step_13.png, caption: "Step 13", stats: { Forest: "58.1%", Coconut: "19.2%", Grassland: "14.9%", Rice: "3.9%", "Cells changed": "56" } }
  - { value: 2011, src: /img/tutorials/land-change/dinamica-on-sibuyan/step_14.png, caption: "Step 14", stats: { Forest: "57.6%", Coconut: "19.4%", Grassland: "15.1%", Rice: "4.0%", "Cells changed": "57" } }
chart:
  type: line
  data: /data/tutorials/land-change/dinamica-on-sibuyan/areas.csv
  x: year
  y: [Forest, Coconut, Grassland, Rice, Other]
  colors: { Forest: "#3b8a5a", Coconut: "#d9a441", Grassland: "#a7c957", Rice: "#4f9fc4", Other: "#b4508a" }
  ylabel: "% of the island"
```

![Legend of the classes](/img/tutorials/land-change/dinamica-on-sibuyan/legend.png)

Move the slider and watch the north-west, where grassland spreads from its old patches into the forest, and the
coasts, where coconut grows around older coconut and rice spreads along the south-western shore. Every year about
57 cells change (356 ha, 0.8% of the island); after fourteen years the shares are those of the observed map of
2011 to the tenth of a percent (forest 57.6%, coconut 19.4%, grassland 15.1%, rice 4.0%). That agreement is built
in: the rates come from the same two maps. It says the chain was applied correctly, not that the model is good.

### What the steps table says

`steps.csv` has a row per step and, for every transition, four columns: `requested_1_3` (the cells the rate asks
for), `expanded_1_3` and `patched_1_3` (made by each function) and `patches_1_3` (patches formed). Check that
every row has `expanded + patched = requested`; on Sibuyan they do at every step. The requests fall slowly, as
the origin classes shrink:

```tfchart
type: line
data: /data/tutorials/land-change/dinamica-on-sibuyan/requested.csv
x: step
y: ["forest to grassland", "forest to rice", "forest to coconut", "grassland to coconut", "rice to coconut"]
colors: { "forest to grassland": "#a7c957", "forest to rice": "#4f9fc4", "forest to coconut": "#d9a441", "grassland to coconut": "#8a5a12", "rice to coconut": "#b4508a" }
xlabel: Step
ylabel: Cells requested
height: 280
caption: The cells each of the main transitions must change at every step (the requested columns of steps.csv). Forest to grassland, the largest, asks for 25 cells in the first year and 23 in the last.
```

### The simulated and the observed change

```tfgallery
columns: 2
caption: The cells that changed between 1997 and 2011, by their new class, in the observed maps (left) and in the simulation (right).
images:
  - { src: /img/tutorials/land-change/dinamica-on-sibuyan/change_observed.png, caption: "Observed" }
  - { src: /img/tutorials/land-change/dinamica-on-sibuyan/change_simulated.png, caption: "Simulated" }
```

```tfcompare
caption: Drag the handle between the observed map of 2011 (left) and the simulated one (right). The large masses agree; the differences are at the edges of the patches.
before: { src: /img/tutorials/land-change/dinamica-on-sibuyan/observed_2011.png, label: Observed 2011 }
after: { src: /img/tutorials/land-change/dinamica-on-sibuyan/simulated_2011.png, label: Simulated 2011 }
```

The simulated change sits in the same regions as the observed one: the grassland of the north-west, the coconut of
the south-east, the rice of the south-western coast. It also invents some (grassland patches in the north-east
that did not appear) and misses some (the scattered small changes of the east). Measured cell by cell, 461 of
the 743 cells that really changed were simulated with the right class (a figure of merit of 0.47); within a
window of 5 × 5 cells, the fuzzy similarity of the two changes is 0.74. What these numbers mean, and why the
93% of cells that agree is not one of them, is the subject of the next series,
[Validating a simulation](/tutorials/land-change/validating-a-simulation/). Add its two nodes to the model to
compute them at every run:

```toml
[[nodes]]
id = "agreement"
op = "validate.agreement"
inputs = { start = "$t0", observed = "$t1", simulated = "simulation.landscape" }

[[nodes]]
id = "similarity"
op = "validate.similarity"
inputs = { start = "$t0", observed = "$t1", simulated = "simulation.landscape" }
```

![The map of a Dinamica simulation of Sibuyan in the application](/img/app/map.png "The Map pane of the application after a run of the model: the simulated map of the last step, with a slider over the steps and small images of all of them.")

### What to report

The calibration period and the time step; the classes; the transitions and their rates per step (the chain);
the drivers, their ranges and the significant weights (part 1); the patch settings (part 2); the seed and the
number of replicates; the validation against an observed map with its metrics at several resolutions (the next
series). Publish the model file: it holds every one of these choices, readable.

## Settings that change the answer

### The seed

The allocation is stochastic. With another seed, the same model gives another landscape, the same amounts of
each class in other places:

```tfgallery
columns: 5
caption: The change simulated in the north-west of Sibuyan from 1997 to 2011 with five seeds (terraflux run sibuyan.toml --set seed=N). The same regions change in every run; the exact patches differ. Figure of merit 0.43 to 0.48; fuzzy similarity in a 5 x 5 window 0.72 to 0.77.
images:
  - { src: /img/tutorials/land-change/dinamica-on-sibuyan/seed_1.png, caption: "seed 1" }
  - { src: /img/tutorials/land-change/dinamica-on-sibuyan/seed_2.png, caption: "seed 2" }
  - { src: /img/tutorials/land-change/dinamica-on-sibuyan/seed_3.png, caption: "seed 3" }
  - { src: /img/tutorials/land-change/dinamica-on-sibuyan/seed_4.png, caption: "seed 4" }
  - { src: /img/tutorials/land-change/dinamica-on-sibuyan/seed_5.png, caption: "seed 5" }
```

![Legend of the change](/img/tutorials/land-change/dinamica-on-sibuyan/change_legend.png)

A single run is one draw among many possible landscapes. The cells that change in every run are consequences of
the rates, the weights and the shapes; those that change in a few runs are where the allocation happened to
fall. Run replicates and report what is common to them ([Calibration and many
runs](/tutorials/land-change/calibration-and-many-runs/)): `terraflux submit sibuyan.toml --replicates 50` runs
fifty seeds on the cores of the computer.

### The ranges of the weights

The model as `terraflux new dinamica` writes it cuts every continuous driver into a hundred buffers (1,435
ranges in all on Sibuyan); with the increments chosen in part 1 there are 677. On the same seed the chosen ranges
give a figure of merit of 0.47 against 0.41 for the automatic ones, and a similarity at the cell level of 0.60
against 0.56: noisy weights of thin ranges place the change less well. Choose the increments (or use
`--increment` on the command line) rather than keep the default.

### Projecting beyond the observed period

The point of fitting a model on 1997 to 2011 is usually to ask what comes after. `--start` sets the map the
simulation starts from and `--simulate` the steps to run, while the pieces are still fitted on `--t0` and
`--t1`:

```bash
terraflux new dinamica --t0 sibuyan_landuse.tif:1997 --t1 sibuyan_landuse.tif:2011 \
    --drivers sibuyan_drivers.tif --steps 14 --start sibuyan_landuse.tif:2011 --simulate 14 -o projection.toml
```

The model gets a fifth input, `start` (the map of 2011), which the loop carries instead of `$t0`, and `times =
14`. The rates, the weights and the patches are those of 1997 to 2011:

```tfgallery
columns: 2
caption: The observed map of 2011 and the map projected for 2025 by the model fitted on 1997-2011, with the same weights' ranges (seed 1). If the trend of 1997-2011 went on, forest would fall to 51% of the island and coconut and grassland would cover 41%.
images:
  - { src: /img/tutorials/land-change/dinamica-on-sibuyan/map_2011.png, caption: "Observed, 2011" }
  - { src: /img/tutorials/land-change/dinamica-on-sibuyan/map_2025.png, caption: "Projected, 2025" }
```

A projection is a scenario, "if the processes of 1997 to 2011 go on", not a forecast. Report it as such, with its
replicates, and compare it with other scenarios (rates given by [[markov.matrix]], a protected area as a weight
set by hand, a road as a changed driver).

### Other settings

- `prune_factor` and `window` of [[dinamica.step]] (part 2); `--prune-factor` on the command line.
- The isometry and the Expander's share of [[patches.observe]] (`--isometry`, `--expansion`): calibrate them
  against the observed map, as Soares-Filho et al. (2013) do for Dinamica EGO's models ([Calibration and many runs](/tutorials/land-change/calibration-and-many-runs/)).
- The number of steps: yearly steps from a 14-year period are the 14th root of the period's matrix; two steps of
  seven years are its square root. Choose the step that matches the questions (and the drivers that change).

## Common pitfalls

- **Validating on the calibration period.** A model fitted on 1997-2011 and compared with 2011 has only been
  checked for consistency. Its real test is a period it did not see: with three dates, fit on the first two and
  validate on the third ([Validating a simulation](/tutorials/land-change/validating-a-simulation/)).
- **Classes the chain cannot change.** A class with no observed transitions has no rates; it stays as it is in
  every simulation (Sibuyan's *Other* stays at 3.9%). That is the model's assumption, not a fact.
- **Drivers in another order or other names.** The weights are tied to the drivers' names. A drivers file for
  2025 must have bands of the same names, or the weights no longer find their driver.
- **Maps not on the same grid.** The two dates and the drivers must share the grid (extent, cell size, rows and
  columns). `terraflux check` and the Wizard warn; `terraflux resample drivers.tif --like landuse.tif` puts a
  raster on another's grid.
- **The loop reading the start map.** Inside the body, read `@landscape` (see the warning above).
- **One run reported as the result.** Report replicates, and what is common to them.

## Going further

- [Validating a simulation](/tutorials/land-change/validating-a-simulation/): the figure of merit, fuzzy
  similarity and the honest test on another period.
- [Calibration and many runs](/tutorials/land-change/calibration-and-many-runs/): replicates, scenarios, and
  settings chosen against an observed map.
- [Dynamic drivers](/tutorials/land-change/dynamic-drivers/): drivers recomputed in the loop.
- [A demand per region](/tutorials/land-change/regional-demand/): rates of their own for parts of the map.
- [Maps larger than memory](/tutorials/land-change/maps-larger-than-memory/): the same model on a continent.
- The operators: [[markov.fit]], [[woe.fit]], [[woe.probability]], [[patches.observe]], [[dinamica.step]],
  [[control.repeat]].

## References

- Bonham-Carter, G. F. (1994). *Geographic Information Systems for Geoscientists: Modelling with GIS*. Pergamon.
- Soares-Filho, B. S., Coutinho Cerqueira, G., & Lopes Pennachin, C. (2002). DINAMICA: a stochastic cellular automata model designed to simulate the landscape dynamics in an Amazonian colonization frontier. *Ecological Modelling*, 154(3), 217–235. https://doi.org/10.1016/S0304-3800(02)00059-5
- Soares-Filho, B. S., Nepstad, D. C., Curran, L. M., Cerqueira, G. C., Garcia, R. A., Ramos, C. A., et al. (2006). Modelling conservation in the Amazon basin. *Nature*, 440(7083), 520–523. https://doi.org/10.1038/nature04389
- Soares-Filho, B., Rodrigues, H., & Follador, M. (2013). A hybrid analytical-heuristic method for calibrating land-use change models. *Environmental Modelling & Software*, 43, 80–87. https://doi.org/10.1016/j.envsoft.2013.01.010
- Verburg, P. H., Soepboer, W., Veldkamp, A., Limpiada, R., Espaldon, V., & Mastura, S. S. A. (2002). Modeling the spatial dynamics of regional land use: the CLUE-S model. *Environmental Management*, 30(3), 391–405. https://doi.org/10.1007/s00267-002-2630-x
