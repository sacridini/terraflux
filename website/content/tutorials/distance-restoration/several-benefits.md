---
title: Several benefits and trade-offs
short: Several benefits
description: Restore for species, carbon, ecological intactness and ecosystems at once - put the benefits on one scale, trace the trade-off between biodiversity and carbon with a dashboard, rank sets of weights as Strassburg et al. (2020) do, share a target among ecosystems for the least risk of collapse, and see what targets per region cost.
order: 60
level: advanced
time: 55 min
template: Restoration plan
ops: [restoration.plan, restoration.ecosystem_areas, restoration.ecosystem_targets, restoration.intactness_gradient, raster.calc, stack.of, raster.read]
series: Distance and restoration
part: 6
---

A restoration programme is rarely about one thing. Funders want carbon, conservation agencies want species, others
want whole ecosystems back or landscapes less pressed by people, and every region wants its share. In this part
of the series you will give [[restoration.plan]] four benefits (the species' extinction risk of the [previous
part](../restoration-plan/), carbon, ecological intactness and the risk of collapse of ecosystems), see how
differently each one alone would plan Serra Verde's 100 km², put them on one scale, trace the trade-off between
species and carbon with a dashboard, rank sets of weights as Strassburg et al. (2020) do, and measure what it costs
to give every region its part.

## The idea

### Four benefits

Strassburg et al. (2020) planned the world's restoration for three benefits at once: biodiversity (the extinction
risk avoided), climate (the carbon restored vegetation would hold) and cost. [[restoration.plan]] takes:

| Benefit | What restoring a cell adds | Inputs |
|---|---|---|
| **species** | extinction risk avoided, \(\sum_s [(\text{after})^z - (\text{before})^z]\) | `table`, `gain` (part 4) |
| **carbon** (any linear benefit) | its amount per km² restored, whatever else is restored | `linear`: a stack, a layer per benefit |
| **intactness** | less human pressure on it and on its neighbours (part 3) | `hfi` (the human footprint), `reduction` |
| **ecosystems** | the risk of collapse avoided for its ecoregion: the same curve on the natural area of every ecoregion | `table`, `gain` from [[restoration.ecosystem_areas]] |

**Carbon is linear**: each km² adds the same, so a plan for carbon alone restores the cells with the most carbon per
km², in full. **Species and ecosystems are concave**: each gives less as it recovers. **Intactness** is measured by its
gradient (part 3): how fast the total intactness of the landscape rises per unit restored, recomputed as the plan
proceeds.

The risk of collapse of an ecosystem follows the logic of the IUCN Red List of Ecosystems, where the loss of an
ecosystem's extent is one of the main criteria (Keith et al., 2013): an ecoregion that keeps a share \(r\) of its
natural area gets the risk \(1 - r^z\), as a species does.

### One scale

The benefits come in different units: a risk avoided of a few tenths, about a million tonnes of carbon, a sum of
intactness in the hundreds. Adding them with weights of 1 lets the largest number win. A simple, transparent scale
divides each benefit by the most it can reach **alone** (the best plan for it alone), so each counts from 0 to 1.

!!! science "Key idea: weighted benefits"
    With \(B_k(x)\) the value of benefit \(k\) for the shares restored \(x\), and \(B_k^*\) the best it reaches alone,

    \[ \max_x \sum_k w_k \frac{B_k(x)}{B_k^{*}} \]

    The weights \(w_k\) say how much each benefit matters. They are a choice, not a fact of the data: the plan makes
    their consequences visible. Varying them traces the **trade-off curve**, every point of which is a plan no other
    plan beats on both benefits at once.

Strassburg et al. (2020) also compare **sets of weights**: for each set, a plan; for each plan, every benefit as a
share of its best; and a **score**, the mean of those shares over their standard deviation, high for plans that do
well on all benefits and evenly. The `weights` setting of [[restoration.plan]] does the same and returns the plan of
the best-ranked set.

## When to use it

- **Programmes with several goals**: carbon finance with biodiversity safeguards, landscape restoration for water,
  people and species.
- **Negotiation**: the trade-off curve shows each party what a little more of their benefit costs the others.
- **Targets per region** (states, watersheds), and **by ecosystem**, so that no ecosystem is left out.

Remember what the benefits leave out: the costs of restoration and of the land (add a `cost` map and a `budget`, part
5), the chance that restoration succeeds (Crouzeilles et al., 2016, found it depends strongly on the landscape around
it), and the people who live there.

## What you need

The model of [part 5](../restoration-plan/), plus, on the same grid:

- `carbon_potential.tif`: the carbon of restored vegetation in every cell, in tonnes per hectare;
- `footprint.tif`: the human footprint (0 to 50, part 3);
- the land cover's `pasture` and `cropland` bands, for the reduction of the footprint when they are restored;
- `ecoregions.tif`: four ecoregions (1 coast, 2 lowland forest, 3 montane forest, 4 savanna);
- `regions.tif`: three administrative regions.

## Build the model

### Carbon and intactness

```tfgraph
#! caption: Carbon in million tonnes per km² restored (nodes 1 and 2) as a linear benefit (node 3), and the footprint and its reduction per unit restored (nodes 4 to 7) for the intactness benefit.
#! fragment: true
#! numbered: true
#! highlight: carbon, linear, reduction
[[nodes]]
id = "carbon_t_ha"
op = "raster.read"
params = { path = "carbon_potential.tif" }

[[nodes]]
id = "carbon"
op = "raster.calc"
inputs = { c = "carbon_t_ha.raster" }
params = { expression = "c * 100 / 1000000" }

[[nodes]]
id = "linear"
op = "stack.of"
inputs = { carbon = "carbon.map" }

[[nodes]]
id = "footprint"
op = "raster.read"
params = { path = "footprint.tif" }

[[nodes]]
id = "pasture"
op = "raster.read"
params = { path = "land_cover.tif", band = "pasture" }

[[nodes]]
id = "cropland"
op = "raster.read"
params = { path = "land_cover.tif", band = "cropland" }

[[nodes]]
id = "reduction"
op = "raster.calc"
inputs = { p = "pasture.raster", c = "cropland.raster" }
params = { expression = "(4 * p + 7 * c) / (p + c) if p + c > 0 else 0" }
```

<div class="steps" markdown>

1. **Carbon** (nodes 1 to 3): tonnes per hectare times 100 hectares per km², divided by a million: million tonnes
   per km² restored, so that a plan's carbon is in Mt. [[stack.of]] makes it a stack with a layer named `carbon`; more
   layers (water, timber) would be more linear benefits, each with its own weight in `linear_weights`.
2. **The reduction** (nodes 4 to 7): restoring pasture removes a footprint of 4, cropland of 7 (the pressures of the
   Human Footprint, Venter et al., 2016). A cell with both gets their mean weighted by their shares. [[raster.read]]
   reads one band by name with `band`.

</div>

### The plan with three benefits

```tfgraph
#! caption: One plan for the species, carbon and intactness, each weighted by one over the most it reaches alone. It continues the graphs above and the model of part 5.
#! fragment: true
#! highlight: plan
[[nodes]]
id = "plan"
op = "restoration.plan"
inputs = { restorable = "vegetation.anthropic", table = "habitat.table", gain = "habitat.gain", linear = "linear.stack", hfi = "footprint.map", reduction = "reduction.map" }
params = { target = 100, weight = 3.89, linear_weights = [0.817], intactness_weight = 0.00672 }
```

`weight` multiplies the risk benefit, `linear_weights` the linear ones in the order of their layers, and
`intactness_weight` the intactness. The values are the inverses of the best of each alone (0.257, 1.224 Mt and
148.9, found by three plans with one benefit each): each benefit counts from 0 to 1. The `benefits` output gives,
per benefit, its value, its weight and the weighted value. Intactness takes its own settings (`radius`, `gamma`,
`beta`, `intactness_z`, as in part 3) and `intactness_form`: `linear` (the gradient, recomputed as the plan
proceeds, as Strassburg et al. do) or `exact` (the total intactness itself, which is not concave: the plan is then a
local optimum with a bound).

### Ecosystems

```tfgraph
#! caption: The ecoregions' natural area now and before conversion (node 1), a plan for their risk of collapse (node 2), and a target of 100 km² shared among them for the least risk (node 3).
#! fragment: true
#! numbered: true
#! highlight: ecosystems, plan_eco, eco_targets
[[nodes]]
id = "ecosystems"
op = "restoration.ecosystem_areas"
inputs = { land_cover = "cover.stack", ecoregions = "ecoregions.map" }
params = { natural = ["forest", "savanna", "mangrove"], anthropic = ["pasture", "cropland"] }

[[nodes]]
id = "plan_eco"
op = "restoration.plan"
inputs = { restorable = "vegetation.anthropic", table = "ecosystems.table", gain = "ecosystems.gain" }
params = { target = 100 }

[[nodes]]
id = "eco_targets"
op = "restoration.ecosystem_targets"
inputs = { table = "ecosystems.table" }
params = { target = 100 }
```

[[restoration.ecosystem_areas]] gives, per ecoregion, its natural area now (`current`) and before conversion
(`potential`, the natural plus the converted land), in the form [[restoration.plan]] takes for its risk benefit
(`table`, `gain`). [[restoration.ecosystem_targets]] answers a different question: how to **share** a target among the
ecosystems for the least total risk of collapse (`method = "exact"`, or the stepwise `steps`), each ecosystem between a
least share (`minimum`) and its converted area. Its `targets` table (an ecosystem's code and its share) can be the
regional targets of a plan, with the ecoregions as `regions`.

### Sets of weights and regions

```toml
[[nodes]]
id = "plan"
op = "restoration.plan"
inputs = { restorable = "vegetation.anthropic", table = "habitat.table", gain = "habitat.gain", linear = "linear.stack", hfi = "footprint.raster", reduction = "reduction.map" }
params = { target = 100, weights = [
  { extinction_risk = 3.89, carbon = 0, intactness = 0 },
  { extinction_risk = 0, carbon = 0.817, intactness = 0 },
  { extinction_risk = 0, carbon = 0, intactness = 0.00672 },
  { extinction_risk = 3.89, carbon = 0.817, intactness = 0.00672 },
  { extinction_risk = 7.78, carbon = 0.817, intactness = 0.00672 },
  { extinction_risk = 3.89, carbon = 0.817, intactness = 0 } ] }
```

With `weights`, a list of sets (the benefits named `extinction_risk`, the linear layers' names and `intactness`), the
node solves a plan per set and returns the `weights` table (per set its weights, its benefits, Strassburg et al.'s
mean, sd, score and rank) and the plan of the best-ranked set.

For **targets per region**, connect a map of region codes to `regions` and give `target` as `{code: area}`, for
instance `target = { 1 = 30, 2 = 40, 3 = 30 }`; `sense` applies to each.

!!! app "In the application"
    Start from **Templates ▸ Conservation and restoration ▸ Restoration plan**, connect the extra maps to
    the plan's `linear`, `hfi` and `reduction` inputs, and set the weights in the Node pane. The `benefits` table
    in the Viewer shows what each benefit gets; the `weights` table, with sets of weights, their ranking.

## Read the results

### Each benefit alone asks for other places

```tfgallery
columns: 2
caption: "The plan of 100 km² for each benefit alone (the share of each cell restored, same scale as part 5). Species: the tamarin's fragments and the mangrove coast. Carbon: the converted cells with the most carbon, in full, along the coast and in the wet lowlands. Intactness: thin strips at the fringes of the converted land, where restoring reconnects intact surroundings. Ecosystems: the coast and the lowland forest, the two ecoregions that have lost a third of their area."
images:
  - { src: /img/tutorials/distance-restoration/several-benefits/alone_species.png, caption: "Species" }
  - { src: /img/tutorials/distance-restoration/several-benefits/alone_carbon.png, caption: "Carbon" }
  - { src: /img/tutorials/distance-restoration/several-benefits/alone_intactness.png, caption: "Intactness" }
  - { src: /img/tutorials/distance-restoration/several-benefits/alone_ecosystems.png, caption: "Ecosystems" }
```

![The carbon of restored vegetation.](/img/tutorials/distance-restoration/several-benefits/carbon.png "The carbon restored vegetation would hold, in tonnes per hectare: most along the coast and in the north-east, least in the drier south-west.")

How each single-benefit plan does on the others, as a share of the best for each:

```tfchart
type: bar
data: /data/tutorials/distance-restoration/several-benefits/cross.csv
x: plan
y: [species, carbon, intactness]
labels: { species: "Species (risk avoided)", carbon: "Carbon", intactness: "Intactness" }
colors: { species: "#3b8a5a", carbon: "#2f6f9f", intactness: "#e8901f" }
ylabel: "% of the best for each benefit"
height: 300
caption: "Every plan of 100 km² scored on the three benefits, as a share of the best plan for each. The plan for species gets 80 % of the carbon, but only 44 % of the intactness; the plan for intactness gets 15 % of the species' benefit. The plan with all three, weighted equally, gets 78 % of the best for the species, 89 % for carbon and 70 % for intactness: each gives up a little, none is sacrificed."
```

The plan for the species and the plan for carbon overlap more than either does with intactness: the coast is rich in
carbon and is where the mangrove was lost. Intactness pulls the other way, towards the fringes of the large converted
areas, where restoring a strip reconnects forest that is already there. A plan with all three keeps most of every
benefit, which is the usual finding of such analyses: optimising jointly costs each benefit little compared with
optimising one and hoping for the others (Strassburg et al., 2020).

![The plan with all three benefits.](/img/tutorials/distance-restoration/several-benefits/all_three.png "The plan of 100 km² for the species, carbon and intactness, equally weighted: the tamarin's fragment, the mangrove coast with its high carbon, and strips along the rivers.")

### The trade-off between species and carbon

```tfslider
label: Weight of the species (against carbon)
caption: "The plan of 100 km² as the weight moves from carbon (0) to the species (1), each benefit scaled by its best alone. The chart: the species' and the carbon benefit as a share of their best. The curve bends: a weight of 0.2 gives the species 68 % of their best for 2 % less carbon, and 0.5 gives them all of it for 18 % less carbon. Beyond 0.5 nothing changes."
frames:
  - { value: 0.0, src: /img/tutorials/distance-restoration/several-benefits/weight_0.0.png, stats: { "Species": "48 %", "Carbon": "100 %" } }
  - { value: 0.1, src: /img/tutorials/distance-restoration/several-benefits/weight_0.1.png, stats: { "Species": "62 %", "Carbon": "99 %" } }
  - { value: 0.2, src: /img/tutorials/distance-restoration/several-benefits/weight_0.2.png, stats: { "Species": "68 %", "Carbon": "98 %" } }
  - { value: 0.3, src: /img/tutorials/distance-restoration/several-benefits/weight_0.3.png, stats: { "Species": "79 %", "Carbon": "94 %" } }
  - { value: 0.5, src: /img/tutorials/distance-restoration/several-benefits/weight_0.5.png, stats: { "Species": "100 %", "Carbon": "82 %" } }
  - { value: 0.7, src: /img/tutorials/distance-restoration/several-benefits/weight_0.7.png, stats: { "Species": "100 %", "Carbon": "82 %" } }
  - { value: 0.9, src: /img/tutorials/distance-restoration/several-benefits/weight_0.9.png, stats: { "Species": "100 %", "Carbon": "81 %" } }
  - { value: 1.0, src: /img/tutorials/distance-restoration/several-benefits/weight_1.0.png, stats: { "Species": "100 %", "Carbon": "80 %" } }
start: 2
chart:
  type: line
  data: /data/tutorials/distance-restoration/several-benefits/weights.csv
  x: w
  y: [species, carbon]
  labels: { species: "Species", carbon: "Carbon" }
  colors: { species: "#3b8a5a", carbon: "#2f6f9f" }
  xlabel: Weight of the species
  ylabel: "% of the best"
```

The first points of the curve are the cheapest: a small weight on the species buys a lot of risk avoided for almost
no carbon. That is the place to start a negotiation. Planners and the people involved choose a point on the curve; the
model makes the price of each point explicit.

### Ranking sets of weights

The `weights` table of the six sets above, each benefit as a share of its best:

| Weights (species - carbon - intactness) | Species | Carbon | Intactness | Score | Rank |
|---|---|---|---|---|---|
| 1 - 1 - 1 | 78 % | 89 % | 70 % | 5.02 | 1 |
| 2 - 1 - 1 | 98 % | 81 % | 50 % | 1.19 | 2 |
| 0 - 1 - 0 | 48 % | 100 % | 41 % | 0.93 | 3 |
| 1 - 0 - 0 | 100 % | 80 % | 43 % | 0.92 | 4 |
| 1 - 1 - 0 | 100 % | 82 % | 40 % | 0.92 | 5 |
| 0 - 0 - 1 | 15 % | 70 % | 100 % | 0.58 | 6 |

The equal weights rank first by far: their plan is the most balanced (a mean of 79 % with little spread). The
score rewards balance; a programme with a clear priority (carbon finance) would rather choose on the trade-off curve.

### Ecosystems

| Ecoregion | Natural area now (km²) | Before conversion | Left | Share of 100 km² |
|---|---|---|---|---|
| Coast | 73.5 | 114.0 | 64.5 % | 40.5 |
| Lowland forest | 1 478.2 | 2 263.8 | 65.3 % | 50.3 |
| Montane forest | 1 917.8 | 1 999.0 | 95.9 % | 0 |
| Savanna | 354.7 | 364.0 | 97.5 % | 9.3 |

[[restoration.ecosystem_targets]] shares a target of 100 km² among the ecoregions for the least total risk of
collapse: the coast and the lowland forest, which keep two thirds of their area, get 91 km²; the montane forest none.
The savanna gets 9 km² although it has lost little: its whole converted area is only 9 km², and a small ecoregion's
last km² still lowers its risk. Giving these as regional targets ensures every ecosystem gets its part of a plan
driven by species or carbon.

### Targets per region

```tfgallery
columns: 2
caption: "Left, Serra Verde's three administrative regions. Right, the plan of 100 km² for the species with a target for each: 30 km² in region 1, 40 in region 2, 30 in region 3."
images:
  - { src: /img/tutorials/distance-restoration/several-benefits/regions.png, caption: "Regions" }
  - { src: /img/tutorials/distance-restoration/several-benefits/regions_plan.png, caption: "The plan with regional targets" }
```

```tfchart
type: bar
data: /data/tutorials/distance-restoration/several-benefits/regions.csv
x: region
y: [without, with]
labels: { without: "Without regional targets", with: "With them" }
colors: { without: "#c9d8c4", with: "#3b8a5a" }
ylabel: km² restored
height: 240
caption: "The area each region restores in the plan for the species, without and with the targets of 30, 40 and 30 km². Without them region 1 gets nothing: its species are in good shape."
```

Sharing the effort costs 9 % of the benefit (0.234 against 0.257): region 1 must restore 30 km² where they help
species that have lost little. That number, the price of the rule, is what a planner can put before the people who
decide whether the rule is worth it.

### Reporting

Report every benefit with its data and units, how they were scaled (here by the best alone) and weighted, the
trade-off curve or the sets of weights compared, and the plan's value for each benefit as a share of its best, so
that readers see what each benefit gave up. With regional or ecosystem targets, report the targets and their price in
benefit.

### In Python

The benefits are methods of the same problem as in [part 5](../restoration-plan/#in-python):
`add_extinction_risk_benefit`, `add_linear_benefit` (carbon) and `add_intactness_benefit`, each with a `weight` and a
`name` (the part's graphs call the species' benefit `extinction_risk`; here it is whatever `name=` says). The inputs
are made again, with the carbon, the footprint, the reduction of the footprint per unit restored (part 3) and the regions:

```python
import numpy as np
import pandas as pd
import terraflux as tf

P = tf.prioritize
cover = tf.load_raster(P.example_data("land_cover"))
past = tf.load_raster(P.example_data("past_cover"))
ecoregions = tf.load_raster(P.example_data("ecoregions"))
features = tf.load_raster(P.example_data("features"))
footprint = tf.load_raster(P.example_data("footprint"))
carbon = tf.load_raster(P.example_data("carbon_potential"))     # t/ha in restored vegetation
regions = tf.load_raster(P.example_data("regions"))              # 3 administrative regions
natural, anthropic = ["forest", "savanna", "mangrove"], ["pasture", "cropland"]

props = past / past.sum("band")                                  # as in part 4
eco_props = props.copy()
for code in np.unique(ecoregions.values[np.isfinite(ecoregions.values)]):
    inside = ecoregions.values == code
    eco_props.values[:, inside] = np.nanmean(props.values[:, inside], axis=1)[:, None]
veg = tf.original_vegetation(cover, natural, anthropic, past=past, ecoregions=eco_props)
suitability = {
    "jaguar": ["forest", "savanna"], "lowland_tapir": ["forest"], "harpy_eagle": ["forest"],
    "maned_wolf": ["savanna"], "golden_lion_tamarin": ["forest"],
    "neotropical_otter": ["forest", "mangrove"], "araucaria_forest": ["forest"], "mangrove": ["mangrove"],
}
aoh = tf.sdm.area_of_habitat((features > 0).astype("float32"), cover, natural, suitability=suitability, original=veg)

pasture, cropland = cover.sel(band="pasture"), cover.sel(band="cropland")
reduction = ((4 * pasture + 7 * cropland) / (pasture + cropland)).fillna(0)   # footprint removed per unit restored
carbon_mt = carbon * 100 / 1e6                                    # t/ha to Mt of carbon per km2 restored
```

Each benefit alone, then the three on one scale: every weight is one over the most its benefit reaches alone (the
node's `weight = 3.89`, `linear_weights = [0.817]`, `intactness_weight = 0.00672`):

```python
base = P.restoration_problem(veg).add_area_target(100)
benefits = {
    "species": lambda q, w=1.0: q.add_extinction_risk_benefit(aoh, weight=w, name="species"),
    "carbon": lambda q, w=1.0: q.add_linear_benefit(carbon_mt, weight=w, name="carbon"),
    "intactness": lambda q, w=1.0: q.add_intactness_benefit(footprint, reduction, radius=3, weight=w, name="intactness"),
}
best = {name: add(base).solve().benefit for name, add in benefits.items()}      # each benefit alone
print({k: round(v, 3) for k, v in best.items()}, {k: round(1 / v, 4) for k, v in best.items()})

multi = base
for name, add in benefits.items():
    multi = add(multi, 1 / best[name])
both = multi.solve()
print(both.status, both.benefits.round(4).to_string(index=False))
print({k: f"{v / best[k]:.0%}" for k, v in both.benefits.set_index("benefit")["value"].items()})
```

```text
{'species': 0.257, 'carbon': 1.224, 'intactness': 148.864} {'species': 3.8889, 'carbon': 0.8173, 'intactness': 0.0067}
optimal    benefit            kind    value  weight  weighted
   species extinction_risk   0.2004  3.8889    0.7795
    carbon          linear   1.0835  0.8173    0.8855
intactness          linear 104.1504  0.0067    0.6996
{'species': '78%', 'carbon': '89%', 'intactness': '70%'}
```

The three weights are the part's, and the plan with all three reaches the part's 78% of the best for the species, 89% for
carbon and 70% for intactness. `both.benefits` is the node's `benefits` output: each benefit's value, weight and weighted
value. A plan for more linear benefits (water, timber) has one `add_linear_benefit` each. For intactness, `form="linear"`
(the default) is the gradient at the plan of nothing, and `form="exact"` optimises the total intactness itself, which is not
concave: the plan is then a local optimum with a bound on the best (status `"local"` unless they meet):

```python
exact = (P.restoration_problem(veg).add_intactness_benefit(footprint, reduction, form="exact", name="intactness")
           .add_area_target(100).solve())
print(exact.status, f"intactness {exact.benefit:.1f}, bound {exact.bound:.1f}")
```

```text
local intactness 172.3, bound 181.5
```

The exact form reaches a total intactness gained of 172.3 against a bound of 181.5 on the best plan: a local optimum, as the
status says, on a scale that is not the gradient's (the 148.9 above). Each plan on its own, scored on all three benefits as a share of the best (`evaluate` scores any raster of shares restored):

```python
cross = {}
for name, add in benefits.items():
    alone = add(base).solve()
    scored = multi.evaluate(restored=alone.restored).benefits.set_index("benefit")["value"]
    cross[name + " alone"] = {k: f"{scored[k] / best[k]:.0%}" for k in best}
print(pd.DataFrame(cross).T.to_string())
```

```text
                 species carbon intactness
species alone       100%    80%        42%
carbon alone         48%   100%        41%
intactness alone     14%    70%       100%
```

The plan for species alone has ties, so its intactness is 42% here against 44% in the part, and the others agree to a
point. The trade-off between species and carbon is a loop over the weight w, and the sets of weights of the part go to
`compare_weights`, which solves a plan per set and ranks the sets as Strassburg et al. do (each benefit rescaled to 0-1
over the sets, the mean over their standard deviation):

```python
for w in (0.0, 0.2, 0.5, 1.0):
    q = benefits["carbon"](benefits["species"](base, w / best["species"]), (1 - w) / best["carbon"])
    v = q.solve().benefits.set_index("benefit")["value"]
    print(f"w = {w:.1f}: species {v['species'] / best['species']:.0%}, carbon {v['carbon'] / best['carbon']:.0%}")

sets = [  # the weights of the part's sets, 1 - 1 - 1 meaning one over the best of each
    {"species": 3.89, "carbon": 0, "intactness": 0}, {"species": 0, "carbon": 0.817, "intactness": 0},
    {"species": 0, "carbon": 0, "intactness": 0.00672}, {"species": 3.89, "carbon": 0.817, "intactness": 0.00672},
    {"species": 7.78, "carbon": 0.817, "intactness": 0.00672}, {"species": 3.89, "carbon": 0.817, "intactness": 0},
]
ranked = multi.compare_weights(sets)
print(ranked.table[["w.species", "w.carbon", "w.intactness", "species", "carbon", "intactness", "score", "rank"]]
      .round(3).to_string(index=False))
```

```text
w = 0.0: species 48%, carbon 100%
w = 0.2: species 68%, carbon 98%
w = 0.5: species 100%, carbon 82%
w = 1.0: species 100%, carbon 80%
 w.species  w.carbon  w.intactness  species  carbon  intactness  score  rank
      3.89     0.000         0.000    0.257   0.976      62.092  0.895   5.0
      0.00     0.817         0.000    0.123   1.224      60.990  0.934   3.0
      0.00     0.000         0.007    0.037   0.857     148.864  0.577   6.0
      3.89     0.817         0.007    0.200   1.083     104.150  5.022   1.0
      7.78     0.817         0.007    0.252   0.992      74.698  1.189   2.0
      3.89     0.817         0.000    0.257   0.999      60.175  0.918   4.0
```

The curve is the part's (a weight of 0.2 gives the species 68% of their best for 2% less carbon, and 0.5 gives them all
of it), and so is the ranking: the equal weights come first, with a score of 5.02. `ranked.best()` is the plan of the
first set and `ranked.solutions` the plan of each. The ecosystems:

```python
eco = P.ecosystem_areas(cover, natural, anthropic, ecoregions)     # natural area now and before conversion
print(eco.table[["species", "current", "potential"]].round(1).to_string(index=False))   # "species": the ecoregion's code

targets = P.ecosystem_targets(eco, 100)                            # 100 km2 shared for the least risk of collapse
print(targets.round(1).to_string(index=False))

plan_eco = P.restoration_problem(veg).add_extinction_risk_benefit(eco, name="ecosystems").add_area_target(100).solve()
print(f"a plan for the ecosystems' risk of collapse: {plan_eco.benefit:.3f}")
```

```text
species  current  potential
      1     73.5      114.0
      2   1478.2     2263.8
      3   1917.8     1999.0
      4    354.7      364.0
 region  target sense
    1.0    40.5    <=
    2.0    50.3    <=
    3.0     0.0    <=
    4.0     9.3    <=
a plan for the ecosystems' risk of collapse: 0.118
```

The table is the part's, the shares of the 100 km² too (40.5, 50.3, 0 and 9.3). `ecosystem_targets` also takes `minimum`
(each ecosystem's least share), `method="steps"` and `sense`. Its table is a set of regional targets: with the ecoregions
as the regions, every ecosystem gets its part of a plan driven by the species:

```python
by_ecoregion = (P.restoration_problem(veg).add_extinction_risk_benefit(aoh, name="species")
                  .add_area_target(targets, regions=ecoregions))
print(f"species plan with the ecosystems' targets: {by_ecoregion.solve().benefit:.3f}")
```

```text
species plan with the ecosystems' targets: 0.248
```

Targets per region take a mapping (or a table with `region`, `target` and optionally `sense`) and the raster of regions;
the part's price of the rule is the loss of benefit:

```python
free = P.restoration_problem(veg).add_extinction_risk_benefit(aoh, name="species").add_area_target(100).solve()
shared = (P.restoration_problem(veg).add_extinction_risk_benefit(aoh, name="species")
            .add_area_target({1: 30, 2: 40, 3: 30}, regions=regions).solve())
for r in (1, 2, 3):
    inside = regions.values == r
    print(f"region {r}: {np.nansum(np.where(inside, free.restored.values, 0)):5.1f} km2 without regional targets, "
          f"{np.nansum(np.where(inside, shared.restored.values, 0)):5.1f} km2 with them")
print(f"benefit {shared.benefit:.3f} against {free.benefit:.3f}")
```

```text
region 1:   0.0 km2 without regional targets,  30.0 km2 with them
region 2:  49.7 km2 without regional targets,  40.0 km2 with them
region 3:  50.3 km2 without regional targets,  30.0 km2 with them
benefit 0.234 against 0.257
```

Region 1 gets nothing without the targets, and the rule costs 9% of the benefit (0.234 against 0.257), as the part says.
`sense=">="` makes minimum targets and `"="` exact ones. Last, the uncertainty of a linear total: with an error of 25% in
each cell's carbon, [`tf.uncertainty.sum_sd`][terraflux.uncertainty.sum_sd] gives the deviation of the carbon of a plan,
for independent errors or fully correlated ones (a bias of the carbon map):

```python
on_plan = np.nan_to_num(free.restored.values)                       # km2 restored per cell (cells of 1 km2)
total = float(np.nansum(carbon.values * 100 * on_plan))             # tonnes
sd = 0.25 * carbon.values * 100
print(f"carbon {total / 1e6:.3f} Mt; sd {tf.uncertainty.sum_sd(sd, weights=on_plan) / 1e6:.3f} Mt if independent, "
      f"{tf.uncertainty.sum_sd(sd, weights=on_plan, correlation=1.0) / 1e6:.3f} Mt if correlated")
```

```text
carbon 0.976 Mt; sd 0.018 Mt if independent, 0.244 Mt if correlated
```

`tf.uncertainty` also has `product_sd`, `delta_method` and `monte_carlo` for other combinations, and the species' z has its own
uncertainty in [part 5](../restoration-plan/#in-python).

## Settings that change the answer

- **The weights**, as the slider shows, and **the scaling**. Scaling by the best alone is one choice; scaling by a
  reference (the benefit of restoring everything) gives other numbers. Say which.
- **The reduction of the footprint** per class (4 for pasture, 7 for cropland) and the parameters of intactness.
- **The carbon map**: the carbon of mature vegetation overstates what young restoration holds for decades.
- **Regional and ecosystem targets**, and their `sense` (at most, at least, exactly).

## Common pitfalls

- **Weights of 1 on benefits in different units.** The benefit with the largest numbers wins. Scale first.
- **Weights read as importance.** They are exchange rates between scaled benefits; their consequences, not their
  values, should be discussed.
- **Linear benefits on a concave model.** A plan for carbon alone restores whole cells with the most carbon; with
  species too, the mix changes in ways the weights alone do not tell. Look at the trade-off curve.
- **Intactness on its own scale.** Its gradient is in units of total intactness per unit restored; always scale it.
- **Targets per region larger than a region's restorable land.** Check the restorable area of each region first.

## Going further

- Which natural land to keep, and where to meet a demand for land: [Conserving and converting](../conserving-and-converting/),
  the next part, in Python.
- The series from the start: [Distance to habitat](../distance-to-habitat/), [Costs and corridors](../costs-and-corridors/),
  [Ecological intactness](../ecological-intactness/), [Original vegetation and habitat](../original-vegetation-and-habitat/),
  [A restoration plan](../restoration-plan/).
- Choosing which places to protect rather than restore: [Systematic conservation planning](/tutorials/spatial-prioritization/systematic-conservation-planning/).
- The operators: [[restoration.plan]], [[restoration.ecosystem_areas]], [[restoration.ecosystem_targets]].

## References

- Crouzeilles, R., Curran, M., Ferreira, M. S., Lindenmayer, D. B., Grelle, C. E. V., & Rey Benayas, J. M. (2016). A global meta-analysis on the ecological drivers of forest restoration success. *Nature Communications*, 7, 11666. https://doi.org/10.1038/ncomms11666
- Keith, D. A., Rodríguez, J. P., Rodríguez-Clark, K. M., Nicholson, E., Aapala, K., Alonso, A., et al. (2013). Scientific foundations for an IUCN Red List of Ecosystems. *PLoS ONE*, 8(5), e62111. https://doi.org/10.1371/journal.pone.0062111
- Strassburg, B. B. N., Iribarrem, A., Beyer, H. L., Cordeiro, C. L., Crouzeilles, R., Jakovac, C. C., et al. (2020). Global priority areas for ecosystem restoration. *Nature*, 586(7831), 724–729. https://doi.org/10.1038/s41586-020-2784-9
- Venter, O., Sanderson, E. W., Magrach, A., Allan, J. R., Beher, J., Jones, K. R., et al. (2016). Sixteen years of change in the global terrestrial human footprint and implications for biodiversity conservation. *Nature Communications*, 7, 12558. https://doi.org/10.1038/ncomms12558
