---
title: Carbon storage and sequestration
short: 2. Carbon
description: InVEST's carbon model in Terraflux - the four carbon pools of each land use class, the carbon a landscape stores, what a scenario gains or loses, its net present value with a price and a discount rate, and how to read and report the totals.
order: 20
level: beginner
time: 40 min
ops: [services.carbon, table.read, raster.read, view.plot]
series: Ecosystem services with InVEST
part: 2
---

How much carbon does a landscape hold, and how much would it lose if the forest at its edges became pasture? In
this tutorial you will build InVEST's carbon model in Terraflux, node by node, on the landscape of Ribeirão Claro:
first the carbon stored in 2020, pool by pool, then the change to a scenario of 2040, then the value of that
change with a price and a discount rate. At the end you will know where the carbon is, which conversions lose
most of it, how much the answer depends on the numbers in the table and on the discount rate, and what to report.

## The idea

Plants take carbon dioxide from the air and keep the carbon in their wood, roots and leaves; part of it moves
into the litter, the dead wood and the soil. Land use decides how much stays: a hectare of mature tropical forest
can hold well over a hundred tons of carbon in its trees alone, a hectare of pasture only a few, although its soil
may still hold a lot. Converting one into the other sends the difference to the atmosphere; restoring the forest
slowly takes it back. **Carbon storage** is the amount a landscape holds now; **sequestration** (in InVEST's sense)
is the change of that amount between two maps.

InVEST's carbon model is the simplest of all its models, and that is its strength. It follows the bookkeeping of
the IPCC's guidelines for national inventories (IPCC, 2006): every land use class has a typical **carbon density**,
in tons of carbon per hectare, in each of four **pools**:

| Pool | Column | What it holds |
|---|---|---|
| Aboveground biomass | `c_above` | all living plant material above the soil: trunks, branches, bark, leaves |
| Belowground biomass | `c_below` | the living roots |
| Soil organic matter | `c_soil` | the organic carbon of the soil, usually the largest pool on land |
| Dead organic matter | `c_dead` | litter and lying or standing dead wood |

The densities are averages for a class: the model assumes that every hectare of "forest" holds the same carbon,
whatever its age or its neighbours, and that a class neither gains nor loses carbon over time.

!!! science "Key idea: storage is density times area"
    The carbon of a cell \(x\) of class \(j\) is the sum of its four pools,

    \[ C_x = c_{\text{above},j} + c_{\text{below},j} + c_{\text{soil},j} + c_{\text{dead},j} \quad (\text{t C / ha}), \]

    and the carbon of the landscape is the sum over its cells of \(C_x\) times the area of a cell in hectares
    (\(a = 30 \times 30 / 10\,000 = 0.09\) ha for cells of 30 m). With a second map, the **change** of a cell is
    \(\Delta C_x = C^{\text{future}}_x - C^{\text{baseline}}_x\): negative where carbon is lost (emitted),
    positive where it is gained.

A cell of forest holds \(140 + 35 + 95 + 12 = 282\) t C / ha in this tutorial's table; the same cell as pasture
holds \(3 + 6 + 60 + 0.5 = 69.5\). Clearing it loses \(212.5\) t C / ha, or \(212.5 \times 0.09 = 19.1\) tons of
carbon for the cell, about 70 tons of CO₂ (multiply carbon by 44/12 = 3.67 to get CO₂).

### What the change is worth

A ton of carbon kept out of the atmosphere avoids damages: InVEST's guide (Natural Capital Alliance, 2026)
recommends valuing it at the *social cost of carbon*, an estimate of those damages per additional ton emitted, rather than at a market price. The
change between the two maps happens over the years between them; InVEST spreads it evenly over those years and
discounts each year's share to the present.

!!! science "Key idea: the net present value of a change"
    For a change \(\Delta C_x\) between the baseline year and the future year, \(n\) years apart, the value per
    hectare is

    \[ V_x = \frac{P\,\Delta C_x}{n} \sum_{t=0}^{n-1} \frac{1}{\left(1 + \frac{r}{100}\right)^t \left(1 + \frac{c}{100}\right)^t} \]

    where \(P\) is the price of a ton of carbon (`price`), \(r\) the annual **discount rate** in percent
    (`discount_rate`: how much less a benefit is worth for every year it is delayed), and \(c\) the annual
    **change in the price** in percent (`rate_change`). A positive \(c\) lowers the value of the later years
    further: it says that a ton sequestered later is worth less to society than one sequestered now, so it acts
    like a second discount rate.

With a price of 50 per ton, \(r = 5\), \(c = 0\) and \(n = 20\) years (2020 to 2040), the factor
\(\frac{P}{n}\sum\dots\) is \(2.5 \times 13.085 = 32.71\): the cleared cell above is worth
\(-212.5 \times 32.71 = -6\,952\) per hectare.

## When to use it

- **Scenarios of land use**: the carbon a development plan would emit, the carbon a restoration plan would
  recover, as tons and as money.
- **Carbon beside other services and species.** Nelson et al. (2008) modelled payments to the landowners of the
  Willamette Basin (Oregon): payments for carbon raised carbon sequestration but did little for the conservation
  of species, payments for species did not raise carbon, and no policy came close to what the landscape could
  provide for both.
- **Mapping where carbon is**, to prioritize protection or to check a REDD+ baseline with a simple, transparent
  method.

What it is **not** for: the model does not grow forests. A restored forest gets the density of its class at
once, and an old forest and a young one hold the same carbon unless you give them different classes. It knows
nothing of fire, harvest or the time soil carbon takes to change. For carbon from a continuous map (tree cover or
canopy height instead of classes), use [[raster.calc]]; for the uncertainty of the densities, see
[Monte Carlo uncertainty](/tutorials/uncertainty/monte-carlo/).

## What you need

- **A land use map** (the *baseline*), projected in metres, with integer class codes. Here
  `lulc_2020.tif` of [Ribeirão Claro](/tutorials/ecosystem-services/ideas/) (download:
  [ribeirao-claro.zip](/data/tutorials/ecosystem-services/ideas/ribeirao-claro.zip)).
- **A table of carbon pools**, a CSV file with the columns `lucode`, `c_above`, `c_below`, `c_soil` and
  `c_dead`, in tons of carbon per hectare (InVEST writes Mg C / ha: a megagram is a ton). Every class of the maps
  must have a row. Where do the numbers come from? Ideally from forest inventories and soil samples in your study
  area; otherwise from the IPCC's default values for your climate zone (IPCC, 2006) or from regional studies. The
  values used here are illustrative:

```tfchart
type: bar
data: /data/tutorials/ecosystem-services/carbon/pools.csv
x: class
y: [c_above, c_below, c_soil, c_dead]
labels: { c_above: "Aboveground", c_below: "Belowground", c_soil: "Soil", c_dead: "Dead matter" }
colors: { c_above: "#3b8a5a", c_below: "#a7c957", c_soil: "#8c564b", c_dead: "#d9a441" }
stacked: true
xlabel: Class
ylabel: t C / ha
height: 300
caption: "The carbon pools of this tutorial (carbon_pools.csv), by class. Forest holds 282 t C / ha, eucalyptus 169, pasture 69.5, cropland 50.5, the town 36 and water none. Soil carbon differs less between the classes than biomass does."
```

- **A future land use map** (optional), on the same grid: here `lulc_2040.tif`.
- **For the valuation** (optional): a price per ton of carbon, a discount rate, the change of the price, and the
  years of the two maps.

## Build the model

### Step 1: the carbon stored

<div class="steps" markdown>

1. Add a [[raster.read]] node (library subject *Getting data*) and set its `path` to `lulc_2020.tif`: the
   baseline land use.
2. Add a [[table.read]] node with `path` = `carbon_pools.csv`: the pools by class.
3. Add [[services.carbon]] (subject *Ecosystem services*). Connect the land use's `raster` to its input `lulc`
   and the table to its input `pools`.
4. Add a [[view.plot]] node and connect the carbon node's output `storage` to its input `value`. Select it and
   press ++ctrl+enter++.

</div>

```tfgraph
#! caption: The carbon stored in 2020: land use, the pools table, the model and a plot of the storage.
#! highlight: carbon, show
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "lulc"
op = "raster.read"
params = { path = "lulc_2020.tif" }
note = "Baseline land use, 30 m, UTM 23S."

[[nodes]]
id = "pools"
op = "table.read"
params = { path = "carbon_pools.csv" }
note = "t C / ha in four pools, by class."

[[nodes]]
id = "carbon"
op = "services.carbon"
inputs = { lulc = "lulc.raster", pools = "pools.table" }

[[nodes]]
id = "show"
op = "view.plot"
inputs = { value = "carbon.storage" }
params = { title = "Carbon stored, t C / ha" }
```

The carbon node gives:

- `storage`: the carbon of each cell, t C / ha (InVEST's `c_storage_bas`);
- `pools`: a *stack* (several maps on one grid, each with a name) of the four pools, its layers named
  `c_above`, `c_below`, `c_soil` and `c_dead`;
- `totals`: a table of one row with the landscape's tons of carbon in the column `baseline`.

Cells with no land use have no carbon (no value), and are not counted.

### Step 2: a future map

Add a second [[raster.read]] with `path` = `lulc_2040.tif` and connect it to the input `future`. The node now
also gives `storage_future` and `pools_future` (the same maps for 2040), `change` (2040 minus 2020, per hectare)
and, in `totals`, the columns `alternate` (the tons of 2040) and `change`.

```tfgraph
#! caption: A future land use connected to the carbon model: storage in 2040 and the change.
#! highlight: future
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "lulc"
op = "raster.read"
params = { path = "lulc_2020.tif" }

[[nodes]]
id = "future"
op = "raster.read"
params = { path = "lulc_2040.tif" }
note = "The scenario of 2040, same grid."

[[nodes]]
id = "pools"
op = "table.read"
params = { path = "carbon_pools.csv" }

[[nodes]]
id = "carbon"
op = "services.carbon"
inputs = { lulc = "lulc.raster", pools = "pools.table", future = "future.raster" }

[[nodes]]
id = "show"
op = "view.plot"
inputs = { value = "carbon.change" }
params = { title = "Change, t C / ha" }
```

### Step 3: the value of the change

Select the carbon node and fill five settings in the Node pane: `price` (50, in your currency per ton of
**carbon**), `discount_rate` (5, percent per year), `rate_change` (0, percent per year), `year` (2020) and
`future_year` (2040). The node now gives the map `npv`, the net present value of each hectare's change, and the
column `npv` of `totals`. The valuation needs the future map, and the future year must come after the baseline
year: otherwise the node stops and says why.

```tfgraph
#! caption: The full carbon model: storage, change and its net present value; the totals written to a CSV file.
#! highlight: carbon, totals
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "lulc"
op = "raster.read"
params = { path = "lulc_2020.tif" }

[[nodes]]
id = "future"
op = "raster.read"
params = { path = "lulc_2040.tif" }

[[nodes]]
id = "pools"
op = "table.read"
params = { path = "carbon_pools.csv" }

[[nodes]]
id = "carbon"
op = "services.carbon"
inputs = { lulc = "lulc.raster", pools = "pools.table", future = "future.raster" }
params = { price = 50, discount_rate = 5, rate_change = 0, year = 2020, future_year = 2040 }
note = "A price per ton of carbon, 5% a year, 20 years."

[[nodes]]
id = "totals"
op = "table.write"
inputs = { table = "carbon.totals" }
params = { path = "carbon_totals.csv" }
```

The same from the command line, with the model saved as `carbon.toml` in the folder of the data:

```bash
terraflux run carbon.toml --out results
```

## Read the results

### Where the carbon is

```tfgallery
columns: 2
caption: The four pools of 2020 on one colour scale (0 to 140 t C / ha). Aboveground biomass draws the forest and the plantations sharply; the soil pool is everywhere, because pasture and cropland keep most of their soil carbon.
images:
  - { src: /img/tutorials/ecosystem-services/carbon/c_above.png, caption: "c_above" }
  - { src: /img/tutorials/ecosystem-services/carbon/c_below.png, caption: "c_below" }
  - { src: /img/tutorials/ecosystem-services/carbon/c_soil.png, caption: "c_soil" }
  - { src: /img/tutorials/ecosystem-services/carbon/c_dead.png, caption: "c_dead" }
```

![Carbon stored in 2020](/img/tutorials/ecosystem-services/carbon/storage_2020.png "The carbon stored in 2020 (output storage), t C / ha: 282 in the forest, 169 in the plantations, 69.5 in pasture, 50.5 in cropland. The landscape holds 583,480 tons of carbon.")

The map has only six values, one per class: it is the land use map seen through the table. All of its
information is in the table and in the areas of the classes, which is why the totals by class are worth a look:

```tftable
data: /data/tutorials/ecosystem-services/carbon/by-class.csv
labels: { class: "Class", "2020": "t C in 2020", "2040": "t C in 2040" }
digits: 0
caption: "The carbon of each class: the sum of the storage map over its cells times 0.09 ha. The forest, 40% of the landscape in 2020, holds three quarters of its carbon."
```

### What the scenario changes

```tfcompare
caption: "Drag the handle: the carbon stored in 2020 (left) and in the scenario of 2040 (right), on the same scale (0, white-yellow, to 282 t C / ha, dark green)."
before: { src: /img/tutorials/ecosystem-services/carbon/storage_2020_plain.png, label: "2020" }
after: { src: /img/tutorials/ecosystem-services/carbon/storage_2040_plain.png, label: "2040" }
```

![Change of carbon from 2020 to 2040](/img/tutorials/ecosystem-services/carbon/change.png "The change (output change), t C / ha: red where carbon is lost, blue where it is gained. The cleared forest edges lose 212.5 (forest to pasture) or 231.5 (forest to cropland); pasture turned into cropland loses 19; the riparian strips restored to forest gain over 200.")

The totals table holds the landscape's numbers:

```tftable
data: /data/tutorials/ecosystem-services/carbon/totals.csv
labels: { column: "Column of totals", meaning: "Meaning", value: "Value", unit: "Unit" }
digits: 1
caption: "The output totals of the full model: the scenario of 2040 loses 112,020 t C (19% of the stock), worth -3.66 million at a price of 50 per ton, 5% a year over 20 years."
```

Which conversions cause the loss? Cross the two land use maps with the change map:

```tftable
data: /data/tutorials/ecosystem-services/carbon/transitions.csv
labels: { from: "From (2020)", to: "To (2040)", hectares: "Hectares", t_C: "Change, t C" }
digits: 1
caption: "Every conversion between 2020 and 2040, its area and the carbon it gains or loses. Clearing 532 ha of forest for pasture explains almost the whole loss; the 155 ha of riparian restoration recover 34,000 t, less than a third of what clearing 614 ha of forest costs."
```

This is the kind of table to report: it says which decisions matter. Here the riparian restoration does not
compensate the clearing, because it restores a quarter of the area and every hectare restored gains about what a
hectare cleared loses.

!!! paper "Reporting a carbon analysis"
    Give the source of every density (with the table in a supplement), the area of each class in each map, the
    total storage of each scenario and the change, in tons of carbon (or of CO₂, saying so), and the map of the
    change. For a valuation, give the price, its unit (per ton of C or of CO₂), the discount rate and the change
    of the price, and test other rates (next section).

## Settings that change the answer

### The discount rate

The net present value depends strongly on the discount rate and on the change of the price. The chart below
comes from running the full model with each pair of rates:

```tfchart
type: line
data: /data/tutorials/ecosystem-services/carbon/discount.csv
x: discount
y: [change_0, change_2, change_5]
labels: { change_0: "Price change 0%", change_2: "Price change 2%", change_5: "Price change 5%" }
colors: { change_0: "#3b8a5a", change_2: "#e8901f", change_5: "#2f6f9f" }
xlabel: Discount rate (% per year)
ylabel: Net present value of the change (millions)
caption: "The value of the loss of 112,020 t C between 2020 and 2040, at 50 per ton, for discount rates from 0 to 12% and three annual changes of the price. Without discounting the loss is worth 5.6 million; at 12%, 2.3 million. A positive change of the price lowers the value as a discount rate does."
```

With no discounting and no change of the price, the value is simply the price times the change (50 times
112,020 t = 5.6 million). Every percent of discount rate lowers it. The choice is an ethical one (how much the
future counts), so report the rate and show the answer for a few.

### The numbers in the table

The densities are the model's only parameters, and some are much more uncertain than others. Soil carbon under
pasture, for example, varies widely with the age and management of the pasture. Move it from 40 to 95 t C / ha
and watch what happens:

```tfslider
label: Soil carbon of pasture (t C / ha)
caption: "The change of carbon from 2020 to 2040 with the soil carbon of pasture (c_soil of class 2) from 40 to 95 t C / ha, the other densities as before. The stock of 2020 grows by 14% across the range, but the change by only 8%: what matters for a change is the difference between the classes that are converted, not their absolute values."
frames:
  - { value: 40, src: /img/tutorials/ecosystem-services/carbon/soil_40.png, stats: { "Stock 2020": "555,047 t C", "Change": "-108,662 t C" } }
  - { value: 50, src: /img/tutorials/ecosystem-services/carbon/soil_50.png, stats: { "Stock 2020": "569,263 t C", "Change": "-110,341 t C" } }
  - { value: 60, src: /img/tutorials/ecosystem-services/carbon/soil_60.png, stats: { "Stock 2020": "583,480 t C", "Change": "-112,020 t C" } }
  - { value: 70, src: /img/tutorials/ecosystem-services/carbon/soil_70.png, stats: { "Stock 2020": "597,696 t C", "Change": "-113,700 t C" } }
  - { value: 80, src: /img/tutorials/ecosystem-services/carbon/soil_80.png, stats: { "Stock 2020": "611,912 t C", "Change": "-115,379 t C" } }
  - { value: 95, src: /img/tutorials/ecosystem-services/carbon/soil_95.png, stats: { "Stock 2020": "633,237 t C", "Change": "-117,898 t C" } }
start: 2
```

Why does the change move the other way (more loss with *more* soil carbon in pasture)? Pasture both receives
forest (where more soil carbon in pasture means a *smaller* loss) and gives way to cropland, the town,
plantations and restored forest (where it means a *larger* loss or a *smaller* gain). In this scenario 532 ha of
forest become pasture but 700 ha of pasture become something else, so the second effect wins. The general lesson: test the densities of the classes involved
in the conversions, and expect storage and change to respond differently. To propagate the uncertainty of all the
densities at once, run the model many times with values drawn from their ranges
([Monte Carlo uncertainty](/tutorials/uncertainty/monte-carlo/)).

## Common pitfalls

- **Carbon or CO₂.** InVEST's tables are in tons of carbon, and its price is per ton of carbon. Prices of the
  social cost of carbon are often per ton of CO₂: multiply them by 3.67 before using them as `price`.
- **Biomass or carbon.** Forest inventories often give dry biomass; carbon is roughly half of it (the IPCC's
  default carbon fraction is 0.47 for wood). Dividing or not makes a factor of two.
- **Instant change.** A forest restored in 2040 holds 282 t C / ha in this model, although real regrowth would
  take decades to get there. Make young forest its own class if the timing matters.
- **Soil carbon changes slowly.** The model moves the whole soil pool at once when a class changes; field data
  suggest soil carbon takes years to decades to settle.
- **Cells without a value.** A cell that is missing in either map has no change and no value, and is not
  counted: compare totals only over the cells both maps have.
- **Years in the wrong order.** `future_year` before `year` stops the model; equal years too (there is no time
  to spread the change over).

## Going further

- [Habitat quality and rarity](/tutorials/ecosystem-services/habitat-quality/), the next part: the same
  landscape seen as habitat.
- [Ecosystem service models: the ideas](/tutorials/ecosystem-services/ideas/) for the landscape and the series'
  models together.
- [Monte Carlo uncertainty](/tutorials/uncertainty/monte-carlo/) to carry the uncertainty of the densities into
  the totals.
- [Several benefits and trade-offs](/tutorials/distance-restoration/several-benefits/) for carbon beside
  biodiversity in a restoration plan.
- The operator's reference: [[services.carbon]].

## References

- IPCC. (2006). *2006 IPCC Guidelines for National Greenhouse Gas Inventories, Volume 4: Agriculture, Forestry and Other Land Use* (H. S. Eggleston, L. Buendia, K. Miwa, T. Ngara, & K. Tanabe, Eds.). Institute for Global Environmental Strategies.
- Natural Capital Alliance. (2026). *InVEST 3.20.2 User's Guide: Carbon storage and sequestration*. https://storage.googleapis.com/releases.naturalcapitalproject.org/invest-userguide/latest/en/carbonstorage.html
- Nelson, E., Polasky, S., Lewis, D. J., Plantinga, A. J., Lonsdorf, E., White, D., Bael, D., & Lawler, J. J. (2008). Efficiency of incentives to jointly increase carbon sequestration and species conservation on a landscape. *Proceedings of the National Academy of Sciences*, 105(28), 9471–9476. https://doi.org/10.1073/pnas.0706178105
