---
title: "Taking a stock: logging and harvest with the Patcher of a continuous state"
short: Taking a stock
description: Simulate selective logging, harvest or fuelwood cutting as patches that take the timber, biomass or carbon of their cells until a yearly quota is met, with Dinamica EGO's Patcher of a continuous state, a loop that carries the stock, and regrowth by map algebra.
order: 130
level: intermediate
time: 40 min
ops: [dinamica.continuous_patcher, control.repeat, raster.calc, nlm.random_cluster, nlm.gaussian_field, nlm.distance_gradient]
series: Dinamica EGO in Terraflux
part: 4
---

Not every change of a landscape is a change of class. A selectively logged forest is still a forest on a land
cover map, but it has lost much of its commercial timber, part of its carbon, and some of its structure. The
same is true of fuelwood cutting, the harvest of a plantation or the extraction of a non-timber product: a
**stock** is taken from the cells, and the class stays. In this part of the series you will simulate such a
harvest with Dinamica EGO's **Patcher of a continuous state** ([[dinamica.continuous_patcher]]): patches that
take the stock of their cells, drawn where the harvest is likely, until a quota of stock is met. You will run it
once on a forest drawn by chance beside a road, then for thirty years in a loop that carries the stock and the
probability from year to year, with the timber growing back by map algebra. At the end you will know how to set
the quota, the units and the patches, how to read the table each harvest gives, and why a harvest that seems
sustainable can still empty a forest.

## The idea

### Change of a stock, not of a class

The [Expander and Patcher](/tutorials/land-change/expander-and-patcher/) of the previous parts change the
**class** of cells: forest becomes pasture, and the amount of change is a number of cells from a Markov chain.
Logging works differently. The demand is a **volume** (cubic metres of logs a year, tonnes of fuelwood, tonnes of
carbon), not an area; the cells differ in how much they hold; and the cells logged remain forest. Asner et al.
(2005), mapping selective logging over the five main timber states of the Brazilian Amazon with satellites,
found 12,075 to 19,823 km² logged a year between 1999 and 2002, an area of the same order as the deforestation
of those years, and almost invisible on land cover maps. Merry et al. (2009) estimated the commercial timber of
the Brazilian Amazon at about 4.5 billion m³ and asked how much of it an industry could harvest on a 30-year
cycle outside the protected areas. Questions like these need a model whose demand is a stock.

Dinamica EGO's answer is the functor *PatcherContinuousState*, the Patcher of a continuous state. Terraflux runs
landschaft's clean-room version of it, written from Dinamica EGO's documentation and black-box runs of its
version 8.13.

![The Patcher of a continuous state on a toy grid](/img/tutorials/land-change/taking-a-stock/toy.png "A real run on a toy grid of 24 x 24 cells. Left, the stock of each cell (2 to 10 units). Middle, the probability (dark low, light high); outlined, the candidates: the most probable cells, which together hold 3 times the quota (prune_factor = 3). Right, the four patches made: they take 151 units for a quota of 150, the last cell overshooting by 1.")

!!! science "Key idea: patches that take a stock up to a quota"
    Every cell \(i\) holds a stock \(s_i\) (timber, biomass, carbon) and has a probability \(P_i\) of being
    harvested. A harvest of quota \(Q\) works in four steps:

    1. **Candidates.** The cells with \(s_i > 0\) and \(P_i > 0\) are ranked from the highest probability; the
       first ones are kept while their summed stock is at most \(f \times Q\), where \(f\) is the
       `prune_factor` (10 by default).
    2. **Seeds.** A seed is drawn among the candidates in proportion to \(P_i\), without replacement.
    3. **Patches.** From the seed a patch grows to a size drawn from a lognormal distribution of the given mean
       and variance. Each new cell is drawn among the cells with stock around the patch, with a weight
       \(P_i \times \text{isometry}^{k}\), \(k\) being the number of the cell's neighbours already in the
       patch (the same growth as the Patcher's). Every cell of the patch gives **all** its stock.
    4. **The quota.** Patches are made until the stock taken reaches \(Q\). A patch stops as soon as the quota is
       met, so the last cell may take a little more than needed.

    \[ \text{remaining} = Q - \sum_{i \in \text{patches}} s_i \, a \]

    where \(a\) is the area of a cell in the unit of the stock. The remaining quota is negative when the last
    cell overshot it, and positive when the candidates ran out before the quota was met.

The probability decides *where* the harvest goes and the quota *how much*; the patches decide its *shape*. The
cells harvested get a stock of 0 and a probability of 0, so a second harvest on the same maps goes elsewhere.

## When to use it

Use the Patcher of a continuous state when a model must take a quantity from the landscape without changing its
classes:

- **Selective logging** of a timber volume per year, as in this tutorial, and its spread away from the roads.
- **Fuelwood and charcoal** cutting around villages, with a demand in tonnes from the population.
- **Harvest of plantations** (eucalyptus, pine) under a yearly demand of wood.
- **Carbon accounting**: the carbon of the forest as the stock, to estimate the emissions of a harvest scenario.

It is *not* the tool for clearing (a change of class): that is [[dinamica.step]] with a Markov chain. The two can
run in the same loop: clearing changes the classes, logging takes the stock of the forest that is left. It is
also not a growth model: how the stock comes back after a harvest is up to you, written in [[raster.calc]] (below).
And it does not model the damage of logging around the trees taken: every cell of a patch gives all its stock.
To model a partial harvest, make the stock the *harvestable* part of a cell (the commercial volume), not its
whole biomass.

Putz et al. (2012), in a meta-analysis of more than 100 studies of logged tropical forests, found that once-logged
forests keep about 76% of their carbon and most of their species, but that timber yields fall by about 46% after
the first harvest. A model of logging is therefore a model of how much a forest can give, and for how long: the
quota, the regrowth and the cycle are the questions.

## What you need

- **The stock**: a map of what each cell holds, per hectare by default (m³ of commercial timber per hectare,
  tonnes of biomass per hectare). In a real study, from a forest inventory, a biomass map or a model of
  commercial volume. Cells without stock: 0 or no value.
- **The probability** of harvest: a map from 0 to 1, high where loggers go first. It can come from a
  [weights of evidence](/tutorials/land-change/weights-of-evidence/) or a
  [logistic regression](/tutorials/land-change/logistic-suitability/) fitted on mapped logging, or be written
  as a rule (here, it falls with the distance to the road).
- **The quota** of each step, in the stock's unit times the area unit (m³ when the stock is m³ per hectare).
- **Patch settings**: the mean and variance of the patch sizes (hectares by default) and the isometry.
- A map with a **coordinate system in metres** (or any map unit) when the stock is per hectare: the area of a
  cell comes from its size.

The models on this page draw everything by chance and run as they are.

## Build the model

### Step 1: a forest, its timber and a road

```tfgraph
#! caption: The data. A forest drawn by chance (node 1) on cells of 100 m, the richness of its timber (node 2), the distance to a road (node 3), the timber in m³ per hectare (node 4) and the probability of logging (node 5).
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "land"
op = "nlm.random_cluster"
params = { rows = 200, columns = 200, resolution = 100, p = 0.55, weights = [0.8, 0.2], seed = 7 }
note = "1 forest, 2 cleared land; cells of 100 m (1 ha)."

[[nodes]]
id = "richness"
op = "nlm.gaussian_field"
params = { rows = 200, columns = 200, resolution = 100, range = 25, seed = 8 }

[[nodes]]
id = "road"
op = "nlm.distance_gradient"
params = { rows = 200, columns = 200, resolution = 100, origin = [60, 60, 1, 200], nlmr_compat = false, rescale = false }
note = "The distance to a road along column 60, in metres."

[[nodes]]
id = "timber"
op = "raster.calc"
inputs = { land = "land.map", r = "richness.map" }
params = { expression = "10 + 30 * r if land == 1 else 0" }
note = "Commercial timber, m3 per hectare."

[[nodes]]
id = "access"
op = "raster.calc"
inputs = { timber = "timber.map", d = "road.map" }
params = { expression = "exp(-d / 3000) if timber > 0 else 0" }
note = "The probability: high beside the road."
```

1. **`land`** ([[nlm.random_cluster]]) draws a landscape of 200 x 200 cells of 100 m (`resolution = 100`), 20
   km by 20 km: 1 forest (30,873 ha, 77% of the map) and 2 cleared land. Each cell is exactly one hectare, which
   keeps the arithmetic easy to follow.
2. **`richness`** ([[nlm.gaussian_field]]) is a smooth random surface from 0 to 1, patches about 25 cells
   across: where the valuable trees are.
3. **`road`** ([[nlm.distance_gradient]]) gives the distance in metres from every cell to a road running north
   to south along column 60 (`origin = [60, 60, 1, 200]`: columns 60 to 60, rows 1 to 200;
   `nlmr_compat = false` and `rescale = false` keep it in metres).
4. **`timber`** ([[raster.calc]]) is the stock: 10 to 40 m³ of commercial timber per hectare in the forest (24.8
   on average, 765,000 m³ in all), 0 elsewhere.
5. **`access`** is the probability: \(e^{-d/3000}\), 1 beside the road, 0.37 at 3 km, 0.05 at 9 km, 0 off the
   forest. It says that loggers go first where hauling is cheap. Any map from 0 to 1 works; one fitted on mapped
   logging is better.

```tfgallery
columns: 2
caption: "The stock and the probability. Left, the commercial timber of each forest cell (m³ per hectare); right, the probability of logging, highest along the road (the bright band) and falling away from it."
images:
  - { src: /img/tutorials/land-change/taking-a-stock/timber.png, caption: "timber.map" }
  - { src: /img/tutorials/land-change/taking-a-stock/access.png, caption: "access.map" }
```

### Step 2: one harvest

```tfgraph
#! caption: One year of logging: 15,000 m³ taken in patches of about 25 ha (node 6).
#! numbered: true
#! highlight: logging
#! params: false
format = "terraflux/1"

[[nodes]]
id = "land"
op = "nlm.random_cluster"
params = { rows = 200, columns = 200, resolution = 100, p = 0.55, weights = [0.8, 0.2], seed = 7 }

[[nodes]]
id = "richness"
op = "nlm.gaussian_field"
params = { rows = 200, columns = 200, resolution = 100, range = 25, seed = 8 }

[[nodes]]
id = "road"
op = "nlm.distance_gradient"
params = { rows = 200, columns = 200, resolution = 100, origin = [60, 60, 1, 200], nlmr_compat = false, rescale = false }

[[nodes]]
id = "timber"
op = "raster.calc"
inputs = { land = "land.map", r = "richness.map" }
params = { expression = "10 + 30 * r if land == 1 else 0" }

[[nodes]]
id = "access"
op = "raster.calc"
inputs = { timber = "timber.map", d = "road.map" }
params = { expression = "exp(-d / 3000) if timber > 0 else 0" }

[[nodes]]
id = "logging"
op = "dinamica.continuous_patcher"
inputs = { stock = "timber.map", probability = "access.map", quota = 15000, seed = 1 }
params = { mean = 25, variance = 200, isometry = 1.5 }
```

**`logging`** ([[dinamica.continuous_patcher]], subject *Dinamica EGO* in the library) is the harvest. Connect
`stock` to `timber.map` and `probability` to `access.map`, and type the `quota` (15000) and the `seed` (1) in the
Node pane. Its settings:

- `mean` and `variance`: the patch sizes, 25 ha on average with a variance of 200 (a standard deviation of about
  14 ha): most patches of 10 to 40 ha, a few larger. Think of a patch as the area logged from one log landing.
- `isometry`: 1.5, slightly compact patches. For this functor it runs from 0 (patches of single cells) through 1
  (growth by the probabilities alone) to 2 (compact).
- `unit` (not set: `hectares`): the stock is per hectare and the sizes are in hectares, as in Dinamica EGO.
  `cells` makes both per cell, `area` per square map unit.
- `prune_factor` (10) and `window` (`[3, 3]`) as in [[dinamica.step]].

Run it with ++ctrl+enter++ on the node. It has five outputs: `stock`, the timber left (0 where taken);
`probability`, the probability with the logged cells at 0; `taken`, the stock taken from each cell (in m³: the
stock times the cell's area); `patch`, the number of each cell's patch (1, 2...; 0 elsewhere); and `info`, a table
of one row:

| quota | taken | remaining | patches | cells |
|---:|---:|---:|---:|---:|
| 15,000 | 15,015.2 | -15.2 | 20 | 583 |

Twenty patches of 29 ha on average took 15,015 m³ from 583 ha; the last cell overshot the quota by 15 m³.

![The first harvest](/img/tutorials/land-change/taking-a-stock/step1.png "The 583 ha logged (orange) in the first year, all within 2.4 km of the road (the black line). Green, forest; pale, cleared land.")

![Legend](/img/tutorials/land-change/taking-a-stock/legend.png)

### Step 3: thirty years

A harvest is a yearly decision on a forest that the previous years have changed. The loop below
([[control.repeat]]) runs the harvest 30 times and **carries** two maps from year to year: the stock and the
probability. Between harvests the timber grows back.

```tfgraph
#! caption: Thirty years of logging. The loop carries the stock (after regrowth) and the probability (with the logged cells at 0); it collects the stock taken, the table and the stock of every year. The quota and the seed are inputs of the model.
#! highlight: logging, regrowth
format = "terraflux/1"

[inputs.quota]
kind = "value"
value = 15000

[inputs.seed]
kind = "value"
value = 1

[[nodes]]
id = "land"
op = "nlm.random_cluster"
params = { rows = 200, columns = 200, resolution = 100, p = 0.55, weights = [0.8, 0.2], seed = 7 }
note = "1 forest, 2 cleared land; cells of 100 m (1 ha)."

[[nodes]]
id = "richness"
op = "nlm.gaussian_field"
params = { rows = 200, columns = 200, resolution = 100, range = 25, seed = 8 }

[[nodes]]
id = "road"
op = "nlm.distance_gradient"
params = { rows = 200, columns = 200, resolution = 100, origin = [60, 60, 1, 200], nlmr_compat = false, rescale = false }
note = "The distance to a road along column 60, in metres."

[[nodes]]
id = "timber"
op = "raster.calc"
inputs = { land = "land.map", r = "richness.map" }
params = { expression = "10 + 30 * r if land == 1 else 0" }
note = "Commercial timber, m3 per hectare."

[[nodes]]
id = "access"
op = "raster.calc"
inputs = { timber = "timber.map", d = "road.map" }
params = { expression = "exp(-d / 3000) if timber > 0 else 0" }
note = "The probability: high beside the road."

[[nodes]]
id = "years"
op = "control.repeat"
inputs = { times = 30, stock = "timber.map", probability = "access.map" }

[nodes.body]
carry = { stock = "regrowth.map", probability = "logging.probability" }
collect = { taken = "logging.taken", info = "logging.info", stocks = "regrowth.map" }

[[nodes.body.nodes]]
id = "logging"
op = "dinamica.continuous_patcher"
inputs = { stock = "@stock", probability = "@probability", quota = "$quota", step = "@step", seed = "$seed" }
params = { mean = 25, variance = 200, isometry = 1.5 }

[[nodes.body.nodes]]
id = "regrowth"
op = "raster.calc"
inputs = { s = "logging.stock", full = "timber.map" }
params = { expression = "min(full, s + full / 30)" }
note = "The timber grows back in 30 years."
```

What changed from step 2:

1. **Two inputs of the model**, `quota` (15000) and `seed` (1), declared at the top of the file (`[inputs.quota]`
   with `kind = "value"`). Nodes read them as `$quota` and `$seed`. A scenario is then one option on the command
   line: `terraflux run logging.toml --set quota=30000` runs the same model with twice the harvest.
2. **The loop** `years` has `times = 30` and two carried values, `stock` (starting as `timber.map`) and
   `probability` (starting as `access.map`). Inside the body they are `@stock` and `@probability`.
3. **`logging`** now reads `@stock` and `@probability`, the maps as the previous year left them. Its `step`
   input is `@step`: the patcher mixes it into the seed (the seed plus the step times a large constant), so every
   year draws different patches while the whole run stays reproducible from one seed. Without `step`, every
   year would draw from the same random numbers.
4. **`regrowth`** ([[raster.calc]]) grows the timber back: every year a cell regains a thirtieth of its original
   stock (`full`, read from `timber.map` outside the loop), up to that original. A cell logged in year 1 is full
   again after 30 years; an unlogged cell stays at `full`.
5. **`carry`** passes the regrown stock and the depleted probability to the next year; **`collect`** keeps, for
   every year, the stock taken (`years.taken`, a series of 30 maps), the table (`years.info`, a table of 30
   rows) and the stock after regrowth (`years.stocks`).

!!! warning "Carry the probability, not only the stock"
    If the loop carried the stock but read `access.map` every year, the logged cells would keep their high
    probability. With a stock of 0 they are not candidates, but as soon as a thirtieth of their timber has grown
    back they are again the most probable cells of the map, and the loggers return the next year for 1 m³ per
    hectare. Carrying `logging.probability`, which is 0 where a patch took the stock, is Dinamica EGO's way of
    saying "this cell has been logged": it stays out of the harvest for the rest of the run. Below you will see
    how to let it back in after a cutting cycle.

!!! app "In the application"
    Add a **control.repeat** node, type 30 in `times`, then add two inputs to it in the Node pane, `stock` and
    `probability`, and connect `timber.map` and `access.map` to them. Double-click the loop to open its body:
    `@step`, `@stock` and `@probability` are in its left column, with the model's inputs (`$quota`, `$seed`).
    Add the two nodes, then set the loop's **carry** and **collect** in its Node pane. Run with ++f5++; the run
    takes a fraction of a second.

From the command line, add outputs to write the series:

```toml
[outputs.info]
from = "years.info"        # info.csv: a row per year

[outputs.taken]
from = "years.taken"       # taken.tif: a band per year
```

```bash
terraflux run logging.toml --out logging
terraflux run logging.toml --out logging_30k --set quota=30000
```

## Read the results

### Year by year

```tfslider
label: Year
caption: The logging of each year (orange) and of the years before (tan) beside the road (black). The chart follows the mean distance from the road of the cells logged each year.
frames:
  - { value: 1, src: /img/tutorials/land-change/taking-a-stock/year_1.png, caption: "Year 1", stats: { "Taken": "15,015 m³", "Patches": "20", "Mean distance from the road": "0.8 km", "Logged so far": "583 ha" } }
  - { value: 2, src: /img/tutorials/land-change/taking-a-stock/year_2.png, caption: "Year 2", stats: { "Taken": "15,002 m³", "Patches": "23", "Mean distance from the road": "0.9 km", "Logged so far": "1,189 ha" } }
  - { value: 4, src: /img/tutorials/land-change/taking-a-stock/year_4.png, caption: "Year 4", stats: { "Taken": "15,004 m³", "Patches": "24", "Mean distance from the road": "1.2 km", "Logged so far": "2,418 ha" } }
  - { value: 6, src: /img/tutorials/land-change/taking-a-stock/year_6.png, caption: "Year 6", stats: { "Taken": "15,011 m³", "Patches": "23", "Mean distance from the road": "1.4 km", "Logged so far": "3,654 ha" } }
  - { value: 8, src: /img/tutorials/land-change/taking-a-stock/year_8.png, caption: "Year 8", stats: { "Taken": "15,022 m³", "Patches": "28", "Mean distance from the road": "1.8 km", "Logged so far": "4,888 ha" } }
  - { value: 10, src: /img/tutorials/land-change/taking-a-stock/year_10.png, caption: "Year 10", stats: { "Taken": "15,028 m³", "Patches": "30", "Mean distance from the road": "2.2 km", "Logged so far": "6,112 ha" } }
  - { value: 12, src: /img/tutorials/land-change/taking-a-stock/year_12.png, caption: "Year 12", stats: { "Taken": "15,005 m³", "Patches": "27", "Mean distance from the road": "2.3 km", "Logged so far": "7,336 ha" } }
  - { value: 14, src: /img/tutorials/land-change/taking-a-stock/year_14.png, caption: "Year 14", stats: { "Taken": "15,006 m³", "Patches": "27", "Mean distance from the road": "2.9 km", "Logged so far": "8,569 ha" } }
  - { value: 16, src: /img/tutorials/land-change/taking-a-stock/year_16.png, caption: "Year 16", stats: { "Taken": "15,026 m³", "Patches": "36", "Mean distance from the road": "3.0 km", "Logged so far": "9,755 ha" } }
  - { value: 18, src: /img/tutorials/land-change/taking-a-stock/year_18.png, caption: "Year 18", stats: { "Taken": "15,019 m³", "Patches": "27", "Mean distance from the road": "3.6 km", "Logged so far": "11,024 ha" } }
  - { value: 20, src: /img/tutorials/land-change/taking-a-stock/year_20.png, caption: "Year 20", stats: { "Taken": "15,001 m³", "Patches": "27", "Mean distance from the road": "3.9 km", "Logged so far": "12,270 ha" } }
  - { value: 22, src: /img/tutorials/land-change/taking-a-stock/year_22.png, caption: "Year 22", stats: { "Taken": "15,020 m³", "Patches": "36", "Mean distance from the road": "4.2 km", "Logged so far": "13,489 ha" } }
  - { value: 24, src: /img/tutorials/land-change/taking-a-stock/year_24.png, caption: "Year 24", stats: { "Taken": "15,025 m³", "Patches": "31", "Mean distance from the road": "4.3 km", "Logged so far": "14,746 ha" } }
  - { value: 26, src: /img/tutorials/land-change/taking-a-stock/year_26.png, caption: "Year 26", stats: { "Taken": "15,003 m³", "Patches": "40", "Mean distance from the road": "4.9 km", "Logged so far": "15,955 ha" } }
  - { value: 28, src: /img/tutorials/land-change/taking-a-stock/year_28.png, caption: "Year 28", stats: { "Taken": "15,014 m³", "Patches": "44", "Mean distance from the road": "5.7 km", "Logged so far": "17,209 ha" } }
  - { value: 30, src: /img/tutorials/land-change/taking-a-stock/year_30.png, caption: "Year 30", stats: { "Taken": "15,005 m³", "Patches": "45", "Mean distance from the road": "7.1 km", "Logged so far": "18,478 ha" } }
chart:
  type: line
  data: /data/tutorials/land-change/taking-a-stock/years.csv
  x: step
  y: [distance]
  labels: { distance: "Mean distance of the year's logging from the road" }
  colors: { distance: "#e8901f" }
  xlabel: Year
  ylabel: km
```

Three things happen together. The **logging front moves away from the road**: the cells nearest the road are
logged first, their probability falls to 0, and every year the candidates are the most accessible cells left, a
little farther out (0.8 km on average in year 1, 7.1 km in year 30). The **quota is met every year** (15,000 m³,
overshot by a few cubic metres), so the area logged per year stays about the same, near 600 ha. And the
**patches get more numerous** (20 in year 1, 45 in year 30): the patches grow only into cells with stock, and as
the logged area spreads, they meet logged cells and cleared land sooner and stop smaller, so more are needed to
fill the quota.

After 30 years, 18,478 ha (60% of the forest) have been logged once. The cells nearest the road, logged first,
have grown back fully by year 30, but none has been logged again: their probability is still 0.

### The table of each year

`years.info` has a row per year with the five columns of the step-2 table. Read it as a check of the scenario:

- `taken` close to `quota` and `remaining` negative and small: the quota was met.
- `remaining` **positive**: the candidates ran out before the quota was met. The forest that the model allows
  the loggers to reach could not supply the demand that year. This is the result to look for in a scenario of
  high demand (below).
- `patches` and `cells`: how the harvest was spread. Their ratio is the mean patch actually made, often smaller
  than `mean`, because patches stop at the quota and at the edge of the stock.

```tfchart
type: area
data: /data/tutorials/land-change/taking-a-stock/years.csv
x: step
y: [standing]
labels: { standing: "Standing commercial timber" }
colors: { standing: "#3b8a5a" }
xlabel: Year
ylabel: Thousand m³
height: 280
caption: The standing timber of the whole forest after each year's harvest and regrowth, from 765 thousand m³ at the start to 547 thousand after 30 years. The forest loses 15 thousand m³ a year to the harvest and regains less, because only the logged cells grow, a thirtieth of their stock a year.
```

### What to report

The stock and where it comes from (inventory, biomass map, its unit), the probability (how it was made or
fitted), the quota per step and its source (a demand, a policy, a mill's capacity), the patch settings with
their unit, the isometry, the `prune_factor`, the regrowth rule and whether logged cells may be logged again.
Report the stock taken and the remaining quota per step, the area logged, and how far from the roads the harvest
went; with several seeds, their range.

## Settings that change the answer

The sliders below are real runs of the models above, each with one setting changed.

### The quota

```tfslider
label: Quota (m³ a year)
caption: The forest logged at least once in 30 years (tan) with a quota of 5,000 to 45,000 m³ a year, everything else as in the model.
frames:
  - { value: 5000, src: /img/tutorials/land-change/taking-a-stock/quota_5000.png, stats: { "Years the quota was met": "30 of 30", "Timber taken": "150 thousand m³", "Forest logged": "20%" } }
  - { value: 15000, src: /img/tutorials/land-change/taking-a-stock/quota_15000.png, stats: { "Years the quota was met": "30 of 30", "Timber taken": "450 thousand m³", "Forest logged": "60%" } }
  - { value: 30000, src: /img/tutorials/land-change/taking-a-stock/quota_30000.png, stats: { "Years the quota was met": "25 of 30", "Timber taken": "765 thousand m³", "Forest logged": "100%" } }
  - { value: 45000, src: /img/tutorials/land-change/taking-a-stock/quota_45000.png, stats: { "Years the quota was met": "16 of 30", "Timber taken": "765 thousand m³", "Forest logged": "100%" } }
start: 1
```

```tfchart
type: line
data: /data/tutorials/land-change/taking-a-stock/quotas.csv
x: step
y: [q5000, q15000, q30000, q45000]
labels: { q5000: "Quota 5,000", q15000: "Quota 15,000", q30000: "Quota 30,000", q45000: "Quota 45,000" }
colors: { q5000: "#1f5f3a", q15000: "#3b8a5a", q30000: "#d9a441", q45000: "#b4508a" }
xlabel: Year
ylabel: Thousand m³ taken
height: 300
caption: The timber taken each year with each quota. With 30,000 and 45,000 m³ the forest runs out of timber the model allows to be logged (in year 26 and year 17), and the harvest falls to nothing.
```

A quota of 30,000 m³ a year is met for 25 years and then collapses: every forest cell has been logged once, and
since the logged cells never come back into the harvest, the regrown timber is out of reach. In the table this
shows as a `remaining` that turns positive. The ecological reading: the higher the demand, the faster the
logging front crosses the forest and the sooner the remote, intact forest is gone.

### A cutting cycle

Real logging returns to the same stands after a **cutting cycle** (Merry et al., 2009, worked with a cycle of 30
years). One more node in the body lets a cell back into the harvest when its timber has grown back:

```tfgraph
#! caption: The same loop, with the logged cells let back in when their stock is back to the original (node reopen). The loop now carries reopen.map as the probability.
#! highlight: reopen
#! params: false
format = "terraflux/1"

[inputs.quota]
kind = "value"
value = 25000

[inputs.seed]
kind = "value"
value = 1

[[nodes]]
id = "land"
op = "nlm.random_cluster"
params = { rows = 200, columns = 200, resolution = 100, p = 0.55, weights = [0.8, 0.2], seed = 7 }

[[nodes]]
id = "richness"
op = "nlm.gaussian_field"
params = { rows = 200, columns = 200, resolution = 100, range = 25, seed = 8 }

[[nodes]]
id = "road"
op = "nlm.distance_gradient"
params = { rows = 200, columns = 200, resolution = 100, origin = [60, 60, 1, 200], nlmr_compat = false, rescale = false }

[[nodes]]
id = "timber"
op = "raster.calc"
inputs = { land = "land.map", r = "richness.map" }
params = { expression = "10 + 30 * r if land == 1 else 0" }

[[nodes]]
id = "access"
op = "raster.calc"
inputs = { timber = "timber.map", d = "road.map" }
params = { expression = "exp(-d / 3000) if timber > 0 else 0" }

[[nodes]]
id = "years"
op = "control.repeat"
inputs = { times = 80, stock = "timber.map", probability = "access.map" }

[nodes.body]
carry = { stock = "regrowth.map", probability = "reopen.map" }
collect = { taken = "logging.taken", info = "logging.info" }

[[nodes.body.nodes]]
id = "logging"
op = "dinamica.continuous_patcher"
inputs = { stock = "@stock", probability = "@probability", quota = "$quota", step = "@step", seed = "$seed" }
params = { mean = 25, variance = 200, isometry = 1.5 }

[[nodes.body.nodes]]
id = "regrowth"
op = "raster.calc"
inputs = { s = "logging.stock", full = "timber.map" }
params = { expression = "min(full, s + full / 30)" }

[[nodes.body.nodes]]
id = "reopen"
op = "raster.calc"
inputs = { s = "regrowth.map", full = "timber.map", p = "logging.probability", p0 = "access.map" }
params = { expression = "p0 if full > 0 and s >= full else p" }
note = "A cell may be logged again once its timber is back."
```

`reopen` gives a cell its original probability (`access.map`) once its stock is back to the original, and keeps
the depleted probability otherwise. The chart compares the two versions over 80 years with a quota of 25,000 m³,
just under what the forest regrows in a year (765,000 m³ over 30 years, about 25,500 m³ a year):

```tfchart
type: line
data: /data/tutorials/land-change/taking-a-stock/cycle.csv
x: step
y: [depleted, cycle]
labels: { depleted: "Logged cells never reopened", cycle: "Reopened after 30 years" }
colors: { depleted: "#b4508a", cycle: "#3b8a5a" }
xlabel: Year
ylabel: Thousand m³ taken
height: 280
caption: "A quota of 25,000 m³ a year for 80 years. Without reopening, the forest is logged through in 31 years and the harvest stops; with a 30-year cycle the quota is met every year."
```

The two are the same for 30 years. Then one stops, and the other goes on: the harvest is sustainable only when
the quota is below the yearly regrowth *and* the logged stands come back into the harvest. The regrowth here is
deliberately simple (linear, back to the original stock); Putz et al. (2012) found that second harvests yield
about half of the first, which you would write as a regrowth to a lower ceiling, for instance
`s if s == full else min(0.55 * full, s + full / 30)`.

### The size of the patches

```tfslider
label: Mean patch size (ha)
caption: The first year's harvest (orange) with patches of 5, 25 and 100 ha on average (variance one third of the mean squared). The same timber, in many small patches or a few large ones.
frames:
  - { value: 5, src: /img/tutorials/land-change/taking-a-stock/mean_5.png, stats: { "Patches": "123", "Cells taken": "598 ha", "Mean patch": "4.9 ha" } }
  - { value: 25, src: /img/tutorials/land-change/taking-a-stock/mean_25.png, stats: { "Patches": "23", "Cells taken": "619 ha", "Mean patch": "26.9 ha" } }
  - { value: 100, src: /img/tutorials/land-change/taking-a-stock/mean_100.png, stats: { "Patches": "9", "Cells taken": "624 ha", "Mean patch": "69.3 ha" } }
start: 1
```

With a mean of 100 ha the patches made average only 69 ha: large patches run into cleared land, and the last one
stops at the quota. Small patches spread the harvest along the whole road; large ones concentrate it in a few
blocks. For the forest's fauna the difference matters: many small gaps leave more of the forest near a logged
patch, a few large ones leave more of it untouched.

### The isometry

```tfslider
label: Isometry
caption: The first year's harvest with isometries from 0 to 2. At 0 every patch is a single cell, scattered along the road; from 0.5 up the patches are lacy at low isometry and compact at 2.
frames:
  - { value: 0, src: /img/tutorials/land-change/taking-a-stock/iso_0.png, stats: { "Patches": "609", "Clumps of logged cells": "409", "Cells taken": "609 ha" } }
  - { value: 0.5, src: /img/tutorials/land-change/taking-a-stock/iso_0.5.png, stats: { "Patches": "20", "Clumps of logged cells": "13", "Cells taken": "583 ha" } }
  - { value: 1, src: /img/tutorials/land-change/taking-a-stock/iso_1.png, stats: { "Patches": "20", "Clumps of logged cells": "15", "Cells taken": "583 ha" } }
  - { value: 1.5, src: /img/tutorials/land-change/taking-a-stock/iso_1.5.png, stats: { "Patches": "20", "Clumps of logged cells": "15", "Cells taken": "583 ha" } }
  - { value: 2, src: /img/tutorials/land-change/taking-a-stock/iso_2.png, stats: { "Patches": "20", "Clumps of logged cells": "17", "Cells taken": "584 ha" } }
start: 3
```

An isometry of 0 is a special case: no patch grows beyond its seed, and the harvest becomes a scatter of single
hectares, the pattern of a selective cut tree by tree at this resolution. Between 0.5 and 2 the number of patches
barely changes; their shape does.

### `prune_factor`: how far from the best cells

```tfslider
label: prune_factor
caption: The first year's harvest with prune factors of 1 to 100. The candidates are the most probable cells holding prune_factor times the quota; the more of them, the farther from the road the seeds can fall.
frames:
  - { value: 1, src: /img/tutorials/land-change/taking-a-stock/prune_1.png, stats: { "Mean distance from the road": "0.2 km", "Farthest logged cell": "0.7 km", "Patches": "21" } }
  - { value: 3, src: /img/tutorials/land-change/taking-a-stock/prune_3.png, stats: { "Mean distance from the road": "0.3 km", "Farthest logged cell": "1.1 km", "Patches": "20" } }
  - { value: 10, src: /img/tutorials/land-change/taking-a-stock/prune_10.png, stats: { "Mean distance from the road": "0.8 km", "Farthest logged cell": "2.4 km", "Patches": "20" } }
  - { value: 30, src: /img/tutorials/land-change/taking-a-stock/prune_30.png, stats: { "Mean distance from the road": "1.8 km", "Farthest logged cell": "6.2 km", "Patches": "21" } }
  - { value: 100, src: /img/tutorials/land-change/taking-a-stock/prune_100.png, stats: { "Mean distance from the road": "2.5 km", "Farthest logged cell": "12.4 km", "Patches": "21" } }
start: 2
```

`prune_factor` is the setting most easily overlooked and one of the most influential. With 1, the candidates
hold just the quota: the harvest is almost deterministic, a strip along the road. With 100, the candidates are
most of the forest and the seeds are drawn in proportion to the probability across all of it: some patches land
12 km from the road. It sets how strictly the loggers follow the probability. Dinamica EGO's default, 10, is a
reasonable start; calibrate it against mapped logging if you have it (the distribution of logged patches by
distance to the road is a good target).

## Common pitfalls

- **Units of the stock and of the quota.** With `unit = "hectares"` (the default) the stock is per hectare, the
  quota in stock units times hectares, and the patch sizes in hectares. On cells of 30 m (0.09 ha) a cell with 25
  m³ per hectare holds 2.25 m³, and a patch of 25 ha is about 278 cells. Give the stock per cell? Then set
  `unit = "cells"`, and the sizes are in cells too.
- **A map without a coordinate system.** The area of a cell comes from the map's cell size. A map in degrees
  gives areas in square degrees, meaningless in hectares: project the maps first (see
  [Getting started](/tutorials/#getting-started)), or use `unit = "cells"`.
- **Missing values.** A cell with no stock or no probability (no value, 0 or below) is never a candidate. The
  output stock keeps the input's missing cells; [[raster.calc]] expressions that use it give no value there too.
- **The probability not carried.** See the warning in step 3: read `@probability`, and carry
  `logging.probability` (or your own rule), never the original map, unless you mean the loggers to come back the
  next year.
- **No `step` inside a loop.** Without the `step` input, every year uses the same random numbers. Connect
  `@step`.
- **An isometry above 2.** For this functor the isometry runs from 0 to 2, and a larger value stops the run with
  an error.
- **Reading one run.** The patches are drawn at random. Run several seeds (`--set seed=2`, or
  `terraflux submit --replicates`) and report the range.

??? note "How close is this to Dinamica EGO?"
    landschaft wrote the functor from Dinamica EGO's documentation and black-box runs of its version 8.13. In
    those runs Dinamica's functor stopped with an error for patches of more than one cell, so the candidates, the
    pruning, the draw of the seeds and the quota were measured with patches of one cell, and the growth of larger
    patches is the published Patcher's. Dinamica's random numbers also depend on more than its seed, so the two
    agree statistically, not cell by cell.

## Going further

- [Roads that grow](/tutorials/land-change/roads-that-grow/), the next part: the roads that loggers and settlers
  build, and the distance to them as a driver that changes every year.
- [Gravity potentials as drivers](/tutorials/land-change/gravity-potentials/): the pull of towns and markets, a
  probability of harvest that depends on how many people are near.
- [Expander and Patcher](/tutorials/land-change/expander-and-patcher/): the same patches for changes of class.
- [Dinamica EGO on Sibuyan](/tutorials/land-change/dinamica-on-sibuyan/): a loop of clearing in which the
  harvest could run beside the allocation.
- The operators: [[dinamica.continuous_patcher]], [[control.repeat]], [[raster.calc]].

## References

- Asner, G. P., Knapp, D. E., Broadbent, E. N., Oliveira, P. J. C., Keller, M., & Silva, J. N. (2005). Selective logging in the Brazilian Amazon. *Science*, 310(5747), 480–482. https://doi.org/10.1126/science.1118051
- Merry, F., Soares-Filho, B., Nepstad, D., Amacher, G., & Rodrigues, H. (2009). Balancing conservation and economic sustainability: the future of the Amazon timber industry. *Environmental Management*, 44(3), 395–407. https://doi.org/10.1007/s00267-009-9337-1
- Putz, F. E., Zuidema, P. A., Synnott, T., Peña-Claros, M., Pinard, M. A., Sheil, D., et al. (2012). Sustaining conservation values in selectively logged tropical forests: the attained and the attainable. *Conservation Letters*, 5(4), 296–303. https://doi.org/10.1111/j.1755-263X.2012.00242.x
- Soares-Filho, B. S., Coutinho Cerqueira, G., & Lopes Pennachin, C. (2002). DINAMICA: a stochastic cellular automata model designed to simulate the landscape dynamics in an Amazonian colonization frontier. *Ecological Modelling*, 154(3), 217–235. https://doi.org/10.1016/S0304-3800(02)00059-5
