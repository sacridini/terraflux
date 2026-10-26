---
title: "A coupled model: macro agents, parcel managers and CRAFTY"
short: A coupled model
description: Put three kinds of agents in one loop, a population whose needs make the demand, parcel managers whose subsidies change the land's capitals, and CRAFTY's farmers and foresters competing for the cells, then let agents freeze land in a CLUE-S model.
order: 730
level: advanced
time: 50 min
ops: [agents.table, agents.parcels, agents.update, agents.summary, agents.map, crafty.allocate, stack.set, raster.calc, control.repeat, lulcc.step]
series: Dynamic drivers and agents
part: 4
---

The previous parts gave a land change model its pieces one at a time: [drivers that move with the
map](/tutorials/land-change/dynamic-drivers/), [agents with rules](/tutorials/land-change/agents/), and
[CRAFTY's competition for land](/tutorials/land-change/crafty/). In this last part you will connect them. A
population of **macro agents** grows, and its needs become the demand for food and timber; four **parcel
managers** decide, step by step, whether to subsidise their land, and their subsidies raise the fertility the
farmers see; **CRAFTY's** farmers and foresters compete for the cells to meet the demand. You will read the loop node
by node, run it for 30 steps with and without one manager's subsidy, and see what a policy on one parcel does to the
whole landscape. A second, shorter model lets two councils decide where a **CLUE-S** model may change the land. At
the end you will know how to make the outputs of one kind of agent the inputs of another, inside one loop.

## The idea

### Three levels of decision

Land use is decided at several levels at once. National demand for food and timber follows population and diets;
regional or local governments set subsidies, zoning and protection; land managers decide what to do with each field.
Land change models usually describe one level and take the others as given: a CLUE-S model takes the demand from an
economic model, a CRAFTY model takes the demand as a table. Verburg (2006) argued that the most important
**feedbacks** of land use cross these levels: what managers do changes the supply, the supply changes prices and
policies, and those change what managers do. Verburg et al. (2008) showed one way to cross them, chaining a
global economic model, an integrated assessment model and CLUE-S's allocation for the land use of Europe.

In Terraflux every level can be a node of the same model, and a loop ([[control.repeat]]) runs them in turn at every
step, so the output of one level at step \(t\) is the input of another at the same step, and what any of them
carries is there at step \(t + 1\).

![Who drives whom in the coupled model](/img/tutorials/land-change/coupled-agents/schematic.png "The coupled model of this part. The macro agents give the demand; the parcel managers' subsidies change a capital; CRAFTY's farmers and foresters compete for the cells with both.")

!!! science "Key idea: one step of the coupled model"
    At every step \(t\):

    1. the **population** \(N_t = 1.02\, N_{t-1}\) grows, and its needs are \(D_{food,t} = 0.25\, N_t\) and
       \(D_{timber,t} = 0.15\, N_t\): the **demand**;
    2. each **parcel manager** \(m\) decides a subsidy \(b_m(t)\) (3 for the first manager from step 6, 1
       otherwise), and the fertility of every cell \(i\) of its parcel becomes \(k_{fert}(i) = \min(b_m\,
       f(i), 1)\), with \(f\) the soil's own fertility;
    3. **CRAFTY** (Murray-Rust et al., 2014) offers a fifth of the cells, and on each the AFT of highest competitiveness
       \(C_a(i) = \sum_s u_s P_{a,s}(i)\) takes it, with \(u_s\) the share of the demand of service \(s\) still
       unmet ([CRAFTY's equations](/tutorials/land-change/crafty/#the-idea)).

    The demand changes the utilities \(u_s\); the subsidy changes the production \(P_{a,s}(i)\) of the farmers; the
    land carried to the next step holds who manages each cell.

### The model and where it comes from

This is the coupled model of Terraflux's documentation on agents (`docs/agents.md`), which its tests run
(`crates/exec/tests/abm.rs`) to check that the population's growth reaches the demand, that the supply follows the
demand, and that the subsidised parcel draws farmers. Its landscape is deliberately plain, so that every effect can
be traced to its cause: 40 × 40 cells whose fertility rises from west to east and whose wood rises from south to
north, divided into four parcels, the quadrants. Nobody manages the land at the start.

## When to use it

A coupled model answers questions that cross levels:

- **How a demand reaches the land.** A population, an economy or a market whose growth is itself simulated (with
  its own rules, scenarios of fertility or diets) drives the land use, instead of a fixed table of areas.
- **What a local policy does to the rest of the landscape.** A subsidy, a zoning rule or a reserve on one parcel
  moves land use there, and also away from elsewhere: a policy can **displace** land use rather than add it, as
  Meyfroidt et al. (2010) found for countries whose forests recovered while their imports of timber and food grew.
- **How decision makers respond to the land.** Managers' rules can read the land map (`agents.parcels` gives each
  manager the mean of any map over its parcel), so a manager can react to what CRAFTY's agents did last step.

Agent-based models of land use carry the warnings of [the agents part](/tutorials/land-change/agents/#when-to-use-it):
rules that are guessed make a model less certain, not more (An, 2012; Rounsevell et al., 2012). A coupled model
multiplies the guesses, so it is a tool for exploring "what if" (what if one region subsidises its farmers?), and
must be described fully enough to be rebuilt (Grimm et al., 2020). For the amount and place of change alone,
without the decisions, use a statistical model ([CLUE-S](/tutorials/land-change/clue-s-ideas/), Dinamica EGO).

## What you need

- **Capitals** for CRAFTY: maps from 0 to 1 named as the AFTs' sensitivities name them (here `fertility` and
  `wood`).
- **A map of parcels**: the id of the parcel each cell belongs to (here the four quadrants).
- **The land at the start**: who manages each cell, 1, 2... in the order of the AFTs, 0 unmanaged (here 0
  everywhere).
- **Macro agents**: their attributes (population, needs per head) and how they change.
- **Rules** for the parcel managers.

Everything here is drawn in the model, so it runs as it is: the [model file](/data/tutorials/land-change/coupled-agents/coupled.toml)
(`terraflux run coupled.toml`, a fraction of a second). In the application, open the templates *Farmers on their
parcels (agents)* and *CRAFTY: farmers and foresters* from **Templates → Land change**: their nodes are the pieces
of this one.

## Build the model

### Step 1: the landscape

```tfgraph
#! caption: The landscape: two gradients make the capitals, a stack names them, the parcels are the four quadrants, and the land starts unmanaged.
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "east"
op = "nlm.planar_gradient"
params = { rows = 40, columns = 40, direction = 90 }

[[nodes]]
id = "north"
op = "nlm.planar_gradient"
params = { rows = 40, columns = 40, direction = 0 }

[[nodes]]
id = "fertility"
op = "raster.calc"
inputs = { e = "east.map" }
params = { expression = "0.0125 + 0.975 * e" }

[[nodes]]
id = "wood"
op = "raster.calc"
inputs = { n = "north.map" }
params = { expression = "0.0125 + 0.975 * n" }

[[nodes]]
id = "capitals"
op = "stack.of"
inputs = { fertility = "fertility.map", wood = "wood.map" }

[[nodes]]
id = "parcels"
op = "raster.calc"
inputs = { e = "east.map", n = "north.map" }
params = { expression = "1 + (e > 0.5) + 2 * (n < 0.5)" }

[[nodes]]
id = "land"
op = "raster.calc"
inputs = { e = "east.map" }
params = { expression = "e * 0" }
```

- **Nodes 1 and 2**, `east` and `north` ([[nlm.planar_gradient]]), draw two linear gradients from 0 to 1, rising
  towards the east (`direction = 90`) and towards the north (`direction = 0`).
- **Nodes 3 and 4**, `fertility` and `wood` ([[raster.calc]]), rescale them to the centres of the cells of the test,
  0.0125 to 0.9875: the soil is poor in the west and rich in the east; the wood is poor in the south and rich in the
  north.
- **Node 5**, `capitals` ([[stack.of]]), puts them in one stack, its layers named after the node's inputs: the
  names CRAFTY's sensitivities use.
- **Node 6**, `parcels` ([[raster.calc]]), numbers the quadrants: 1 north-west, 2 north-east, 3 south-west, 4
  south-east (comparisons give 1 or 0).
- **Node 7**, `land`, is a map of zeros on the same grid: no cell is managed at the start.

```tfgallery
columns: 3
caption: The landscape. The fertility rises to the east, the wood to the north; parcel 1, the north-west, has poor soil and rich wood.
images:
  - { src: /img/tutorials/land-change/coupled-agents/fertility.png, caption: "Fertility" }
  - { src: /img/tutorials/land-change/coupled-agents/wood.png, caption: "Wood" }
  - { src: /img/tutorials/land-change/coupled-agents/parcels.png, caption: "Parcels" }
```

With real data, nodes 1 to 4 are [[raster.read]] nodes of capitals scaled to 0 to 1, node 6 a cadastre or a map of
municipalities rasterized with [[vector.rasterize]], and node 7 a land cover map reclassified to the AFTs.

### Step 2: two populations of agents

```tfgraph
#! caption: The agents: one macro agent written in the model, and an agent per parcel.
#! fragment: true
#! highlight: people, managers
format = "terraflux/1"

[[nodes]]
id = "parcels"
op = "raster.calc"
params = { expression = "1 + (e > 0.5) + 2 * (n < 0.5)" }

[[nodes]]
id = "people"
op = "agents.table"
params = { agents = [{ id = 1, population = 1000, food_per_head = 0.25, timber_per_head = 0.15 }] }

[[nodes]]
id = "managers"
op = "agents.parcels"
inputs = { parcels = "parcels.map" }
```

- **`people`** ([[agents.table]]) writes the macro agents in the model: here one, the population of the region,
  with 1,000 people who each need 0.25 units of food and 0.15 of timber per step. Several rows would make several
  macro agents (countries, markets); their needs are added up below.
- **`managers`** ([[agents.parcels]]) makes an agent per parcel id: four managers with the columns `id` and
  `cells` (400 each). Connect other maps to it (an input named by you in the Node pane) to give each manager the mean
  of that map over its parcel.

### Step 3: the loop

```tfgraph
#! caption: The whole coupled model. The loop carries the land and the population; its body grows the population (grow) and sums its needs (need), lets the managers decide (decide), paints their subsidies (boost), applies them to the fertility (fert, caps), and runs CRAFTY's step (alloc).
#! highlight: simulation, grow, need, decide, boost, fert, caps, alloc
#! params: false
format = "terraflux/1"

[[nodes]]
id = "east"
op = "nlm.planar_gradient"
params = { rows = 40, columns = 40, direction = 90 }

[[nodes]]
id = "north"
op = "nlm.planar_gradient"
params = { rows = 40, columns = 40, direction = 0 }

[[nodes]]
id = "fertility"
op = "raster.calc"
inputs = { e = "east.map" }
params = { expression = "0.0125 + 0.975 * e" }

[[nodes]]
id = "wood"
op = "raster.calc"
inputs = { n = "north.map" }
params = { expression = "0.0125 + 0.975 * n" }

[[nodes]]
id = "capitals"
op = "stack.of"
inputs = { fertility = "fertility.map", wood = "wood.map" }

[[nodes]]
id = "parcels"
op = "raster.calc"
inputs = { e = "east.map", n = "north.map" }
params = { expression = "1 + (e > 0.5) + 2 * (n < 0.5)" }

[[nodes]]
id = "land"
op = "raster.calc"
inputs = { e = "east.map" }
params = { expression = "e * 0" }

[[nodes]]
id = "people"
op = "agents.table"
params = { agents = [{ id = 1, population = 1000, food_per_head = 0.25, timber_per_head = 0.15 }] }

[[nodes]]
id = "managers"
op = "agents.parcels"
inputs = { parcels = "parcels.map" }

[[nodes]]
id = "simulation"
op = "control.repeat"
inputs = { times = 30, land = "land.map", people = "people.agents" }

[nodes.body]
carry = { land = "alloc.land", people = "grow.agents" }
collect = { cover = "alloc.cover", info = "alloc.info", demand = "need.table" }

[[nodes.body.nodes]]
id = "grow"
op = "agents.update"
inputs = { agents = "@people" }
params = { rules = "population = population * 1.02\nfood = population * food_per_head\ntimber = population * timber_per_head" }

[[nodes.body.nodes]]
id = "need"
op = "agents.summary"
inputs = { agents = "grow.agents" }

[[nodes.body.nodes]]
id = "decide"
op = "agents.update"
inputs = { agents = "managers.agents", step = "@step" }
params = { rules = "subsidy = 3 if id == 1 and step > 5 else 1" }

[[nodes.body.nodes]]
id = "boost"
op = "agents.map"
inputs = { agents = "decide.agents", parcels = "parcels.map" }
params = { column = "subsidy" }

[[nodes.body.nodes]]
id = "fert"
op = "raster.calc"
inputs = { caps = "capitals.stack", boost = "boost.map" }
params = { expression = "min(caps.fertility * boost, 1)" }

[[nodes.body.nodes]]
id = "caps"
op = "stack.set"
inputs = { stack = "capitals.stack", map = "fert.map" }
params = { name = "fertility" }

[[nodes.body.nodes]]
id = "alloc"
op = "crafty.allocate"
inputs = { land = "@land", capitals = "caps.stack", demand = "need.table", step = "@step", seed = 3 }
params = { services = { food = "food_sum", timber = "timber_sum" }, search = 0.2, giving_up_rate = 0.2, afts = [
  { name = "farmer", class = 2, production = { food = 1.0 }, sensitivity = { fertility = 2.0 }, giving_up = 0.05, giving_in = 0.1 },
  { name = "forester", class = 3, production = { timber = 1.0 }, sensitivity = { wood = 1.0 }, giving_up = 0.05, giving_in = 0.1 },
] }

[outputs.info]
from = "simulation.info"

[outputs.demand]
from = "simulation.demand"
```

The loop **`simulation`** ([[control.repeat]]) runs 30 steps and carries two values: `land` (who manages each
cell, from `land.map`) and `people` (the population, from `people.agents`). `carry` says what each becomes after a
step: the land CRAFTY gives (`alloc.land`) and the population after its growth (`grow.agents`). `collect` keeps three
series: the land cover (`alloc.cover`), CRAFTY's table of supply and demand (`alloc.info`) and the demand
(`need.table`). Its body, node by node:

| Node | Operator | Reads | Gives | Why |
|---|---|---|---|---|
| `grow` | [[agents.update]] | `@people` | the population of this step | the macro agents' behaviour: growth by 2% and the needs it brings |
| `need` | [[agents.summary]] | `grow.agents` | a table of one row: `n`, `population_sum`, `food_sum`, `timber_sum`... | the demand: the sums of the needs over all macro agents |
| `decide` | [[agents.update]] | `managers.agents`, `@step` | the managers with a `subsidy` | the managers' decision this step |
| `boost` | [[agents.map]] | `decide.agents`, `parcels.map` | the subsidy of each cell's manager | a decision of agents made into a map |
| `fert` | [[raster.calc]] | `capitals.stack`, `boost.map` | the fertility, multiplied, at most 1 | the subsidy's effect on the capital |
| `caps` | [[stack.set]] | `capitals.stack`, `fert.map` | the capitals with the layer `fertility` replaced | CRAFTY reads capitals as one stack, by name |
| `alloc` | [[crafty.allocate]] | `@land`, `caps.stack`, `need.table`, `@step`, seed 3 | `land`, `cover`, `info` | the competition for the cells |

Three connections carry the coupling:

- **`need.table` → `alloc.demand`.** CRAFTY's demand is a table, a column per service and a row per step; the
  summary has one row, so CRAFTY reads it whatever the step. `services = { food = "food_sum", timber = "timber_sum" }`
  tells it which column is the demand of each service: the summary names its columns `<attribute>_sum`.
- **`boost.map` → `fert` → `caps.stack` → `alloc.capitals`.** The subsidy reaches CRAFTY as a capital. The farmer's
  production is \(k_{fert}^2\) (sensitivity 2), so tripling a fertility of 0.3 multiplies the farmer's food there by 9.
  [[stack.set]] replaces the layer `fertility` of the capitals and keeps `wood`.
- **`@people` and `@land`.** Both are carried, so the population grows from where it was, and CRAFTY starts each step
  from the managers of the step before.

The managers, on the other hand, read `managers.agents`, outside the loop: they decide afresh each step from their
starting attributes and the step number, with no memory. A manager that should remember (a budget spent, a decision
taken) must be carried like the population: add an input to the loop and a line to its `carry`.

!!! app "In the application"
    Build the landscape and the two populations first, then add a **Loop** (`control.repeat`) and double-click it to
    edit its body. In the loop's Node pane, add the inputs `land` and `people`, connect `land.map` and
    `people.agents` to them, and set `carry`. Inside the body, `@people`, `@land` and `@step` appear as ports to drag
    from; nodes outside the loop (`managers`, `parcels`, `capitals`) are dragged from as usual. The agents' and
    CRAFTY's operators are in the library under **Land change simulation → Agents**.

## Read the results

### Thirty steps, supply against demand

```tfslider
label: Step
caption: Who manages each cell (ochre farmers, green foresters, beige unmanaged) at each step of the run with the first manager's subsidy (from step 6). The chart follows the supply of food and timber against the demand the population makes.
frames:
  - { value: 1, src: /img/tutorials/land-change/coupled-agents/step_1.png, stats: { "Food (demand)": "87 (255)", "Timber (demand)": "109 (153)", Farmers: "145", Foresters: "183", Unmanaged: "1272", "Farmers in parcel 1": "0" } }
  - { value: 2, src: /img/tutorials/land-change/coupled-agents/step_2.png, stats: { "Food (demand)": "170 (260)", "Timber (demand)": "148 (156)", Farmers: "323", Foresters: "255", Unmanaged: "1022", "Farmers in parcel 1": "11" } }
  - { value: 3, src: /img/tutorials/land-change/coupled-agents/step_3.png, stats: { "Food (demand)": "236 (265)", "Timber (demand)": "160 (159)", Farmers: "482", Foresters: "262", Unmanaged: "856", "Farmers in parcel 1": "41" } }
  - { value: 4, src: /img/tutorials/land-change/coupled-agents/step_4.png, stats: { "Food (demand)": "271 (271)", "Timber (demand)": "162 (162)", Farmers: "543", Foresters: "266", Unmanaged: "791", "Farmers in parcel 1": "45" } }
  - { value: 5, src: /img/tutorials/land-change/coupled-agents/step_5.png, stats: { "Food (demand)": "261 (276)", "Timber (demand)": "166 (166)", Farmers: "547", Foresters: "267", Unmanaged: "786", "Farmers in parcel 1": "50" } }
  - { value: 6, src: /img/tutorials/land-change/coupled-agents/step_6.png, stats: { "Food (demand)": "282 (282)", "Timber (demand)": "169 (169)", Farmers: "530", Foresters: "278", Unmanaged: "792", "Farmers in parcel 1": "61" } }
  - { value: 7, src: /img/tutorials/land-change/coupled-agents/step_7.png, stats: { "Food (demand)": "287 (287)", "Timber (demand)": "172 (172)", Farmers: "530", Foresters: "293", Unmanaged: "777", "Farmers in parcel 1": "78" } }
  - { value: 8, src: /img/tutorials/land-change/coupled-agents/step_8.png, stats: { "Food (demand)": "293 (293)", "Timber (demand)": "176 (176)", Farmers: "526", Foresters: "309", Unmanaged: "765", "Farmers in parcel 1": "81" } }
  - { value: 9, src: /img/tutorials/land-change/coupled-agents/step_9.png, stats: { "Food (demand)": "299 (299)", "Timber (demand)": "180 (179)", Farmers: "527", Foresters: "319", Unmanaged: "754", "Farmers in parcel 1": "86" } }
  - { value: 10, src: /img/tutorials/land-change/coupled-agents/step_10.png, stats: { "Food (demand)": "305 (305)", "Timber (demand)": "183 (183)", Farmers: "524", Foresters: "326", Unmanaged: "750", "Farmers in parcel 1": "95" } }
  - { value: 11, src: /img/tutorials/land-change/coupled-agents/step_11.png, stats: { "Food (demand)": "311 (311)", "Timber (demand)": "187 (186)", Farmers: "533", Foresters: "325", Unmanaged: "742", "Farmers in parcel 1": "101" } }
  - { value: 12, src: /img/tutorials/land-change/coupled-agents/step_12.png, stats: { "Food (demand)": "317 (317)", "Timber (demand)": "191 (190)", Farmers: "536", Foresters: "334", Unmanaged: "730", "Farmers in parcel 1": "103" } }
  - { value: 13, src: /img/tutorials/land-change/coupled-agents/step_13.png, stats: { "Food (demand)": "323 (323)", "Timber (demand)": "194 (194)", Farmers: "532", Foresters: "337", Unmanaged: "731", "Farmers in parcel 1": "112" } }
  - { value: 14, src: /img/tutorials/land-change/coupled-agents/step_14.png, stats: { "Food (demand)": "329 (330)", "Timber (demand)": "198 (198)", Farmers: "538", Foresters: "350", Unmanaged: "712", "Farmers in parcel 1": "112" } }
  - { value: 15, src: /img/tutorials/land-change/coupled-agents/step_15.png, stats: { "Food (demand)": "337 (336)", "Timber (demand)": "202 (202)", Farmers: "547", Foresters: "354", Unmanaged: "699", "Farmers in parcel 1": "115" } }
  - { value: 16, src: /img/tutorials/land-change/coupled-agents/step_16.png, stats: { "Food (demand)": "325 (343)", "Timber (demand)": "200 (206)", Farmers: "528", Foresters: "353", Unmanaged: "719", "Farmers in parcel 1": "112" } }
  - { value: 17, src: /img/tutorials/land-change/coupled-agents/step_17.png, stats: { "Food (demand)": "350 (350)", "Timber (demand)": "211 (210)", Farmers: "545", Foresters: "377", Unmanaged: "678", "Farmers in parcel 1": "117" } }
  - { value: 18, src: /img/tutorials/land-change/coupled-agents/step_18.png, stats: { "Food (demand)": "345 (357)", "Timber (demand)": "214 (214)", Farmers: "536", Foresters: "375", Unmanaged: "689", "Farmers in parcel 1": "117" } }
  - { value: 19, src: /img/tutorials/land-change/coupled-agents/step_19.png, stats: { "Food (demand)": "348 (364)", "Timber (demand)": "217 (218)", Farmers: "533", Foresters: "382", Unmanaged: "685", "Farmers in parcel 1": "128" } }
  - { value: 20, src: /img/tutorials/land-change/coupled-agents/step_20.png, stats: { "Food (demand)": "367 (372)", "Timber (demand)": "222 (223)", Farmers: "553", Foresters: "393", Unmanaged: "654", "Farmers in parcel 1": "137" } }
  - { value: 21, src: /img/tutorials/land-change/coupled-agents/step_21.png, stats: { "Food (demand)": "352 (379)", "Timber (demand)": "222 (227)", Farmers: "536", Foresters: "400", Unmanaged: "664", "Farmers in parcel 1": "137" } }
  - { value: 22, src: /img/tutorials/land-change/coupled-agents/step_22.png, stats: { "Food (demand)": "381 (386)", "Timber (demand)": "231 (232)", Farmers: "573", Foresters: "418", Unmanaged: "609", "Farmers in parcel 1": "141" } }
  - { value: 23, src: /img/tutorials/land-change/coupled-agents/step_23.png, stats: { "Food (demand)": "356 (394)", "Timber (demand)": "228 (236)", Farmers: "544", Foresters: "411", Unmanaged: "645", "Farmers in parcel 1": "129" } }
  - { value: 24, src: /img/tutorials/land-change/coupled-agents/step_24.png, stats: { "Food (demand)": "391 (402)", "Timber (demand)": "242 (241)", Farmers: "588", Foresters: "421", Unmanaged: "591", "Farmers in parcel 1": "142" } }
  - { value: 25, src: /img/tutorials/land-change/coupled-agents/step_25.png, stats: { "Food (demand)": "363 (410)", "Timber (demand)": "234 (246)", Farmers: "554", Foresters: "414", Unmanaged: "632", "Farmers in parcel 1": "132" } }
  - { value: 26, src: /img/tutorials/land-change/coupled-agents/step_26.png, stats: { "Food (demand)": "408 (418)", "Timber (demand)": "248 (251)", Farmers: "615", Foresters: "424", Unmanaged: "561", "Farmers in parcel 1": "144" } }
  - { value: 27, src: /img/tutorials/land-change/coupled-agents/step_27.png, stats: { "Food (demand)": "365 (427)", "Timber (demand)": "233 (256)", Farmers: "551", Foresters: "415", Unmanaged: "634", "Farmers in parcel 1": "134" } }
  - { value: 28, src: /img/tutorials/land-change/coupled-agents/step_28.png, stats: { "Food (demand)": "406 (435)", "Timber (demand)": "253 (261)", Farmers: "609", Foresters: "437", Unmanaged: "554", "Farmers in parcel 1": "150" } }
  - { value: 29, src: /img/tutorials/land-change/coupled-agents/step_29.png, stats: { "Food (demand)": "421 (444)", "Timber (demand)": "261 (266)", Farmers: "617", Foresters: "460", Unmanaged: "523", "Farmers in parcel 1": "159" } }
  - { value: 30, src: /img/tutorials/land-change/coupled-agents/step_30.png, stats: { "Food (demand)": "422 (453)", "Timber (demand)": "266 (272)", Farmers: "612", Foresters: "470", Unmanaged: "518", "Farmers in parcel 1": "159" } }
chart:
  type: line
  data: /data/tutorials/land-change/coupled-agents/supply.csv
  x: step
  y: [food_demand, food_supply, timber_demand, timber_supply]
  labels: { food_demand: "Food demand", food_supply: "Food supply", timber_demand: "Timber demand", timber_supply: "Timber supply" }
  colors: { food_demand: "#8a6a1f", food_supply: "#d9a441", timber_demand: "#1f5f3a", timber_supply: "#3b8a5a" }
  xlabel: Step
  ylabel: Supply and demand
```

The run has three phases:

1. **Settling (steps 1 to 4).** The land starts empty, so the utility of both services is 1 and each AFT takes the
   offered cells where it is the more competitive: farmers in the fertile east, foresters in the west, where the soil is
   too poor for farming. A fifth of the
   cells is offered each step, so it takes four steps for the supply to reach the demand (food 271 for 271 at step 4).
2. **Following the demand (steps 4 to 15).** The population grows by 2% a step and the demand with it; each step a
   few more cells are taken and the supply stays within a unit of the demand. Half of the land is still
   unmanaged at step 4, and 44% at step 15: the demand does not need it.
3. **Falling behind (steps 16 to 30).** Food starts to fall short, by 5 to 62 units, every second or third step.
   The good soil is taken; each new unit of food now needs a cell of poorer soil, whose production (fertility
   squared) is small, and only a fifth of the cells is offered each step, so the farmers catch up with the demand
   and lose it again. At step 30 the land supplies 422 of food for a demand of 453, and 266 of
   timber for 272, with a third of the cells still unmanaged.

### What one manager's subsidy did

```tfcompare
caption: "The land at step 30 without the subsidy (left of the handle) and with the first manager's subsidy (right). Ochre farmers, green foresters, beige unmanaged. With the subsidy, farmers move into the north-west, the subsidised parcel."
before: { src: /img/tutorials/land-change/coupled-agents/final_without.png, label: No subsidy }
after: { src: /img/tutorials/land-change/coupled-agents/final_with.png, label: Parcel 1 subsidised }
```

![Legend](/img/tutorials/land-change/coupled-agents/cover_legend.png)

```tfchart
type: line
data: /data/tutorials/land-change/coupled-agents/parcel1.csv
x: step
y: [with, without]
labels: { with: "With the subsidy (from step 6)", without: "Without" }
colors: { with: "#e8901f", without: "#7f7f7f" }
xlabel: Step
ylabel: Farmers' cells in parcel 1
height: 280
caption: The farmers' cells in the subsidised parcel, step by step, in both runs.
```

Parcel 1 has the poorest soil of the map (its fertility is 0.01 to 0.49) and the best wood. Without the subsidy,
farmers hardly go there (13 cells at step 30). The subsidy triples its fertility from step 6, which makes much of it
as good for farming as the east, and farmers move in: 159 cells at step 30. But the demand is the same in both
runs, so the farmers who went to parcel 1 are farmers who did not go elsewhere:

| Step 30 | Parcel 1 (NW) | Parcel 2 (NE) | Parcel 3 (SW) | Parcel 4 (SE) | All |
|---|---:|---:|---:|---:|---:|
| Farmers, no subsidy | 13 | 261 | 80 | 330 | 684 |
| Farmers, with the subsidy | 159 | 139 | 52 | 262 | 612 |
| Foresters, no subsidy | 210 | 69 | 158 | 19 | 456 |
| Foresters, with the subsidy | 126 | 135 | 188 | 21 | 470 |

The subsidy **displaced** farming from the fertile north-east into the north-west, and the foresters it pushed out of
parcel 1 went to the north-east in their turn. It also changed the total: with it, 612 cells of farmers produce 422
of food; without it, 684 cells produce 396. The subsidised cells are productive enough that fewer cells grow more
food, and the land spared stays unmanaged. Whether that is good for a forest species depends on which forest was
cleared and which spared: the maps, not the totals, answer it.

### How to report it

For each level: the agents, their attributes and rules (the model file holds them all); how the levels are
connected (which output feeds which input, at which point in the step); the AFTs' parameters as in
[CRAFTY](/tutorials/land-change/crafty/#how-to-report-it). Report runs as comparisons (with and without the policy,
several growth rates, several seeds), with the supply against the demand and the maps of the land, following the ODD
protocol (Grimm et al., 2020).

## Agents in a CLUE-S model

Agents can drive any model of this category through a map. Here two councils, the west and the east of Plum Island,
decide whether their land may change, and their decision becomes the **restrictions** of the CLUE-S model of
[the CLUE-S series](/tutorials/land-change/clue-s-model/) ([part 5](/tutorials/land-change/clue-s-rules/) explains
restrictions).

```tfgraph
#! caption: lulcc's CLUE-S model of Plum Island with two councils: from step 5 (1990) the east council freezes its land, and its decision, painted on its territory, is the restrictions of the allocation.
#! numbered: true
#! highlight: west_east, towns, councils, zoning, frozen, allocate
#! params: false
format = "terraflux/1"

[inputs.observed]
kind = "stack"
path = "pie_landuse.tif"

[inputs.start]
kind = "map"
path = "pie_landuse.tif"
band = "1985"

[inputs.drivers]
kind = "stack"
path = "pie_drivers.tif"

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
inputs = { seed = 1 }

[[nodes]]
id = "history"
op = "lulcc.history"
inputs = { landscape = "$start" }

[[nodes]]
id = "west_east"
op = "nlm.planar_gradient"
inputs = { like = "$start" }
params = { direction = 90 }

[[nodes]]
id = "towns"
op = "raster.calc"
inputs = { g = "west_east.map" }
params = { expression = "1 + (g > 0.5)" }

[[nodes]]
id = "councils"
op = "agents.parcels"
inputs = { parcels = "towns.map" }

[[nodes]]
id = "simulation"
op = "control.repeat"
inputs = { times = "demand.steps", landscape = "$start", history = "history.history" }

[nodes.body]
carry = { landscape = "allocate.landscape", history = "allocate.history" }
collect = { maps = "allocate.landscape", info = "allocate.info" }

[[nodes.body.nodes]]
id = "zoning"
op = "agents.update"
inputs = { agents = "councils.agents", step = "@step" }
params = { rules = "frozen = 1 if id == 2 and step > 4 else 0" }

[[nodes.body.nodes]]
id = "frozen"
op = "agents.map"
inputs = { agents = "zoning.agents", parcels = "towns.map" }
params = { column = "frozen" }

[[nodes.body.nodes]]
id = "allocate"
op = "lulcc.step"
inputs = { landscape = "@landscape", history = "@history", suitability = "suitability.suitability", demand = "demand.demand", step = "@step", rng = "rng.rng", restrictions = "frozen.map" }
params = { allocation = { method = "clue", jitter = 0.0002, scale = 0.000001, max_iter = 1000, max_diff = 50, mean_diff = 50 }, elasticity = [0.2, 0.2, 0.2] }

[outputs.maps]
from = "simulation.maps"

[outputs.steps]
from = "simulation.info"
```

Nodes 1 to 5 and the loop are the CLUE-S model of [part 4 of that series](/tutorials/land-change/clue-s-model/).
The new nodes:

- **`west_east`** ([[nlm.planar_gradient]]) draws a gradient on the grid of the start map (its input `like`), and
  **`towns`** ([[raster.calc]]) cuts it in two: council 1 the west, council 2 the east. With real councils, a
  rasterized map of municipalities.
- **`councils`** ([[agents.parcels]]) makes an agent per council.
- In the body, **`zoning`** ([[agents.update]]) is the councils' decision: the east council freezes its land from
  step 5 (1990). **`frozen`** ([[agents.map]]) paints it on the councils' territories, and its map goes to the
  `restrictions` input of **`allocate`** ([[lulcc.step]]): cells where it is not 0 keep their class.

The model is [here](/data/tutorials/land-change/coupled-agents/councils.toml), to run in a folder with Plum Island's
[`pie_landuse.tif`](/data/tutorials/land-change/clue-s-ideas/pie_landuse.tif) and
[`pie_drivers.tif`](/data/tutorials/land-change/clue-s-ideas/pie_drivers.tif).

![Where the land changed after the east council froze it](/img/tutorials/land-change/coupled-agents/councils_changes.png "The cells that changed between 1989 and 1999 (by their new class) when the east council froze its land from 1990: all of the change went to the west.")

```tfchart
type: line
data: /data/tutorials/land-change/coupled-agents/councils.csv
x: year
y: ["West, east frozen from 1990", "East, frozen from 1990", "West, no council", "East, no council"]
colors: { "West, east frozen from 1990": "#b4508a", "East, frozen from 1990": "#2f6f9f", "West, no council": "#e3b3cf", "East, no council": "#9fb7c9" }
xlabel: Year
ylabel: Cells changed in the year
height: 300
caption: The cells that changed each year in each half, with the councils and without them (the first year, the reshuffle of part 4, is off the chart's scale at about 6,000 to 7,000 cells per half).
```

The demand is for the whole map, and every year still met it (`converged` = 1): from 1990 the west took all of the
change, about 450 cells a year instead of about 220. Between 1985 and 1999 the west gained 6,704 cells of new built
land with the councils and 4,572 without them; the east 2,255 (all before 1990) instead of 4,389. Protecting land in
one place, with a demand that does not change, moves the change elsewhere: the displacement the subsidy showed, with
another model. To make the east's protection *reduce* the change, the demand itself must come from agents that
respond to it, as the population does in the coupled model above.

## Settings that change the answer

### The growth of the population

```tfslider
label: Population growth per step
caption: The land at step 30 with each growth rate of the population (rule `population = population * 1.02` changed; each a real run with the subsidy). The numbers compare the supply of step 30 with its demand.
frames:
  - { value: "0%", src: /img/tutorials/land-change/coupled-agents/growth_0.0.png, stats: { "Food (demand)": "251 (250)", "Timber (demand)": "151 (150)", Unmanaged: "928 of 1,600" } }
  - { value: "1%", src: /img/tutorials/land-change/coupled-agents/growth_0.01.png, stats: { "Food (demand)": "338 (337)", "Timber (demand)": "202 (202)", Unmanaged: "699 of 1,600" } }
  - { value: "2%", src: /img/tutorials/land-change/coupled-agents/growth_0.02.png, stats: { "Food (demand)": "422 (453)", "Timber (demand)": "266 (272)", Unmanaged: "518 of 1,600" } }
  - { value: "3%", src: /img/tutorials/land-change/coupled-agents/growth_0.03.png, stats: { "Food (demand)": "528 (607)", "Timber (demand)": "340 (364)", Unmanaged: "284 of 1,600" } }
  - { value: "4%", src: /img/tutorials/land-change/coupled-agents/growth_0.04.png, stats: { "Food (demand)": "567 (811)", "Timber (demand)": "377 (487)", Unmanaged: "119 of 1,600" } }
start: 2
```

```tfchart
type: line
data: /data/tutorials/land-change/coupled-agents/growth_food.csv
x: step
y: ["demand 1%", "supply 1%", "demand 3%", "supply 3%", "demand 4%", "supply 4%"]
colors: { "demand 1%": "#9fd0b0", "supply 1%": "#3b8a5a", "demand 3%": "#f0d79f", "supply 3%": "#d9a441", "demand 4%": "#e3b3cf", "supply 4%": "#b4508a" }
xlabel: Step
ylabel: Food
height: 300
caption: The food demanded and supplied, step by step, with three growth rates.
```

With no growth the land settles in a few steps and more than half of it stays unmanaged; with 1% the supply keeps
up to the end. From 2% the food falls short in the last steps, and with 4% the demand at step 30 (811) passes what
the whole landscape could give if every cell were farmed (722 with the subsidy, the sum of the squared fertility):
the model can only show the gap. The number that sets the demand of the whole model is one line of the macro
agents' rules, and the landscape's response to it is not proportional: compute the most the land can produce of each
service before reading a shortfall as a result.

### Other numbers to move

- **The subsidy** (3) and its start (step 6): how strong a subsidy must be, and for how long, to draw farmers
  to poor soil.
- **The needs per head** (0.25 and 0.15): they scale the demand as the growth does.
- **CRAFTY's settings** (`search`, `giving_up`, `giving_in`, `giving_up_rate`): [CRAFTY's part](/tutorials/land-change/crafty/#settings-that-change-the-answer)
  shows each.
- **The seed** of `alloc` (3): which cells are offered and who gives up. Run several
  (`terraflux submit --replicates`) and report the range.

## Common pitfalls

- **Reading a population from outside the loop.** `grow` must read `@people`: reading `people.agents` would start
  every step from 1,000 people, and the demand would never grow.
- **Demand columns that CRAFTY cannot find.** [[agents.summary]] names its columns `<attribute>_sum` and
  `<attribute>_mean`; `services` must map each service to one of them. A service with no column is not demanded.
- **A subsidy painted on the wrong grid.** [[agents.map]] paints on the parcels map it is given; it must be the map
  the agents were made from, on the grid of the capitals.
- **Capitals above 1.** A subsidy that multiplies a capital can pass 1; the `min(..., 1)` of `fert` keeps it in
  CRAFTY's range.
- **Expecting a local policy to change a fixed demand.** With the demand set elsewhere, a restriction or a subsidy
  moves land use, it does not remove it (the councils above). Say so when reporting a policy's effect.
- **Decisions without memory.** The managers here decide from the step number alone. Rules that should depend on
  last step (wealth, a decision once taken) need the agents carried by the loop.

## Going further

- [Dynamic drivers](/tutorials/land-change/dynamic-drivers/), [agents](/tutorials/land-change/agents/) and
  [CRAFTY](/tutorials/land-change/crafty/): the earlier parts of this series.
- [CLUE-S's rules](/tutorials/land-change/clue-s-rules/): restrictions, conversion matrices and neighbourhoods, the
  ways agents' maps can enter CLUE-S.
- [A demand per region](/tutorials/land-change/regional-demand/): a demand for each council instead of one for the
  map.
- The operators: [[agents.table]], [[agents.parcels]], [[agents.update]], [[agents.summary]], [[agents.map]],
  [[crafty.allocate]], [[stack.set]], [[lulcc.step]].

## References

- An, L. (2012). Modeling human decisions in coupled human and natural systems: review of agent-based models. *Ecological Modelling*, 229, 25–36. https://doi.org/10.1016/j.ecolmodel.2011.07.010
- Grimm, V., Railsback, S. F., Vincenot, C. E., Berger, U., Gallagher, C., DeAngelis, D. L., et al. (2020). The ODD protocol for describing agent-based and other simulation models: a second update to improve clarity, replication, and structural realism. *Journal of Artificial Societies and Social Simulation*, 23(2), 7. https://doi.org/10.18564/jasss.4259
- Meyfroidt, P., Rudel, T. K., & Lambin, E. F. (2010). Forest transitions, trade, and the global displacement of land use. *Proceedings of the National Academy of Sciences*, 107(49), 20917–20922. https://doi.org/10.1073/pnas.1014773107
- Murray-Rust, D., Brown, C., van Vliet, J., Alam, S. J., Robinson, D. T., Verburg, P. H., & Rounsevell, M. (2014). Combining agent functional types, capitals and services to model land use dynamics. *Environmental Modelling & Software*, 59, 187–201. https://doi.org/10.1016/j.envsoft.2014.05.019
- Rounsevell, M. D. A., Robinson, D. T., & Murray-Rust, D. (2012). From actors to agents in socio-ecological systems models. *Philosophical Transactions of the Royal Society B*, 367(1586), 259–269. https://doi.org/10.1098/rstb.2011.0187
- Verburg, P. H. (2006). Simulating feedbacks in land use and land cover change models. *Landscape Ecology*, 21(8), 1171–1183. https://doi.org/10.1007/s10980-006-0029-4
- Verburg, P. H., Eickhout, B., & van Meijl, H. (2008). A multi-scale, multi-model approach for analyzing the future dynamics of European land use. *The Annals of Regional Science*, 42(1), 57–77. https://doi.org/10.1007/s00168-007-0136-4
