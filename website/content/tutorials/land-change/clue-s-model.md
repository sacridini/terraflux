---
title: "A complete CLUE-S model of Plum Island"
short: "CLUE-S: the whole model"
description: Write lulcc's CLUE-S model of Plum Island with terraflux new clue, read every node and the loop that carries the map and its history, run fourteen years, validate them against 1999, and see what the elasticities and the allocation's settings do.
order: 530
level: intermediate
time: 50 min
template: CLUE-S
ops: [glm.fit, glm.predict, demand.from_maps, rng.r, lulcc.history, lulcc.step, control.repeat, validate.agreement, validate.three_map]
series: "Demand-driven allocation: CLUE-S and lulcc"
part: 4
---

The first three parts of this series built CLUE-S's pieces one at a time: the [competition between
classes](/tutorials/land-change/clue-s-ideas/), the [suitability of each class](/tutorials/land-change/logistic-suitability/)
from logistic regressions, and the [demand](/tutorials/land-change/land-demand/). In this part you assemble the
whole model of the Plum Island Ecosystems with one command, `terraflux new clue`, and read it node by node: what
each of its seven nodes does, how they connect, and how the loop carries two values from year to year, the map and
its history, while R's random number generator keeps drawing. You will run the fourteen years from 1985 to 1999,
watch the demand being met year by year, compare the simulated 1999 with the observed one, and move the two
settings that change the answer most: the **elasticities** and the allocation's **scale**. At the end you will
have a CLUE-S model you can adapt to your own maps.

## The idea

### The model in one sentence

Every year, every cell takes the land use whose suitability (plus a bonus for staying as it is, the elasticity,
plus a number per class adjusted until the areas match) is highest, so that the map holds the cells the demand
asks for that year; the next year starts from that map (Verburg et al., 2002). Three things are fitted or computed
once, before the years:

| Piece | What it is | Node | Part |
|---|---|---|---|
| suitability | a logistic regression of each class on its drivers, and its map | [[glm.fit]], [[glm.predict]] | [2](/tutorials/land-change/logistic-suitability/) |
| demand | the cells of each class in each year, interpolated from the observed maps | [[demand.from_maps]] | [3](/tutorials/land-change/land-demand/) |
| randomness | R's random number generator, seeded | [[rng.r]] | this part |
| history | how many years each cell has had its class | [[lulcc.history]] | [5](/tutorials/land-change/clue-s-rules/) |

and one thing runs every year, inside a loop: [[lulcc.step]], the allocation of that year's demand.

!!! science "Key idea: the loop of CLUE-S"
    For \(t = 1 \dots T\) (the years of the demand after the first):

    1. start from the map \(L_{t-1}\) (the observed map of 1985 at \(t = 1\)) and the history \(H_{t-1}\);
    2. for every cell \(i\) and class \(u\), \(TPROP_{i,u} = P_{i,u} + ELAS_u [L_{t-1}(i) = u] + ITER_u\), with
       \(P\) the suitability and the elasticity added only for the cell's current class;
    3. give each cell its class of highest \(TPROP\), count, move each \(ITER_u\) by `scale` × (demand − cells),
       and repeat until the counts are within `max_diff` (and on average `mean_diff`) of the demand of year \(t\);
    4. \(L_t\) is the map so allocated; \(H_t(i) = H_{t-1}(i) + 1\) where the class stayed, 0 where it changed.

### Why carry the history, and the generator

The **history** (`lulcc.history`) does nothing in this model: it is there because lulcc's conversion rules can
depend on it ("forest can become built land only after 5 years as other land": part 5), and because a rule needs
the history of the year it is applied in, so the history must travel with the map. The **generator** gives the
small random *jitter* added to the suitabilities to break ties. It is R's own generator (Mersenne-Twister after
`set.seed(seed)`): Terraflux draws exactly the numbers R would, which is why a run here gives lulcc's maps cell for
cell (Moulds et al., 2015). The generator is one object that every step draws from in turn, so the draws of a year
continue where the previous year's stopped, as in R.

## When to use it

Use the whole model when you have (or can assume) the **amount** of each land use year by year, and want its
**location**: scenarios of regional land use, the consequences of a target ("no net loss of forest"), several
classes growing at once. The [first part](/tutorials/land-change/clue-s-ideas/#when-to-use-it) discusses when CLUE-S
is the right family; [Choosing a land change model](/tutorials/land-change/choosing-a-model/) compares it with the
others.

## What you need

- **Observed maps** of land use, one band per date, named by year (`pie_landuse.tif`: 1985, 1991, 1999; 434 × 497
  cells of about 100 m; 1 forest, 2 built, 3 other; 255 no data).
- **Drivers** on the same grid (`pie_drivers.tif`: `elevation`, `slope`, `dist_built_1985`).
- **Formulas**: which drivers explain each class (from part 2).
- The files are on [the first part's page](/tutorials/land-change/clue-s-ideas/#what-you-need). The application's
  template **CLUE-S** builds the same model on maps drawn by chance.

## Build the model

### With one command

```bash
terraflux new clue --observed pie_landuse.tif --times 1985,1991,1999 --drivers pie_drivers.tif \
  --formulas '{"1": ["elevation", "slope"], "2": ["elevation", "slope", "dist_built_1985"], "3": ["elevation", "slope"]}' \
  --elasticity 0.2,0.2,0.2 \
  --settings '{"jitter": 0.0002, "scale": 0.000001, "max_iter": 1000, "max_diff": 50, "mean_diff": 50}' \
  -o clue.toml
terraflux run clue.toml --out clue
```

These are the settings of lulcc's vignette for Plum Island. On Windows' PowerShell, write the JSON with single
quotes as here; in `cmd`, escape the double quotes. Every option:

```tfcli
new clue
```

!!! note "The start band"
    `terraflux new clue` names the start map by its band's name when the observed file has band names
    (`band = "1985"` here), and by its index (`band = "0"`) when it has none. A name survives a file whose bands
    are reordered; with a file of unnamed bands, check that the index is the date you mean.

The run takes about 15 seconds on Plum Island (113,563 cells, 14 years; the slowest years iterate a few
hundred times) and writes `clue/maps.tif` (a band per year, 1986 to 1999) and `clue/steps.csv`.

!!! app "In the application"
    **Wizard...** writes the same model: choose *CLUE-S* in *The model*, the observed maps and the drivers in *The
    data* (the dates come from the band names), and the elasticity in *The settings*. The formulas are written with
    every driver for every class; edit them in the `regression` node's settings.

### Step 1: suitability, demand, generator and history

```tfgraph
#! caption: The pieces computed once: the regressions (1) and their suitability maps (2), the demand (3), R's generator (4) and the history (5).
#! numbered: true
#! highlight: regression, suitability, demand, rng, history
format = "terraflux/1"

[inputs.observed]
kind = "stack"
path = "pie_landuse.tif"

[inputs.drivers]
kind = "stack"
path = "pie_drivers.tif"

[inputs.start]
kind = "map"
path = "pie_landuse.tif"
band = "1985"

[inputs.seed]
kind = "value"
value = 1

[[nodes]]
id = "regression"
op = "glm.fit"
inputs = { landscape = "$start", drivers = "$drivers" }
params = { formulas = { 1 = ["elevation", "slope"], 2 = ["elevation", "slope", "dist_built_1985"], 3 = ["elevation", "slope"] } }
note = "Logistic regression of the presence of each class on its drivers (lulcc's glmModels)."

[[nodes]]
id = "suitability"
op = "glm.predict"
inputs = { model = "regression.model", drivers = "$drivers", landscape = "$start" }
note = "The suitability of each class at every cell; the drivers are static, so it is computed once."

[[nodes]]
id = "demand"
op = "demand.from_maps"
inputs = { maps = "$observed" }
params = { times = [1985, 1991, 1999] }
note = "The cells of each class at each step interpolated linearly from the observed maps."

[[nodes]]
id = "rng"
op = "rng.r"
inputs = { seed = "$seed" }
note = "R's generator after set.seed(seed), as lulcc draws."

[[nodes]]
id = "history"
op = "lulcc.history"
inputs = { landscape = "$start" }
note = "How long each cell has had its class (1 where not given)."
```

The model's four inputs are the observed maps (a stack: every band), the drivers (a stack), the start (one band
of the observed maps, 1985) and the seed.

1. **`regression`** ([[glm.fit]]) fits, on the start map, one logistic regression per class: presence of the
   class (1) or not (0) against the drivers listed in `formulas` (a table of class to drivers). Its `model` output
   holds the coefficients ([part 2](/tutorials/land-change/logistic-suitability/) reads them).
2. **`suitability`** ([[glm.predict]]) turns the regressions into a stack with one layer per class (named `1`,
   `2`, `3`), the probability of each class in each cell. It needs the drivers again, and the start map for its
   grid and its cells. Because Plum Island's drivers do not change during the simulation, it is computed once,
   outside the loop; a dynamic driver would move it inside (see [Dynamic drivers](/tutorials/land-change/dynamic-drivers/)).
3. **`demand`** ([[demand.from_maps]]) counts the cells of each class in each observed map and interpolates them
   linearly year by year (`times`: the date of each band). It has two outputs: `demand`, the table of cells per
   class and year, and `steps`, its number of years after the first (14), which the loop uses as its `times`. So
   the loop runs exactly as many years as the demand has.
4. **`rng`** ([[rng.r]]) makes R's generator from the seed.
5. **`history`** ([[lulcc.history]]) gives every cell a history of 1 (or reads a map of histories from its
   optional input).

### Step 2: the loop

```tfgraph
#! caption: The whole model. The loop (node 6) carries the map and its history; its body is one lulcc.step (node 7), which reads the suitability, the demand and the generator from outside and @landscape, @history and @step from the loop.
#! numbered: true
#! highlight: simulation, allocate
#! params: false
format = "terraflux/1"

[inputs.observed]
kind = "stack"
path = "pie_landuse.tif"

[inputs.drivers]
kind = "stack"
path = "pie_drivers.tif"

[inputs.start]
kind = "map"
path = "pie_landuse.tif"
band = "1985"

[inputs.seed]
kind = "value"
value = 1

[[nodes]]
id = "regression"
op = "glm.fit"
inputs = { landscape = "$start", drivers = "$drivers" }
params = { formulas = { 1 = ["elevation", "slope"], 2 = ["elevation", "slope", "dist_built_1985"], 3 = ["elevation", "slope"] } }

[[nodes]]
id = "suitability"
op = "glm.predict"
inputs = { model = "regression.model", drivers = "$drivers", landscape = "$start" }

[[nodes]]
id = "demand"
op = "demand.from_maps"
inputs = { maps = "$observed" }
params = { times = [1985, 1991, 1999] }

[[nodes]]
id = "rng"
op = "rng.r"
inputs = { seed = "$seed" }

[[nodes]]
id = "history"
op = "lulcc.history"
inputs = { landscape = "$start" }

[[nodes]]
id = "simulation"
op = "control.repeat"
inputs = { times = "demand.steps", landscape = "$start", history = "history.history" }

[nodes.body]
carry = { landscape = "allocate.landscape", history = "allocate.history" }
collect = { maps = "allocate.landscape", info = "allocate.info" }

[[nodes.body.nodes]]
id = "allocate"
op = "lulcc.step"
inputs = { landscape = "@landscape", history = "@history", suitability = "suitability.suitability", demand = "demand.demand", step = "@step", rng = "rng.rng" }
params = { allocation = { method = "clue", jitter = 0.0002, scale = 0.000001, max_iter = 1000, max_diff = 50, mean_diff = 50 }, elasticity = [0.2, 0.2, 0.2] }

[outputs.maps]
from = "simulation.maps"

[outputs.steps]
from = "simulation.info"
```

The loop **`simulation`** ([[control.repeat]]) has three inputs: `times`, connected to `demand.steps` (a number
that comes from a node, not typed), and two carried values, `landscape` (the start map) and `history` (node 5).
Its body:

- `carry = { landscape = "allocate.landscape", history = "allocate.history" }`: after each year, both carried
  values take what the allocation gives, so next year's `@landscape` and `@history` are this year's results;
- `collect = { maps = "allocate.landscape", info = "allocate.info" }`: the map and the table of every year are
  kept (`simulation.maps`, `simulation.info`).

**`allocate`** ([[lulcc.step]]) is the year. Its inputs, and why each comes from where it does:

| Input | Connected to | Why |
|---|---|---|
| `landscape` | `@landscape` | this year's map: last year's result |
| `history` | `@history` | its history, carried with it |
| `suitability` | `suitability.suitability` | fitted once, the same every year |
| `demand` | `demand.demand` | the whole table; the step picks its row |
| `step` | `@step` | which row of the demand (1 = 1986) |
| `rng` | `rng.rng` | the generator, drawn from in turn |

Its settings: `allocation` (`method = "clue"` and the five numbers of [part 1](/tutorials/land-change/clue-s-ideas/#settings-that-change-the-answer)),
`elasticity` (one per class, in the order of the demand's classes, or `{ 2 = 0.9 }` by class), and the rules of
part 5 (`allowed`, `neighbourhood`; the input `restrictions`).

!!! tip "Read the carried values"
    As in every loop, the body must read `@landscape` and `@history`; reading `$start` would allocate every year
    from 1985. The suitability, the demand and the generator, on the other hand, are read from outside on
    purpose: they do not belong to a year.

## Read the results

### Fourteen years, and the demand met

```tfslider
label: Year
caption: Plum Island simulated from 1985 to 1999 by CLUE-S (lulcc's vignette settings, seed 1). The numbers compare each class's cells with the demand of the year; the chart follows the cells of built land and forest and their demand. Forest green, built purple, other ochre.
frames:
  - { value: 1985, src: /img/tutorials/land-change/clue-s-model/year_1985.png, caption: "1985, the start", stats: { Forest: "49,013", Built: "37,122", Other: "27,428", "Cells changed": "0" } }
  - { value: 1986, src: /img/tutorials/land-change/clue-s-model/year_1986.png, stats: { Forest: "48,690 (asked 48,683)", Built: "37,610 (asked 37,660)", Other: "27,263 (asked 27,220)", "Cells changed": "13,023" } }
  - { value: 1987, src: /img/tutorials/land-change/clue-s-model/year_1987.png, stats: { Forest: "48,374 (asked 48,352)", Built: "38,151 (asked 38,198)", Other: "27,038 (asked 27,013)", "Cells changed": "597" } }
  - { value: 1988, src: /img/tutorials/land-change/clue-s-model/year_1988.png, stats: { Forest: "48,011 (asked 48,022)", Built: "38,697 (asked 38,736)", Other: "26,855 (asked 26,805)", "Cells changed": "581" } }
  - { value: 1989, src: /img/tutorials/land-change/clue-s-model/year_1989.png, stats: { Forest: "47,676 (asked 47,692)", Built: "39,240 (asked 39,274)", Other: "26,647 (asked 26,597)", "Cells changed": "596" } }
  - { value: 1990, src: /img/tutorials/land-change/clue-s-model/year_1990.png, stats: { Forest: "47,343 (asked 47,361)", Built: "39,784 (asked 39,812)", Other: "26,436 (asked 26,390)", "Cells changed": "586" } }
  - { value: 1991, src: /img/tutorials/land-change/clue-s-model/year_1991.png, stats: { Forest: "47,010 (asked 47,031)", Built: "40,321 (asked 40,350)", Other: "26,232 (asked 26,182)", "Cells changed": "581" } }
  - { value: 1992, src: /img/tutorials/land-change/clue-s-model/year_1992.png, stats: { Forest: "46,801 (asked 46,824)", Built: "40,714 (asked 40,738)", Other: "26,048 (asked 26,001)", "Cells changed": "459" } }
  - { value: 1993, src: /img/tutorials/land-change/clue-s-model/year_1993.png, stats: { Forest: "46,595 (asked 46,618)", Built: "41,100 (asked 41,126)", Other: "25,868 (asked 25,819)", "Cells changed": "444" } }
  - { value: 1994, src: /img/tutorials/land-change/clue-s-model/year_1994.png, stats: { Forest: "46,395 (asked 46,411)", Built: "41,482 (asked 41,514)", Other: "25,686 (asked 25,638)", "Cells changed": "440" } }
  - { value: 1995, src: /img/tutorials/land-change/clue-s-model/year_1995.png, stats: { Forest: "46,185 (asked 46,204)", Built: "41,872 (asked 41,902)", Other: "25,506 (asked 25,457)", "Cells changed": "445" } }
  - { value: 1996, src: /img/tutorials/land-change/clue-s-model/year_1996.png, stats: { Forest: "45,982 (asked 45,997)", Built: "42,260 (asked 42,291)", Other: "25,321 (asked 25,275)", "Cells changed": "447" } }
  - { value: 1997, src: /img/tutorials/land-change/clue-s-model/year_1997.png, stats: { Forest: "45,772 (asked 45,790)", Built: "42,649 (asked 42,679)", Other: "25,142 (asked 25,094)", "Cells changed": "438" } }
  - { value: 1998, src: /img/tutorials/land-change/clue-s-model/year_1998.png, stats: { Forest: "45,563 (asked 45,584)", Built: "43,039 (asked 43,067)", Other: "24,961 (asked 24,912)", "Cells changed": "444" } }
  - { value: 1999, src: /img/tutorials/land-change/clue-s-model/year_1999.png, stats: { Forest: "45,355 (asked 45,377)", Built: "43,427 (asked 43,455)", Other: "24,781 (asked 24,731)", "Cells changed": "441" } }
chart:
  type: line
  data: /data/tutorials/land-change/clue-s-model/years.csv
  x: year
  y: [Forest, Forest demand, Built, Built demand]
  colors: { Forest: "#3b8a5a", "Forest demand": "#9fd0b0", Built: "#b4508a", "Built demand": "#e3b3cf" }
  ylabel: Cells
```

Every year the allocation met the demand within its tolerance of 50 cells (`steps.csv`: `converged` = 1, from 52
iterations in 1986 to a few hundred later). The map, though, tells a second story: the **first year changed 13,023
cells** (11% of the landscape) for a net change of a few hundred, and the following years about 450 to 600 each.
In the first year every cell whose suitability favours another class by more than the elasticity of 0.2 switches:
the map is "redrawn" to agree with the regressions. After that the map is consistent with them and only the
demand's yearly increments move cells. This first-year reshuffle is the strongest sign that the elasticities are
too low (below).

### Against the observed map of 1999

```tfcompare
caption: Drag the handle between the observed map of 1999 (left) and the simulated one (right). Forest green, built purple, other ochre.
before: { src: /img/tutorials/land-change/clue-s-model/observed_1999.png, label: Observed 1999 }
after: { src: /img/tutorials/land-change/clue-s-model/simulated_1999.png, label: Simulated 1999 }
```

![Hits, misses, wrong hits and false alarms of the CLUE-S simulation](/img/tutorials/land-change/clue-s-model/hits_misses.png "The simulation of 1985-1999 against the observed change, cell by cell.")

The comparison of [Validating a simulation](/tutorials/land-change/validating-a-simulation/) ([[validate.agreement]]
and [[validate.three_map]], from 1985): 1,609 hits, 5,948 misses, 1,021 wrong hits and **16,892 false alarms**, a
figure of merit of **0.063**. The simulated map agrees with 79.0% of the observed one, while persistence (the map
of 1985 unchanged) agrees with 92.4%: the false alarms are the reshuffle of the first year. Measured at coarser
blocks, the figure of merit rises:

```tfchart
type: line
data: /data/tutorials/land-change/clue-s-model/fom.csv
x: block
y: [CLUE-S, Persistence]
colors: { CLUE-S: "#3b8a5a", Persistence: "#7f7f7f" }
xlabel: Block side (cells of about 100 m)
ylabel: Figure of merit
xlog: true
height: 280
caption: The figure of merit of the simulation of 1999 at blocks of 1 to 16 cells (validate.three_map). Persistence scores 0 at every resolution.
```

This is lulcc's published example, run exactly; it is a demonstration of the method, not a calibrated model. The
next section shows the first thing to change.

### Replicates

The jitter is tiny (0.0002 against suitabilities that differ by hundredths), so the seed matters little here: runs
with seeds 2 to 5 differ from seed 1 in 27 to 33 cells of 1999, and 48 cells are built in some of the five runs but
not all. CLUE-S's allocation is nearly deterministic; its uncertainty is in its inputs (the demand, the
regressions, the elasticities), which scenarios explore better than seeds.

### What to report

The demand (its source, its table); the regressions (formulas, coefficients, their fit, part 2); the
elasticities and any rules; the allocation's settings; whether every year converged (`steps.csv`); the validation
on an observed map at several resolutions, against persistence.

## Settings that change the answer

### The elasticities

```tfslider
label: Elasticity (forest, built, other)
caption: The change from 1985 to 1999 (cells that end in another class, by their new class) with other elasticities, each a real run of the model written by terraflux new clue --elasticity. Grey, unchanged.
frames:
  - { value: "0, 0, 0", src: /img/tutorials/land-change/clue-s-model/elasticity_0.0_0.0_0.0.png, stats: { "Cells changed by 1999": "33,430", "Figure of merit": "0.058", "Same class as 1999": "68.6%", "Every year converged": "yes" } }
  - { value: "0.2, 0.2, 0.2", src: /img/tutorials/land-change/clue-s-model/elasticity_0.2_0.2_0.2.png, stats: { "Cells changed by 1999": "19,522", "Figure of merit": "0.063", "Same class as 1999": "79.0%", "Every year converged": "yes" } }
  - { value: "0.4, 0.4, 0.4", src: /img/tutorials/land-change/clue-s-model/elasticity_0.4_0.4_0.4.png, stats: { "Cells changed by 1999": "8,267", "Figure of merit": "0.063", "Same class as 1999": "87.3%", "Every year converged": "yes" } }
  - { value: "0.6, 0.6, 0.6", src: /img/tutorials/land-change/clue-s-model/elasticity_0.6_0.6_0.6.png, stats: { "Cells changed by 1999": "6,446", "Figure of merit": "0.064", "Same class as 1999": "88.6%", "Every year converged": "no (1 of 14)" } }
  - { value: "0.8, 0.8, 0.8", src: /img/tutorials/land-change/clue-s-model/elasticity_0.8_0.8_0.8.png, stats: { "Cells changed by 1999": "6,351", "Figure of merit": "0.065", "Same class as 1999": "88.7%", "Every year converged": "no (7 of 14)" } }
  - { value: "0.2, 0.9, 0.2", src: /img/tutorials/land-change/clue-s-model/elasticity_0.2_0.9_0.2.png, stats: { "Cells changed by 1999": "14,856", "Figure of merit": "0.061", "Same class as 1999": "82.6%", "Every year converged": "yes" } }
start: 1
```

![Legend of the change](/img/tutorials/land-change/clue-s-model/elasticity_legend.png)

The observed change from 1985 to 1999 was 8,578 cells. With no elasticity, a third of the landscape (33,430 cells)
ends in another class: the model is free to redraw everything its regressions disagree with. Raising the
elasticity to 0.4 brings the change down to 8,267 cells, the observed amount, and the agreement with the map of 1999
from 79% to 87%. Higher elasticities change little more but make the allocation struggle: with 0.8, the iteration
variables must climb far above the elasticity before a cell will move, and with `scale` = 0.000001 the 1,000
iterations are not enough in seven years of fourteen (`converged` = 0; Terraflux warns). Raise `max_iter`, or the
`scale`, with high elasticities. Giving built land alone a high elasticity (0.9, last frame) keeps the towns from
turning into forest, a realistic rule, and cuts the reshuffle by a quarter.

The figure of merit barely moves (0.058 to 0.065): the elasticities decide *how much* of the landscape is
reshuffled, not whether the new built land lands on the right cells, which is the regressions' job. Choose them
from what is known of each class (in CLUE-S's logic, near 1 for land uses that are costly to convert, such as
built land or orchards, lower for land that changes easily, such as shifting farmland; Verburg et al., 2002) and check the gross change of the first year against the observed
yearly change. [Part 5](/tutorials/land-change/clue-s-rules/) adds conversion rules for finer control.

### The scale

```tfslider
label: Scale of the iteration variables
caption: The first year (1985 to 1986) with four values of scale, the other settings as in the vignette (real runs). Small scales converge; from 0.0001 the iteration variables overshoot and the whole landscape flips to one class.
frames:
  - { value: "0.000001", src: /img/tutorials/land-change/clue-s-model/scale_0.000001.png, stats: { Converged: "yes", Iterations: "52", "Largest difference": "50 cells", Forest: "48,690", Built: "37,610", Other: "27,263" } }
  - { value: "0.00001", src: /img/tutorials/land-change/clue-s-model/scale_0.00001.png, stats: { Converged: "yes", Iterations: "5", "Largest difference": "23 cells", Forest: "48,660", Built: "37,668", Other: "27,235" } }
  - { value: "0.0001", src: /img/tutorials/land-change/clue-s-model/scale_0.0001.png, stats: { Converged: "no", Iterations: "1001", "Largest difference": "64,880 cells", Forest: "113,563", Built: "0", Other: "0" } }
  - { value: "0.0005", src: /img/tutorials/land-change/clue-s-model/scale_0.0005.png, stats: { Converged: "no", Iterations: "1001", "Largest difference": "64,880 cells", Forest: "113,563", Built: "0", Other: "0" } }
```

`scale` × (the difference in cells) is how far an iteration variable moves in one pass. With 0.00001, a
difference of 5,000 cells moves it by 0.05, about the gaps between suitabilities: the allocation converges in 5
passes. With 0.0001 the same difference moves it by 0.5, more than the whole range of the suitabilities, and the
variables swing from one extreme to the other: in the last pass every cell of the landscape is forest, and the
map of that year is useless. lulcc's default (0.0005) is far too large here; on maps of 100,000 cells and more,
start from 0.000001 to 0.00001 and check the first year. Always read `converged` in `steps.csv`.

## Common pitfalls

- **Ignoring the first year.** A gross change far larger than the net change in the first year means the
  elasticities let the regressions redraw the map. Compare it with the observed yearly change.
- **A start map the demand does not begin from.** The first row of the demand must be the start map's counts:
  [[demand.from_maps]] guarantees it when the start is the first observed map.
- **Regressions fitted on another map than the start.** The `regression` and `suitability` nodes read `$start`;
  when projecting from a later map (`--start` of `terraflux new clue`), the regressions are fitted on it.
- **`converged` = 0.** The map of that year does not have the demanded areas; fix `scale`, `max_iter` or the
  tolerances.
- **Treating lulcc's vignette as calibrated.** Its settings show the method; calibrate yours (elasticities,
  formulas) against an observed map ([Calibration and many runs](/tutorials/land-change/calibration-and-many-runs/)).
- **Classes of the map missing from the demand**, or a stray code: the step stops ([part 1](/tutorials/land-change/clue-s-ideas/#common-pitfalls)).

## Going further

- [Part 5: Rules](/tutorials/land-change/clue-s-rules/): conversion matrices, neighbourhoods and restrictions.
- [Validating a simulation](/tutorials/land-change/validating-a-simulation/) and
  [Calibration and many runs](/tutorials/land-change/calibration-and-many-runs/).
- [A demand per region](/tutorials/land-change/regional-demand/): CLUE-S with a demand for each part of the map.
- [Dynamic drivers](/tutorials/land-change/dynamic-drivers/): the suitability recomputed in the loop.
- The operators: [[lulcc.step]], [[glm.fit]], [[glm.predict]], [[demand.from_maps]], [[rng.r]], [[lulcc.history]].

## References

- Moulds, S., Buytaert, W., & Mijic, A. (2015). An open and extensible framework for spatially explicit land use change modelling: the lulcc R package. *Geoscientific Model Development*, 8(10), 3215–3229. https://doi.org/10.5194/gmd-8-3215-2015
- Verburg, P. H., Soepboer, W., Veldkamp, A., Limpiada, R., Espaldon, V., & Mastura, S. S. A. (2002). Modeling the spatial dynamics of regional land use: the CLUE-S model. *Environmental Management*, 30(3), 391–405. https://doi.org/10.1007/s00267-002-2630-x
