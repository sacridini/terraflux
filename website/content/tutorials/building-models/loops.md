---
title: "Loops: write your own cellular automaton"
short: Loops
description: Run part of a model again and again with control.repeat, pass a map from step to step with carry, keep every step with collect, and write a cellular automaton of a growing city.
order: 30
level: intermediate
time: 45 min
template: A city that grows (a loop)
ops: [control.repeat, raster.calc, nlm.gaussian_field, nlm.distance_gradient, view.plot, raster.write]
---

A simulation is the same rule applied again and again: this year's forest decides next year's, today's city
decides where it grows tomorrow. In Terraflux that repetition is a *loop*, the container [[control.repeat]]. In
this tutorial you open the template "A city that grows (a loop)", take its loop apart, tune its rule, and watch a
city spread over 25 steps. You will know how a value is passed from one step to the next (`carry`), how every
step is kept (`collect`), what `@step` is for, and how a cellular automaton's rule decides the patterns it
makes.

## The idea

A *cellular automaton* (CA) is a grid of cells, each in one of a few states (city or not, burnt or not, occupied
or empty), and a rule that gives a cell's next state from its own state and the states of its *neighbours*. The
rule is applied to every cell at once, and again at the next step, and so on. John von Neumann and Stanisław
Ulam invented the idea in the 1940s; simple rules turned out to make complex patterns.

Ecologists took CAs up as a way to model space explicitly: plants that spread to neighbouring cells, fires,
epidemics, competition between species (Hogeweg, 1988; Balzter et al., 1998). Geographers used them for cities:
a cell becomes urban when its neighbours are urban and the land is suitable, and the rules reproduce the
fractal edges of real cities (White & Engelen, 1993) and their history of growth (SLEUTH: Clarke et al., 1997).
Land change models such as Dinamica EGO are CAs too, with rules fitted to observed maps (Soares-Filho et al.,
2002).

!!! science "Key idea"
    Writing \(x_t(i)\) for the state of cell \(i\) at step \(t\) and \(N(i)\) for its neighbours, a CA is
    \[ x_{t+1}(i) = f\big(x_t(i),\ \{x_t(j) : j \in N(i)\},\ z(i),\ t\big) \]
    where \(f\) is the rule and \(z(i)\) are fixed properties of the cell (its suitability, its elevation).
    Every cell's new state is computed from the states of the step before: the whole grid is updated at once
    (*synchronously*).

A loop is what turns a rule into a simulation. [[control.repeat]] runs its *body* (a few nodes) once per step,
`times` times. Two lists say what flows through it:

- **carry**: values passed from one step to the next. `carry = { city = "grow.map" }` means: inside the body the
  city is `@city`; at the first step it is the loop node's input `city`; at every next step it is what `grow.map`
  gave at the step before.
- **collect**: values kept at every step. `collect = { maps = "grow.map" }` makes a *series*, the map of every
  step, available after the loop as `growth.maps`.

Inside the body, `@step` is the number of the step (1, 2, ..., `times`). After the loop, every carried value is
an output with its last value (`growth.city`), and every collected one a series (`growth.maps`).

![How carry and collect move values through the loop](/img/tutorials/building-models/loops/loop_diagram.png "The loop step by step. The first @city is the loop's input (town.map); each step's grow.map is carried to the next as @city (orange) and collected into growth.maps (green). After the last step, growth.city is the last city and growth.maps holds all 25.")

## When to use it

- **Cellular automata and spread.** Urban growth, fire, invasion, dispersal: a state that moves to neighbours
  step after step.
- **Simulations with time.** Land change models (Dinamica EGO, CLUE-S, PLUS) are loops whose body is an
  allocation; their models built by `terraflux new` and the templates are loops you can open and change.
- **Dynamic variables.** A driver recomputed from the simulated map at every step (the distance to farmland, the
  share of forest around each cell), put in the drivers' stack with [[stack.set]] (see
  [Stacks and layers](../stacks-and-layers/)).
- **Agents.** Populations whose attributes change every step, carried along with the map (see the
  [agents](../../land-change/) tutorials).

What a loop is *not* for: running the same model with many settings or seeds. That is a batch of runs
(`terraflux submit`, the **Many runs** pane), each run independent; see [Randomness and reproducibility](../randomness/)
and the [parallel computing guide](/parallel/).

## What you need

Only Terraflux: the template **A city that grows (a loop)** draws its landscape with neutral landscape models,
so it runs as soon as it opens (**Templates** menu, group *Land change*). Its landscape is 150 × 150 cells:

- a relief from 0 to 1 ([[nlm.gaussian_field]]), whose lowest land (below 0.3) is water and highest (above
  0.75) mountains;
- an *attraction* from 0 to 1 (another Gaussian field), standing for jobs, roads and services;
- the distance to the old town in the middle ([[nlm.distance_gradient]]).

## Build the model

### Step 1: what does not change goes outside the loop

```tfgraph
#! caption: The landscape and the suitability, computed once before the loop.
#! highlight: suitability
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "relief"
op = "nlm.gaussian_field"
params = { rows = 150, columns = 150, range = 30, nugget = 0, seed = 41 }

[[nodes]]
id = "attraction"
op = "nlm.gaussian_field"
params = { rows = 150, columns = 150, range = 20, seed = 42 }

[[nodes]]
id = "centre"
op = "nlm.distance_gradient"
params = { rows = 150, columns = 150, origin = [72, 78, 72, 78], nlmr_compat = false, rescale = false }

[[nodes]]
id = "suitability"
op = "raster.calc"
inputs = { a = "attraction.map", d = "centre.map", r = "relief.map" }
params = { expression = "None if r < 0.3 or r > 0.75 else 0.6 * a + 0.4 * (1 - d / 106)" }
note = "Attractive and near the centre; no value in the water and on the mountains."
```

The suitability (node 4) is 60 % attraction and 40 % closeness to the centre (106 cells is about the farthest a
cell can be from it), and has **no value** in the water and on the mountains. A city will never grow into a cell
without a value: comparisons with None are None, and `a if c else b` with a condition of None gives None (see
[Map algebra](../map-algebra/#missing-values)).

Everything that is the same at every step belongs **outside** the loop. The body can read any node of the model
(`suitability.map`) directly; it is computed once, not 25 times.

```tfgallery
columns: 2
caption: The relief and the suitability it leaves (no value, white, in the water and on the mountains).
images:
  - { src: /img/tutorials/building-models/loops/relief.png, caption: "relief.map" }
  - { src: /img/tutorials/building-models/loops/suitability.png, caption: "suitability.map" }
```

### Step 2: the state at the start

```text
None if isnull(suit) else d == 0
```

The node `town` makes the first state: 1 in the old town (where the distance to the centre is 0, a square of
7 × 7 cells less its water: 33 cells), 0 on the rest of the land, and no value where nothing can ever grow.

### Step 3: the loop

```tfgraph
#! caption: The loop of 25 steps. Its body's one node reads the city of the step before (@city), the step number (@step) and the suitability from outside, and its map is carried to the next step and collected.
#! highlight: grow
format = "terraflux/1"

[[nodes]]
id = "relief"
op = "nlm.gaussian_field"
params = { rows = 150, columns = 150, range = 30, nugget = 0, seed = 41 }

[[nodes]]
id = "attraction"
op = "nlm.gaussian_field"
params = { rows = 150, columns = 150, range = 20, seed = 42 }

[[nodes]]
id = "centre"
op = "nlm.distance_gradient"
params = { rows = 150, columns = 150, origin = [72, 78, 72, 78], nlmr_compat = false, rescale = false }

[[nodes]]
id = "suitability"
op = "raster.calc"
inputs = { a = "attraction.map", d = "centre.map", r = "relief.map" }
params = { expression = "None if r < 0.3 or r > 0.75 else 0.6 * a + 0.4 * (1 - d / 106)" }

[[nodes]]
id = "town"
op = "raster.calc"
inputs = { d = "centre.map", suit = "suitability.map" }
params = { expression = "None if isnull(suit) else d == 0" }

[[nodes]]
id = "growth"
op = "control.repeat"
inputs = { times = 25, city = "town.map" }

[nodes.body]
carry = { city = "grow.map" }
collect = { maps = "grow.map" }

[[nodes.body.nodes]]
id = "grow"
op = "raster.calc"
inputs = { c = "@city", suit = "suitability.map", step = "@step" }
params = { expression = "1 if c == 0 and nbsum(c == 1, 1) >= 3 and suit > 0.6 - step * 0.008 else c" }
note = "A cell becomes city when three of its neighbours are and it is suitable enough; the bar falls every step."

[[nodes]]
id = "plot_growth"
op = "view.plot"
inputs = { value = "growth.maps" }
params = { title = "The city, step by step" }
```

The loop node `growth` has two inputs: `times = 25` and `city = "town.map"`, the first value of `@city`. Its
body has one node, `grow`, a [[raster.calc]] with three inputs: `c` is `@city`, `suit` the suitability from outside
the loop, and `step` is `@step`. Its rule:

```text
1 if c == 0 and nbsum(c == 1, 1) >= 3 and suit > 0.6 - step * 0.008 else c
```

reads: a cell that is not city yet (`c == 0`) becomes city (`1`) when at least 3 of the 9 cells of its 3 × 3
window are city, and its suitability is above a bar; every other cell keeps its state (`else c`). The bar starts
at 0.592 and falls by 0.008 every step, to 0.4 at step 25: the best land goes first, and the city takes poorer
land as it runs out of good land.

`carry = { city = "grow.map" }` passes the new city to the next step; `collect = { maps = "grow.map" }` keeps
every step.

!!! note "Why the bar starts where it does"
    The template's rule is the one above: three city cells in the window and a bar from 0.592 down to 0.4. Try a
    stricter bar, `suit > 0.8 - step * 0.008`, and the city does not move: the cells touching the old town have a
    suitability of 0.50 to 0.59, so none passes the bar in 25 steps, and the map after the loop is the old town's
    33 cells. A bar above all the land within reach is a rule that is never true, and the first thing to look for
    when a simulation does not move.

!!! science "Counting neighbours: nbsum, not nbcount"
    The rule counts the city cells of the window with `nbsum(c == 1, 1)`. `c == 1` is 1 on a city cell and 0 on
    the rest of the land, so its sum over the 3 × 3 window is the number of city cells in it. Map algebra also has
    `nbcount(x, r)`, and its name is a trap here: it counts the cells of the window that **have a value**,
    whatever the value is. Since `c == 1` has a value (0 or 1) on all the land, `nbcount(c == 1, 1)` is 9 almost
    everywhere, the neighbour test is always true, and the city appears at once in every cell that passes the
    bar, far from the old town (18,299 cells after 25 steps instead of 321). `nbcount` is for counting the cells
    with data, for example to take a mean over the cells that have one.

    ```tfgallery
    columns: 2
    caption: The city after 25 steps with this tutorial's rule, its neighbours counted by nbsum (left) and by nbcount (right). With nbsum it grows out of the old town; with nbcount every cell passing the bar becomes city, wherever it is.
    images:
      - { src: /img/tutorials/building-models/loops/nbsum.png, caption: "nbsum(c == 1, 1) >= 3: 321 cells" }
      - { src: /img/tutorials/building-models/loops/nbcount.png, caption: "nbcount(c == 1, 1) >= 3: 18,299 cells" }
    ```

!!! app "In the application"
    Open **Templates**, group *Land change*, **A city that grows (a loop)**. Double-click the node `growth`
    (or select it and click **Edit the body** in the Node pane) to go inside the loop; the trail at the top takes
    you back. Inside, the column on the left lists what the body can read: `@city`, `@step` and the outer nodes.
    Select `grow` and change its expression. To build a loop from scratch: add [[control.repeat]] (library
    subject **Model flow**), give it `times` and an input named like the carried value (`city`), add the body's
    nodes, then in the Node pane's **Body** section choose `carry` or `collect`, type the name and the
    reference (`grow.map`) and click **Set**.

### Step 4: see and keep the steps

[[view.plot]] of `growth.maps` shows the steps in a tab, with a slider. To write them, connect `growth.maps` to
[[raster.write]]: the file has one band per step. From the command line, `terraflux run model.toml` runs the
loop and prints each step.

!!! tip "Name the series maps"
    In the application, a loop's collected series named `maps` is followed as the run goes: the cells of each
    class at every step are counted and drawn in the **Areas** pane.

## Read the results

```tfslider
label: Step
caption: "The city at every step of the loop (purple), the cells it took at that step in orange. Growth is slow while the bar is high, then speeds up as more land passes it; it flows around the lake and stops at the water."
frames:
  - { value: 0, src: /img/tutorials/building-models/loops/step_0.png, stats: { "City (cells)": "33", "New this step": "0" } }
  - { value: 1, src: /img/tutorials/building-models/loops/step_1.png, stats: { "City (cells)": "33", "New this step": "0" } }
  - { value: 2, src: /img/tutorials/building-models/loops/step_2.png, stats: { "City (cells)": "34", "New this step": "1" } }
  - { value: 3, src: /img/tutorials/building-models/loops/step_3.png, stats: { "City (cells)": "34", "New this step": "0" } }
  - { value: 4, src: /img/tutorials/building-models/loops/step_4.png, stats: { "City (cells)": "35", "New this step": "1" } }
  - { value: 5, src: /img/tutorials/building-models/loops/step_5.png, stats: { "City (cells)": "36", "New this step": "1" } }
  - { value: 6, src: /img/tutorials/building-models/loops/step_6.png, stats: { "City (cells)": "39", "New this step": "3" } }
  - { value: 7, src: /img/tutorials/building-models/loops/step_7.png, stats: { "City (cells)": "40", "New this step": "1" } }
  - { value: 8, src: /img/tutorials/building-models/loops/step_8.png, stats: { "City (cells)": "44", "New this step": "4" } }
  - { value: 9, src: /img/tutorials/building-models/loops/step_9.png, stats: { "City (cells)": "49", "New this step": "5" } }
  - { value: 10, src: /img/tutorials/building-models/loops/step_10.png, stats: { "City (cells)": "53", "New this step": "4" } }
  - { value: 11, src: /img/tutorials/building-models/loops/step_11.png, stats: { "City (cells)": "57", "New this step": "4" } }
  - { value: 12, src: /img/tutorials/building-models/loops/step_12.png, stats: { "City (cells)": "67", "New this step": "10" } }
  - { value: 13, src: /img/tutorials/building-models/loops/step_13.png, stats: { "City (cells)": "79", "New this step": "12" } }
  - { value: 14, src: /img/tutorials/building-models/loops/step_14.png, stats: { "City (cells)": "90", "New this step": "11" } }
  - { value: 15, src: /img/tutorials/building-models/loops/step_15.png, stats: { "City (cells)": "101", "New this step": "11" } }
  - { value: 16, src: /img/tutorials/building-models/loops/step_16.png, stats: { "City (cells)": "117", "New this step": "16" } }
  - { value: 17, src: /img/tutorials/building-models/loops/step_17.png, stats: { "City (cells)": "136", "New this step": "19" } }
  - { value: 18, src: /img/tutorials/building-models/loops/step_18.png, stats: { "City (cells)": "155", "New this step": "19" } }
  - { value: 19, src: /img/tutorials/building-models/loops/step_19.png, stats: { "City (cells)": "178", "New this step": "23" } }
  - { value: 20, src: /img/tutorials/building-models/loops/step_20.png, stats: { "City (cells)": "200", "New this step": "22" } }
  - { value: 21, src: /img/tutorials/building-models/loops/step_21.png, stats: { "City (cells)": "222", "New this step": "22" } }
  - { value: 22, src: /img/tutorials/building-models/loops/step_22.png, stats: { "City (cells)": "246", "New this step": "24" } }
  - { value: 23, src: /img/tutorials/building-models/loops/step_23.png, stats: { "City (cells)": "270", "New this step": "24" } }
  - { value: 24, src: /img/tutorials/building-models/loops/step_24.png, stats: { "City (cells)": "297", "New this step": "27" } }
  - { value: 25, src: /img/tutorials/building-models/loops/step_25.png, stats: { "City (cells)": "321", "New this step": "24" } }
chart:
  type: line
  data: /data/tutorials/building-models/loops/city.csv
  x: step
  y: [city, new]
  labels: { city: "City (cells)", new: "New this step" }
  colors: { city: "#b4508a", new: "#e8901f" }
  xlabel: Step
  ylabel: Cells
```

The city grows from 33 cells to 321 in 25 steps, almost ten times. Its growth curve has the shape of many real
ones: slow at first, then faster and steadier as the bar falls into the range of suitabilities around the town
(about 0.5 to 0.6). The lake south of the old town stays empty because water has no suitability, and the city
wraps around it.

What to look at when you check a CA:

- **The curve of growth.** A curve that jumps at the first step, or never moves, usually means a rule that is
  always or never true (a neighbour test written with `nbcount`, a bar above every suitability within reach).
- **The shape.** Compact, round growth comes from a rule that needs many city neighbours; scattered growth from
  one that needs few, or none.
- **Where it stops.** Cells without a value (water, mountains, outside the study area) are walls.

## Settings that change the answer

### How fast the bar falls

The bar `0.6 - step * rate` (this tutorial's start) decides how much land is suitable enough at every step. It is the main brake of
this model:

```tfslider
label: The bar falls by (per step)
caption: "The city after 25 steps when the bar falls by 0.004 to 0.012 per step. A slow fall (0.004) leaves the city at 92 cells; a fast one lets it take 403."
frames:
  - { value: 0.004, src: /img/tutorials/building-models/loops/rate_0.004.png, stats: { "City after 25 steps": "92 cells" } }
  - { value: 0.006, src: /img/tutorials/building-models/loops/rate_0.006.png, stats: { "City after 25 steps": "223 cells" } }
  - { value: 0.008, src: /img/tutorials/building-models/loops/rate_0.008.png, stats: { "City after 25 steps": "321 cells" } }
  - { value: 0.010, src: /img/tutorials/building-models/loops/rate_0.010.png, stats: { "City after 25 steps": "377 cells" } }
  - { value: 0.012, src: /img/tutorials/building-models/loops/rate_0.012.png, stats: { "City after 25 steps": "403 cells" } }
start: 2
```

### How many neighbours

The number of city neighbours a cell needs controls the *shape* of growth as much as its amount:

```tfgallery
columns: 4
caption: "The city after 25 steps when a cell needs at least 1, 2, 3 or 4 city cells in its 3 × 3 window (the window includes the cell itself, which is not city yet, so at most 8). One neighbour: 1,978 cells in long tongues along the best land. Two: 864 cells in a diamond. Three: 321 cells, rounder. Four: the old town never grows (33 cells), because no cell around it touches four of its cells."
images:
  - { src: /img/tutorials/building-models/loops/neighbours_1.png, caption: ">= 1" }
  - { src: /img/tutorials/building-models/loops/neighbours_2.png, caption: ">= 2" }
  - { src: /img/tutorials/building-models/loops/neighbours_3.png, caption: ">= 3" }
  - { src: /img/tutorials/building-models/loops/neighbours_4.png, caption: ">= 4" }
```

Rules like these are exactly what calibration is for: a CA of a real city is run with many settings and compared
with the observed growth ([[validate.similarity]], `terraflux calibrate`; see the [land change](../../land-change/)
tutorials). Report the rule and its settings with the results; a CA with unreported rules cannot be reproduced.

## Common pitfalls

- **`nbcount` is not a count of neighbours in a state.** It counts cells with a value. Use `nbsum(x == k, r)`
  for "how many neighbours are k".
- **The cell counts itself.** `nbsum(c == 1, 1)` includes the centre. For a cell that is not city yet it does not
  matter; for a rule about city cells ("a city cell with fewer than 2 city neighbours is abandoned") subtract it:
  `nbsum(c == 1, 1) - (c == 1)`.
- **A carried value needs a first value.** A name in `carry` must also be an input of the loop node; otherwise
  the run stops with *no first value for @city*.
- **`times` must be a whole number.** A number such as `25.5` stops the run. It can come from another node
  (`demand.steps` in a CLUE-S model).
- **Work done 25 times for nothing.** Anything that does not depend on `@step` or a carried value belongs outside
  the body.
- **Memory.** A collected series keeps every step's map in memory: 1,000 steps of a 10,000 × 10,000 map do not
  fit. Collect only what you need, or fewer steps.
- **The edge of the map.** Cells on the border have fewer neighbours (the window is cut by the edge), so they
  pass a neighbour rule less often. Keep the area of interest away from the border, or pad the map.
- **No randomness here.** This CA is deterministic: the same model gives the same city. Real growth has chance in
  it; the land change operators ([[dinamica.step]], [[lulcc.step]]) draw random numbers from a seed. See
  [Randomness and reproducibility](../randomness/).

## Going further

- [Map algebra from scratch](../map-algebra/): every function a rule can use.
- [Groups and submodels](../groups-and-submodels/): put a rule in a model file of its own and reuse it.
- [Randomness and reproducibility](../randomness/): seeds, replicates, and generators carried along a loop.
- The `until` of a loop's body ends it early when a yes-or-no value becomes true; CLUMondo's step
  ([[clumondo.step]]) gives one (`stop`, a year without a solution).
- The operator reference: [[control.repeat]], [[raster.calc]], [[view.plot]].

## References

- Balzter, H., Braun, P. W., & Köhler, W. (1998). Cellular automata models for vegetation dynamics. *Ecological Modelling*, 107(2–3), 113–125. https://doi.org/10.1016/S0304-3800(97)00202-0
- Clarke, K. C., Hoppen, S., & Gaydos, L. (1997). A self-modifying cellular automaton model of historical urbanization in the San Francisco Bay area. *Environment and Planning B: Planning and Design*, 24(2), 247–261. https://doi.org/10.1068/b240247
- Hogeweg, P. (1988). Cellular automata as a paradigm for ecological modeling. *Applied Mathematics and Computation*, 27(1), 81–100. https://doi.org/10.1016/0096-3003(88)90100-2
- Soares-Filho, B. S., Cerqueira, G. C., & Pennachin, C. L. (2002). DINAMICA: a stochastic cellular automata model designed to simulate the landscape dynamics in an Amazonian colonization frontier. *Ecological Modelling*, 154(3), 217–235. https://doi.org/10.1016/S0304-3800(02)00059-5
- White, R., & Engelen, G. (1993). Cellular automata and fractal urban form: a cellular modelling approach to the evolution of urban land-use patterns. *Environment and Planning A*, 25(8), 1175–1199. https://doi.org/10.1068/a251175
