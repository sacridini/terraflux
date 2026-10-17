---
title: "Roads that grow: Dinamica EGO's endogenous road constructor"
short: Roads that grow
description: Build new roads every year from the existing network towards attractive forest, down the cheapest way over a friction surface, and use the distance to the growing network as a dynamic driver of deforestation, as in DINAMICA's Amazon scenario models.
order: 140
level: advanced
time: 50 min
ops: [dinamica.roads, dinamica.cost_map, dinamica.destinations, dinamica.pathways, dinamica.step, dynamic.distance_to, control.repeat, map.calc, stack.of, markov.matrix, patches.settings]
series: Dinamica EGO in Terraflux
part: 5
---

On a tropical frontier, deforestation follows the roads, and the roads follow the deforestation. A highway brings
settlers; loggers and settlers open side roads into the forest; along each side road the forest is cleared, and
from the clearings new roads push farther. A model that keeps the roads of the first year fixed for thirty
years misses this feedback: its clearing piles up along the old roads, while on the ground it spreads in a
branching pattern deep into the forest. In this part you will build roads the way DINAMICA's Amazon models do,
with Dinamica EGO's **road constructor**: first one step by hand with its three pieces
([[dinamica.destinations]], [[dinamica.cost_map]] and [[dinamica.pathways]]), then with the single node
[[dinamica.roads]] inside a loop where the distance to the growing network drives the clearing of
[[dinamica.step]]. You will compare it with the same model whose roads never grow, and see what the settings of
the road constructor (the distance range, the quadrants, the friction, a protected area) do to the forest that
is left. At the end you will be able to add endogenous roads to your own land change models and to read what
they change.

## The idea

### Roads on a frontier

Roads are the strongest single driver of tropical deforestation: they make land worth clearing by making it
reachable, and they cut the forest themselves (Laurance et al., 2009). In the Brazilian Amazon, most roads are not
built by the state. Brandão and Souza (2006), mapping roads on Landsat images in the central-west of Pará between
1985 and 2001, found 25,196 km of roads, of which 20,769 km were **unofficial**, built without government
incentives, and their growth almost doubled in ten years, reaching more than 40 km per 10,000 km² a year in the
most active regions. Loggers open many of them to reach valuable timber (Arima et al., 2005; Walker et al.,
2013); settlers then follow.

Soares-Filho et al. (2004), simulating the paving of the BR-163 highway between Cuiabá and Santarém, coupled
DINAMICA with scenarios of road paving and governance, and DINAMICA's Amazon models built new side roads as part
of the simulation, the **endogenous road constructor**. The same family of models underlies the basin-wide
scenarios of Soares-Filho et al. (2006), which projected that current trends would eliminate 40% of the Amazon's
forests by 2050. In Dinamica EGO the constructor is made of three functors, which Terraflux has as separate
operators, and one operator that chains them:

| Dinamica EGO's functor | Terraflux | What it does |
|---|---|---|
| CalcDestinationMap | [[dinamica.destinations]] | draws the ends of the new roads, by attraction, in quadrants of the map |
| CalcCostMap | [[dinamica.cost_map]] | the cost of reaching every cell from the network over a friction surface |
| CalcPathwayMap | [[dinamica.pathways]] | a path from each end down the cost, to the network |
| (the road constructor) | [[dinamica.roads]] | one step of the three, with the distance to the new network |

Terraflux runs landschaft's clean-room versions, written from Dinamica EGO's documentation and black-box runs of
its version 8.13.

![One step of road building](/img/tutorials/land-change/roads-that-grow/one_step.png "One step of the road constructor on the map of the first year (a highway, black, with cleared land beside it). 1: the attraction is kept only 1.5 to 4 km from the roads. 2: the map is cut into 2 x 2 quadrants and one destination (orange) is drawn in each quadrant that has attractive cells: the eastern quadrants, more than 8 km from the highway, have none. 3: the cost of reaching every cell from the network over the friction. 4: from each destination a new road (red) runs down the cost to the network, avoiding rough ground (dark).")

!!! science "Key idea: a step of the road constructor"
    1. **Where a road may end.** The attraction \(A_i\) of each cell (how much a road builder wants to reach it:
       timber, good soil, the probability of clearing) is kept only where the distance \(d_i\) to the nearest
       road is within a range, \(d_{\min} \le d_i \le d_{\max}\) (`distance`, 1,500 to 3,000 m by default), and
       where a `mask` allows; 0 elsewhere.
    2. **Destinations.** The map is cut into `quadrants` (horizontal x vertical parts, 2 x 10 by default). In
       each, `per_quadrant` cells (1) are drawn in proportion to their attraction; a draw closer than `spacing`
       (1,000 m) to an earlier destination is drawn again, up to `max_attempts` (15) times.
    3. **The cost of reaching each cell.** From every road cell (cost 0), the cheapest path to each cell, where
       a step into cell \(j\) costs its friction \(f_j\), \(\sqrt 2\) times more on a diagonal:
       \(C_j = \min_{i \in N(j)} (C_i + f_j \, \delta_{ij})\), with \(\delta_{ij} = 1\) for a straight step and
       \(\sqrt 2\) for a diagonal one, over the eight neighbours \(N(j)\) (Dijkstra's algorithm).
    4. **Pathways.** From each destination a road goes to the cheapest of its eight neighbours, again and again,
       until it reaches a cell of cost 0, the network. Ties are drawn at random (`lottery`).

    The new road cells join the network, and the distance to the network is computed again: the **distance to
    the roads**, a driver that changes every year.

### The feedback

In a loop, the roads and the clearing feed each other. The attraction is computed from the land of this year (a
road ends in forest, not in cleared land); the roads grow towards it; the distance to the new roads raises the
probability of clearing beside them; the clearing of this year moves next year's attraction farther out. The road
constructor is therefore a **dynamic driver** in the sense of [Dynamic drivers](/tutorials/land-change/dynamic-drivers/),
with one difference: the roads are not recomputed from the map, they *accumulate*, so the network must be carried
from step to step like the map.

## When to use it

Use the road constructor when new roads are a consequence of the process you simulate and their pattern matters:

- **Agricultural and logging frontiers** in tropical forests, the use it was designed for.
- **Scenarios of governance**: what changes if roads may not end, or not pass, inside protected areas (the
  `mask` and the friction, below), as in the scenarios of Soares-Filho et al. (2004, 2006).
- **Fragmentation studies** that need the simulated forest's shape, not only its area: roads cut the forest into
  pieces and bring edges into its interior.

It is *not* the tool for roads that are planned (a highway to be paved in 2030): give those as maps, one per
period, read inside the loop. Nor is it a model of transport: the roads have no traffic or quality, only a
presence. For least-cost paths between given places (a corridor, a pipeline), see the operators of
[grid distances](/tutorials/#grid-distances). And if roads are just one more class of the land use map, the
distance to that class can be recomputed every step with [[dynamic.distance_to]] without building any roads.

## What you need

- **The network**: a map with a value other than 0 on the road cells (0 or no value elsewhere). Rasterize a
  road layer with [[vector.rasterize]].
- **The friction**: the effort to build a road across each cell, positive everywhere it can be crossed (1 on flat
  open ground, more on slopes, swamps, rivers); no value where a road can never go.
- **The attraction**: how much each cell is worth reaching (0 or no value: never), usually computed from the land
  use map of the step.
- Optionally, a **mask**: where roads may end (a value other than 0).
- **Square cells in a projected coordinate system**: the distances (`distance`, `spacing`) are in map units,
  metres in a UTM projection. The constructor refuses cells that are not square.

The models on this page draw everything by chance on a grid of 250 x 250 cells of 100 m (25 by 25 km) and run as
they are.

## Build the model

### Step 1: a frontier beside a highway

```tfgraph
#! caption: The data. A highway along column 40 (node 1) made into the network (node 5); the land cleared beside it (node 6); the friction of road building (node 7) and what the forest is worth (node 4).
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "highway"
op = "nlm.distance_gradient"
params = { rows = 250, columns = 250, resolution = 100, origin = [40, 40, 1, 250], nlmr_compat = false, rescale = false }
note = "The distance to a highway along column 40, in metres."

[[nodes]]
id = "noise"
op = "nlm.gaussian_field"
params = { rows = 250, columns = 250, resolution = 100, range = 15, seed = 21 }

[[nodes]]
id = "relief"
op = "nlm.gaussian_field"
params = { rows = 250, columns = 250, resolution = 100, range = 40, seed = 22 }

[[nodes]]
id = "value"
op = "nlm.gaussian_field"
params = { rows = 250, columns = 250, resolution = 100, range = 30, seed = 23 }
note = "What the forest is worth: timber, soil."

[[nodes]]
id = "roads0"
op = "map.calc"
inputs = { d = "highway.map" }
params = { expression = "if d == 0 then 1 else 0" }

[[nodes]]
id = "land0"
op = "map.calc"
inputs = { d = "highway.map", g = "noise.map" }
params = { expression = "if d < 500 + 2500 * g then 2 else 1" }
note = "1 forest, 2 cleared beside the highway."

[[nodes]]
id = "friction"
op = "map.calc"
inputs = { r = "relief.map" }
params = { expression = "1 + 30 * r * r * r" }
note = "Building is harder on rough ground."
```

1. **`highway`** ([[nlm.distance_gradient]]) gives the distance in metres to a highway running north to south 4
   km from the western edge.
2. **`noise`**, **`relief`** and **`value`** ([[nlm.gaussian_field]]) are three smooth random surfaces from 0 to
   1: an irregularity for the edge of the old clearing, the roughness of the ground, and what the forest is worth
   to the road builders.
3. **`roads0`** ([[map.calc]]) is the network: 1 on the highway, 0 elsewhere.
4. **`land0`** is the land at the start: forest (1), and cleared land (2) within 0.5 to 3 km of the highway,
   14.7% of the map.
5. **`friction`** is the effort of building: 1 on smooth ground, up to 31 on the roughest (the cube of the
   roughness keeps most of the map cheap and makes a few ridges expensive).

![The landscape at the start](/img/tutorials/land-change/roads-that-grow/start.png "The start: forest (green), the cleared land beside the highway (beige) and the highway (black).")

### Step 2: one step by hand

The model below adds the three functors (and, to compare, [[dinamica.roads]]) to the data of step 1. Only the
new nodes are drawn; the whole model is in the note under it.

```tfgraph
#! caption: One step of road building by hand: the attraction of the forest (attraction), kept 1.5 to 4 km from the roads (near, band); a destination per quadrant (ends); the cost from the network (cost); the paths down the cost (paths). The node roads does the same in one step.
#! numbered: true
#! fragment: true
[[nodes]]
id = "attraction"
op = "map.calc"
inputs = { land = "land0.map", v = "value.map" }
params = { expression = "if land == 1 then v else 0" }

[[nodes]]
id = "near"
op = "dynamic.distance_to"
inputs = { landscape = "roads0.map" }
params = { classes = 1 }

[[nodes]]
id = "band"
op = "map.calc"
inputs = { a = "attraction.map", d = "near.distance" }
params = { expression = "if d >= 1500 and d <= 4000 then a else null" }

[[nodes]]
id = "ends"
op = "dinamica.destinations"
inputs = { attraction = "band.map", seed = 1 }
params = { quadrants = [2, 2], min_distance = 2000 }

[[nodes]]
id = "cost"
op = "dinamica.cost_map"
inputs = { sources = "roads0.map", friction = "friction.map" }

[[nodes]]
id = "paths"
op = "dinamica.pathways"
inputs = { network = "roads0.map", sources = "ends.map", cost = "cost.cost", seed = 2 }
params = { lottery = true }

[[nodes]]
id = "roads"
op = "dinamica.roads"
inputs = { network = "roads0.map", friction = "friction.map", attraction = "attraction.map", seed = 1 }
params = { quadrants = [2, 2], distance = [1500, 4000], spacing = 2000 }
```

??? note "The whole model of step 2"

    ```tfgraph
    #! caption: The data of step 1 and one step of road building, by hand and with dinamica.roads.
    #! highlight: attraction, near, band, ends, cost, paths, roads
    #! params: false
    format = "terraflux/1"

    [[nodes]]
    id = "highway"
    op = "nlm.distance_gradient"
    params = { rows = 250, columns = 250, resolution = 100, origin = [40, 40, 1, 250], nlmr_compat = false, rescale = false }

    [[nodes]]
    id = "noise"
    op = "nlm.gaussian_field"
    params = { rows = 250, columns = 250, resolution = 100, range = 15, seed = 21 }

    [[nodes]]
    id = "relief"
    op = "nlm.gaussian_field"
    params = { rows = 250, columns = 250, resolution = 100, range = 40, seed = 22 }

    [[nodes]]
    id = "value"
    op = "nlm.gaussian_field"
    params = { rows = 250, columns = 250, resolution = 100, range = 30, seed = 23 }

    [[nodes]]
    id = "roads0"
    op = "map.calc"
    inputs = { d = "highway.map" }
    params = { expression = "if d == 0 then 1 else 0" }

    [[nodes]]
    id = "land0"
    op = "map.calc"
    inputs = { d = "highway.map", g = "noise.map" }
    params = { expression = "if d < 500 + 2500 * g then 2 else 1" }

    [[nodes]]
    id = "friction"
    op = "map.calc"
    inputs = { r = "relief.map" }
    params = { expression = "1 + 30 * r * r * r" }

    [[nodes]]
    id = "attraction"
    op = "map.calc"
    inputs = { land = "land0.map", v = "value.map" }
    params = { expression = "if land == 1 then v else 0" }

    [[nodes]]
    id = "near"
    op = "dynamic.distance_to"
    inputs = { landscape = "roads0.map" }
    params = { classes = 1 }

    [[nodes]]
    id = "band"
    op = "map.calc"
    inputs = { a = "attraction.map", d = "near.distance" }
    params = { expression = "if d >= 1500 and d <= 4000 then a else null" }

    [[nodes]]
    id = "ends"
    op = "dinamica.destinations"
    inputs = { attraction = "band.map", seed = 1 }
    params = { quadrants = [2, 2], min_distance = 2000 }

    [[nodes]]
    id = "cost"
    op = "dinamica.cost_map"
    inputs = { sources = "roads0.map", friction = "friction.map" }

    [[nodes]]
    id = "paths"
    op = "dinamica.pathways"
    inputs = { network = "roads0.map", sources = "ends.map", cost = "cost.cost", seed = 2 }
    params = { lottery = true }

    [[nodes]]
    id = "roads"
    op = "dinamica.roads"
    inputs = { network = "roads0.map", friction = "friction.map", attraction = "attraction.map", seed = 1 }
    params = { quadrants = [2, 2], distance = [1500, 4000], spacing = 2000 }
    ```

Node by node:

1. **`attraction`** ([[map.calc]]): the value of the forest where there is forest, 0 on cleared land. Road
   builders aim at forest; a probability of clearing, or of logging, would serve as well.
2. **`near`** ([[dynamic.distance_to]]) gives the straight-line distance in metres from every cell to the nearest
   road cell (class 1 of `roads0`).
3. **`band`** keeps the attraction only 1.5 to 4 km from the roads, and gives no value elsewhere. This is the
   reach of one step of building: roads advance a few kilometres a year, not across the map.
4. **`ends`** ([[dinamica.destinations]]) draws the destinations. `quadrants = [2, 2]` cuts the map into two
   parts across and two down; in each quadrant one cell (`per_quadrant`, 1) is drawn in proportion to its
   attraction, at least 2 km (`min_distance = 2000`, in map units) from the others. Its output has the
   attraction of the destinations and no value elsewhere. Here only the two western quadrants have cells in the
   band, so there are two destinations.
5. **`cost`** ([[dinamica.cost_map]]) computes, from every road cell (`sources`, any value other than 0), the
   cheapest accumulated friction to every cell, and the direction each cell is reached from. The costs grow
   slowly across smooth ground and quickly across rough ground.
6. **`paths`** ([[dinamica.pathways]]) walks from each destination (`sources = ends.map`) down the cost, cell by
   cell to the cheapest neighbour, until a cell of cost 0: a road cell. `lottery = true` draws ties at random. The
   output is the network with the new roads (value 1 by default), no value elsewhere: two new roads, 8 km in
   all.

The last node, **`roads`** ([[dinamica.roads]]), does steps 2 to 6 at once with `distance = [1500, 4000]` and
`spacing = 2000`, and gives the same network as `paths` (with no `step` input, its seed is used as it is for the
destinations, and the seed plus one for the lottery, as in the hand-made version). It also gives the distance to
the new network, in map units. Inside a loop you will use this node; the three functors are there when a model
needs something the constructor does not do: destinations from another rule, paths along a cost computed
differently, a network of another kind (canals, power lines).

### Step 3: twenty years of roads and clearing

```tfgraph
#! caption: Twenty years. The loop carries the land and the network; each year the attraction is the forest's value on the land of the year, dinamica.roads builds new roads, the probability of clearing falls with the distance to this year's roads, and dinamica.step allocates 1.5% of the forest in patches.
#! highlight: years
#! params: false
format = "terraflux/1"

[inputs.seed]
kind = "value"
value = 1

[[nodes]]
id = "highway"
op = "nlm.distance_gradient"
params = { rows = 250, columns = 250, resolution = 100, origin = [40, 40, 1, 250], nlmr_compat = false, rescale = false }

[[nodes]]
id = "noise"
op = "nlm.gaussian_field"
params = { rows = 250, columns = 250, resolution = 100, range = 15, seed = 21 }

[[nodes]]
id = "relief"
op = "nlm.gaussian_field"
params = { rows = 250, columns = 250, resolution = 100, range = 40, seed = 22 }

[[nodes]]
id = "value"
op = "nlm.gaussian_field"
params = { rows = 250, columns = 250, resolution = 100, range = 30, seed = 23 }

[[nodes]]
id = "roads0"
op = "map.calc"
inputs = { d = "highway.map" }
params = { expression = "if d == 0 then 1 else 0" }

[[nodes]]
id = "land0"
op = "map.calc"
inputs = { d = "highway.map", g = "noise.map" }
params = { expression = "if d < 500 + 2500 * g then 2 else 1" }

[[nodes]]
id = "friction"
op = "map.calc"
inputs = { r = "relief.map" }
params = { expression = "1 + 30 * r * r * r" }

[[nodes]]
id = "rates"
op = "markov.matrix"
params = { classes = [1, 2], single = [[0.985, 0.015], [0, 1]] }
note = "1.5% of the forest cleared a year."

[[nodes]]
id = "patches"
op = "patches.settings"
params = { transitions = [{ from = 1, to = 2, expansion = 0.7, expander = [12, 100, 1.5], patcher = [8, 50, 1.5] }] }

[[nodes]]
id = "years"
op = "control.repeat"
inputs = { times = 20, landscape = "land0.map", network = "roads0.map" }

[nodes.body]
carry = { landscape = "clearing.landscape", network = "roads.network" }
collect = { maps = "clearing.landscape", networks = "roads.network", info = "clearing.info" }

[[nodes.body.nodes]]
id = "attraction"
op = "map.calc"
inputs = { land = "@landscape", v = "value.map" }
params = { expression = "if land == 1 then v else 0" }

[[nodes.body.nodes]]
id = "roads"
op = "dinamica.roads"
inputs = { network = "@network", friction = "friction.map", attraction = "attraction.map", step = "@step", seed = "$seed" }
params = { quadrants = [2, 2], distance = [1500, 4000], spacing = 2000 }

[[nodes.body.nodes]]
id = "probability"
op = "map.calc"
inputs = { land = "@landscape", d = "roads.distance" }
params = { expression = "if land == 1 then exp(-d / 1000) else 0" }
note = "Clearing likely near the roads of this year."

[[nodes.body.nodes]]
id = "transitions"
op = "stack.of"
inputs = { 1_to_2 = "probability.map" }

[[nodes.body.nodes]]
id = "clearing"
op = "dinamica.step"
inputs = { landscape = "@landscape", probability = "transitions.stack", rates = "rates.markov", patches = "patches.patches", step = "@step", seed = "$seed" }
```

Outside the loop, besides the data, two nodes set the clearing as in the
[Expander and Patcher](/tutorials/land-change/expander-and-patcher/): **`rates`** ([[markov.matrix]]), a chain
of two classes in which 1.5% of the forest is cleared each year, and **`patches`** ([[patches.settings]]), 70% of
the clearing by the Expander (growing old clearings) in patches of 12 ha on average, the rest by the Patcher in new
patches of 8 ha.

The loop `years` ([[control.repeat]]) carries two values: `landscape` (from `land0.map`) and `network` (from
`roads0.map`). Its body, every year:

1. **`attraction`** reads `@landscape`: forest of this year only. As the forest is cleared, the attraction moves.
2. **`roads`** ([[dinamica.roads]]) builds from `@network`, the roads as last year left them, with `step = @step`
   and `seed = $seed` (the seed of the year is the seed plus the step times a large constant). Its `network`
   output is carried to next year; its `distance` output is the distance to the network *including* this year's
   roads.
3. **`probability`** turns that distance into a probability of clearing: \(e^{-d/1000}\) in the forest, 1 beside
   a road, 0.37 at 1 km, 0.05 at 3 km. In a real model this is a fitted relation: put the distance into the
   drivers under the name the weights or the regression know, with [[stack.set]], and compute
   [[woe.probability]] or [[glm.predict]] inside the loop, as in
   [Dynamic drivers](/tutorials/land-change/dynamic-drivers/).
4. **`transitions`** ([[stack.of]]) puts the probability in a stack under the name `1_to_2`, the transition
   [[dinamica.step]] looks for.
5. **`clearing`** ([[dinamica.step]]) clears this year's forest.

`carry` passes the new land and the new network on; `collect` keeps the map, the network and the allocation's
table of every year.

!!! app "In the application"
    The four Dinamica EGO operators are in the library under *Dinamica EGO*. To carry the network, add an input
    `network` to the loop and connect `roads0.map`; in the body, connect `@network` to the `network` input of
    `roads`, and set the loop's carry to `network = roads.network`. Select `roads` after a run (++ctrl+enter++)
    to see the network of the last year on the map.

The run takes about a second. The same model with the roads frozen, for comparison, has no `roads` node and reads
the distance to the highway (`highway.map`) in `probability`.

## Read the results

### Year by year

```tfslider
label: Year
caption: The growing network (red) and the land cleared by the model (ochre) every two years. The chart compares the forest interior (forest more than 1 km from any clearing or road, as a share of the map) with growing roads and with the roads frozen.
frames:
  - { value: 2, src: /img/tutorials/land-change/roads-that-grow/year_2.png, caption: "Year 2", stats: { "New roads so far": "11 km", "Cleared": "17.2%", "Forest interior": "70.9%" } }
  - { value: 4, src: /img/tutorials/land-change/roads-that-grow/year_4.png, caption: "Year 4", stats: { "New roads so far": "23 km", "Cleared": "19.7%", "Forest interior": "68.5%" } }
  - { value: 6, src: /img/tutorials/land-change/roads-that-grow/year_6.png, caption: "Year 6", stats: { "New roads so far": "37 km", "Cleared": "22.1%", "Forest interior": "63.8%" } }
  - { value: 8, src: /img/tutorials/land-change/roads-that-grow/year_8.png, caption: "Year 8", stats: { "New roads so far": "58 km", "Cleared": "24.4%", "Forest interior": "57.0%" } }
  - { value: 10, src: /img/tutorials/land-change/roads-that-grow/year_10.png, caption: "Year 10", stats: { "New roads so far": "79 km", "Cleared": "26.6%", "Forest interior": "49.5%" } }
  - { value: 12, src: /img/tutorials/land-change/roads-that-grow/year_12.png, caption: "Year 12", stats: { "New roads so far": "97 km", "Cleared": "28.8%", "Forest interior": "42.8%" } }
  - { value: 14, src: /img/tutorials/land-change/roads-that-grow/year_14.png, caption: "Year 14", stats: { "New roads so far": "115 km", "Cleared": "30.9%", "Forest interior": "36.8%" } }
  - { value: 16, src: /img/tutorials/land-change/roads-that-grow/year_16.png, caption: "Year 16", stats: { "New roads so far": "134 km", "Cleared": "33.0%", "Forest interior": "30.6%" } }
  - { value: 18, src: /img/tutorials/land-change/roads-that-grow/year_18.png, caption: "Year 18", stats: { "New roads so far": "149 km", "Cleared": "35.0%", "Forest interior": "26.1%" } }
  - { value: 20, src: /img/tutorials/land-change/roads-that-grow/year_20.png, caption: "Year 20", stats: { "New roads so far": "165 km", "Cleared": "36.9%", "Forest interior": "21.9%" } }
start: 9
chart:
  type: line
  data: /data/tutorials/land-change/roads-that-grow/years.csv
  x: step
  y: [interior, interior_static]
  labels: { interior: "Growing roads", interior_static: "Roads frozen" }
  colors: { interior: "#c0392b", interior_static: "#8a8f8b" }
  xlabel: Year
  ylabel: "Forest interior, % of the map"
```

![Legend](/img/tutorials/land-change/roads-that-grow/legend.png)

The network grows by 5 to 12 km a year, 165 km in twenty years, in a branching pattern: each road ends 1.5 to 4
km beyond the network of its year, the next year's roads start from it, and the roads bend around the rough
ground. The clearing follows the roads, as the probability says, and the Expander widens it along them. The
**amount** of clearing is the same in both runs (36.9% of the map after 20 years): it is set by the Markov chain,
not by the roads. What the roads change is **where** it goes, and what is left.

### Growing roads against frozen roads

```tfcompare
caption: Drag the handle to compare the land after 20 years with the roads frozen (left) and growing (right). The same area cleared; in different places.
before: { src: /img/tutorials/land-change/roads-that-grow/final_static.png, label: Roads frozen }
after: { src: /img/tutorials/land-change/roads-that-grow/final_growing.png, label: Growing roads }
```

| After 20 years | Roads frozen | Growing roads |
|---|---:|---:|
| Cleared (% of the map) | 36.9 | 36.9 |
| New clearing more than 5 km from the highway | 1,303 ha | 6,503 ha |
| Forest interior (> 1 km from clearing or road, % of the map) | 53.8 | 21.9 |
| Patches of forest | 159 | 195 |
| Largest patch of forest (% of the forest) | 99.1 | 93.4 |

With the roads frozen, the clearing advances as a front from the highway and leaves the east of the map intact:
more than half of the map is still forest interior. With growing roads, the clearing reaches deep into the forest
along the side roads, and the interior falls to a fifth of the map. For species that avoid edges and roads
(Laurance et al., 2009), the two scenarios are very different landscapes with the same deforestation rate. This
is the main reason to model the roads: the rate of a scenario can be right while its pattern, and everything a
study computes from the pattern, is wrong.

### What to report

The initial network and its source, the friction (what it represents and its values), the attraction, the
distance range, the quadrants, the spacing and the number of destinations per quadrant, the mask, and the seed.
Report the length of new roads per step, and compare it with mapped rates of road building in the region (such as
those of Brandão & Souza, 2006); a constructor that builds several times faster than observed will fragment the
simulated forest too fast. Report the landscape metrics that matter to your question (the interior, the
patches, the edge) for runs with and without growing roads.

## Settings that change the answer

The frames below are the land after 20 years with one setting of [[dinamica.roads]] changed (the same seed, the
same 36.9% cleared).

### The distance range

```tfslider
label: Distance from the roads of a new end
caption: The network and the clearing after 20 years with new roads ending 0.5 to 1.5 km, 1.5 to 4 km or 4 to 8 km from the roads of their year.
frames:
  - { value: "0.5-1.5 km", src: /img/tutorials/land-change/roads-that-grow/dist_500.png, stats: { "New roads": "53 km", "Cleared": "36.9%", "Forest interior": "48.9%" } }
  - { value: "1.5-4 km", src: /img/tutorials/land-change/roads-that-grow/dist_1500.png, stats: { "New roads": "165 km", "Cleared": "36.9%", "Forest interior": "21.9%" } }
  - { value: "4-8 km", src: /img/tutorials/land-change/roads-that-grow/dist_4000.png, stats: { "New roads": "90 km", "Cleared": "36.9%", "Forest interior": "23.4%" } }
start: 1
```

The range is the reach of a year's building. Short roads (0.5 to 1.5 km) densify the network near the old
clearing and barely advance it. Long ones (4 to 8 km) advance it fast, but fewer destinations fit in the band of
each quadrant, so fewer roads are built (90 km against 165). Calibrate the range on the lengths of the side
roads mapped in your region, measured year by year.

### The quadrants

```tfslider
label: Quadrants
caption: The same model with 1 x 1 to 4 x 4 quadrants, one destination in each.
frames:
  - { value: "1 x 1", src: /img/tutorials/land-change/roads-that-grow/quad_1.png, stats: { "New roads": "56 km", "Cleared": "36.9%", "Forest interior": "51.2%" } }
  - { value: "2 x 2", src: /img/tutorials/land-change/roads-that-grow/quad_2.png, stats: { "New roads": "165 km", "Cleared": "36.9%", "Forest interior": "21.9%" } }
  - { value: "3 x 3", src: /img/tutorials/land-change/roads-that-grow/quad_3.png, stats: { "New roads": "270 km", "Cleared": "36.9%", "Forest interior": "1.9%" } }
  - { value: "4 x 4", src: /img/tutorials/land-change/roads-that-grow/quad_4.png, stats: { "New roads": "267 km", "Cleared": "36.9%", "Forest interior": "1.2%" } }
start: 1
```

The quadrants and `per_quadrant` set **how many** roads a step can build: up to quadrants across x quadrants
down x `per_quadrant`. They also spread the roads over the map: with one quadrant, the single destination of a
year goes wherever the attraction is highest; with sixteen, every part of the map with forest in the band gets a
road. With 3 x 3, 270 km of roads leave almost no forest more than a kilometre from a road or a clearing. Beyond
3 x 3 there is little change here, because the band and the spacing of 2 km limit the destinations, not the
quadrants. Dinamica EGO's default, 2 x 10, was chosen for maps far larger than this one: choose the quadrants for
the size of your map and the rate of road building you want.

### The friction

```tfcompare
caption: Drag the handle to compare the roads after 20 years on flat ground (left: friction 1 everywhere) and with the friction of rough ground (right).
before: { src: /img/tutorials/land-change/roads-that-grow/flat.png, label: Flat ground }
after: { src: /img/tutorials/land-change/roads-that-grow/final_growing.png, label: Rough ground }
```

On flat ground the roads run almost straight to their destinations (152 km in 20 years); with a friction they
bend around the rough ground and take longer routes (165 km). The friction decides the *route* of a road, not its
*end*: the destinations are drawn from the attraction. Give no value to cells a road can never cross (large
rivers without bridges, a strictly protected area), and a road will go around them or not be built.

### A protected area

```tfgallery
columns: 3
caption: "A protected area (pale green) east of the frontier, after 20 years. Left, no protection. Middle, a mask: roads may not end inside it. Right, the mask and no value in the friction: roads may neither end inside nor cross it."
images:
  - { src: /img/tutorials/land-change/roads-that-grow/pa_none.png, caption: "No protection" }
  - { src: /img/tutorials/land-change/roads-that-grow/pa_mask.png, caption: "mask" }
  - { src: /img/tutorials/land-change/roads-that-grow/pa_closed.png, caption: "mask and friction" }
```

![Legend](/img/tutorials/land-change/roads-that-grow/legend_pa.png)

| Scenario | Roads inside the area | Cleared inside the area |
|---|---:|---:|
| No protection | 12.8 km | 749 ha (7.8%) |
| `mask`: roads may not end inside | 1.7 km | 25 ha (0.3%) |
| `mask` and no friction inside: roads may not enter | 0 km | 37 ha (0.4%) |

The protected area is a rectangle of 9,546 ha drawn with [[nlm.distance_gradient]] (`origin = [150, 235, 70,
180]`, a distance of 0 inside); the mask is a [[map.calc]] of it, 0 inside and 1 outside, connected to the
`mask` input of `roads`; for the third scenario the friction is `if inside == 0 then null else 1 + 30 * r * r * r`.
Neither scenario forbids the clearing itself inside the area: the probability there is still the distance to the
roads. Keeping the roads out is enough to keep the clearing out, in this model, which is the mechanism by which
protected areas mitigate the effect of roads on deforestation (Barber et al., 2014). With the mask alone, a road
to a destination beyond the area can still cross part of it (1.7 km here). Soares-Filho et al. (2006)
estimated that an expanded and enforced network of protected areas could avoid as much as a third of the
projected forest loss of the Amazon.

### Other settings

- `per_quadrant` (1): more destinations per quadrant, as more quadrants do, but without spreading them.
- `spacing` (1,000 m): the least distance between the destinations of one step; larger, fewer roads per step in
  a narrow band.
- `lottery` (true): ties between equally cheap neighbours drawn at random; false takes the first in row order
  (north-west, north, north-east, west, east...).
- `diagonal` (true): diagonal steps cost \(\sqrt 2\) times more, so the cost is close to a Euclidean distance;
  false makes them as cheap as straight steps.
- `relative` (false): with true, frictions are per map unit and multiplied by the cell's side, so the costs are
  in effort per metre times metres.

## Common pitfalls

- **Distances in cells.** `distance` and `spacing` are in map units (metres in UTM). On a map of 30 m cells, the
  default range of 1,500 to 3,000 m is 50 to 100 cells; on a map without a coordinate system, it is 1,500 to
  3,000 *cells*, more than most maps: no destination is ever drawn.
- **Cells that are not square, or in degrees.** The constructor stops with an error on cells that are not square.
  Project the maps (to metres) before you build roads.
- **A friction of 0.** Frictions must be positive; a cell with 0 or less stops the run. Use no value for
  impassable cells, and at least a small positive number (0.1) for very cheap ones.
- **The network not carried.** Inside the loop, `roads` must read `@network` and the loop carry
  `network = roads.network`. Reading `roads0.map` every year would build each year's roads from the highway
  again, and none would accumulate.
- **No value off the network.** The `network` output has the road cells and *no value* elsewhere (not 0). Any
  value other than 0 counts as a road, so it can be carried as it is; to draw it or count its cells, use
  `if isnull(n) then 0 else 1` in [[map.calc]].
- **No attraction in the band.** If no cell between the two distances has an attraction above 0 (all cleared, or
  all masked), no road is built that step, silently. Look at the number of new road cells per step.
- **Paths down another cost.** [[dinamica.pathways]] walks down the cost until a cell of cost 0. A cost map
  computed from another network than the one given has no 0 where the paths arrive, or local minima where they
  stop: it is an error. Compute the cost from the same network you give to the pathways (as `roads` does).
- **One run.** The destinations and the clearing are drawn at random. Compare scenarios over several seeds.

## Going further

- [Gravity potentials as drivers](/tutorials/land-change/gravity-potentials/), the next part: the pull of towns,
  another driver that DINAMICA's Amazon models used, and a better attraction for roads than the forest alone.
- [Taking a stock](/tutorials/land-change/taking-a-stock/): the logging that often opens these roads.
- [Dynamic drivers](/tutorials/land-change/dynamic-drivers/): a driver recomputed from the simulated map and put
  back into the regressions.
- [Dinamica EGO on Sibuyan](/tutorials/land-change/dinamica-on-sibuyan/): the model whose probabilities come from
  weights of evidence, into which the distance to growing roads can be put.
- [Validating a simulation](/tutorials/land-change/validating-a-simulation/): compare the simulated pattern with
  an observed one.
- The operators: [[dinamica.roads]], [[dinamica.destinations]], [[dinamica.cost_map]], [[dinamica.pathways]].

## References

- Arima, E. Y., Walker, R. T., Perz, S. G., & Caldas, M. (2005). Loggers and forest fragmentation: behavioral models of road building in the Amazon basin. *Annals of the Association of American Geographers*, 95(3), 525–541. https://doi.org/10.1111/j.1467-8306.2005.00473.x
- Barber, C. P., Cochrane, M. A., Souza, C. M., & Laurance, W. F. (2014). Roads, deforestation, and the mitigating effect of protected areas in the Amazon. *Biological Conservation*, 177, 203–209. https://doi.org/10.1016/j.biocon.2014.07.004
- Brandão, A. O., & Souza, C. M. (2006). Mapping unofficial roads with Landsat images: a new tool to improve the monitoring of the Brazilian Amazon rainforest. *International Journal of Remote Sensing*, 27(1), 177–189. https://doi.org/10.1080/01431160500353841
- Laurance, W. F., Goosem, M., & Laurance, S. G. W. (2009). Impacts of roads and linear clearings on tropical forests. *Trends in Ecology & Evolution*, 24(12), 659–669. https://doi.org/10.1016/j.tree.2009.06.009
- Soares-Filho, B. S., Alencar, A., Nepstad, D., Cerqueira, G., Vera Diaz, M. del C., Rivero, S., Solórzano, L., & Voll, E. (2004). Simulating the response of land-cover changes to road paving and governance along a major Amazon highway: the Santarém–Cuiabá corridor. *Global Change Biology*, 10(5), 745–764. https://doi.org/10.1111/j.1529-8817.2003.00769.x
- Soares-Filho, B. S., Nepstad, D. C., Curran, L. M., Cerqueira, G. C., Garcia, R. A., Ramos, C. A., et al. (2006). Modelling conservation in the Amazon basin. *Nature*, 440(7083), 520–523. https://doi.org/10.1038/nature04389
- Walker, R., Arima, E., Messina, J., Soares-Filho, B., Perz, S., Vergara, D., et al. (2013). Modeling spatial decisions with graph theory: logging roads and forest fragmentation in the Brazilian Amazon. *Ecological Applications*, 23(1), 239–254. https://doi.org/10.1890/11-1800.1
