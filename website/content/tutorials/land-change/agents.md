---
title: "Agents: decision makers inside the loop"
short: Agents
description: Give a land change model its decision makers, a farmer per parcel whose wealth follows the soil and the market, and watch the land they manage change with their decisions.
order: 710
level: intermediate
time: 40 min
ops: [agents.parcels, agents.update, agents.summary, agents.map, agents.table, control.repeat, nlm.mosaic_tess, nlm.gaussian_field]
series: Dynamic drivers and agents
part: 2
---

Land does not change by itself: farmers clear it, abandon it, sell it; councils zone it; companies plant it.
Most land change models describe the outcome of those decisions statistically, as a suitability and a demand.
**Agent-based models** describe the decision makers themselves: each has a state (its wealth, its land, its
age) and rules for changing it, and the land changes because they act. In this part you will build, node by
node, the template *Farmers on their parcels (agents)*: sixty farmers, one per parcel, whose wealth follows the
fertility of their soil and a price that falls year after year, and who stop farming when they are broke. You
will learn the five operators for agents in Terraflux, how a population is carried through a loop like a map,
how to read what the agents did, and how sensitive the outcome is to one number in their rules. At the end you
will be able to write agents of your own and connect them to the land change models of this category.

## The idea

### Agents, populations and rules

An **agent** is a decision maker represented individually: a household, a farm, a firm, a municipality. An
agent-based model of land use couples a population of agents with a map: the agents decide, their decisions
change the land, and the land feeds back on their next decisions (Parker et al., 2003). Since the 1990s such
models have been used to study deforestation by colonists, the abandonment of farmland, the spread of
suburbs and the effect of subsidies on what farmers grow (Matthews et al., 2007). Their appeal for an ecologist
is that they can ask *who* changes the land and *why*: what happens to a forest when the price of beef falls,
when only large farms get credit, when young people leave. Their difficulty is that human decisions are hard to
observe and to calibrate (An, 2012; Rounsevell et al., 2012).

In Terraflux a **population** is a table: a row per agent, a column per **attribute** (a number: `id`,
`wealth`, `farming`...). Five operators make and change populations:

| Operator | What it does |
|---|---|
| [[agents.table]] | agents written in the model, such as a few countries or markets |
| [[agents.parcels]] | an agent per parcel of a map of parcel ids, with its size and the mean of other maps over its parcel |
| [[agents.update]] | the agents' behaviour: rules `attribute = expression`, applied in order to every agent |
| [[agents.map]] | an attribute painted on the agents' parcels, as a map |
| [[agents.summary]] | the number of agents and the sum and mean of every attribute, as a table of one row |

A population is a value like a map, so a [[control.repeat]] loop can **carry** it: at every step the body reads
the population of the step before (`@farmers`) and gives the population of this step, which the next step will
read.

!!! science "Key idea: the rules of the farmers"
    Every year \(t\), each farmer \(a\) with a parcel of mean fertility \(f_a\) (0 to 1) applies, in order,

    \[ p_t = 10 - 0.25\,t \]

    \[ y_{a,t} = \begin{cases} 2\, f_a\, p_t & \text{if the farmer farms} \\ 1 & \text{otherwise} \end{cases} \]

    \[ w_{a,t} = w_{a,t-1} + y_{a,t} - 7 \]

    \[ \text{farming}_{a,t} = \text{farming}_{a,t-1} \text{ if } w_{a,t} > 0, \text{ else } 0 \]

    \(p_t\) is the price of the crop, falling by a quarter every year; \(y_{a,t}\) the income; \(w_{a,t}\) the
    wealth, starting at 20; and 7 a yearly cost of living. A farmer breaks even when \(2 f_a p_t = 7\), that
    is when \(f_a = 3.5 / p_t\): a fertility of 0.36 in the first year (price 9.75), of 0.70 in the twentieth
    (price 5). As the price falls, the break-even fertility rises past more and more farms.

### Agents and the map

The farmers here act on the map through one attribute, `farming`, which [[agents.map]] paints on their parcels
every year: a map of the parcels still farmed. In a full land change model the same map would become a
restriction, a multiplier of a transition probability or the land of a CRAFTY simulation (see
[CRAFTY](/tutorials/land-change/crafty/) and [a coupled model](/tutorials/land-change/coupled-agents/)).

## When to use it

Use agents when the question is about the decision makers: how many farmers would give up under a price, a
tax, a subsidy; which parcels a council would protect; how the size of farms changes what is grown. Valbuena
et al. (2010), for example, simulated the farmers of a Dutch region as agents of a few types, each with its own
rules for expanding, abandoning or diversifying, to see how the landscape would change under policies.

Agents are *not* needed when only the amount and location of change matter and their causes can stay
statistical: a Markov chain and weights of evidence (Dinamica EGO), or a demand and logistic regressions
(CLUE-S, [the first series](/tutorials/land-change/clue-s-ideas/)), are easier to fit and validate. They are
also not a way to add precision: a model of agents whose rules are guessed is less certain, not more, than a
statistical one fitted on maps. Choose them when the decisions are the subject.

Terraflux's agents are deliberately simple: rules are expressions over numbers, applied to every agent in the
same order; agents do not see each other or trade with each other within a step. For agents that negotiate,
learn or move, a dedicated platform (NetLogo, Mesa, GAMA) is the better tool; Terraflux's agents fit models
where a population of decision makers modifies maps that a land change model then uses.

## What you need

- **A map of parcels**: a raster whose cells hold the id of the parcel (the farm, the property, the
  municipality) they belong to, missing where none. A cadastre rasterized with [[vector.rasterize]] is typical.
  The template draws one: a Voronoi mosaic of 60 parcels.
- **Maps of what matters to the decisions**: here the soil's fertility, 0 to 1. [[agents.parcels]] gives each
  agent the mean of each map over its parcel.
- **Rules**: what each agent knows and does each step. They are the hypothesis of the model, and should come
  from interviews, surveys, economic data or the literature.

The template *Farmers on their parcels (agents)* (in the application, **Templates**) has everything drawn by
chance, so it runs as it is.

## Build the model

### Step 1: the landscape and its parcels

```tfgraph
#! caption: The landscape: a Voronoi mosaic made into parcel ids, and the fertility of the soil.
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "mosaic"
op = "nlm.mosaic_tess"
params = { rows = 120, columns = 120, germs = 60, seed = 51 }

[[nodes]]
id = "parcels"
op = "raster.calc"
inputs = { m = "mosaic.map" }
params = { expression = "1 + floor(m * 100000)" }

[[nodes]]
id = "fertility"
op = "nlm.gaussian_field"
params = { rows = 120, columns = 120, range = 25, seed = 52 }
```

1. **`mosaic`** ([[nlm.mosaic_tess]]) draws a Voronoi mosaic of 60 cells on a grid of 120 x 120: every
   cell takes the random value of its nearest of 60 random points, so the map is made of 60 polygons.
2. **`parcels`** ([[raster.calc]]) turns each polygon's value (0 to 1) into a whole number, its parcel id. With
   a real cadastre this node and the one before are a [[raster.read]] of the parcel ids.
3. **`fertility`** ([[nlm.gaussian_field]]) draws a smooth random surface from 0 to 1, the soil's fertility,
   with patches about 25 cells across.

```tfgallery
columns: 3
caption: The landscape of the template. Left, the 60 parcels (random colours); middle, the soil's fertility; right, the mean fertility of each parcel, what each farmer knows of its land.
images:
  - { src: /img/tutorials/land-change/agents/parcels.png, caption: "Parcels" }
  - { src: /img/tutorials/land-change/agents/fertility.png, caption: "Fertility" }
  - { src: /img/tutorials/land-change/agents/parcel_fertility.png, caption: "Fertility per parcel" }
```

### Step 2: the agents

```tfgraph
#! caption: An agent per parcel, with the mean fertility of its land, then the attributes every farmer starts with.
#! highlight: farms, start
format = "terraflux/1"

[[nodes]]
id = "mosaic"
op = "nlm.mosaic_tess"
params = { rows = 120, columns = 120, germs = 60, seed = 51 }

[[nodes]]
id = "parcels"
op = "raster.calc"
inputs = { m = "mosaic.map" }
params = { expression = "1 + floor(m * 100000)" }

[[nodes]]
id = "fertility"
op = "nlm.gaussian_field"
params = { rows = 120, columns = 120, range = 25, seed = 52 }

[[nodes]]
id = "farms"
op = "agents.parcels"
inputs = { parcels = "parcels.map", fertility = "fertility.map" }

[[nodes]]
id = "start"
op = "agents.update"
inputs = { agents = "farms.agents" }
params = { rules = "wealth = 20\nfarming = 1" }
```

- **`farms`** ([[agents.parcels]]) makes an agent per parcel id found in `parcels.map`, in increasing order of
  id, with the columns `id` and `cells` (the parcel's size in cells). Every other map connected to it becomes a
  column of the same name holding the mean of the map over the parcel: the input is named `fertility`, so each
  farmer gets `fertility`. In the application, after adding the node, type a name in the Node pane to add an
  input, then drag `fertility.map` onto it. Connect as many maps as the rules need (slope, distance to the
  market, a soil class...).
- **`start`** ([[agents.update]]) gives every farmer its starting attributes. A rule's left side is an
  attribute, new or existing; its right side an expression of map algebra (see
  [map algebra](/tutorials/#building-models)) over the agent's attributes and the node's number inputs. Here
  both rules are constants: every farmer starts with a wealth of 20 and farming.

Rules are lines of `attribute = expression`, applied **in order**, each line seeing the result of the lines
before it, to every agent separately. The expressions are those of [[raster.calc]] without the neighbourhood
functions: arithmetic, comparisons (1 true, 0 false), `and`, `or`, `not`, `... if ... else ...`, `min`,
`max`, `clamp`, `exp`, `log`, `round`... For instance:

```text
price = 10 - step * 0.25
income = fertility * price * 2 if farming == 1 else 1
wealth = wealth + income - 7
farming = farming if wealth > 0 else 0
```

`price` is a new attribute, the same for every farmer since it depends only on `step`; `income` reads the
`price` just computed; `wealth` the `income`; and `farming` the new `wealth`. Swap the last two lines and a
farmer would decide on last year's wealth.

### Step 3: the years

```tfgraph
#! caption: The loop of 20 years. Its body runs the year's rules on @farmers (the farmers of the year before), sums them up and paints who still farms; carry passes the farmers on, collect keeps a table and a map per year.
#! highlight: year, totals, farmed
format = "terraflux/1"

[[nodes]]
id = "mosaic"
op = "nlm.mosaic_tess"
params = { rows = 120, columns = 120, germs = 60, seed = 51 }

[[nodes]]
id = "parcels"
op = "raster.calc"
inputs = { m = "mosaic.map" }
params = { expression = "1 + floor(m * 100000)" }

[[nodes]]
id = "fertility"
op = "nlm.gaussian_field"
params = { rows = 120, columns = 120, range = 25, seed = 52 }

[[nodes]]
id = "farms"
op = "agents.parcels"
inputs = { parcels = "parcels.map", fertility = "fertility.map" }

[[nodes]]
id = "start"
op = "agents.update"
inputs = { agents = "farms.agents" }
params = { rules = "wealth = 20\nfarming = 1" }

[[nodes]]
id = "years"
op = "control.repeat"
inputs = { times = 20, farmers = "start.agents" }

[nodes.body]
carry = { farmers = "year.agents" }
collect = { summary = "totals.table", maps = "farmed.map" }

[[nodes.body.nodes]]
id = "year"
op = "agents.update"
inputs = { agents = "@farmers", step = "@step" }
params = { rules = "price = 10 - step * 0.25\nincome = fertility * price * 2 if farming == 1 else 1\nwealth = wealth + income - 7\nfarming = farming if wealth > 0 else 0" }

[[nodes.body.nodes]]
id = "totals"
op = "agents.summary"
inputs = { agents = "year.agents" }

[[nodes.body.nodes]]
id = "farmed"
op = "agents.map"
inputs = { agents = "year.agents", parcels = "parcels.map" }
params = { column = "farming" }
```

The loop `years` ([[control.repeat]]) has two inputs: `times = 20` and `farmers = start.agents`. Any input
other than `times` is a **carried value**: inside the body it is `@farmers`, the starting farmers at step 1 and,
at every later step, what `carry = { farmers = "year.agents" }` says: the farmers after the year's rules. The
body:

1. **`year`** ([[agents.update]]) applies the four rules above to `@farmers`. Its second input, `step`,
   connected to `@step`, is a number input: the rules can use it by that name. Any number can be connected
   this way (a price from a table, a policy switch), named as the rules call it.
2. **`totals`** ([[agents.summary]]) sums the farmers up: a table of one row with `n` (the number of agents),
   then `<attribute>_sum` and `<attribute>_mean` for every attribute (`farming_sum` is the number still
   farming, `wealth_mean` their mean wealth).
3. **`farmed`** ([[agents.map]]) paints the attribute `farming` of each farmer on the cells of its parcel:
   1 farmed, 0 given up, missing outside any parcel.

`collect = { summary = "totals.table", maps = "farmed.map" }` keeps both every year: `years.summary` is a
series of 20 tables (written as one CSV, a row per year) and `years.maps` a series of 20 maps (written as a
raster of 20 bands). After the loop, `years.farmers` is the population of the last year.

!!! warning "Read @farmers, not start.agents, inside the body"
    If `year` read `start.agents` instead of `@farmers`, every year would start again from the first year's
    farmers: the loop would run, but nothing would accumulate. The carried value is the only way a step sees
    what the step before did.

!!! app "In the application"
    Open the template from **Templates**, double-click `years` to see its body, and click `year` to read and
    edit its rules in the Node pane. To carry a new value, add an input to the loop (its name becomes `@name`
    inside) and set what it takes at the end of each step in the loop's **carry**. Press ++ctrl+enter++ on the
    loop to run up to it; the plots `plot_farming`, `plot_wealth` and `plot_fertility` open as tabs.

??? note "The whole template"

    ```tfgraph
    #! caption: The template Farmers on their parcels (agents): after the loop, the wealth of the last year painted on the parcels, and three maps shown in tabs.
    #! highlight: wealth
    #! params: false
    format = "terraflux/1"

    [[nodes]]
    id = "mosaic"
    op = "nlm.mosaic_tess"
    params = { rows = 120, columns = 120, germs = 60, seed = 51 }

    [[nodes]]
    id = "parcels"
    op = "raster.calc"
    inputs = { m = "mosaic.map" }
    params = { expression = "1 + floor(m * 100000)" }

    [[nodes]]
    id = "fertility"
    op = "nlm.gaussian_field"
    params = { rows = 120, columns = 120, range = 25, seed = 52 }

    [[nodes]]
    id = "farms"
    op = "agents.parcels"
    inputs = { parcels = "parcels.map", fertility = "fertility.map" }

    [[nodes]]
    id = "start"
    op = "agents.update"
    inputs = { agents = "farms.agents" }
    params = { rules = "wealth = 20\nfarming = 1" }

    [[nodes]]
    id = "years"
    op = "control.repeat"
    inputs = { times = 20, farmers = "start.agents" }

    [nodes.body]
    carry = { farmers = "year.agents" }
    collect = { summary = "totals.table", maps = "farmed.map" }

    [[nodes.body.nodes]]
    id = "year"
    op = "agents.update"
    inputs = { agents = "@farmers", step = "@step" }
    params = { rules = "price = 10 - step * 0.25\nincome = fertility * price * 2 if farming == 1 else 1\nwealth = wealth + income - 7\nfarming = farming if wealth > 0 else 0" }

    [[nodes.body.nodes]]
    id = "totals"
    op = "agents.summary"
    inputs = { agents = "year.agents" }

    [[nodes.body.nodes]]
    id = "farmed"
    op = "agents.map"
    inputs = { agents = "year.agents", parcels = "parcels.map" }
    params = { column = "farming" }

    [[nodes]]
    id = "wealth"
    op = "agents.map"
    inputs = { agents = "years.farmers", parcels = "parcels.map" }
    params = { column = "wealth" }

    [[nodes]]
    id = "plot_wealth"
    op = "view.plot"
    inputs = { value = "wealth.map" }
    params = { title = "Wealth after 20 years" }

    [[nodes]]
    id = "plot_fertility"
    op = "view.plot"
    inputs = { value = "fertility.map" }
    params = { title = "Fertility" }

    [[nodes]]
    id = "plot_farming"
    op = "view.plot"
    inputs = { value = "years.maps" }
    params = { title = "Parcels still farmed, year by year" }
    ```

From the command line, the series can be written as files by adding outputs to the model file:

```toml
[outputs.summary]
from = "years.summary"     # summary.csv, a row per year

[outputs.farmers]
from = "years.farmers"     # farmers.csv, a row per farmer after the last year
```

```bash
terraflux run farmers.toml --out farmers
```

The run takes a few hundredths of a second.

## Read the results

### Year by year

```tfslider
label: Year
caption: The parcels still farmed (green) and given up (ochre) each year. The chart follows the number of farmers and their mean wealth (all 60 agents, farming or not).
frames:
  - { value: 1, src: /img/tutorials/land-change/agents/year_1.png, caption: "Year 1", stats: { Farming: "60 of 60", "Mean wealth": "23.0", Price: "9.75" } }
  - { value: 2, src: /img/tutorials/land-change/agents/year_2.png, caption: "Year 2", stats: { Farming: "60 of 60", "Mean wealth": "25.7", Price: "9.50" } }
  - { value: 3, src: /img/tutorials/land-change/agents/year_3.png, caption: "Year 3", stats: { Farming: "60 of 60", "Mean wealth": "28.2", Price: "9.25" } }
  - { value: 4, src: /img/tutorials/land-change/agents/year_4.png, caption: "Year 4", stats: { Farming: "60 of 60", "Mean wealth": "30.4", Price: "9.00" } }
  - { value: 5, src: /img/tutorials/land-change/agents/year_5.png, caption: "Year 5", stats: { Farming: "60 of 60", "Mean wealth": "32.4", Price: "8.75" } }
  - { value: 6, src: /img/tutorials/land-change/agents/year_6.png, caption: "Year 6", stats: { Farming: "59 of 60", "Mean wealth": "34.1", Price: "8.50" } }
  - { value: 7, src: /img/tutorials/land-change/agents/year_7.png, caption: "Year 7", stats: { Farming: "59 of 60", "Mean wealth": "35.5", Price: "8.25" } }
  - { value: 8, src: /img/tutorials/land-change/agents/year_8.png, caption: "Year 8", stats: { Farming: "58 of 60", "Mean wealth": "36.7", Price: "8.00" } }
  - { value: 9, src: /img/tutorials/land-change/agents/year_9.png, caption: "Year 9", stats: { Farming: "56 of 60", "Mean wealth": "37.5", Price: "7.75" } }
  - { value: 10, src: /img/tutorials/land-change/agents/year_10.png, caption: "Year 10", stats: { Farming: "55 of 60", "Mean wealth": "38.0", Price: "7.50" } }
  - { value: 11, src: /img/tutorials/land-change/agents/year_11.png, caption: "Year 11", stats: { Farming: "53 of 60", "Mean wealth": "38.2", Price: "7.25" } }
  - { value: 12, src: /img/tutorials/land-change/agents/year_12.png, caption: "Year 12", stats: { Farming: "53 of 60", "Mean wealth": "38.1", Price: "7.00" } }
  - { value: 13, src: /img/tutorials/land-change/agents/year_13.png, caption: "Year 13", stats: { Farming: "52 of 60", "Mean wealth": "37.7", Price: "6.75" } }
  - { value: 14, src: /img/tutorials/land-change/agents/year_14.png, caption: "Year 14", stats: { Farming: "52 of 60", "Mean wealth": "37.0", Price: "6.50" } }
  - { value: 15, src: /img/tutorials/land-change/agents/year_15.png, caption: "Year 15", stats: { Farming: "51 of 60", "Mean wealth": "36.1", Price: "6.25" } }
  - { value: 16, src: /img/tutorials/land-change/agents/year_16.png, caption: "Year 16", stats: { Farming: "50 of 60", "Mean wealth": "34.9", Price: "6.00" } }
  - { value: 17, src: /img/tutorials/land-change/agents/year_17.png, caption: "Year 17", stats: { Farming: "50 of 60", "Mean wealth": "33.4", Price: "5.75" } }
  - { value: 18, src: /img/tutorials/land-change/agents/year_18.png, caption: "Year 18", stats: { Farming: "48 of 60", "Mean wealth": "31.7", Price: "5.50" } }
  - { value: 19, src: /img/tutorials/land-change/agents/year_19.png, caption: "Year 19", stats: { Farming: "44 of 60", "Mean wealth": "29.6", Price: "5.25" } }
  - { value: 20, src: /img/tutorials/land-change/agents/year_20.png, caption: "Year 20", stats: { Farming: "43 of 60", "Mean wealth": "27.2", Price: "5.00" } }
chart:
  type: line
  data: /data/tutorials/land-change/agents/years.csv
  x: year
  y: [farming, wealth]
  labels: { farming: "Farmers still farming", wealth: "Mean wealth" }
  colors: { farming: "#3b8a5a", wealth: "#e8901f" }
  xlabel: Year
  ylabel: Farmers / wealth
```

For five years every farmer survives: even those on poor soil lose money, but they started with a wealth of 20.
The first gives up in year 6, on the poorest parcel (mean fertility 0.20). From then on farmers leave steadily,
and faster at the end, when the price has fallen so far that farms of middling fertility lose money every year:
by year 20, 43 of 60 still farm. The mean wealth rises for ten years and then falls, because those who quit
keep paying their cost of living while earning 1 elsewhere, and because the price falls for those who stay.

### Who gave up

![Fertility and wealth of every farmer after 20 years](/img/tutorials/land-change/agents/fertility_wealth.png "Each dot a farmer after 20 years: the mean fertility of its parcel and its wealth. Every farmer below a fertility of about 0.40 gave up (ochre); every one above it is still farming (green), richer the better its soil.")

The 17 farmers who gave up are exactly the 17 with the poorest soil (mean fertility 0.20 to 0.40). The rules
contain nothing else that differs between farmers, so the pattern of abandonment is the pattern of soil: the
map of parcels farmed in year 20 is a map of fertility cut at a threshold. That is a check that the model does
what its rules say, and a warning: an agent-based model whose agents differ in one attribute only produces a
threshold of that attribute. Real farmers differ in their age, debts, skills, family and alternatives, and
models give agents several attributes, often drawn from surveys, so that farms of the same soil can make
different choices (Valbuena et al., 2010).

![Wealth of each farmer after 20 years](/img/tutorials/land-change/agents/wealth.png "The wealth after 20 years painted on the parcels (wealth.map): red, in debt; blue, the richest. The poorest farmers kept losing money after they quit, a property of the rules (below).")

### How to report it

An agent-based model must be described fully enough to be rebuilt: the agents, their attributes, the rules in
order, the time step, what is drawn at random, and what is observed. The **ODD protocol** (Overview, Design
concepts, Details) is the standard way to do it (Grimm et al., 2006, updated by Grimm et al., 2020). In
Terraflux the model file itself holds the rules, so publish it with the paper. Report the outcomes as
distributions over runs with different parameters, as below, not as one run.

## Settings that change the answer

The yearly cost of 7 is a guess, as most numbers in agent rules are. Move the slider to see the parcels still
farmed after 20 years with a cost of 5 to 9 (each a real run of the template with that number changed in the
rule `wealth = wealth + income - 7`):

```tfslider
label: Yearly cost
caption: The parcels still farmed after 20 years (green) with each yearly cost.
frames:
  - { value: 5, src: /img/tutorials/land-change/agents/cost_5.png, stats: { "Farming after 20 years": "56 of 60" } }
  - { value: 6, src: /img/tutorials/land-change/agents/cost_6.png, stats: { "Farming after 20 years": "52 of 60" } }
  - { value: 7, src: /img/tutorials/land-change/agents/cost_7.png, stats: { "Farming after 20 years": "43 of 60" } }
  - { value: 8, src: /img/tutorials/land-change/agents/cost_8.png, stats: { "Farming after 20 years": "34 of 60" } }
  - { value: 9, src: /img/tutorials/land-change/agents/cost_9.png, stats: { "Farming after 20 years": "27 of 60" } }
start: 2
```

```tfchart
type: line
data: /data/tutorials/land-change/agents/cost_curves.csv
x: year
y: ["cost 5", "cost 6", "cost 7", "cost 8", "cost 9"]
colors: { "cost 5": "#1f5f3a", "cost 6": "#3b8a5a", "cost 7": "#7fae6a", "cost 8": "#d9a441", "cost 9": "#b4508a" }
xlabel: Year
ylabel: Farmers still farming
height: 300
caption: The number of farmers year by year with each yearly cost. A cost of 9 instead of 7 leaves 27 farmers after 20 years instead of 43.
```

Every unit of cost moves the break-even fertility up by \(1 / (2 p_t)\), about 0.1 at the end of the run, and
the soils of this landscape are spread around the middle (half the parcels have a mean fertility between 0.39
and 0.63), so a small change in cost moves many farms across the line. The outcome is sensitive to a number
nobody measured: such a model is a tool for exploring "what if" (what if costs rise by a third?), not for
predicting how many farmers there will be. Run a range of values, as here, and report how the conclusions
change across it.

Other settings worth exploring in the template: the starting wealth (a buffer that delays abandonment), the
slope of the price, and the income of those who quit.

## Common pitfalls

**Rules in the wrong order.** Each line sees the lines above it. `farming = ...` before `wealth = ...` decides
on last year's wealth. Read the rules top to bottom as a sequence of steps, not as equations.

**Reading the start instead of the carried value.** Inside the body, read `@farmers`; reading
`start.agents` restarts from the first year at every step (see the warning above).

**Parcels without an agent, agents without a parcel.** [[agents.map]] paints only the parcels whose id an
agent has; other cells are missing. If parcel ids are not whole numbers (a mosaic's raw values), or if the map
of parcels is not the one the agents came from, the painted map is empty or patchy. Make ids whole numbers, as
the template's `parcels` node does, and paint on the same map.

**Rules that keep running after an agent leaves.** In the template, a farmer who quits still pays 7 and earns 1
every year, so its wealth falls without end (the red parcels above). That may be meant (debts) or not (a farmer
who sells the farm leaves the model). Make the intention explicit: `wealth = if farming == 1 then wealth +
income - 7 else wealth` keeps a quitter's wealth where it was.

**Mean values of a mixed population.** `wealth_mean` averages over all agents, farming or not. To follow the
farmers only, add an attribute (`farm_wealth = wealth if farming == 1 else 0`) and divide its sum by
`farming_sum`.

**One run is one story.** Rules with thresholds turn small differences into all-or-nothing outcomes. Explore
the parameters, and when rules draw random numbers, run replicates (`terraflux submit --replicates`).

## Going further

- [CRAFTY](/tutorials/land-change/crafty/) makes agents compete for land: agent functional types that
  produce services from capitals and take over the cells where they are most competitive.
- [A coupled model](/tutorials/land-change/coupled-agents/) puts macro agents, parcel managers and CRAFTY in
  one loop, and shows how agents can freeze land in a CLUE-S model.
- [Dynamic drivers](/tutorials/land-change/dynamic-drivers/), the first part, explains the loop's carry and
  collect in a land change model.
- The operators: [[agents.parcels]], [[agents.update]], [[agents.summary]], [[agents.map]], [[agents.table]].

## References

- An, L. (2012). Modeling human decisions in coupled human and natural systems: review of agent-based models. *Ecological Modelling*, 229, 25–36. https://doi.org/10.1016/j.ecolmodel.2011.07.010
- Grimm, V., Berger, U., Bastiansen, F., Eliassen, S., Ginot, V., Giske, J., et al. (2006). A standard protocol for describing individual-based and agent-based models. *Ecological Modelling*, 198(1–2), 115–126. https://doi.org/10.1016/j.ecolmodel.2006.04.023
- Grimm, V., Railsback, S. F., Vincenot, C. E., Berger, U., Gallagher, C., DeAngelis, D. L., et al. (2020). The ODD protocol for describing agent-based and other simulation models: a second update to improve clarity, replication, and structural realism. *Journal of Artificial Societies and Social Simulation*, 23(2), 7. https://doi.org/10.18564/jasss.4259
- Matthews, R. B., Gilbert, N. G., Roach, A., Polhill, J. G., & Gotts, N. M. (2007). Agent-based land-use models: a review of applications. *Landscape Ecology*, 22(10), 1447–1459. https://doi.org/10.1007/s10980-007-9135-1
- Parker, D. C., Manson, S. M., Janssen, M. A., Hoffmann, M. J., & Deadman, P. (2003). Multi-agent systems for the simulation of land-use and land-cover change: a review. *Annals of the Association of American Geographers*, 93(2), 314–337. https://doi.org/10.1111/1467-8306.9302004
- Rounsevell, M. D. A., Robinson, D. T., & Murray-Rust, D. (2012). From actors to agents in socio-ecological systems models. *Philosophical Transactions of the Royal Society B*, 367(1586), 259–269. https://doi.org/10.1098/rstb.2011.0187
- Valbuena, D., Verburg, P. H., Bregt, A. K., & Ligtenberg, A. (2010). An agent-based approach to model land-use change at a regional scale. *Landscape Ecology*, 25(2), 185–199. https://doi.org/10.1007/s10980-009-9380-6
