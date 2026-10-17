---
title: "Ecosystem service models: the ideas"
short: 1. The ideas
description: What an ecosystem service model is, how InVEST turns a land use map and a few tables into maps of carbon, habitat, water and sediment, how Terraflux runs InVEST's models, and a first run on four services at once.
order: 10
level: beginner
time: 35 min
ops: [services.carbon, services.habitat_quality, services.water_yield, services.sdr, table.read, table.of, raster.read, vector.read]
series: Ecosystem services with InVEST
part: 1
---

A forest stores carbon, shelters species, takes water from the soil and holds the soil on the slope. Clear it for
pasture and all four change at once: less carbon, less habitat, more water running off, more mud in the river.
**Ecosystem service models** put numbers on those changes, map by map, so that a land use plan, a restoration
project or a scenario of the future can be compared on more than its hectares. This series teaches the four
models of **InVEST** that Terraflux runs: carbon storage, habitat quality, annual water yield and sediment
retention. In this first part you will learn what these models have in common, what they need and what they can
and cannot tell you, and you will run all four on one landscape, for today and for a scenario of 2040.

## The idea

*Ecosystem services* are the benefits people obtain from ecosystems: food and timber, clean water, a stable
climate, pollination, recreation. The phrase became widely known when Costanza et al. (1997) put a (much debated)
price on the services of the whole biosphere. What changed the practice of conservation was less the global price
than a local question: **if we change the land here, what do we gain and what do we lose, and where?** To answer
it you need models that start from the thing decision makers actually change, the map of land use, and end at
something people care about.

Tallis and Polasky (2009) called these models *production functions*: functions that take the state of an
ecosystem (here, mostly its land cover) and give the amount of a service it provides. The Natural Capital
Project, a partnership led from Stanford University, built a free set of them, **InVEST** (Integrated Valuation
of Ecosystem Services and Tradeoffs; Natural Capital Alliance, 2026), and showed with them how a river basin's land use plans trade carbon,
species and commodities against each other (Nelson et al., 2009). Guerry et al. (2015) review how
such models have since moved from research into the decisions of governments and companies.

!!! science "Key idea: a land use map plus a table by class"
    Every InVEST model in this series has the same skeleton:

    1. a **land use map** (a *raster*, a grid of square cells, each holding an integer code: 1 forest,
       2 pasture...);
    2. a **table by class**, one row per code, with the numbers the model needs for that class: tons of carbon
       per hectare, how sensitive the class is to roads, how deep its roots go, how much it protects the soil;
    3. for the hydrological models, **maps of the physical setting**: rain, evaporation, soils, elevation;
    4. a **model** that combines them cell by cell (and, for water and sediment, along the paths water takes
       downhill), giving **maps of the service** and **totals** for the landscape or for each watershed.

    Change the land use map (a scenario) and run again: the difference between the two runs is the change in
    the service.

![Ribeirão Claro, the land use of 2020](/img/tutorials/ecosystem-services/ideas/lulc_2020.png "The land use of Ribeirão Claro in 2020: 7.2 by 5.4 km in cells of 30 m. Native forest on the steep slopes and ridges, pasture and cropland below, four eucalyptus plantations, a town near the outlets and its rivers. The shares are of the 43,200 cells.")

Because the land use map is at the centre, these models fit naturally after a model of land change: simulate
where farming will expand (see the [land change tutorials](/tutorials/land-change/what-land-change-models-are/)),
then ask what that costs in carbon, habitat, water and soil.

### Supply, service and value

Three words are easy to mix up, and InVEST keeps them apart:

- the **supply** is what the ecosystem does, in physical units: tons of carbon stored, millimetres of water
  yielded, tons of soil kept on the slope;
- the **service** is the part of that supply someone uses or benefits from: the water a town or a power plant
  actually takes, the sediment that would otherwise fill a reservoir;
- the **value** is that service in money or another common unit: a price per ton of carbon, the electricity a
  dam produces from the water.

Every model of this series gives the supply. Some also take the step to the service or the value: the carbon
model can value a change of storage with a price and a discount rate, the water model can subtract what people
consume and value the rest as hydropower. The habitat quality model is different: it does not measure a service
to people at all but the state of biodiversity, which is why it is so often run beside the others.

### InVEST in Terraflux

Terraflux's four operators (in the library's subject *Ecosystem services*) are InVEST 3.20's models as
**landschaft**, the library behind Terraflux, reimplements them in C++: the same inputs, the same equations, and
the same arithmetic (the same 32-bit floating point numbers, the same order of the sums), so that the maps are
InVEST's **bit for bit**. You can check it: the table below compares Terraflux with InVEST 3.20.2 on InVEST's own
runs (the reference cases landschaft is tested against), cell by cell.

```tftable
data: /data/tutorials/ecosystem-services/ideas/against-invest.csv
labels: { model: "Operator", map: "Map", cells: "Cells with a value", identical: "Identical to InVEST", max_diff: "Largest difference" }
digits: 0
caption: "Terraflux against InVEST 3.20.2 on four of InVEST's runs (a carbon valuation, habitat quality with a future and a base, water yield, sediment with multiple flow directions): every cell of every map compared holds the same 32-bit number."
```

Why this matters for you: a study made with Terraflux can say it used InVEST 3.20's models, cite InVEST's papers
and user's guide, and compare its numbers with any published InVEST study. What Terraflux adds is the rest of the
workflow in one graph: preparing the maps (reprojecting, aligning, reclassifying), simulating the land use
scenarios, running the models many times for sensitivity or uncertainty, and drawing the results.

## When to use it

Use ecosystem service models when the question is about **where** and **how much** a change of land use changes
the services a landscape provides:

- **Comparing scenarios**: business as usual against a land use plan, or a road against no road. Nelson et al.
  (2009) compared scenarios of land use drawn up by stakeholders of the Willamette Basin (Oregon) for several
  services, biodiversity and the value of commodities: the scenarios with more development produced more
  commodities but fewer services and less biodiversity, a trade-off that payments for carbon eased.
- **Targeting** restoration or protection: the cells where reforestation would keep the most sediment out of a
  reservoir, the forests that store the most carbon.
- **Payments for ecosystem services** and water funds: who upstream provides the service, who downstream
  benefits.
- **Several objectives at once**: maps of carbon and of habitat quality are inputs to a spatial prioritization
  (see the [spatial prioritization tutorials](/tutorials/spatial-prioritization/systematic-conservation-planning/))
  or to a restoration plan with several benefits
  ([several benefits](/tutorials/distance-restoration/several-benefits/)).

What these models are **not**:

- They are not precise local predictions. They are deliberately simple, built on annual averages and tables of
  values by class, and they are best at **comparing** maps (scenario A against B, place X against Y). Their
  absolute numbers can be far from measured ones unless you check them. Redhead et al. (2016) compared the water
  yield model with the river flows measured in 22 catchments of the United Kingdom: with global input data the
  fit was moderate and varied widely between catchments, with national data most catchments came within about
  10% of the measured flow. The data you give matter as much as the model.
- They are not process models. A daily hydrological model such as SWAT+ ([[swat.run]]) simulates floods,
  low flows and nutrient loads through the year; the water yield model gives one number per cell for an average
  year.
- Habitat quality is not a species distribution: it is a generic index of how intact the habitat is around
  threats. For one species, use a [species distribution model](/tutorials/species-distributions/)
  or a connectivity analysis.

## What you need

**A land use map** in a *projected* coordinate system, in metres (a *CRS*, coordinate reference system, says how
the numbers of the map relate to places on Earth; a projected one, such as UTM, measures in metres). The four
operators refuse a map in longitude and latitude, as InVEST does, because they need the area of a cell in
hectares and distances in metres. The codes must be whole numbers; cells with no value (outside the study area)
are left out. See [Coordinate systems](/tutorials/getting-started/coordinate-systems/) if your map is not
projected yet.

**Every other map on the same grid**: same number of rows and columns, same cells. Terraflux checks it and stops
with a message naming the map that differs; [One grid for all your maps](/tutorials/getting-started/one-grid/)
shows how to bring them onto one grid.

**A table for each model, by class.** InVEST's tables are CSV files with a `lucode` column (the land use code) and
one column per parameter. In Terraflux a table is a node of its own:

- [[table.read]] reads a CSV file (or a TSV, or a file Excel wrote with `;` and decimal commas: the separator
  and the decimal mark are found by themselves). The columns of text are left out, because a table in Terraflux
  holds numbers: InVEST's optional `name` or `description` column does no harm, it is simply not read. The
  column names are not case-sensitive (`LUCODE` and `lucode` are the same).
- [[table.of]] writes a small table in the node's settings, without a file: handy for a first try.

Every class of the map must be in the table, or the operator stops and lists the missing codes ("the land use map
has class(es) [7] that the table lacks"). Classes in the table that the map lacks do no harm.

**Watershed polygons** for the water and sediment models: a vector file (GeoPackage, Shapefile...) read by
[[vector.read]], with an integer `ws_id` attribute.

### The landscape of this series

The four tutorials use **Ribeirão Claro**, an imaginary landscape made for them (the numbers are invented, the
methods are real): 7.2 by 5.4 km of hills between 530 and 1,100 m in cells of 30 m, in SIRGAS 2000 / UTM zone 23S
(EPSG:31983). Two valleys drain south; native forest covers the steep slopes, pasture and cropland the gentler
ground, with four eucalyptus plantations and a town near the outlets. Download it to follow along:
[ribeirao-claro.zip](/data/tutorials/ecosystem-services/ideas/ribeirao-claro.zip) (0.6 MB). It holds:

| Files | What they are |
|---|---|
| `lulc_1985.tif`, `lulc_2020.tif`, `lulc_2040.tif` | land use: 1 forest, 2 pasture, 3 cropland, 4 eucalyptus, 5 urban, 6 water |
| `dem.tif` | elevation, m |
| `precipitation.tif`, `eto.tif`, `depth.tif`, `pawc.tif` | annual rain and reference evapotranspiration (mm), soil depth (mm), plant available water (0 to 1) |
| `erosivity.tif`, `erodibility.tif` | the RUSLE's R and K factors |
| `crops_*.tif`, `roads_*.tif`, `urban_*.tif` | the threats to habitat in 2020 and 2040 (1 where the threat is) |
| `watersheds.gpkg`, `subwatersheds.gpkg`, `access.gpkg` | two watersheds, four sub-watersheds, a park and its buffer zone |
| `carbon_pools.csv`, `sensitivity.csv`, `threats.csv`, `biophysical_wy.csv`, `demand.csv`, `valuation.csv`, `biophysical_sdr.csv` | the models' tables |

![Elevation, rivers and watersheds of Ribeirão Claro](/img/tutorials/ecosystem-services/ideas/terrain.png "Elevation over a shaded relief, the rivers (blue) and the two watersheds draining to the southern edge, traced with Terraflux's hydro.watersheds from the filled elevation model.")

The scenario of **2040** clears forest edges on the gentler ground for pasture, turns gentle pasture into
cropland, doubles the town, adds a plantation and, as a compensation, restores forest along the rivers. The map
of **1985** had more forest, no plantations and a village instead of a town.

```tfgallery
columns: 2
caption: "The land use of 1985 (left), the base map for the rarity of the habitat quality model, and the scenario of 2040 (right), with the share of each class."
images:
  - { src: /img/tutorials/ecosystem-services/ideas/lulc_1985.png, caption: "1985" }
  - { src: /img/tutorials/ecosystem-services/ideas/lulc_2040.png, caption: "2040" }
```

```tfcompare
caption: Drag the handle to compare the land use of 2020 (left) with the scenario of 2040 (right). Forest 39.6% of the cells in 2020 and 27.2% in 2040; cropland 19.7% and 33.8%; the town 1.4% and 3.3%. Colours as in the map above.
before: { src: /img/tutorials/ecosystem-services/ideas/lulc_2020_plain.png, label: "2020" }
after: { src: /img/tutorials/ecosystem-services/ideas/lulc_2040_plain.png, label: "2040" }
```

## Build the model

Start with the simplest of the four: carbon storage needs only the land use map and a table.

<div class="steps" markdown>

1. In the application, open the library (the left pane, or *Operators* in the toolbar) at the subject
   **Getting data** and click [[raster.read]]; in the Node pane set its `path` to `lulc_2020.tif`.
2. Add a [[table.read]] node and set its `path` to `carbon_pools.csv`.
3. In the subject **Ecosystem services**, click [[services.carbon]].
4. Drag from the output `raster` of the first node to the input `lulc` of the carbon node, and from the output
   `table` of the table node to its input `pools`.
5. Select the carbon node and press ++ctrl+enter++: it runs with everything it needs. Its `storage` output is
   the map of carbon, its `totals` the table of the landscape's tons.

</div>

```tfgraph
#! caption: The smallest ecosystem service model: a land use map, a table by class, the carbon model.
#! highlight: carbon
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "lulc"
op = "raster.read"
params = { path = "lulc_2020.tif" }
note = "Land use of 2020, codes 1 to 6, UTM metres."

[[nodes]]
id = "pools"
op = "table.read"
params = { path = "carbon_pools.csv" }
note = "lucode, c_above, c_below, c_soil, c_dead (t C / ha)."

[[nodes]]
id = "carbon"
op = "services.carbon"
inputs = { lulc = "lulc.raster", pools = "pools.table" }
```

The same model with the table written in the settings, so it runs without the CSV file. The table has the
columns' names and one row of numbers per class:

```tfgraph
#! caption: The pools typed in the settings of table.of instead of read from a file.
#! highlight: pools
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "lulc"
op = "raster.read"
params = { path = "lulc_2020.tif" }

[[nodes]]
id = "pools"
op = "table.of"
params = { columns = ["lucode", "c_above", "c_below", "c_soil", "c_dead"], rows = [[1, 140, 35, 95, 12], [2, 3, 6, 60, 0.5], [3, 4, 1, 45, 0.5], [4, 75, 18, 70, 6], [5, 5, 1, 30, 0], [6, 0, 0, 0, 0]] }

[[nodes]]
id = "carbon"
op = "services.carbon"
inputs = { lulc = "lulc.raster", pools = "pools.table" }
```

From the command line, save the model as `carbon.toml` in the folder of the data and run it:

```bash
terraflux run carbon.toml --out results
```

The other three models follow the same pattern with more inputs: habitat quality adds the maps of the threats and
a sensitivity table, water yield adds four rasters of climate and soil and the watersheds, sediment adds the
elevation, two soil loss factors and the watersheds. Each has its own tutorial in this series. Run once with the
land use of 2020, they give these four maps:

```tfgallery
columns: 2
caption: The four models on the land use of 2020 (settings as in the following tutorials). Carbon follows the forest; habitat quality is high in the forest far from crops, roads and the town; water yield is highest where rain is high and evaporation low (the town, the cropland); sediment export is highest on steep pasture close to the rivers. Sediment on a logarithmic scale.
images:
  - { src: /img/tutorials/ecosystem-services/ideas/carbon.png, caption: "Carbon stored, t C / ha" }
  - { src: /img/tutorials/ecosystem-services/ideas/quality.png, caption: "Habitat quality, 0 to 1" }
  - { src: /img/tutorials/ecosystem-services/ideas/wyield.png, caption: "Water yield, mm / year" }
  - { src: /img/tutorials/ecosystem-services/ideas/export.png, caption: "Sediment export, t / ha / year" }
```

!!! app "In the application"
    Connect a [[view.plot]] node to any map output (or to a table) to see it in a tab of its own, refreshed at
    every run. A loop over scenarios ([[control.repeat]], see [Loops](/tutorials/building-models/loops/)) runs the
    same models on many land use maps.

## Read the results

Run the four models again with the land use of 2040 and compare the totals:

```tftable
data: /data/tutorials/ecosystem-services/ideas/four-services.csv
labels: { service: "Service" }
caption: "The four services of Ribeirão Claro in 2020 and in the scenario of 2040, from the same models and tables. Habitat quality is the mean over the forest cells with the half-saturation constant 0.35; water and sediment are the totals of the two watersheds."
```

This is the typical shape of a land use change: no scenario wins on every service. The scenario of 2040 loses a
fifth of the landscape's carbon and a tenth of its forest's habitat quality, and sends 40% more sediment to the
rivers, but it **yields 4% more water**, because pasture and crops evaporate less than forest. Whether more water
is good depends on who uses it and when: a reservoir for hydropower gains, but the model says nothing about
whether the extra water arrives as floods in the rainy season or as flow in the dry one. Reading these numbers
well means knowing what each model includes, which is the subject of the next four tutorials.

When you report a study, give for each model: the version of the model (InVEST 3.20, through Terraflux), the
maps and their resolution, the tables (in a supplement), the settings, and the totals for each scenario; show
the maps of the **difference** between scenarios, not only the maps of each.

## Settings that change the answer

Three choices change every model of this series:

- **The tables.** The numbers by class are the model. A carbon density, a sensitivity to roads or a cover
  factor from a global table may be far from your landscape's; use local measurements when you have them, and
  test how much the answer moves when a value moves (each tutorial shows how). The values of Ribeirão Claro are
  illustrative, chosen to be plausible, not measured.
- **The classes.** A class is assumed to be the same everywhere: every hectare of "forest" holds the same carbon
  and the same roots. Splitting a class (old-growth and secondary forest, degraded and managed pasture) is often
  the cheapest way to make a model more realistic.
- **The resolution.** Water and sediment follow the flow of water over the elevation model: their results
  depend on the size of the cells far more than carbon does.

## Common pitfalls

- **A map in longitude and latitude.** The operators stop with "...is in longitude / latitude: give it
  projected, in metres". Reproject with [[raster.reproject]] first.
- **Grids that differ by a row.** Two maps from different sources rarely share their grid. Match them first
  ([[raster.match]]).
- **Missing classes.** A code in the map that the table lacks stops the model; a class with code 0 used as "no
  data" must be a missing value of the raster, not a class.
- **Units.** Carbon in t C per hectare (InVEST's Mg C / ha: the same thing), water in millimetres, sediment in
  tons per hectare and year, distances in metres. A table in t CO₂ instead of t C overestimates carbon by 3.67
  times.
- **Absolute numbers taken at face value.** Without calibration, compare scenarios, and say so.

## Going further

- [Carbon storage and sequestration](/tutorials/ecosystem-services/carbon/): the four pools, a future map and the
  value of the change.
- [Habitat quality and rarity](/tutorials/ecosystem-services/habitat-quality/): threats, their decay with
  distance, sensitivity, access and rarity.
- [Annual water yield](/tutorials/ecosystem-services/water-yield/): Budyko's curve, consumption and hydropower.
- [Sediment delivery ratio](/tutorials/ecosystem-services/sediment/): soil loss, connectivity, export and
  retention.
- The operators' reference: [[services.carbon]], [[services.habitat_quality]], [[services.water_yield]],
  [[services.sdr]].

## References

- Costanza, R., d'Arge, R., de Groot, R., Farber, S., Grasso, M., Hannon, B., Limburg, K., Naeem, S., O'Neill, R. V., Paruelo, J., Raskin, R. G., Sutton, P., & van den Belt, M. (1997). The value of the world's ecosystem services and natural capital. *Nature*, 387(6630), 253–260. https://doi.org/10.1038/387253a0
- Guerry, A. D., Polasky, S., Lubchenco, J., Chaplin-Kramer, R., Daily, G. C., Griffin, R., Ruckelshaus, M., Bateman, I. J., Duraiappah, A., Elmqvist, T., Feldman, M. W., Folke, C., Hoekstra, J., Kareiva, P. M., Keeler, B. L., Li, S., McKenzie, E., Ouyang, Z., Reyers, B., Ricketts, T. H., Rockström, J., Tallis, H., & Vira, B. (2015). Natural capital and ecosystem services informing decisions: From promise to practice. *Proceedings of the National Academy of Sciences*, 112(24), 7348–7355. https://doi.org/10.1073/pnas.1503751112
- Natural Capital Alliance. (2026). *InVEST 3.20.2 User's Guide*. https://storage.googleapis.com/releases.naturalcapitalproject.org/invest-userguide/latest/en/index.html
- Nelson, E., Mendoza, G., Regetz, J., Polasky, S., Tallis, H., Cameron, D. R., Chan, K. M. A., Daily, G. C., Goldstein, J., Kareiva, P. M., Lonsdorf, E., Naidoo, R., Ricketts, T. H., & Shaw, M. R. (2009). Modeling multiple ecosystem services, biodiversity conservation, commodity production, and tradeoffs at landscape scales. *Frontiers in Ecology and the Environment*, 7(1), 4–11. https://doi.org/10.1890/080023
- Redhead, J. W., Stratford, C., Sharps, K., Jones, L., Ziv, G., Clarke, D., Oliver, T. H., & Bullock, J. M. (2016). Empirical validation of the InVEST water yield ecosystem service model at a national scale. *Science of the Total Environment*, 569–570, 1418–1426. https://doi.org/10.1016/j.scitotenv.2016.06.227
- Tallis, H., & Polasky, S. (2009). Mapping and valuing ecosystem services as an approach for conservation and natural-resource management. *Annals of the New York Academy of Sciences*, 1162(1), 265–283. https://doi.org/10.1111/j.1749-6632.2009.04152.x
