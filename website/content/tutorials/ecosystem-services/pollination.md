---
title: Crop pollination by wild bees
short: 8. Pollination
description: InVEST's crop pollination model in Terraflux - where each guild of wild bees nests and finds flowers, how far it flies, the pollinator supply and abundance it gives, the pollination and yield of coffee farms, and what a scenario of 2040 costs the farms.
order: 80
level: intermediate
time: 50 min
ops: [services.pollination, table.read, vector.read, raster.read, table.write]
series: Ecosystem services with InVEST
part: 8
---

Coffee flowers for a few days after the first rains, and the bees that visit it nest somewhere else: in the
ground of a pasture, in the hollow trunks of a forest, in dead wood. How many reach a farm depends on the land
around it, which makes pollination a service of the *landscape*, not of the field. In this tutorial you will build
InVEST's crop pollination model in Terraflux on Ribeirão Claro, with its cropland as coffee: you will describe
three guilds of wild bees and the resources each land use offers them, map where they come from and where they
forage, put sixteen coffee farms on the map and read their pollination and yield, see how the bees' flight distance
and the crop's half saturation change the answer, and compare the scenario of 2040 with 2020. At the end you will
know what the model's maps mean, where its numbers come from, and how not to over-read them.

## The idea

Most crops grown for fruit or seed yield more when animals pollinate them. Klein et al. (2007) found that 87 of
the leading global food crops depend on animal pollination to some degree, while 28 do not; Ricketts et al.
(2004) measured coffee yields 20% higher within about 1 km of a forest in Costa Rica, thanks to the bees that live
in it. Wild bees are *central-place foragers*: they nest in one place and fly out to the flowers around it, so a
farm gets bees only if nesting sites and flowers lie within flight range of it. Kremen et al. (2007) put this into a
conceptual framework for services delivered by mobile organisms, and Lonsdorf et al. (2009) turned it into the
first quantitative model that predicts the abundance of pollinators across a landscape from land use. That model
is InVEST's crop pollination model.

The model works with **guilds**: groups of species that nest and forage alike (ground-nesting bees, bees nesting in
cavities, bumble bees...), or with single species when they are known well. For each guild it asks four questions
of every cell:

1. **Can it nest here?** Each land use offers some availability of each nesting *substrate* (bare ground,
   cavities in trees, stems...), and each guild uses some substrates.
2. **What flowers are within its reach?** Each land use has floral resources in each *season*, and each guild is
   more or less active in each season. The flowers around a cell are weighted by distance, with a kernel set by the
   guild's typical **foraging distance**.
3. **How many bees does it send out?** The **pollinator supply**: nesting sites times the flowers within reach.
4. **How many bees forage here?** The **pollinator abundance**: the bees nesting around a cell, shared out among
   the flowers they can reach.

!!! science "Key idea: from nesting and flowers to pollinator supply"
    For a cell \(x\) of land use \(l(x)\) and a guild \(s\), the **nesting suitability** is that of the best
    substrate:

    \[ HN(x,s) = \max_n \big[\, N(l(x),n)\; ns(s,n) \,\big] \]

    where \(N(l,n)\) is the availability of substrate \(n\) in land use \(l\) (0 to 1) and \(ns(s,n)\) how much
    the guild uses it (0 to 1). The flowers the guild finds in the cell are the floral resources of each season
    \(j\), \(RA(l,j)\) (0 to 1), weighted by its **relative activity** \(fa(s,j)\) (its activity indices divided
    by their sum): \(FE(x,s) = \sum_j RA(l(x),j)\, fa(s,j)\). The **floral resources within reach** are their
    mean around \(x\), weighted by distance:

    \[ FR(x,s) = \frac{\sum_{x'} e^{-D(x,x')/\alpha_s}\, FE(x',s)}{\sum_{x'} e^{-D(x,x')/\alpha_s}} \]

    where \(D(x,x')\) is the distance between the cells and \(\alpha_s\) the guild's typical foraging distance.
    The **pollinator supply** is

    \[ PS(x,s) = FR(x,s)\; HN(x,s)\; sa(s), \]

    with \(sa(s)\) the guild's relative abundance (its share of all the guilds' abundances, which sum to 1).

!!! science "Key idea: pollinator abundance, where the bees forage"
    The bees nesting around \(x\) are shared out among the flowers they can reach, in proportion to the flowers.
    The **abundance** of guild \(s\) in season \(j\) is

    \[ PA(x,s,j) = \frac{RA(l(x),j)\, fa(s,j)}{FR(x,s)}\;
       \frac{\sum_{x'} PS(x',s)\, e^{-D(x,x')/\alpha_s}}{\sum_{x'} e^{-D(x,x')/\alpha_s}} \]

    the supply within flight range, times the flowers of the cell relative to the flowers within reach. The
    **total abundance** of a season adds the guilds: \(PAT(x,j) = \sum_s PA(x,s,j)\). Both are indices (no
    units), not numbers of bees.

The kernel \(e^{-D/\alpha}\) gives every cell a weight that falls with distance: at \(D = \alpha\) a flower counts
37% of one next to the nest, at \(2\alpha\) 14%. Terraflux, as InVEST's code, computes it in cells (\(\alpha\)
divided by the cell size), cuts it at \(5\alpha\), where the weight is below 1%, and divides it by its sum.

![The foraging kernels of the three guilds](/img/tutorials/ecosystem-services/pollination/kernel.png "The weight of a flower with its distance from the nest, exp(-d / alpha), for the three guilds of this tutorial. The dots mark where the kernel is cut, at five times alpha. A guild with a larger alpha draws on flowers, and sends its bees, farther.")

A worked cell: take the stingless bees (nesting only in cavities, equally active in both seasons, so
\(fa = 0.5\) in each; \(sa = 0.4\)) and a coffee cell whose flight range is half forest and half coffee, the flowers
being the same everywhere in it. With this tutorial's table, forest offers \(FE = 0.5 \times 0.7 + 0.5 \times 0.5
= 0.6\) and coffee \(0.5 \times 0.6 + 0.5 \times 0.1 = 0.35\), so \(FR = 0.475\). The forest's cells supply
\(PS = 0.475 \times 1.0 \times 0.4 = 0.19\) bees, the coffee's \(0.475 \times 0.05 \times 0.4 = 0.0095\): a mean
of 0.0998 within range. In the wet season the coffee cell gets \(PA = (0.6 \times 0.5 / 0.475) \times 0.0998 =
0.063\). On the real map the stingless bees' mean abundance on the coffee within 60 m of forest is 0.065.

### On the farms

With **farms**, polygons of a crop, the model turns the abundance into pollination and yield. Each farm names the
season its crop is pollinated, its **half-saturation** constant \(h\), the share of its pollination given by
managed bees (hives), and how much of its yield depends on pollinators.

!!! science "Key idea: pollination and yield"
    On a farm \(f\) whose crop is pollinated in season \(j\), the pollination owed to the wild bees is a
    saturating curve of their abundance:

    \[ FP(x) = \frac{PAT(x,j)\,(1 - h)}{h\,\big(1 - 2\,PAT(x,j)\big) + PAT(x,j)} \]

    which is 0 with no bees, 0.5 when \(PAT = h\) and tends to 1 as \(PAT\) grows. With \(mp\) the share of
    pollination given by managed bees, the **pollinator-dependent yield** is \(PYT(x) = \min(mp + FP(x), 1)\),
    and the part owed to wild bees \(PYW(x) = \max(0, PYT(x) - mp)\). For the farm, with \(\nu\) the share of its
    yield that depends on pollinators, InVEST reports the **total yield** \(Y_T = 1 - \nu\,(1 - \overline{PYT})\)
    and the **wild yield** \(Y_W = \nu\, \overline{PYW}\), the bars being means over the farm's cells. Both are
    shares of the yield the farm would have with full pollination.

![The half-saturation curve](/img/tutorials/ecosystem-services/pollination/half-sat.png "FP against the abundance on a farm for four half-saturation constants. Each curve reaches 0.5 at PAT = h (the dots). The grey band is the range of the wet season's abundance on the farms of this tutorial: with h = 0.05 (InVEST's suggested default) they all sit on the flat part of the curve.")

Farm 1 of this tutorial has a mean abundance \(PAT = 0.189\) in the wet season, \(h = 0.05\), no hives and
\(\nu = 0.25\): \(FP = 0.189 \times 0.95 / (0.05 \times (1 - 0.378) + 0.189) = 0.816\), so \(Y_W = 0.25 \times
0.816 = 0.204\) and \(Y_T = 1 - 0.25 \times (1 - 0.816) = 0.954\). Without any bee it would still yield
\(1 - 0.25 = 0.75\) of its potential: \(\nu\) caps what pollinators can add.

## When to use it

- **Where do a crop's pollinators come from?** The supply maps show which forest, pasture or field margins feed the
  farms, and so which patches are worth keeping for agriculture's sake.
- **Scenarios.** A plan that clears forest edges, plants a crop or restores riparian forest changes nesting and
  flowers at once; the model says what it does to the bees on the farms.
- **Farm practices.** Flower strips, hedgerows or nest boxes change a farm's own floral and nesting values, which
  the farm polygons carry.
- **Beside the other services**, as in [the ideas of this series](/tutorials/ecosystem-services/ideas/): carbon,
  habitat and water for the same scenario, and pollination for its agriculture.

Lonsdorf et al. (2009) tested the model against field data: coffee in Costa Rica, watermelon and sunflower in
California, watermelon in New Jersey and Pennsylvania. In Costa Rica and California it explained up to 80% of the
variance among farms in the bees observed; in New Jersey and Pennsylvania it did not predict them, probably because
the land use map missed the small patches of flowers and nesting sites around the farms. Their sensitivity
analyses found the predictions depend strongly on the resources inside the crop fields themselves. Ricketts et al.
(2008) synthesised 23 studies of 16 crops: the visitation of native bees falls exponentially with the distance from
natural habitat, to half at 0.6 km, more steeply in the tropics.

What it is **not**: it is an index of relative abundance, not a count of bees and not a population model (InVEST's
guide says so plainly). It does not know the size of habitat patches, the dynamics of the populations, pesticides,
or how effective each species is as a pollinator, and its yields are relative to a full pollination. Compare
scenarios run with the same tables, not absolute numbers between studies. For the share of habitat around sites at
several radii, see [Sampling, time series and redundancy](/tutorials/landscape-metrics/sampling-and-change/); for
one species' distribution, a [species distribution model](/tutorials/species-distributions/ideas/).

## What you need

- **A land use map** (`lulc_2020.tif` of [Ribeirão Claro](/tutorials/ecosystem-services/ideas/), from
  [ribeirao-claro.zip](/data/tutorials/ecosystem-services/ideas/ribeirao-claro.zip)), projected in metres, with
  integer codes; `lulc_2040.tif` for the scenario.
- **The guild table**: a row per guild (or species) with the columns `species`, `alpha` (the typical foraging
  distance, metres), `relative_abundance`, `foraging_activity_<season>_index` (0 to 1) for each season and
  `nesting_suitability_<substrate>_index` (0 to 1) for each substrate. Here two seasons, `dry` (April to
  September) and `wet` (October to March), and two substrates, `ground` and `cavity`:

| species | nesting ground | nesting cavity | activity dry | activity wet | alpha (m) | relative abundance |
|---|---|---|---|---|---|---|
| ground | 1 | 0 | 0.5 | 1 | 400 | 0.4 |
| stingless | 0 | 1 | 1 | 1 | 700 | 0.4 |
| carpenter | 0 | 1 | 0.6 | 1 | 1,000 | 0.2 |

The three guilds are ground-nesting solitary bees, stingless bees nesting in tree cavities, and large carpenter bees
boring into dead wood. In `guilds.csv` the columns have their full names (`nesting_suitability_ground_index`,
`foraging_activity_wet_index`...).

- **The biophysical table**: a row per land use with `lucode`, `nesting_<substrate>_availability_index` for each
  substrate and `floral_resources_<season>_index` for each season, all from 0 to 1. Every class of the map needs a
  row. Coffee flowers after the first rains, eucalyptus in the dry season; pasture offers bare ground to nest in,
  forest the cavities:

| lucode | class | nesting ground | nesting cavity | flowers dry | flowers wet |
|---|---|---|---|---|---|
| 1 | Forest | 0.5 | 1.0 | 0.5 | 0.7 |
| 2 | Pasture | 0.8 | 0.1 | 0.2 | 0.4 |
| 3 | Cropland (coffee) | 0.3 | 0.05 | 0.1 | 0.6 |
| 4 | Eucalyptus | 0.4 | 0.3 | 0.5 | 0.2 |
| 5 | Urban | 0.1 | 0.2 | 0.3 | 0.3 |
| 6 | Water | 0 | 0 | 0 | 0 |

- **Farms** (optional): polygons with the columns `crop_type`, `half_sat`, `season` (the season the crop is
  pollinated, named as in the tables), `p_dep` (the share of the yield that depends on pollinators, \(\nu\)),
  `p_managed` (the share of pollination given by managed bees, \(mp\)), `fr_<season>` and `n_<substrate>` (the
  farm's own flowers and nesting, which replace the land use's under the polygon). Here sixteen coffee farms, the
  patches of cropland of at least 13.5 ha that are cropland in 2020 and in 2040, all with `half_sat` 0.05, season
  `wet`, `p_dep` 0.25, and the cropland's flowers and nesting; farms 3 and 9 rent hives of honey bees
  (`p_managed` 0.3), the others have none.

![The coffee farms](/img/tutorials/ecosystem-services/pollination/farms.png "The land use of 2020 with the sixteen coffee farms of farms.gpkg outlined and numbered. Farms 1 to 6 lie among the large forests of the north; 10 to 16 in the more open south.")

The guild table, the biophysical table and the farms are in
[pollination-inputs.zip](/data/tutorials/ecosystem-services/pollination/pollination-inputs.zip): unzip it beside
the files of Ribeirão Claro. Every value is **illustrative**. In a real study the guilds and their foraging
distances come from surveys of the bees visiting the crop and from the literature (Greenleaf et al. (2007) related
the foraging distances of 62 bee species to their body size: larger bees fly farther), and the nesting and floral
values from field surveys or experts, documented for every class.

## Build the model

### Step 1: the land use and the guilds

<div class="steps" markdown>

1. Add a [[raster.read]] node (library subject *Getting data*) with `path` = `lulc_2020.tif`, and a
   [[table.read]] with `path` = `biophysical.csv`.
2. Add [[services.pollination]] (subject *Ecosystem services*). Connect the land use's `raster` to its input
   `lulc` and the table to `biophysical`.
3. In the Node pane, give the setting `guilds`: the list of guilds, each with the columns of the guild table, or
   the name of the table's file.

</div>

In the model file the guilds read as a list:

```tfgraph
#! caption: Pollination of 2020 from the land use, the biophysical table and three guilds given in the settings.
#! highlight: pollination
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "lulc"
op = "raster.read"
params = { path = "lulc_2020.tif" }
note = "Land use, 30 m, UTM 23S."

[[nodes]]
id = "biophysical"
op = "table.read"
params = { path = "biophysical.csv" }
note = "Nesting and flowers by class."

[[nodes]]
id = "pollination"
op = "services.pollination"
inputs = { lulc = "lulc.raster", biophysical = "biophysical.table" }
params = { guilds = [
  { species = "ground", nesting_suitability_ground_index = 1, nesting_suitability_cavity_index = 0, foraging_activity_dry_index = 0.5, foraging_activity_wet_index = 1, alpha = 400, relative_abundance = 0.4 },
  { species = "stingless", nesting_suitability_ground_index = 0, nesting_suitability_cavity_index = 1, foraging_activity_dry_index = 1, foraging_activity_wet_index = 1, alpha = 700, relative_abundance = 0.4 },
  { species = "carpenter", nesting_suitability_ground_index = 0, nesting_suitability_cavity_index = 1, foraging_activity_dry_index = 0.6, foraging_activity_wet_index = 1, alpha = 1000, relative_abundance = 0.2 },
] }
```

`guilds = "guilds.csv"` (InVEST's guild table) gives the same maps (checked for this tutorial). Like the `path`
of [[raster.read]], the file name is taken from the model's folder. The seasons and the substrates are read from
the names of the columns, and the two tables must name the same ones: a biophysical table with a season the guild
table lacks stops the node and says which.

Run it (++ctrl+enter++ on the node). Its outputs, each a *stack* (several maps on one grid, each layer named):

- `supply`: the pollinator supply \(PS\) of each guild, its layers named as the guilds;
- `abundance`: the abundance \(PA\) of each guild in each season, its layers named `<species>_<season>`
  (`ground_dry`, `ground_wet`, ...);
- `total_abundance`: the abundance of all guilds, \(PAT\), by season, its layers named as the seasons.

### Step 2: the farms

Read `farms.gpkg` with [[vector.read]] and connect its `vector` to the input `farms`. The farms may be in another
coordinate system: the node reprojects them to the land use's grid, as InVEST does. From now on the guilds come from
the file.

```tfgraph
#! caption: The farms connected: pollination and yield on sixteen coffee farms.
#! highlight: farms
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "lulc"
op = "raster.read"
params = { path = "lulc_2020.tif" }

[[nodes]]
id = "biophysical"
op = "table.read"
params = { path = "biophysical.csv" }

[[nodes]]
id = "farms"
op = "vector.read"
params = { path = "farms.gpkg" }
note = "Coffee: half_sat, season, p_dep, p_managed, fr_, n_."

[[nodes]]
id = "pollination"
op = "services.pollination"
inputs = { lulc = "lulc.raster", biophysical = "biophysical.table", farms = "farms.vector" }
params = { guilds = "guilds.csv" }
```

With farms the node also gives:

- `farm_pollinators`: \(FP\) on the farms' cells, each farm in its own season (no value elsewhere);
- `total_yield` and `wild_yield`: \(PYT\) and \(PYW\) on the farms' cells;
- `farm_table`: a row per farm, in the order of the polygons, with `farm` (1, 2, ...), `p_abund` (the mean
  abundance in the farm's season), `y_tot` (\(Y_T\)), `pdep_y_w` (\(\overline{PYW}\)) and `y_wild` (\(Y_W\)).

### Step 3: 2020 and 2040 side by side

For the scenario, add a second land use reader (`lulc_2040.tif`) and a second pollination node with the same table,
guilds and farms, and write both farm tables with [[table.write]]:

```tfgraph
#! caption: The full model: the same coffee farms in the landscape of 2020 and in the scenario of 2040, their tables written to CSV files.
#! highlight: pollination_2040, table_2020, table_2040
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "lulc_2020"
op = "raster.read"
params = { path = "lulc_2020.tif" }

[[nodes]]
id = "lulc_2040"
op = "raster.read"
params = { path = "lulc_2040.tif" }

[[nodes]]
id = "biophysical"
op = "table.read"
params = { path = "biophysical.csv" }

[[nodes]]
id = "farms"
op = "vector.read"
params = { path = "farms.gpkg" }

[[nodes]]
id = "pollination_2020"
op = "services.pollination"
inputs = { lulc = "lulc_2020.raster", biophysical = "biophysical.table", farms = "farms.vector" }
params = { guilds = "guilds.csv" }

[[nodes]]
id = "pollination_2040"
op = "services.pollination"
inputs = { lulc = "lulc_2040.raster", biophysical = "biophysical.table", farms = "farms.vector" }
params = { guilds = "guilds.csv" }

[[nodes]]
id = "table_2020"
op = "table.write"
inputs = { table = "pollination_2020.farm_table" }
params = { path = "farms_2020.csv" }

[[nodes]]
id = "table_2040"
op = "table.write"
inputs = { table = "pollination_2040.farm_table" }
params = { path = "farms_2040.csv" }
```

From the command line, with the model saved as `pollination.toml` in the folder of the data:

```bash
terraflux run ./pollination.toml --out results
```

How close is this to InVEST? Terraflux's tests compare the operator, map by map, with landschaft's version of
InVEST 3.20's model on five cases (several guilds, farms, a wide kernel...): every cell holds the same 32-bit
number, and the farm table agrees to within \(10^{-12}\). landschaft's own tests compare those cases with InVEST's
outputs, to the bit on Windows.

## Read the results

### Where the bees come from

```tfgallery
columns: 3
caption: "The pollinator supply of each guild (output supply), on one scale. The stingless and carpenter bees nest only in cavities, so they come from the forest; the ground-nesting bees come mostly from pasture, which offers the most bare ground, and less from forest and coffee."
images:
  - { src: /img/tutorials/ecosystem-services/pollination/supply_ground.png, caption: "ground" }
  - { src: /img/tutorials/ecosystem-services/pollination/supply_stingless.png, caption: "stingless" }
  - { src: /img/tutorials/ecosystem-services/pollination/supply_carpenter.png, caption: "carpenter" }
```

The supply is the map of **sources**: where the nests are, weighted by the flowers within their reach. Its maximum
is set by the relative abundance: the carpenter bees, with 0.2, reach at most 0.10, the others about 0.2.

### Where they forage

```tfgallery
columns: 3
caption: "The abundance of each guild in the wet season (layers ground_wet, stingless_wet and carpenter_wet of the output abundance), on one scale. The bees spread from their nests over the flowers within reach: the forest guilds into the coffee and pasture next to the forest, the ground-nesting bees over the whole landscape."
images:
  - { src: /img/tutorials/ecosystem-services/pollination/abundance_ground_wet.png, caption: "ground, wet" }
  - { src: /img/tutorials/ecosystem-services/pollination/abundance_stingless_wet.png, caption: "stingless, wet" }
  - { src: /img/tutorials/ecosystem-services/pollination/abundance_carpenter_wet.png, caption: "carpenter, wet" }
```

A row of the map shows the two steps at work:

![Supply and abundance along a transect](/img/tutorials/ecosystem-services/pollination/transect.png "The first 3.3 km of row 62 of the map. Top: the land use. Middle: the supply jumps with the nesting sites (the cavity nesters supply only from the forest; the ground-nesting bees most from pasture). Bottom: the abundance follows the flowers of the season: highest in the forest (0.7) and the coffee (0.6), lower in pasture (0.4), and for the forest guilds higher in coffee next to the forest.")

```tfgallery
columns: 2
caption: "The total abundance of all guilds (output total_abundance) in the dry and the wet season, on one scale. In the wet season the coffee and the forest are full of bees; in the dry season the coffee has few flowers and few bees, and the eucalyptus plantations, which flower then, stand out."
images:
  - { src: /img/tutorials/ecosystem-services/pollination/total_dry.png, caption: "dry" }
  - { src: /img/tutorials/ecosystem-services/pollination/total_wet.png, caption: "wet" }
```

For the crop, the season that matters is the one it flowers in: the wet season's map over the coffee. Its mean is
0.174 on the coffee, against 0.215 in the forest and 0.118 in pasture. How does it change with the distance to the
forest?

```tfchart
type: line
data: /data/tutorials/ecosystem-services/pollination/distance.csv
x: distance
y: [total, ground, stingless, carpenter]
labels: { total: "All guilds", ground: "Ground-nesting bees", stingless: "Stingless bees", carpenter: "Carpenter bees" }
colors: { total: "#1c2420", ground: "#b5651d", stingless: "#2f6f9f", carpenter: "#7b4fa0" }
xlabel: Distance from the coffee cell to the nearest forest (m)
ylabel: Mean abundance, wet season
height: 320
caption: "The wet season's abundance on the coffee of 2020, averaged in bands of distance to the nearest forest. The stingless bees fall by 42% from the coffee next to the forest to the coffee 450 m and more away, the carpenter bees by 33%, all guilds together by 18%; the ground-nesting bees, which nest in pasture and in the fields, rise slightly."
```

This is the pattern Ricketts et al. (2008) found in the field, with a twist the model makes explicit: it holds for
the guilds that nest in the forest, not for those that nest in the open. The gradient here is gentle because no
coffee of Ribeirão Claro is more than 600 m from a forest (half of it is within 150 m).

### The farms

![Pollination on the farms](/img/tutorials/ecosystem-services/pollination/farm_pollinators.png "The output farm_pollinators: FP on each farm in the wet season, over the land use (pale). The farms among the northern forests (1 to 6) get the most, the farms of the open south (10 to 16) the least; the scale goes from 0.70 to 0.85.")

```tftable
data: /data/tutorials/ecosystem-services/pollination/farms.csv
labels: { farm: "Farm", ha: "Area (ha)", dist: "Mean distance to forest (m)", p_managed: "p_managed", p_abund: "p_abund", pdep_y_w: "pdep_y_w", y_tot: "y_tot", y_wild: "y_wild" }
digits: 3
caption: "The output farm_table of 2020, with each farm's area and mean distance to the nearest forest added. The abundance (p_abund) is highest on the northern farms. Farms 3 and 9 have hives: their total yield is complete (1.0) and the part owed to wild bees is capped at 1 - 0.3 = 0.7 of the dependent yield."
```

Three things to read in this table. First, **the abundance varies more than the yield**: p_abund goes from 0.145
to 0.189 (30%), y_wild from 0.191 to 0.204 (7%), because with \(h = 0.05\) every farm is on the flat part of the
curve. Second, **the distance to the nearest forest is not the whole story**: farm 4, 160 m from the forest on
average, has as many bees as farm 6 at 51 m (0.188 and 0.189), and more than farm 8 at 142 m (0.176); what counts
is how much forest and pasture lies within the bees' range, not only the nearest edge. Third, **managed bees mask the wild ones**: on farms 3 and 9 the hives
fill the yield whatever the wild bees do, so their y_wild does not tell what the wild bees could do.

!!! paper "Reporting a pollination analysis"
    Give the guild table and the biophysical table (with their sources), the farms' half saturation, pollinator
    dependence and managed pollination, the maps of supply and abundance for the season of the crop, and for each
    scenario the mean abundance on the crop and the farms' yields. Say that the abundances are relative indices and
    that the yields are shares of the yield with full pollination.

## Settings that change the answer

### The foraging distance

\(\alpha\) is the number most worth testing: published foraging distances vary widely, and it sets how far the
forest's bees reach into the fields. Move the stingless bees' \(\alpha\) from 100 m to 1,400 m, the other guilds as
before:

```tfslider
label: Foraging distance of the stingless bees, alpha (m)
caption: "The stingless bees' abundance in the wet season for five foraging distances (the same colour scale). With a short alpha they stay at the forest's edges and the coffee 240 m and more from the forest gets a fifth of what the coffee next to it gets; with a long one they spread over the landscape and the difference nearly disappears."
frames:
  - { value: 100, src: /img/tutorials/ecosystem-services/pollination/alpha_100.png, stats: { "On coffee within 60 m of forest": "0.064", "On coffee beyond 240 m": "0.012", "Farms' mean y_wild": "0.189" } }
  - { value: 200, src: /img/tutorials/ecosystem-services/pollination/alpha_200.png, stats: { "On coffee within 60 m of forest": "0.068", "On coffee beyond 240 m": "0.023", "Farms' mean y_wild": "0.192" } }
  - { value: 350, src: /img/tutorials/ecosystem-services/pollination/alpha_350.png, stats: { "On coffee within 60 m of forest": "0.068", "On coffee beyond 240 m": "0.032", "Farms' mean y_wild": "0.194" } }
  - { value: 700, src: /img/tutorials/ecosystem-services/pollination/alpha_700.png, stats: { "On coffee within 60 m of forest": "0.065", "On coffee beyond 240 m": "0.040", "Farms' mean y_wild": "0.195" } }
  - { value: 1400, src: /img/tutorials/ecosystem-services/pollination/alpha_1400.png, stats: { "On coffee within 60 m of forest": "0.060", "On coffee beyond 240 m": "0.048", "Farms' mean y_wild": "0.196" } }
chart:
  type: line
  data: /data/tutorials/ecosystem-services/pollination/alpha.csv
  x: alpha
  y: [near, far]
  labels: { near: "Coffee within 60 m of forest", far: "Coffee beyond 240 m" }
  colors: { near: "#3b8a5a", far: "#e8901f" }
  ylabel: "Stingless bees, wet season"
start: 3
```

A longer flight distance does not create bees: it moves them. The coffee next to the forest gets slightly fewer as
\(\alpha\) grows past 200 m, the coffee far from it many more. If your question is *which farms lose when a forest
goes*, the answer depends on \(\alpha\); run the model with the range of distances the literature gives for your
guilds.

### The half saturation

The farms' yields depend on \(h\) much more than on anything else, because it sets where on the curve the farms
sit. The same farms with four values of `half_sat`:

```tftable
data: /data/tutorials/ecosystem-services/pollination/half-sat.csv
labels: { h: "half_sat (h)", pdep_mean: "Mean pdep_y_w", pdep_min: "Lowest", pdep_max: "Highest", y_wild_mean: "Mean y_wild" }
digits: 3
caption: "The farms' wild-pollinator yield for four half-saturation constants, the abundance unchanged. From h = 0.05 to 0.3 the share of the dependent yield owed to wild bees falls from 0.78 to 0.32."
```

InVEST's guide suggests 0.05 as a default, with a range of 0.05 to 0.1, and calls it a parameter to tune after a
first run. Like the half-saturation constant of [habitat quality](/tutorials/ecosystem-services/habitat-quality/),
it does not change the ranking of the farms, but it changes every yield you report: choose it once, say how, and
keep it for every scenario.

## The scenario of 2040

The scenario clears forest on the gentle edges, turns pasture into coffee, plants a new eucalyptus block in the
north-east, grows the town and restores forest along the rivers. The farms stay as they are; their surroundings
change.

```tfcompare
caption: "Drag the handle: the total abundance of the wet season in 2020 (left) and in the scenario of 2040 (right), on the same scale (dark: few bees, yellow: many)."
before: { src: /img/tutorials/ecosystem-services/pollination/wet_2020_plain.png, label: "2020" }
after: { src: /img/tutorials/ecosystem-services/pollination/wet_2040_plain.png, label: "2040" }
```

![Change of pollinator abundance](/img/tutorials/ecosystem-services/pollination/change.png "The change of the wet season's total abundance, 2040 minus 2020: red where it falls, blue where it rises. The strongest losses are in the forest cleared and in the new eucalyptus block (few flowers in the wet season); the abundance falls a little almost everywhere, as the forest's bees become fewer; it rises along the rivers, where riparian forest is restored.")

```tftable
data: /data/tutorials/ecosystem-services/pollination/scenarios.csv
labels: { year: "Map", forest_ha: "Forest (ha)", coffee_ha: "Coffee (ha)", coffee_dist: "Mean distance of the coffee to forest (m)", coffee_pat: "Mean abundance on the coffee", farms_pat: "Farms' mean p_abund", farms_y_wild: "Farms' mean y_wild" }
digits: 3
caption: "The scenario loses 31% of the forest and gains 71% of coffee. The mean abundance on the coffee falls by 17%, on the sixteen farms by 19%, but their wild yield only by 5%: with h = 0.05 the farms were near saturation."
```

```tfchart
type: bar
data: /data/tutorials/ecosystem-services/pollination/farms-2040.csv
x: farm
y: [p_abund_2020, p_abund_2040]
labels: { p_abund_2020: "2020", p_abund_2040: "2040" }
colors: { p_abund_2020: "#3b8a5a", p_abund_2040: "#e8901f" }
xlabel: Farm
ylabel: Mean abundance, wet season (p_abund)
height: 300
caption: "The abundance on each farm in 2020 and 2040. Every farm loses bees, from 14% (farm 11) to 23% (farm 3); the farms of the south-west (10 to 13), among those that had the fewest, lose the least."
```

Note what the mean distance of the coffee to the forest says: almost nothing (177 m against 176 m). The loss
comes from the **amount** of forest within the bees' range, which a distance to the nearest edge does not measure
and the model does.

## Common pitfalls

- **Names with an underscore.** Seasons and substrates are read from the column names
  (`foraging_activity_wet_index`), so their names cannot contain `_`: `wet_season` is not read as a season. Use
  `wet`, `dry`, `ground`, `cavity`.
- **Tables that do not match.** The guild table, the biophysical table and the farms must name the same seasons and
  substrates; a farm's `season` must be one of them. The node stops with a message that names the mismatch.
- **A missing class.** Every class of the land use needs a row of the biophysical table; a class without one
  stops the node.
- **Geographic coordinates.** \(\alpha\) is in metres and is divided by the cell size; a map in degrees makes it
  thousands of cells. Reproject the land use first (see
  [Coordinate systems](/tutorials/getting-started/coordinate-systems/)).
- **The edge of the map.** Near the edge, the flowers and the nests beyond it are not counted: the mean is over the
  part of the flight range inside the map. Include a margin of land around the farms of at least the largest
  \(\alpha\), better twice.
- **Small resources the map misses.** Field margins, hedgerows and gardens are often smaller than a cell and absent
  from a land use map, and Lonsdorf et al. (2009) found that they can decide the result. Give the farms their own
  `fr_` and `n_` values when you know them.
- **Abundance read as bees.** 0.17 is not 0.17 bees: the indices rank places and scenarios, they do not count
  insects.
- **Yields read as harvests.** \(Y_T = 0.95\) is 95% of the yield the farm would have with full pollination, not
  a number of bags of coffee.

## Going further

- [Ecosystem service models: the ideas](/tutorials/ecosystem-services/ideas/), part 1, for the landscape and the
  other services.
- [Habitat quality and rarity](/tutorials/ecosystem-services/habitat-quality/) for the forest the bees nest in,
  seen as habitat.
- [Carbon storage and sequestration](/tutorials/ecosystem-services/carbon/) for the same scenario in tons of carbon.
- [Several benefits and trade-offs](/tutorials/distance-restoration/several-benefits/) to weigh pollination with
  other benefits in a restoration plan.
- [Sampling, time series and redundancy](/tutorials/landscape-metrics/sampling-and-change/) for the landscape
  around field sites at several radii, to relate observed bees to the land around them.
- The operator's reference: [[services.pollination]].

## References

- Greenleaf, S. S., Williams, N. M., Winfree, R., & Kremen, C. (2007). Bee foraging ranges and their relationship to body size. *Oecologia*, 153(3), 589–596. https://doi.org/10.1007/s00442-007-0752-9
- Klein, A.-M., Vaissière, B. E., Cane, J. H., Steffan-Dewenter, I., Cunningham, S. A., Kremen, C., & Tscharntke, T. (2007). Importance of pollinators in changing landscapes for world crops. *Proceedings of the Royal Society B: Biological Sciences*, 274(1608), 303–313. https://doi.org/10.1098/rspb.2006.3721
- Kremen, C., Williams, N. M., Aizen, M. A., Gemmill-Herren, B., LeBuhn, G., Minckley, R., Packer, L., Potts, S. G., Roulston, T., Steffan-Dewenter, I., Vázquez, D. P., Winfree, R., Adams, L., Crone, E. E., Greenleaf, S. S., Keitt, T. H., Klein, A.-M., Regetz, J., & Ricketts, T. H. (2007). Pollination and other ecosystem services produced by mobile organisms: a conceptual framework for the effects of land-use change. *Ecology Letters*, 10(4), 299–314. https://doi.org/10.1111/j.1461-0248.2007.01018.x
- Lonsdorf, E., Kremen, C., Ricketts, T., Winfree, R., Williams, N., & Greenleaf, S. (2009). Modelling pollination services across agricultural landscapes. *Annals of Botany*, 103(9), 1589–1600. https://doi.org/10.1093/aob/mcp069
- Natural Capital Alliance. (2026). *InVEST User's Guide: Crop pollination (pollinator abundance)*. https://storage.googleapis.com/releases.naturalcapitalproject.org/invest-userguide/latest/en/croppollination.html
- Ricketts, T. H., Daily, G. C., Ehrlich, P. R., & Michener, C. D. (2004). Economic value of tropical forest to coffee production. *Proceedings of the National Academy of Sciences*, 101(34), 12579–12582. https://doi.org/10.1073/pnas.0405147101
- Ricketts, T. H., Regetz, J., Steffan-Dewenter, I., Cunningham, S. A., Kremen, C., Bogdanski, A., Gemmill-Herren, B., Greenleaf, S. S., Klein, A. M., Mayfield, M. M., Morandin, L. A., Ochieng', A., & Viana, B. F. (2008). Landscape effects on crop pollination services: are there general patterns? *Ecology Letters*, 11(5), 499–515. https://doi.org/10.1111/j.1461-0248.2008.01157.x
