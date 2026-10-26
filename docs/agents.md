# Agents

Agents are decision makers whose state changes from step to step alongside the map, and who act on it. In
Terraflux they live in the same graph as the cellular automata, as values that a loop carries like the map.

A **population** (type `Agents`) is a table of attributes: a column per attribute, a row per agent.

| Operator | What it does |
|---|---|
| `agents.table` | agents written in the model: macro agents such as countries or markets |
| `agents.parcels` | an agent per parcel of a map of parcel ids: `id`, `cells` and the mean of other maps over the parcel |
| `agents.update` | behaviour: rules `attribute = expression`, in order, over the attributes and numbers (the step, a price) |
| `agents.map` | an attribute painted on the agents' parcels: a potential, a restriction, a multiplier |
| `agents.summary` | the count, sums and means of the attributes, as a table to collect along the loop |
| `crafty.allocate` | CRAFTY's competition for cells between agent functional types |

The rules use the expressions of map algebra (see [modelling.md](modelling.md)), without neighbourhoods:

```text
population = population * (1 + growth)
food = population * food_per_head
subsidy = 1.5 if wealth < 100 and step > 5 else 1
```

## CRAFTY

Agent functional types (AFTs) are farmers, foresters, conservationists and the like. Each produces services
from the capitals of the cells it manages:

```text
production[a, s](i) = production[a][s] × Π_c capital_c(i) ^ sensitivity[a][c]
```

**Utility of a service.** The demand left unmet makes a service worth producing:

```text
u[s] = max(D[s] − S[s], 0) / D[s]
```

**Competitiveness** of an AFT on a cell:

```text
C[a](i) = Σ_s u[s] × production[a, s](i)
```

**What changes each step:**

- Managers below their `giving_up` threshold leave their cell with probability `giving_up_rate`.
- A share `search` of the cells is offered. The most competitive AFT takes an offered cell when the cell is
  unmanaged, or when it beats the incumbent by the incumbent's `giving_in`.
- The supply and the utilities follow every change, so the land settles where the supply meets the demand.

**Determinism:** random numbers come from a counter (seed, step, cell), so the same seed gives the same maps.

The land map holds the AFT of each cell: 1, 2... in the order of `afts`, and 0 for unmanaged cells.
`cover` gives their land cover classes.

## A coupled model

Macro agents give the demand. Parcel managers decide subsidies that raise the fertility of their parcels.
The AFTs compete for the cells. The whole model is in `crates/exec/tests/abm.rs`; its loop is:

```toml
[[nodes]]
id = "simulation"
op = "control.repeat"
inputs = { times = 30, land = "$land", people = "people.agents" }

[nodes.body]
carry = { land = "alloc.land", people = "grow.agents" }
collect = { cover = "alloc.cover", info = "alloc.info" }

[[nodes.body.nodes]]
id = "grow"                               # the population grows, and its needs with it
op = "agents.update"
inputs = { agents = "@people" }
params = { rules = "population = population * 1.02\nfood = population * food_per_head\ntimber = population * timber_per_head" }

[[nodes.body.nodes]]
id = "need"                               # the demand: the sums over the population
op = "agents.summary"
inputs = { agents = "grow.agents" }

[[nodes.body.nodes]]
id = "decide"                             # parcel managers decide their subsidies
op = "agents.update"
inputs = { agents = "managers.agents", step = "@step" }
params = { rules = "subsidy = 3 if id == 1 and step > 5 else 1" }

[[nodes.body.nodes]]
id = "boost"
op = "agents.map"
inputs = { agents = "decide.agents", parcels = "$parcels" }
params = { column = "subsidy" }

[[nodes.body.nodes]]
id = "fert"                               # the subsidy raises the fertility capital
op = "raster.calc"
inputs = { caps = "$capitals", boost = "boost.map" }
params = { expression = "min(caps.fertility * boost, 1)" }

[[nodes.body.nodes]]
id = "caps"
op = "stack.set"
inputs = { stack = "$capitals", map = "fert.map" }
params = { name = "fertility" }

[[nodes.body.nodes]]
id = "alloc"                              # the AFTs compete for the cells
op = "crafty.allocate"
inputs = { land = "@land", capitals = "caps.stack", demand = "need.table", step = "@step", seed = "$seed" }
params = { services = { food = "food_sum", timber = "timber_sum" }, search = 0.2, giving_up_rate = 0.2, afts = [
  { name = "farmer", class = 2, production = { food = 1.0 }, sensitivity = { fertility = 2.0 }, giving_up = 0.05, giving_in = 0.1 },
  { name = "forester", class = 3, production = { timber = 1.0 }, sensitivity = { wood = 1.0 }, giving_up = 0.05, giving_in = 0.1 },
] }
```

Agents also work with the other models. For example, `agents.map` gives a map that `raster.calc` multiplies
into the transition probabilities of Dinamica EGO's model, or turns into a restriction of CLUE-S.
