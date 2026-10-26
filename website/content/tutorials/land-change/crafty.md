---
title: "CRAFTY: competition for land between agent functional types"
short: CRAFTY
description: Farmers and foresters produce food and timber from the capitals of a landscape and compete for its cells to meet a demand that grows; learn CRAFTY's production, utility, competitiveness, giving up and giving in.
order: 720
level: intermediate
time: 45 min
ops: [crafty.allocate, table.of, stack.of, control.repeat, nlm.gaussian_field, nlm.random_cluster]
series: Dynamic drivers and agents
part: 3
---

The previous part gave every parcel a farmer of its own. Large regions have millions of land managers, too many
to describe one by one, and too varied to describe as one. **CRAFTY** groups them into *agent functional
types*, as ecologists group plants into plant functional types: intensive farmers, extensive farmers,
foresters, conservationists. Each type produces **services** (food, timber, recreation, carbon) from the
**capitals** of the land it manages (soil, forest, infrastructure, people), and the types compete for land
wherever the demand for their services is not met. In this part you will build the template *CRAFTY: farmers
and foresters* node by node, run it for 20 steps while the demand for food grows, follow the supply of food
and timber against their demand, and see how the settings of the competition (how much land is on offer each
step, how readily a manager gives way) shape the landscape. At the end you will know what each number of a
CRAFTY model means and how to read its results.

## The idea

### Functional types, capitals and services

CRAFTY (Competition for Resources between Agent Functional TYpes) was introduced by Murray-Rust et al. (2014)
to model land use over large regions with agents, without describing every land manager. It was later applied
to the whole of Great Britain and of Europe (Brown et al., 2018, 2019). Its vocabulary:

- **Capitals**: what a cell offers, each scaled from 0 to 1. In CRAFTY-EU they follow the five capitals of
  the sustainable livelihoods framework: natural (soil fertility, climate suitability for crops, growing
  stock), human, social, manufactured (roads, irrigation) and financial. Here two natural capitals: the soil's
  `fertility` and the forest's growing stock, `wood`.
- **Services**: what society demands from the land, in its own units: food, timber, meat, recreation,
  biodiversity. Here `food` and `timber`.
- **Agent functional types (AFTs)**: kinds of land managers, each with how much of each service it can produce
  and how much it depends on each capital. Here `farmer` and `forester`.
- **Demand**: how much of each service is wanted at each step, from scenarios of population, diets and
  trade.

### Production, utility and competitiveness

!!! science "Key idea: CRAFTY's equations, as Terraflux computes them"
    The production of service \(s\) by AFT \(a\) on cell \(i\) is

    \[ P_{a,s}(i) = p_{a,s} \prod_c k_c(i)^{e_{a,c}} \]

    where \(p_{a,s}\) is the most the AFT produces of the service on a cell of full capitals (`production`),
    \(k_c(i)\) the value of capital \(c\) in the cell (0 to 1) and \(e_{a,c}\) the AFT's **sensitivity** to it
    (0: does not depend on it; 1: production proportional to it; 2: falls steeply as the capital falls).

    The **utility** of a service is the share of its demand still unmet:

    \[ u_s = \frac{\max(D_s - S_s,\ 0)}{D_s} \]

    with \(D_s\) the demand and \(S_s\) the current supply (the sum of the production of all managed cells).

    The **competitiveness** of AFT \(a\) on cell \(i\) is the value of what it would produce there:

    \[ C_a(i) = \sum_s u_s\, P_{a,s}(i) \]

The utility makes the competition follow the demand: a service whose demand is met is worth nothing more
(\(u_s = 0\)), so the AFTs that produce only it lose all competitiveness; a service far from its demand is worth
much, so the AFTs that produce it win cells. The supply and the utilities are updated after every change of
manager, so the land settles where the supply meets the demand, if the landscape can produce it.

!!! note "Simpler than CRAFTY's own benefit functions"
    In CRAFTY the value of unmet demand is given by a *benefit function* chosen per service (linear, with
    thresholds, with a different slope for oversupply), and the published models also include institutions
    and the variation of thresholds among agents of one type (Murray-Rust et al., 2014; Brown et al., 2019).
    Terraflux's [[crafty.allocate]] uses one function for every service, the share of unmet demand, and one
    threshold per AFT. It reproduces the logic of CRAFTY (production from capitals, competition driven by
    unmet demand, giving up and giving in), not the numbers of a published CRAFTY run.

### Giving up and giving in

Two thresholds per AFT describe how attached its managers are to their land:

- **Giving up** (`giving_up`): a manager whose competitiveness on its own cell falls below this threshold may
  abandon the cell, which becomes unmanaged. At each step it leaves with probability `giving_up_rate`. A high
  threshold describes managers who need a good return to stay (commercial farmers); a low one, managers who
  stay whatever happens (smallholders with no alternative).
- **Giving in** (`giving_in`): a managed cell is taken over by another AFT only if that AFT's
  competitiveness exceeds the incumbent's by more than the incumbent's giving-in threshold. It is the
  resistance to being bought out.

Every step, a share `search` of the cells is offered, in a random order. For each offered cell, the most
competitive AFT takes it if the cell is unmanaged (and the AFT's competitiveness is above 0), or if it beats
the incumbent by the incumbent's `giving_in`. Random numbers come from a counter of the seed, the step and the
cell, so the same seed always gives the same maps.

```tfgallery
columns: 2
caption: What each AFT of the template would produce on each cell, from the capitals of the landscape. Left, a farmer's food, 1 x fertility²; right, a forester's timber, 1 x wood¹. The farmer's sensitivity of 2 makes its production fall fast on poor soil.
images:
  - { src: /img/tutorials/land-change/crafty/prod_farmer.png, caption: "Farmer: food" }
  - { src: /img/tutorials/land-change/crafty/prod_forester.png, caption: "Forester: timber" }
```

## When to use it

CRAFTY answers questions about how land use responds to the demand for what land produces: which mix of land
uses meets a demand for food, timber and recreation in 2050; where farming retreats if yields rise; what
happens to forests when demand for meat grows; how attachment to land (giving-up and giving-in thresholds)
slows or speeds the response. Brown et al. (2018) used it to explore how land managers' behaviours change the
pathways towards visions of the future of land in Great Britain; Brown et al. (2019) found that, across
Europe, plausible behaviours could lead to the breakdown of the supply of some services under some scenarios.

Use it when the demand is for services rather than for areas of classes. When the demand is the area of each
land use, CLUE-S ([the first series](/tutorials/land-change/clue-s-ideas/)) is simpler; when it is the supply of
goods by land systems with intensities, see [CLUMondo](/tutorials/land-change/clumondo/). CRAFTY is not a
model of individual decisions: the managers of one AFT are interchangeable. For individual agents with their
own state, see [agents](/tutorials/land-change/agents/).

## What you need

- **Capitals**: maps from 0 to 1, a layer each, named. Rescale real variables (yield potential, growing stock,
  distance to markets inverted) to 0 to 1, where 1 is the best for the AFTs that use them.
- **The land at the start**: who manages each cell, as 1, 2... in the order of the AFTs, 0 for unmanaged
  cells, missing outside the region. From a land cover map, reclassify each class to the AFT that manages it.
- **The AFTs**: for each, its production of each service, its sensitivity to each capital, its giving-up and
  giving-in thresholds, and optionally a land cover class for the output map.
- **A demand**: a table with a column per service and a row per step. If it has fewer rows than steps, the last
  row holds for the steps after it.

The template *CRAFTY: farmers and foresters* (in the application, **Templates**) draws its capitals and its
starting land by chance, so it runs as it is.

## Build the model

### Step 1: the capitals and the land at the start

```tfgraph
#! caption: The capitals (two Gaussian fields, as a stack with named layers) and who manages each cell at the start.
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "fertility"
op = "nlm.gaussian_field"
params = { rows = 100, columns = 100, range = 15, seed = 101 }

[[nodes]]
id = "wood"
op = "nlm.gaussian_field"
params = { rows = 100, columns = 100, range = 25, seed = 102 }

[[nodes]]
id = "capitals"
op = "stack.of"
inputs = { fertility = "fertility.map", wood = "wood.map" }

[[nodes]]
id = "start"
op = "nlm.random_cluster"
params = { rows = 100, columns = 100, p = 0.5, weights = [0.5, 0.5], seed = 103 }
```

1. **`fertility`** ([[nlm.gaussian_field]]) draws the soil's fertility, 0 to 1, in patches about 15 cells
   across. With real data, a [[raster.read]] of a capital already scaled to 0 to 1.
2. **`wood`** draws the forest's growing stock, in larger patches, independent of the soil.
3. **`capitals`** ([[stack.of]]) puts them in one stack whose layers take the names of the node's inputs:
   `fertility` and `wood`. These are the names the AFTs' sensitivities use.
4. **`start`** ([[nlm.random_cluster]]) draws clusters of two classes, half each: 1 farmers, 2 foresters. The
   numbers must be the positions of the AFTs in the list given to the allocation (1 the first, 2 the second).

```tfgallery
columns: 3
caption: The landscape of the template. The two capitals and who manages each cell at the start.
images:
  - { src: /img/tutorials/land-change/crafty/fertility.png, caption: "Fertility" }
  - { src: /img/tutorials/land-change/crafty/wood.png, caption: "Wood" }
  - { src: /img/tutorials/land-change/crafty/start.png, caption: "Managers at the start" }
```

### Step 2: the demand and the loop

```tfgraph
#! caption: The whole template. The demand is a table of 20 rows (one per step); the loop carries the land (who manages each cell) and collects the land cover and the supply of every step.
#! highlight: demand, simulation, compete
format = "terraflux/1"

[[nodes]]
id = "fertility"
op = "nlm.gaussian_field"
params = { rows = 100, columns = 100, range = 15, seed = 101 }

[[nodes]]
id = "wood"
op = "nlm.gaussian_field"
params = { rows = 100, columns = 100, range = 25, seed = 102 }

[[nodes]]
id = "capitals"
op = "stack.of"
inputs = { fertility = "fertility.map", wood = "wood.map" }

[[nodes]]
id = "start"
op = "nlm.random_cluster"
params = { rows = 100, columns = 100, p = 0.5, weights = [0.5, 0.5], seed = 103 }

[[nodes]]
id = "demand"
op = "table.of"
params = { columns = ["food", "timber"], rows = [[1500, 1500], [1600, 1500], [1700, 1500], [1800, 1500], [1900, 1500], [2000, 1500], [2100, 1500], [2200, 1500], [2300, 1500], [2400, 1500], [2500, 1500], [2600, 1500], [2700, 1500], [2800, 1500], [2900, 1500], [3000, 1500], [3100, 1500], [3200, 1500], [3300, 1500], [3400, 1500]] }

[[nodes]]
id = "simulation"
op = "control.repeat"
inputs = { times = 20, land = "start.map" }

[nodes.body]
carry = { land = "compete.land" }
collect = { cover = "compete.cover", info = "compete.info" }

[[nodes.body.nodes]]
id = "compete"
op = "crafty.allocate"
inputs = { land = "@land", capitals = "capitals.stack", demand = "demand.table", step = "@step", seed = 7 }
params = { search = 0.2, giving_up_rate = 0.2, afts = [
  { name = "farmer", class = 1, production = { food = 1.0 }, sensitivity = { fertility = 2.0 }, giving_up = 0.05, giving_in = 0.1 },
  { name = "forester", class = 2, production = { timber = 1.0 }, sensitivity = { wood = 1.0 }, giving_up = 0.05, giving_in = 0.1 },
] }
```

- **`demand`** ([[table.of]]) writes the demand as a table: columns `food` and `timber`, a row per step. Food
  grows by 100 every step, from 1500 to 3400; timber stays at 1500. The units are those of the production: a
  cell of full fertility managed by a farmer produces 1 of food. With real data, a table read from a file of
  scenarios, or a demand computed by agents (see [a coupled model](/tutorials/land-change/coupled-agents/)).
- **`simulation`** ([[control.repeat]]) runs 20 steps. It carries one value, `land`: `@land` is the
  starting map at step 1 and, after it, the map the previous step gave (`carry = { land = "compete.land" }`).
  It collects `cover` (the land cover of each step) and `info` (the supply and demand of each step).
- **`compete`** ([[crafty.allocate]]), in the body, runs one step of the competition. Its inputs: `land` the
  managers at the start of the step (`@land`); `capitals` the stack, by name; `demand` the table; `step` the
  row of the demand to use (`@step`); `seed` the seed of its random numbers. Its settings:
    - `afts`, the list of AFTs, in order (1 farmer, 2 forester). For each, `production` (here one service
      each, at most 1 per cell), `sensitivity` (the farmer depends on fertility with an exponent of 2, the
      forester on wood with 1), `giving_up = 0.05` and `giving_in = 0.1`, and `class`, the land cover class
      written in the `cover` output for its cells.
    - `search = 0.2`: a fifth of the cells offered each step.
    - `giving_up_rate = 0.2`: a manager below its giving-up threshold leaves with a chance of one in five
      each step.
    - `services` (not set here): which column of the demand each service reads; by default every column, by
      its name.

Its outputs: `land` (1, 2, or 0 for unmanaged cells) for the next step, `cover` (the `class` of each cell's
AFT, 0 unmanaged), and `info`, a table with a row per service (`service` 0 is the first in alphabetical order,
here food, 1 timber): its `demand`, the `supply_before` the step and the `supply` after it.

!!! app "In the application"
    Open the template from **Templates** and double-click `simulation` to see `compete`. The AFTs are edited in
    the Node pane as text. Press ++ctrl+enter++ on `plot_cover` to run the model and open the land cover of
    every step in a tab, with a slider over the steps.

From the command line, add outputs for what you want written:

```toml
[outputs.cover]
from = "simulation.cover"      # cover.tif, a band per step

[outputs.info]
from = "simulation.info"       # info.csv, two rows (food, timber) per step
```

```bash
terraflux run crafty.toml --out crafty
```

The 20 steps over 10,000 cells take a few hundredths of a second.

## Read the results

### Supply and demand, step by step

```tfslider
label: Step
caption: The managers of each cell, step by step (ochre farmers, green foresters, light grey unmanaged). The chart shows the supply of food and timber against their demand.
frames:
  - { value: 1, src: /img/tutorials/land-change/crafty/step_1.png, stats: { Farmers: "43.5%", Foresters: "43.0%", Unmanaged: "13.5%" } }
  - { value: 2, src: /img/tutorials/land-change/crafty/step_2.png, stats: { Farmers: "45.0%", Foresters: "33.8%", Unmanaged: "21.2%" } }
  - { value: 3, src: /img/tutorials/land-change/crafty/step_3.png, stats: { Farmers: "47.0%", Foresters: "27.3%", Unmanaged: "25.8%" } }
  - { value: 4, src: /img/tutorials/land-change/crafty/step_4.png, stats: { Farmers: "45.5%", Foresters: "26.1%", Unmanaged: "28.4%" } }
  - { value: 5, src: /img/tutorials/land-change/crafty/step_5.png, stats: { Farmers: "45.0%", Foresters: "25.8%", Unmanaged: "29.2%" } }
  - { value: 6, src: /img/tutorials/land-change/crafty/step_6.png, stats: { Farmers: "44.9%", Foresters: "25.4%", Unmanaged: "29.7%" } }
  - { value: 7, src: /img/tutorials/land-change/crafty/step_7.png, stats: { Farmers: "46.0%", Foresters: "25.2%", Unmanaged: "28.8%" } }
  - { value: 8, src: /img/tutorials/land-change/crafty/step_8.png, stats: { Farmers: "47.1%", Foresters: "25.0%", Unmanaged: "27.9%" } }
  - { value: 9, src: /img/tutorials/land-change/crafty/step_9.png, stats: { Farmers: "48.3%", Foresters: "24.7%", Unmanaged: "27.0%" } }
  - { value: 10, src: /img/tutorials/land-change/crafty/step_10.png, stats: { Farmers: "49.7%", Foresters: "24.4%", Unmanaged: "25.9%" } }
  - { value: 11, src: /img/tutorials/land-change/crafty/step_11.png, stats: { Farmers: "51.0%", Foresters: "24.2%", Unmanaged: "24.8%" } }
  - { value: 12, src: /img/tutorials/land-change/crafty/step_12.png, stats: { Farmers: "53.0%", Foresters: "24.3%", Unmanaged: "22.7%" } }
  - { value: 13, src: /img/tutorials/land-change/crafty/step_13.png, stats: { Farmers: "53.9%", Foresters: "23.5%", Unmanaged: "22.6%" } }
  - { value: 14, src: /img/tutorials/land-change/crafty/step_14.png, stats: { Farmers: "56.1%", Foresters: "24.0%", Unmanaged: "19.9%" } }
  - { value: 15, src: /img/tutorials/land-change/crafty/step_15.png, stats: { Farmers: "56.6%", Foresters: "23.3%", Unmanaged: "20.1%" } }
  - { value: 16, src: /img/tutorials/land-change/crafty/step_16.png, stats: { Farmers: "58.9%", Foresters: "24.1%", Unmanaged: "17.0%" } }
  - { value: 17, src: /img/tutorials/land-change/crafty/step_17.png, stats: { Farmers: "59.5%", Foresters: "22.7%", Unmanaged: "17.8%" } }
  - { value: 18, src: /img/tutorials/land-change/crafty/step_18.png, stats: { Farmers: "61.2%", Foresters: "23.9%", Unmanaged: "14.9%" } }
  - { value: 19, src: /img/tutorials/land-change/crafty/step_19.png, stats: { Farmers: "61.9%", Foresters: "22.9%", Unmanaged: "15.2%" } }
  - { value: 20, src: /img/tutorials/land-change/crafty/step_20.png, stats: { Farmers: "63.3%", Foresters: "23.4%", Unmanaged: "13.3%" } }
chart:
  type: line
  data: /data/tutorials/land-change/crafty/supply.csv
  x: step
  y: [food_demand, food_supply, timber_demand, timber_supply]
  labels: { food_demand: "Food demand", food_supply: "Food supply", timber_demand: "Timber demand", timber_supply: "Timber supply" }
  colors: { food_demand: "#8a6a1f", food_supply: "#d9a441", timber_demand: "#1f5f3a", timber_supply: "#3b8a5a" }
  xlabel: Step
  ylabel: Supply and demand
```

Three phases can be read in the chart and the maps:

1. **Too much timber.** At the start half the cells are foresters, whose timber (2,950 before the first step)
   is twice the demand of 1500. The utility of timber is 0, so the foresters' competitiveness is 0, below their
   giving-up threshold, and a fifth of them leave at each step. Within three steps the supply of timber falls to
   the demand, and almost 30 % of the land is unmanaged.
2. **Timber balanced, food short.** From step 4 the timber supply stays close to 1500, a little below it: as
   soon as it rises above the demand, foresters become uncompetitive and some leave; as soon as it falls below,
   foresters take cells back. Food is short from the start, so farmers take the unmanaged cells and some of the
   foresters' cells where fertility is high.
3. **A demand the landscape cannot meet.** The food demand grows by 100 a step, the supply by about 50. If every
   one of the 10,000 cells were farmed, the landscape would produce 2,953 of food (the sum of fertility²): from
   step 15 the demand exceeds what the land could ever give. At step 20, 63 % of the land is farmed and
   produces 2,317 for a demand of 3,400.

![The land after 20 steps](/img/tutorials/land-change/crafty/cover_last.png "Who manages each cell after 20 steps. Farmers hold the fertile land, foresters the land with wood where the soil is poor, and 13 % of the cells are unmanaged although food is short.")

Why do 13 % of the cells stay unmanaged while food is short? The farmers' competitiveness on a cell is
\(u_{food} \times \text{fertility}^2\). At step 20, \(u_{food} = (3400 - 2317)/3400 \approx 0.32\); on a cell of
fertility 0.4 that is \(0.32 \times 0.16 = 0.05\), exactly the giving-up threshold. Farmers take such cells and
leave them again; the unmanaged cells have a mean fertility of 0.41, against 0.59 on the farmed ones. They are
marginal land, not worth farming at the value food has.

### How to report it

Report the AFTs with their production, sensitivities and thresholds (a table), the capitals and how they were
scaled, the demand scenario, the search and giving-up rate, the number of steps and the seeds. Show the supply
against the demand of each service over time (the chart above), the land cover at the end, and the share of
unmanaged land. Compare scenarios of demand, and of behaviour (the thresholds), rather than presenting one run as
a forecast.

## Settings that change the answer

### The share of cells offered each step

`search` sets how fast the land can respond: only the offered cells can change hands. Move the slider to see
the land after 20 steps with 5 %, 10 %, 20 % and 40 % of the cells offered each step; the chart shows the food
supply over the steps for each.

```tfslider
label: search
caption: The land after 20 steps with each share of cells offered per step (ochre farmers, green foresters, grey unmanaged). The chart shows the food supply of each run against the demand.
frames:
  - { value: 0.05, src: /img/tutorials/land-change/crafty/search_0.05.png, stats: { "Food at step 20": "1,895", Unmanaged: "23.5%" } }
  - { value: 0.1, src: /img/tutorials/land-change/crafty/search_0.1.png, stats: { "Food at step 20": "2,166", Unmanaged: "17.7%" } }
  - { value: 0.2, src: /img/tutorials/land-change/crafty/search_0.2.png, stats: { "Food at step 20": "2,317", Unmanaged: "13.3%" } }
  - { value: 0.4, src: /img/tutorials/land-change/crafty/search_0.4.png, stats: { "Food at step 20": "2,419", Unmanaged: "9.0%" } }
start: 2
chart:
  type: line
  data: /data/tutorials/land-change/crafty/search.csv
  x: step
  y: [demand, "search 0.05", "search 0.1", "search 0.2", "search 0.4"]
  colors: { demand: "#1c2420", "search 0.05": "#e7c98a", "search 0.1": "#d9a441", "search 0.2": "#b07a1c", "search 0.4": "#6e4a0e" }
  xlabel: Step
  ylabel: Food
```

With 5 % of the cells offered, the land lags far behind the demand and almost a quarter is still unmanaged
after 20 steps; with 40 %, it follows the demand closely until it reaches what the landscape can give. In
CRAFTY, the share of land that changes hands each year is a property of a region (how active its land market
is), and with yearly steps a few per cent is realistic. A model whose steps are years and whose `search` is 0.4
describes a land market far more fluid than any real one.

### Giving in

| `giving_in` | Changes of manager over 20 steps | Food short at step 20 | Timber short at step 20 |
|---|---|---|---|
| 0 | 26,390 | 996 | 135 |
| 0.1 | 20,044 | 1,083 | 110 |
| 0.3 | 19,012 | 1,092 | 110 |
| 0.6 | 19,012 | 1,092 | 110 |

```tfgallery
columns: 4
caption: The land after 20 steps with each giving-in threshold (the same for both AFTs).
images:
  - { src: /img/tutorials/land-change/crafty/giving_in_0.0.png, caption: "giving_in = 0" }
  - { src: /img/tutorials/land-change/crafty/giving_in_0.1.png, caption: "giving_in = 0.1" }
  - { src: /img/tutorials/land-change/crafty/giving_in_0.3.png, caption: "giving_in = 0.3" }
  - { src: /img/tutorials/land-change/crafty/giving_in_0.6.png, caption: "giving_in = 0.6" }
```

With a giving-in threshold of 0, a manager gives way to any AFT even slightly more competitive, and cells
change hands a third more often: the land reorganises faster and meets more of the food demand. From 0.3 on,
nothing changes: competitiveness here never exceeds about 0.3 (a utility of at most 1 times a production of at
most 1, and much less in practice), so no AFT can ever beat an incumbent by that much, and only cells that
were abandoned change hands. A threshold above the range of competitiveness switches take-overs off: check the
range of \(C_a(i)\) in your model before choosing thresholds.

### Giving up

The giving-up threshold and rate decide how much land is abandoned when a service is oversupplied, as timber is
at the start here. A high threshold with a high rate empties land quickly (the 30 % unmanaged of step 6); a
threshold of 0 keeps every manager in place, whatever the demand. Try `giving_up = 0` for the foresters in the
template: timber stays oversupplied for the whole run.

## Common pitfalls

**Capitals outside 0 to 1.** Production uses \(k^e\) with \(k\) clipped to 1; a capital in raw units (a
growing stock in m³/ha) is clipped to 1 almost everywhere, and the AFT sees a uniform landscape. Rescale every
capital to 0 to 1 first, with [[raster.calc]] or [[nlm.rescale]].

**Capital names that do not match.** Sensitivities name layers of the capitals stack. A sensitivity to a
capital that is not in the stack stops the run with an error naming it; a capital the AFTs do not name is
ignored. Name the layers with [[stack.of]] inputs (or [[stack.set]]).

**Land codes that do not match the AFT list.** The land map holds 1, 2... in the order of `afts`, 0 for
unmanaged. A land cover map with its own codes (say 2 cropland, 5 forest) must be reclassified first; cells with
codes beyond the number of AFTs are treated as unmanaged.

**A demand in other units than the production.** If a farmer on full capitals produces 1 and the demand is in
tonnes, the utilities are always near 1 (the supply is tiny) or always 0. Express production per cell in the
units of the demand (tonnes per cell per year), or scale the demand.

**A demand beyond the landscape.** Compute the most the landscape could produce of each service (the sum over
cells of the best AFT's production) and compare it with the demand. Here the food demand passes it at step 15:
the model can only show the shortfall.

**Reading one run.** The giving up and the order of the offered cells are random. Run several seeds
(`terraflux submit --replicates`) and report the range.

## Going further

- [A coupled model](/tutorials/land-change/coupled-agents/) makes the demand come from a population of agents
  and lets parcel managers change the capitals.
- [Agents](/tutorials/land-change/agents/) builds individual agents with their own state.
- [CLUMondo](/tutorials/land-change/clumondo/) meets a demand for goods and services with land systems of
  different intensities.
- The operator: [[crafty.allocate]].

## References

- Brown, C., Holzhauer, S., Metzger, M. J., Paterson, J. S., & Rounsevell, M. (2018). Land managers' behaviours modulate pathways to visions of future land systems. *Regional Environmental Change*, 18(3), 831–845. https://doi.org/10.1007/s10113-016-0999-y
- Brown, C., Seo, B., & Rounsevell, M. (2019). Societal breakdown as an emergent property of large-scale behavioural models of land use change. *Earth System Dynamics*, 10(4), 809–845. https://doi.org/10.5194/esd-10-809-2019
- Murray-Rust, D., Brown, C., van Vliet, J., Alam, S. J., Robinson, D. T., Verburg, P. H., & Rounsevell, M. (2014). Combining agent functional types, capitals and services to model land use dynamics. *Environmental Modelling & Software*, 59, 187–201. https://doi.org/10.1016/j.envsoft.2014.05.019
