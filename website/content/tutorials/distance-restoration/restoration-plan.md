---
title: A restoration plan
short: Restoration plan
description: Decide how much of every converted cell to restore for the most reduction of the species' extinction risk, with the model of Strassburg et al. (2020) in restoration.plan - area targets, budgets and the cheapest plan for a benefit, the exact method against the stepwise procedure, a priority gradient and the uncertainty of z.
order: 50
level: advanced
time: 60 min
template: Restoration plan
ops: [restoration.plan, restoration.original_vegetation, sdm.area_of_habitat, raster.read, raster.calc, stack.of]
series: Distance and restoration
part: 5
---

Serra Verde has 916 km² of pasture and cropland, and money to restore a ninth of it. Where should those 100 km² go?
In this part you will answer with [[restoration.plan]], the model behind the global restoration priorities of
Strassburg et al. (2020): it decides how much of every converted cell to restore so that the species' **risk of
extinction** falls the most. You will build the plan on the original vegetation and the area of habitat of the
[previous part](../original-vegetation-and-habitat/), see why it goes almost entirely to two species, move the area
target with a dashboard, compare the exact solution with Strassburg et al.'s stepwise procedure and with restoring
everywhere alike, plan with a budget, map where to begin, and report the uncertainty that the species-area exponent
brings.

## The idea

### Restoration as an optimisation

Restoration is expensive and slow, and the same hectare restored can be worth a lot or almost nothing depending on
where it is. Strassburg et al. (2019) showed for Brazil's Atlantic Forest that choosing the places carefully could
triple the conservation gains of a restoration programme and halve its costs, compared with restoring without a plan.
Strassburg et al. (2020) applied the same model to the world's converted land and mapped global priorities for
restoration, for biodiversity, climate and costs together. [[restoration.plan]] is that model; its implementation
took the authors' code as its reference, and its results were checked against it.

Its decision is **a share of every cell**: from 0 to the share of the cell that is converted (pasture and cropland,
the `restorable` map of [[restoration.original_vegetation]]). Its benefits are sums over the species (this part) and
other benefits such as carbon and intactness ([part 6](../several-benefits/)).

### The benefit: extinction risk avoided

[Part 4](../original-vegetation-and-habitat/) measured, for every species, its area of habitat today (\(H_s\)) and
before conversion (\(P_s\)), and the habitat each cell would give it if restored (\(g_{is}\) per unit of cell \(i\)).
With the species-area relationship, a species that keeps a share \(r\) of its original habitat has an extinction risk
\(1 - r^z\) (Thomas et al., 2004).

!!! science "Key idea: the risk a plan avoids"
    Restoring shares \(x_i\) of the cells gives species \(s\) the habitat \(H_s + \sum_i g_{is} x_i\), and lowers its
    risk from \(1 - (H_s/P_s)^z\) to \(1 - \big((H_s + \sum_i g_{is} x_i)/P_s\big)^z\). The benefit of the plan is the
    sum over the species of these reductions,

    \[ B(x) = \sum_s w_s \left[ \Big(\frac{H_s + \sum_i g_{is} x_i}{P_s}\Big)^{z} - \Big(\frac{H_s}{P_s}\Big)^{z} \right] \]

    to be made as large as possible subject to \(\sum_i a_i x_i \le T\), the **area target** (\(a_i\) the area of a
    cell), and \(0 \le x_i \le R_i\), the restorable share. With \(z < 1\) each term is **concave**: the first km²
    restored for a species that has lost much avoids far more risk than the last. Concave benefits have no false
    peaks, so a plan that cannot be improved by moving restoration between cells is the best of all.

Concavity is the reason the plan concentrates: the species furthest down the steep part of the curve (the golden lion
tamarin and the mangrove, which keep 56 % and 65 % of their habitat) gain the most per km², until they are restored to
their potential and the curve flattens.

### Exact and stepwise

Strassburg et al. (2020) solve the problem in **steps**: they rank the cells by their benefit per km², restore the
best ones up to a part of the target, recompute the benefits (which have fallen for the species just helped), and
repeat. It works well, but gives no guarantee, and its answer depends on the number of steps. Terraflux's default,
`method = "exact"`, solves the same problem to optimality, by a sequence of linear programs that also give an upper
**bound** on the best possible benefit; it stops when the plan and the bound agree within a tolerance (`tolerance`,
1e-7). `method = "uniform"` restores every cell by the same share of its converted land: the baseline of "restoring
everywhere alike".

## When to use it

- **Where to restore** for biodiversity, with a fixed area (a national commitment, a share of the land) or a fixed
  budget, at any scale from a watershed to the globe (Brancalion et al., 2019, mapped restoration opportunities across
  the tropical rainforests).
- **The cheapest restoration** that reaches a given benefit (`min_benefit` with a cost).
- **Sharing a target among regions** (states, municipalities, watersheds) and measuring what that rule costs in
  benefit ([part 6](../several-benefits/)).
- **Priority gradients**: where to start, and where to go next as the target grows.

It is *not* for restoring connectivity between particular patches (corridors: [part 2](../costs-and-corridors/)),
nor for choosing whole planning units against targets ([spatial prioritization](/tutorials/spatial-prioritization/linear-programming/)).
It assumes that restored land becomes habitat (the `restoration_discount` setting relaxes it), and that the
species-area relationship describes extinction risk: a coarse model, best used to compare plans.

## What you need

From [part 4](../original-vegetation-and-habitat/), for Serra Verde (60 × 90 cells of 1 km²):

- `vegetation.anthropic`: the share of every cell in pasture and cropland, the land a plan can restore;
- `habitat.table`: every species' current and potential area of habitat;
- `habitat.gain`: the habitat each species gains per unit of each cell restored.

![The share of every cell that can be restored.](/img/tutorials/distance-restoration/restoration-plan/restorable.png "The land a plan can restore: the share of every cell in pasture or cropland (916 km² in all), along the highway, the rivers and around the three cities.")

Optionally a `cost` map (thousand USD per km² restored), `regions` for targets per region, `linear` benefits (carbon)
and the human footprint for intactness (part 6). The template **Templates ▸ Conservation and restoration ▸
Restoration plan** builds the whole chain on maps drawn by chance: a forest, its cropland, three species'
ranges, their area of habitat, and a plan of 100 km².

## Build the model

The plan continues the model of part 4 with one node:

```tfgraph
#! caption: The restoration plan (node 3) on the converted land of the original vegetation (node 1) and the area of habitat of the eight species (node 2): 100 km² for the most reduction of their extinction risk. Nodes 1 and 2 are those of part 4.
#! fragment: true
#! numbered: true
#! highlight: plan
[[nodes]]
id = "vegetation"
op = "restoration.original_vegetation"
inputs = { land_cover = "cover.stack", past = "past.stack", ecoregions = "eco_shares.stack" }
params = { natural = ["forest", "savanna", "mangrove"], anthropic = ["pasture", "cropland"] }

[[nodes]]
id = "habitat"
op = "sdm.area_of_habitat"
inputs = { ranges = "ranges.stack", land_cover = "cover.stack", original = "vegetation.original", restore = "vegetation.restore", anthropic = "vegetation.anthropic" }
params = { natural = ["forest", "savanna", "mangrove"] }
note = "With the suitability of part 4."

[[nodes]]
id = "plan"
op = "restoration.plan"
inputs = { restorable = "vegetation.anthropic", table = "habitat.table", gain = "habitat.gain" }
params = { target = 100, z = 0.25, z_sd = 0.1 }

[[nodes]]
id = "show"
op = "view.plot"
inputs = { value = "plan.restored" }
params = { title = "Share of each cell restored" }
```

<div class="steps" markdown>

1. **What can be restored** (node 1's `anthropic` output to `restorable`): the share of every cell that is pasture
   or cropland.
2. **The species** (node 2's `table` to `table`, its `gain` to `gain`): the risk benefit needs both. Species whose
   `valid` column is 0 in the table (no habitat in the problem) do not count.
3. **The plan** (node 3), in the Node pane:
    - `target`: the area restored, in km² (the cells' area is read from the grid, or `cell_area`); `sense` says
      whether it is a ceiling (`<=`, the default), a floor (`>=`) or exact (`=`); `relative = true` makes it a share of
      the restorable area;
    - `z`: the species-area exponent (0.25, one number or one per species); `z_sd` its standard deviation, for the
      uncertainty of the result; `weight` the weight of the risk benefit;
    - `method`: `exact` (the default), `steps` (the stepwise procedure, with `steps` parts) or `uniform`;
    - with a `cost` map: `budget` (the most the plan may cost) or `min_benefit` (the cheapest plan reaching it);
    - `levels`: a trade-off curve and a priority gradient (below);
    - `restoration_discount`: the share of habitat value restored land does not deliver (0; for young or degraded
      restoration).
4. **Outputs**: `restored` (a map: the share of every cell restored), `summary` (the weighted benefit, the area, the
   cost, the bound and whether it is optimal, the rounds, the risk reduction and its standard deviation), `benefits`
   (each benefit's value and weight) and `species` (per species: the habitat gained, the risk without and with the
   plan, the reduction).

</div>

!!! app "In the application"
    **Templates ▸ Conservation and restoration ▸ Restoration plan** has the whole chain on maps drawn by
    chance. Select `plan`, set `target` in the Node pane and press ++ctrl+enter++: the map of shares restored opens
    in a tab; click the `species` port to see each species' risk before and after.

## Read the results

![The plan for 100 km².](/img/tutorials/distance-restoration/restoration-plan/plan_100.png "The share of every cell restored by the plan of 100 km² (grey: converted land not restored, or land that is not converted). It restores 280 cells, 278 of them in full (all their converted land): the farmland around the tamarin's two fragments, a strip of mangrove along the coast, the otter's river valleys.")

The `summary`: a risk reduction of 0.257, an area of 100 km², a bound equal to the benefit (optimal, after 2 rounds).
Of the 280 cells it touches, 278 are restored in full: the optimum is nearly all-or-nothing, because once a cell is
worth restoring for a species on the steep part of its curve, all of it is.

```tfchart
type: bar
data: /data/tutorials/distance-restoration/restoration-plan/species_100.csv
x: species
y: [before, after]
labels: { before: "Risk without the plan", after: "Risk with the plan" }
colors: { before: "#c9d8c4", after: "#3b8a5a" }
ylabel: Extinction risk (1 - r^0.25)
height: 300
caption: "Every species' extinction risk without and with the plan of 100 km² (the species output). The golden lion tamarin is restored to its full potential habitat (45 km² gained, risk 0.136 to 0) and the mangrove almost (23 km², 0.104 to 0.004); the otter and the tapir gain a little; the others, which have lost little, nothing."
```

| Species | Habitat gained (km²) | Risk before | Risk after |
|---|---|---|---|
| Jaguar | 5.5 | 0.014 | 0.013 |
| Lowland tapir | 11.1 | 0.045 | 0.041 |
| Harpy eagle | 0 | 0.000 | 0.000 |
| Maned wolf | 0 | 0.011 | 0.011 |
| Golden lion tamarin | 45.0 | 0.136 | 0.000 |
| Neotropical otter | 22.6 | 0.045 | 0.028 |
| Araucaria forest | 0 | 0.011 | 0.011 |
| Mangrove | 23.3 | 0.104 | 0.004 |

Two species carry 92 % of the risk avoided (0.136 + 0.100 of 0.257). The plan does what the model asks: it brings the
species furthest down the species-area curve back up it. Whether that is the right goal is a question for the
people who make the plan: a plan for carbon or for intactness would look very different (part 6).

### Reporting a restoration plan

Report the land cover and which classes are restorable, the source of the original vegetation and of the species'
ranges and habitats (part 4), the benefit and its parameters (z and its standard deviation, any discount), the target
or budget, the method (exact, with its tolerance and bound), and the results: the area, the cost, the risk reduction
with its uncertainty, the change of every species' risk, and the map of shares restored.

## Settings that change the answer

### The area target

```tfslider
label: Area restored (km²)
caption: "The plan for every target from 25 to 600 km². The first 25 km² go to the tamarin's fragments and the mangrove coast; then the otter's rivers; from 150 km² the tapir's and the jaguar's forests along the rivers. Beyond about 400 km² the plan stops growing: every converted cell left gives habitat to no species at risk, and the target is a ceiling (sense <=), not an obligation. In the chart, the exact plan and the stepwise procedure overlap (toggle one off to see the other); restoring the same share everywhere avoids far less."
frames:
  - { value: 25, src: /img/tutorials/distance-restoration/restoration-plan/target_25.png, stats: { "Risk avoided": "0.097", "Cells restored": "65" } }
  - { value: 50, src: /img/tutorials/distance-restoration/restoration-plan/target_50.png, stats: { "Risk avoided": "0.175", "Cells restored": "122" } }
  - { value: 100, src: /img/tutorials/distance-restoration/restoration-plan/target_100.png, stats: { "Risk avoided": "0.257", "Cells restored": "280" } }
  - { value: 150, src: /img/tutorials/distance-restoration/restoration-plan/target_150.png, stats: { "Risk avoided": "0.290", "Cells restored": "414" } }
  - { value: 200, src: /img/tutorials/distance-restoration/restoration-plan/target_200.png, stats: { "Risk avoided": "0.312", "Cells restored": "624" } }
  - { value: 300, src: /img/tutorials/distance-restoration/restoration-plan/target_300.png, stats: { "Risk avoided": "0.346", "Cells restored": "879" } }
  - { value: 400, src: /img/tutorials/distance-restoration/restoration-plan/target_400.png, stats: { "Risk avoided": "0.359", "Cells restored": "1 209" } }
  - { value: 600, src: /img/tutorials/distance-restoration/restoration-plan/target_600.png, stats: { "Risk avoided": "0.359", "Cells restored": "1 209" } }
start: 2
chart:
  type: line
  data: /data/tutorials/distance-restoration/restoration-plan/target.csv
  x: target
  y: [exact, steps, uniform]
  labels: { exact: "Exact plan", steps: "Stepwise (10 steps)", uniform: "Same share everywhere" }
  colors: { exact: "#3b8a5a", steps: "#e8901f", uniform: "#8a6d2f" }
  xlabel: Area restored (km²)
  ylabel: Risk avoided
```

The curve of risk avoided rises steeply and then flattens: the first 50 km² avoid 0.175, the next 50 only 0.083, and
the 200 km² from 200 to 400 less than half of what the first 25 do. The total risk of the eight species is 0.366; restoring every cell that
helps any of them avoids 0.359 of it. This diminishing return is the most useful figure of a restoration plan: it says
how much each further km² (and each further million) buys.

### Exact, stepwise and uniform

```tfgallery
columns: 3
caption: "Three plans of 100 km² (the same colour scale, 0 to 1 of the cell restored). Exact: 0.257 of risk avoided, proven optimal. Stepwise (10 steps of 10 km²): 0.257, within 0.1 % of it here, with no bound. The same share everywhere: 0.045, a sixth; every converted cell restored by 11 % of its converted land."
images:
  - { src: /img/tutorials/distance-restoration/restoration-plan/method_exact.png, caption: "Exact" }
  - { src: /img/tutorials/distance-restoration/restoration-plan/method_steps.png, caption: "Stepwise" }
  - { src: /img/tutorials/distance-restoration/restoration-plan/method_uniform.png, caption: "Uniform" }
```

![The uniform plan on its own scale.](/img/tutorials/distance-restoration/restoration-plan/method_uniform_scaled.png "The uniform plan of 100 km² on a scale of 0 to 0.12: every converted cell restored by 11 % of its converted land, most where it is most converted, around the cities and along the highway, where few species at risk live.")

On Serra Verde, the stepwise procedure comes within 0.1 % of the optimum at every target: with one benefit and
smooth curves, a greedy method does well. The exact method's value is the proof (the bound), and its behaviour with
several benefits, budgets and regional targets, where steps can go astray. Restoring everywhere alike avoids almost
six times less risk for the same area: most of Serra Verde's converted land gives habitat to species that have lost
little.

### A budget

With a `cost` map (thousand USD per km² restored), `budget` limits the spending:

```tfchart
type: line
data: /data/tutorials/distance-restoration/several-benefits/budget.csv
x: budget
y: [reduction]
labels: { reduction: "Risk avoided" }
colors: { reduction: "#3b8a5a" }
xlabel: Budget (million USD)
ylabel: Risk avoided
height: 260
caption: "The plan for budgets from 1 to 12 million USD (with the target of 100 km² as a ceiling). The first million avoids 0.050 of risk with 12 km²; past about 9.3 million the area target, not the budget, limits the plan."
```

The inverse question, the cheapest plan that reaches a benefit, is `min_benefit`. The cheapest plan avoiding 95 % of
the risk of the first plan (0.244) restores 85 km² for 7.9 million USD; the first plan cost 9.2 million. The last 5 %
of the benefit costs 1.3 million, 14 % of the money: a number decision makers want to see.

![The cheapest plan for 95 % of the benefit.](/img/tutorials/distance-restoration/several-benefits/cheapest.png "The cheapest plan that avoids 95 % of the risk the first plan avoids: 85 km² for 7.9 million USD. It drops the dearest coastal cells and keeps the tamarin's fragments, whose farmland is cheap.")

### Where to begin: a priority gradient

A plan of 200 km² does not say where to start. With `levels = [0.25, 0.5, 0.75, 1]` (shares of the target),
[[restoration.plan]] solves a plan for each level, each containing the previous one (`nested`, true by default), and
returns a `tradeoff` table and a `gradient` map: how many levels act on every cell.

![The priority gradient of a plan of 200 km².](/img/tutorials/distance-restoration/restoration-plan/gradient.png "The priority gradient of a plan of 200 km² in four levels: the cells restored in the first 50 km² (dark), by 100, by 150 and only at 200 km² (pale). The tamarin's fragments and the mangrove come first; the rivers next; the tapir's forests last.")

The first quarter of the area (50 km²) avoids 0.175 of the 0.312 the whole plan avoids, 56 %: begin there.

### The species-area exponent z

| z | Risk avoided by the plan of 100 km² |
|---|---|
| 0.15 | 0.158 |
| 0.25 | 0.257 |
| 0.35 | 0.351 |

z is uncertain: 0.25 is a typical value, but estimates range from about 0.15 to 0.35. Here the tamarin and the mangrove get the
same habitat whatever z (they still come first), but the size of the benefit changes with it. With `z_sd =
0.1`, the `summary` reports the standard deviation of the risk avoided: 0.257 ± 0.096 when the species' z vary together
(`uncertainty = "correlated"`, the default, as when they come from one study), ± 0.063 when they vary independently.
Report the benefit with its uncertainty; compare plans on the same z.

## Common pitfalls

- **A target larger than what helps.** The target is a ceiling by default: beyond the useful area, the plan stops
  (here at about 400 km²). Use `sense = "="` only when the area must be restored whatever it gives.
- **Species with no loss.** A species that has lost no habitat has no risk to avoid; a plan for it alone restores
  nothing. Check the `table` of part 4.
- **Reading the risk as a probability of extinction.** It is an index from a species-area relationship, good for
  comparing plans and places, not for predicting extinctions.
- **Restored land as full habitat.** Young restoration is not old forest; `restoration_discount` counts it for less.
- **A plan without its cost.** Add the cost of restoring (the land, the work, the income forgone), and show what
  each budget buys.

## Going further

- [Several benefits and trade-offs](../several-benefits/), the next part: carbon, intactness and ecosystems together,
  weights, and targets per region.
- The inputs: [Original vegetation and area of habitat](../original-vegetation-and-habitat/).
- Choosing whole planning units against targets: [Optimal plans with linear programming](/tutorials/spatial-prioritization/linear-programming/).
- The operator: [[restoration.plan]].

## References

- Brancalion, P. H. S., Niamir, A., Broadbent, E., Crouzeilles, R., Barros, F. S. M., Almeyda Zambrano, A. M., et al. (2019). Global restoration opportunities in tropical rainforest landscapes. *Science Advances*, 5(7), eaav3223. https://doi.org/10.1126/sciadv.aav3223
- Strassburg, B. B. N., Beyer, H. L., Crouzeilles, R., Iribarrem, A., Barros, F., de Siqueira, M. F., et al. (2019). Strategic approaches to restoring ecosystems can triple conservation gains and halve costs. *Nature Ecology & Evolution*, 3(1), 62–70. https://doi.org/10.1038/s41559-018-0743-8
- Strassburg, B. B. N., Iribarrem, A., Beyer, H. L., Cordeiro, C. L., Crouzeilles, R., Jakovac, C. C., et al. (2020). Global priority areas for ecosystem restoration. *Nature*, 586(7831), 724–729. https://doi.org/10.1038/s41586-020-2784-9
- Thomas, C. D., Cameron, A., Green, R. E., Bakkenes, M., Beaumont, L. J., Collingham, Y. C., et al. (2004). Extinction risk from climate change. *Nature*, 427(6970), 145–148. https://doi.org/10.1038/nature02121
