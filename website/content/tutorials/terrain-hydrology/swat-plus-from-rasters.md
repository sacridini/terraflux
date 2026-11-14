---
title: A SWAT+ project from rasters, and its results on the map
short: SWAT+ from rasters
description: Divide a terrain, a land use map and a soil map into subbasins, channels and HRUs as QSWAT+ does, write the SWAT+ project with its weather, change a parameter, run it, and map the soil loss, the runoff and the flow of the channels.
order: 50
level: advanced
time: 60 min
template: SWAT+ from rasters
ops: [swat.watershed, swat.wgn, swat.from_tables, swat.set, swat.run, swat.hru_map, swat.channel_map, swat.reach_discharge]
---

The [previous tutorial](../swat-plus/) ran a SWAT+ project somebody else had built. Most studies start earlier: with a
terrain, a map of land use, a map of soils and a table of weather, and the work of turning them into a project that
SWAT+ can read. In this tutorial you do that in Terraflux. You divide a small valley into subbasins, channels and
*hydrologic response units*, give it soils and two years of daily weather, write the project, change two of its
parameters, run it, and bring the results back onto the map: the soil each HRU loses, the water it sheds, and the
flow of every channel. At the end you have one model that goes from rasters to maps of results, and whose
parameters a batch or a calibration can vary.

## The idea

Building a SWAT+ project by hand means writing dozens of text files that describe the same landscape from different
sides: which HRUs there are and what land use, soil and slope each has, which channel each one drains to, how long
and how wide the channels are, which weather station each object takes its weather from. Two programs did this
until now. **QSWAT+** (Dile et al., 2016), a plugin of QGIS, divides the watershed (the part that works on rasters);
the **SWAT+ Editor** then writes the files, adds the plants, the management and the weather, and starts SWAT+
(Bieger et al., 2017). Terraflux does both through landschaft, which ports their methods one for one:
[[swat.watershed]] is QSWAT+'s division and [[swat.from_tables]] the Editor's writing, and the files they write
are the Editor's, down to the spacing of the columns.

!!! science "Key idea: from cells to HRUs"
    QSWAT+ divides the terrain in four steps. The DEM is filled and every cell drains to its steepest neighbour
    (D8, as in [part 1](../depressions-and-flow/)); every cell that drains an area of a *stream threshold* or more
    is a stream, and each piece of stream between two junctions is a **channel**; the land that drains to each
    channel is its **subbasin**; and inside a subbasin the cells are grouped by **land use**, **soil** and
    **slope class**. A group is an **HRU**. An HRU is not one patch: all the cells of a subbasin with the same
    combination are one HRU, wherever they are. SWAT+ computes one water balance for it and gives every cell the
    result.

    How many HRUs a subbasin keeps is a choice, and the next section's table shows how much it matters.
    QSWAT+'s default keeps one, the largest combination (*dominant HRU*), and gives the area of the others to it;
    *filter* keeps every combination above a share of the land use, the soil and the slope; *area* keeps those
    above an area.

The maps of results go the other way. SWAT+ gives a table with a row per HRU and per channel; [[swat.hru_map]] and
[[swat.channel_map]] put each value on the cells of its HRU or channel, so that a soil loss is a map you can
compare with the land use, and [[swat.reach_discharge]] gives the daily flow of every channel as a table, for a
hydrograph.

## When to use it

- You have maps and weather but no SWAT+ project, and want the same watershed model a QSWAT+ user would build,
  from a script that can be repeated.
- You want to try a *scenario* of the land: change the land use map, the soil map or the HRU method and run again;
  every step follows from the maps.
- You want to **calibrate** the model or run it many times with other values (a *batch*): a project that is a
  value in the graph, with parameters that read the model's inputs, is what the application's **Many runs** and
  **Calibration** vary.

What it is not: a replacement for care. The soils here are two made-up loams, the weather is made up, and the numbers
mean nothing for a real valley. With real data, give the soil table, the land use names and the weather the
attention that QSWAT+ and the Editor ask of you, and calibrate against measured flow.

## What you need

- **A DEM** in a projected coordinate system (metres), or in longitude and latitude: the model needs the
  latitude of every HRU for the sun. A map without a coordinate system is refused.
- **A land use map and a soil map**, whole numbers, on the DEM's grid or any other (they are put on the DEM's
  cells by the nearest cell).
- **Two lookup tables**: which plant or urban land use of SWAT+ each land use code is (`FRSD` deciduous forest,
  `PAST` pasture, `AGRL` generic agriculture... the names of SWAT+'s plants and urban databases), and which soil
  each soil code is.
- **The soils**: a table with a row per layer, in SWAT+'s columns (`name`, `hyd_grp`, `dp_tot`, `texture`, `dp`,
  `bd`, `awc`, `soil_k`, `carbon`, `clay`, `silt`, `sand`, `usle_k`...), or the `usersoil` table of ArcSWAT and QSWAT+.
- **The weather**: the weather stations (name, latitude, longitude, elevation) and a table of daily values each
  of precipitation, maximum and minimum temperature, with a row per day (`year` and `day`, or a `date`) and a
  column per station. Solar radiation, humidity and wind are optional: SWAT+ generates what is missing.
- **Here** all of this is drawn or written in the template **SWAT+ from rasters**: a valley of 3 km by 3 km in
  cells of 100 m, a land use of forest and pasture, two soils, one station and two years of weather made up. To
  use your own data, replace the nodes that draw or write them with [[raster.read]] and [[table.read]].

## Build the model

### Divide the watershed

<div class="steps" markdown>

1. In the library, **Terrain hydrology**, click [[swat.watershed]]. Connect the DEM to `dem`, the land use map to
   `landuse`, the soil map to `soils`, and the three tables to `landuse_lookup`, `soil_lookup` and `soil_table`.
2. Set **stream_threshold** (km²): the drainage area that makes a stream. Leave **hru_method** at `dominant_hru`.
3. Run it (++ctrl+enter++). It gives the **watershed** (a value that [[swat.from_tables]] reads), the maps of
   the **subbasins**, the **channels** and the **hrus**, and the table of the HRUs.

</div>

```tfgraph
#! caption: A terrain, a land use and a soil drawn by chance, and QSWAT+'s division of them.
#! highlight: watershed
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "relief"
op = "nlm.fbm"
params = { rows = 30, columns = 30, dimension = 1.4, seed = 91, resolution = 100, crs = "+proj=tmerc +lat_0=-15 +lon_0=-47 +k=1 +x_0=0 +y_0=0 +datum=WGS84 +units=m" }
note = "The shape of the terrain, in cells of 100 m on a made-up place."

[[nodes]]
id = "dem"
op = "raster.calc"
inputs = { f = "relief.map" }
params = { expression = "700 + 300 * f" }

[[nodes]]
id = "cover"
op = "nlm.random_cluster"
inputs = { like = "relief.map" }
params = { p = 0.5, weights = [0.6, 0.4], seed = 92 }
note = "The land use: 1 forest, 2 pasture."

[[nodes]]
id = "soils"
op = "raster.calc"
inputs = { f = "relief.map" }
params = { expression = "10 if f > 0.4 else 20" }
note = "The soils: 10 on the uplands (above 820 m), 20 in the lower valley."

[[nodes]]
id = "landuse_lookup"
op = "table.of"
params = { columns = ["code", "name"], rows = [[1, "FRSD"], [2, "PAST"]] }

[[nodes]]
id = "soil_lookup"
op = "table.of"
params = { columns = ["code", "name"], rows = [[10, "UPLAND_LOAM"], [20, "VALLEY_CLAY_LOAM"]] }

[[nodes]]
id = "soil_table"
op = "table.read"
params = { path = "soil_table.csv" }
note = "A row per layer of each soil, in SWAT+'s soils.sol columns (the template writes it inline)."

[[nodes]]
id = "watershed"
op = "swat.watershed"
inputs = { dem = "dem.map", landuse = "cover.map", soils = "soils.map", landuse_lookup = "landuse_lookup.table", soil_lookup = "soil_lookup.table", soil_table = "soil_table.table" }
params = { stream_threshold = 0.3 }
note = "Subbasins, channels and one HRU per subbasin."
```

The valley comes out as nine subbasins with nine channels and an HRU each (714 ha of the 900). Look at the maps
(`subbasins`, `channels`, `hrus`) and at the table of HRUs, which lists for each its land use, soil, slope in
percent and area:

![The subbasins, the channels and the HRUs](/img/tutorials/terrain-hydrology/swat-plus-from-rasters/division.png "The division of the valley: the nine subbasins (left, one colour each), the channels (middle, the darker the larger) and the HRUs (right: forest in green, pasture in ochre, the upland loam darker; the cells whose combination was folded into the dominant HRU of their subbasin are empty).")

The **stream_threshold** sets how fine the network is, and with it the number of subbasins and of HRUs:

```tftable
data: /data/tutorials/terrain-hydrology/swat-plus-from-rasters/threshold.csv
digits: 2
caption: "The stream threshold of a valley of 9 km2 against the number of subbasins and of HRUs (one per subbasin with dominant_hru). The template uses 0.3 km2; QSWAT+ proposes 1% of the area."
```

**hru_method** and the other HRU settings decide how much of each subbasin's variety survives:

```tftable
data: /data/tutorials/terrain-hydrology/swat-plus-from-rasters/methods.csv
digits: 0
caption: "The HRUs of the same valley (threshold 0.3 km2) by method: one per subbasin (dominant_hru and dominant), every combination above 10% of land use, soil and slope (filter), and the same with the slope in three bands (slope_bands 15 and 35). More HRUs resolve the land better and cost more time."
```

### Write the project

SWAT+ needs the weather and the soils' *weather generator*, the monthly statistics it uses to make up the days and the
variables a series lacks. [[swat.wgn]] computes them from the daily series (the means and spreads of the
temperatures, the rain on wet days, the chance of a wet day after a dry and after a wet one, as SWAT's WXGEN
defines them); what daily precipitation and temperature cannot give (the largest half-hour rain of each month, the
means of sun, dew point and wind) you give in **monthly**.

<div class="steps" markdown>

1. Read the weather with [[table.read]] (a CSV with `year`, `day`, `pcp`, `tmax` and `tmin`) and make a table of each
   variable with [[table.select]]: `year`, `day` and the variable, **renamed** to the station's name.
2. Add a [[table.of]] with the **stations**: `name`, `lat`, `lon`, `elev`.
3. Add [[swat.wgn]], connect the three series, and give **monthly** (`pcp_hhr`, `slr_ave`, `dew_ave`, `wnd_ave`).
4. Add [[swat.from_tables]]: the **watershed**, the **stations**, the three series and the **wgn**. It gives the
   **project**, the files of a TxtInOut folder as a value.

</div>

```tfgraph
#! caption: The weather as tables, the weather generator's statistics and the SWAT+ project.
#! highlight: wgn, project
#! numbered: true
#! fragment: true
format = "terraflux/1"

[[nodes]]
id = "weather"
op = "table.read"
params = { path = "weather.csv" }
note = "A row per day: year, day of the year, pcp (mm), tmax and tmin (deg C)."

[[nodes]]
id = "pcp"
op = "table.select"
inputs = { table = "weather.table" }
params = { columns = ["year", "day", "pcp"], rename = { pcp = "valley" } }

[[nodes]]
id = "tmax"
op = "table.select"
inputs = { table = "weather.table" }
params = { columns = ["year", "day", "tmax"], rename = { tmax = "valley" } }

[[nodes]]
id = "tmin"
op = "table.select"
inputs = { table = "weather.table" }
params = { columns = ["year", "day", "tmin"], rename = { tmin = "valley" } }

[[nodes]]
id = "stations"
op = "table.of"
params = { columns = ["name", "lat", "lon", "elev"], rows = [["valley", -14.98, -46.98, 850]] }

[[nodes]]
id = "wgn"
op = "swat.wgn"
inputs = { pcp = "pcp.table", tmax = "tmax.table", tmin = "tmin.table" }
params = { monthly = { pcp_hhr = 8, slr_ave = [4, 7, 11, 16, 20, 23, 25, 21, 15, 9, 5, 3], dew_ave = 4, wnd_ave = 2.5 } }

[[nodes]]
id = "project"
op = "swat.from_tables"
inputs = { watershed = "watershed.watershed", stations = "stations.table", pcp = "pcp.table", tmax = "tmax.table", tmin = "tmin.table", wgn = "wgn.wgn" }
note = "The SWAT+ Editor's files, from the watershed and the weather."
```

The project simulates the days all the weather files cover (**start** and **end** change that), takes its
**codes** from the Editor's defaults (**codes** = `{ pet = 2 }` chooses Hargreaves' evapotranspiration, for example)
and keeps the watershed it came from, which is what lets the maps below put results on the cells. If you do not
connect **wgn**, [[swat.from_tables]] computes the same statistics from the series; the node is there to show them
and to replace them with the ones of SWAT+'s own weather generator database.

!!! warning "What is refused"
    Wetland land uses are refused (the port does not simulate wetlands), and so is a land use name that is neither
    a plant of `plants.plt` nor an urban land use of `urban.urb` ("... does not exist in plants_plt or urban_urb").
    The first line of each file the project writes carries the date it was written, as the Editor's do.

### Change a parameter

[[swat.set]] changes a parameter as SWAT+'s *hard calibration* does: it adds a row to the project's
`calibration.cal`, which SWAT+ applies before it starts. Choose the **parameter** (`cn2`, the curve number of
runoff, `awc`, the water the soil holds, `esco`, `perco`... the 69 the port calibrates), **change** it by a
percentage (`pctchg`), an amount (`abschg`) or to a value (`absval`), and, for a soil parameter, choose the layers
(**first_layer**, **last_layer**) and, for any, the HRUs (**objects**, from 1). Nodes in a chain make their changes in
order, and none changes the project it is given.

The point of the node is the **setting that reads a model input**: `value = "$cn2_change"`. The percentage is now an
input of the model, and anything that varies inputs varies it: **Many runs** with a list of values, a sensitivity
analysis, a calibration against observed flow. The template has two, `cn2_change` and `awc_change`, both 0.

### Run it, and put the results on the map

A [[swat.run]] node prints its tables at one step, so the model runs the project twice: once for the HRUs' average
year (**step** `aa`, **tables** `hru_wb` and `hru_ls`) and once for the channels' days (**step** `day`, **tables**
`channel_sd`). The first of the two years is the spin-up the project skips (the `print.prt` the project writes has
**nyskip** 1: the soil water and the aquifers settle in it); set **nyskip** of the [[swat.run]] to 0 to print both.

The maps read those tables: connect `yearly.hru_ls` to the `hru_ls` input of [[swat.hru_map]] and choose the
**variable** (`sedyld`, the soil lost in t/ha a year; `surq_gen`, the surface runoff in mm; `wateryld`, `et`...);
connect `daily.channel_sd` to [[swat.channel_map]] (by default the mean discharge in m³/s) and to
[[swat.reach_discharge]]. Without a table connected they run the project themselves, which works but simulates it
again for each map.

```tfgraph
#! caption: Two runs of the project, and the maps and the table made from them.
#! highlight: sediment, runoff, flow, discharge
#! numbered: true
#! fragment: true
format = "terraflux/1"

[[nodes]]
id = "curve_number"
op = "swat.set"
inputs = { project = "project.project" }
params = { parameter = "cn2", change = "pctchg", value = "$cn2_change" }

[[nodes]]
id = "soil_water"
op = "swat.set"
inputs = { project = "curve_number.project" }
params = { parameter = "awc", change = "pctchg", value = "$awc_change" }

[[nodes]]
id = "yearly"
op = "swat.run"
inputs = { project = "soil_water.project" }
params = { step = "aa", tables = ["hru_wb", "hru_ls"] }

[[nodes]]
id = "daily"
op = "swat.run"
inputs = { project = "soil_water.project" }
params = { step = "day", tables = ["channel_sd"] }

[[nodes]]
id = "sediment"
op = "swat.hru_map"
inputs = { project = "soil_water.project", hru_ls = "yearly.hru_ls" }
params = { variable = "sedyld", how = "annual" }

[[nodes]]
id = "runoff"
op = "swat.hru_map"
inputs = { project = "soil_water.project", hru_wb = "yearly.hru_wb" }
params = { variable = "surq_gen", how = "annual" }

[[nodes]]
id = "flow"
op = "swat.channel_map"
inputs = { project = "soil_water.project", channel_sd = "daily.channel_sd" }
params = { column = "flo_out", how = "mean" }

[[nodes]]
id = "discharge"
op = "swat.reach_discharge"
inputs = { project = "soil_water.project", channel_sd = "daily.channel_sd" }
```

=== "Application"

    Start from the template **SWAT+ from rasters** (group *Surfaces and uncertainty*). Run the whole model
    (++f5++) and open each map's tab in the **Viewer**. The two model inputs, `cn2_change` and `awc_change`, can be
    changed and the model run again, or varied over a list of values in **Many runs**.

=== "Command line"

    The model reads its inputs by name; `--set` gives them a value:

    ```bash
    terraflux run swat-plus-from-rasters.toml --out output --set cn2_change=-10
    ```

## Read the results

```tfgallery
columns: 3
caption: "The results of the project on the map. Left: the soil lost by each HRU (t/ha a year, MUSLE). Middle: the surface runoff (mm a year). Right: the mean discharge leaving each channel (m3/s), on the stream cells."
images:
  - { src: /img/tutorials/terrain-hydrology/swat-plus-from-rasters/sediment.png, caption: "swat.hru_map: sedyld" }
  - { src: /img/tutorials/terrain-hydrology/swat-plus-from-rasters/runoff.png, caption: "swat.hru_map: surq_gen" }
  - { src: /img/tutorials/terrain-hydrology/swat-plus-from-rasters/flow.png, caption: "swat.channel_map: flo_out" }
```

Each HRU is one value, so the maps show the division more than the terrain: look for the contrast between HRUs (the
pasture and the forest, the two soils) and for the channels that grow downstream, where the discharge adds up. The
mean discharge of the channel that leaves the valley is the largest value of the flow map. The soil losses are very
high (hundreds of tonnes per hectare a year on the worst HRUs): this is a project of made-up soils, weather and land
use, with the model's defaults and no calibration, and its numbers are for learning the method, not for reading as a
prediction; the [previous tutorial](../swat-plus/) shows how far default parameters can be from a real watershed.

The daily discharge of every channel, from [[swat.reach_discharge]], gives the hydrograph:

![The daily discharge of the channels](/img/tutorials/terrain-hydrology/swat-plus-from-rasters/hydrograph.png "The daily discharge (m3/s) leaving the channel at the outlet of the valley and the headwater channels, over the second year (the first is skipped). The flow follows the rainy summer of the made-up weather and falls in the dry winter.")

!!! paper "In the literature"
    Report how the project was built: the DEM and its resolution, the land use and soil maps and the source of the
    soil properties, the stream threshold and the HRU method with its thresholds, the weather stations and how
    the generator's statistics were found, the simulation period and the years skipped, the version of SWAT+ (here
    62.0.1 through landschaft), the parameters changed and the calibration. Report the number of subbasins and HRUs.

## Settings that change the answer

**The parameters of the project.** The two inputs of the template change the whole project at once. A percentage on
the curve number or on the water the soil holds moves the averages of the valley a little, in the direction the
processes say for the curve number (a higher one, more runoff and more soil lost) and not always for the water
the soil holds, which also changes the evapotranspiration and the percolation:

```tftable
data: /data/tutorials/terrain-hydrology/swat-plus-from-rasters/parameters.csv
digits: 3
caption: "The mean over the HRUs (or the channels) of the soil lost, the surface runoff and the discharge, when the curve number (cn2) and the soil's water capacity (awc) are changed by a percentage in every HRU, and the largest daily discharge of any channel."
```

**The division.** More subbasins and HRUs follow a lower stream threshold or another **hru_method**, and every result
follows (the tables above). Compare the maps of the results with two divisions before you trust the detail they show.

**The spin-up.** A year of **nyskip** is skipped by default; with a shorter weather record, choose the years
printed deliberately.

**The soil loss's cover factor.** [[swat.run]]'s **c_factor** (and the same setting of the maps, when they run the
project themselves) chooses MUSLE's cover factor: `swatplus` (SWAT+ 62's, the default) or `plant` (the plants'
minimum, the earlier method). The sediment of a project whose plants are bare changes by orders of magnitude; see the
[previous tutorial](../swat-plus/).

## Common pitfalls

- **A DEM without a coordinate system.** [[swat.watershed]] refuses it. The template's relief carries one; with a
  [[raster.read]], the file's own is used.
- **Codes that are not in the lookup.** A land use or a soil code the lookup table does not list is an error
  ("codes [...] are not in the land use lookup"); the cells with no value take QSWAT+'s default (code 0 if listed,
  else the first listed). Name every code the map holds.
- **The weather period.** The project simulates the days that **all** the series cover; a station whose series is
  shorter shortens the run. Missing days (empty cells) are generated.
- **Maps of a project from `swat.load`.** [[swat.hru_map]] and [[swat.channel_map]] need the watershed the project
  came from, which only [[swat.from_tables]] keeps; a project read from a folder has no map of its HRUs.
- **A table of another step.** A map reads a table of the step it needs (`aa` for the annual HRU values, `day` for
  the others); the error says what to change in the [[swat.run]].
- **Mixing up HRUs and patches.** An HRU is every cell of a subbasin with its combination. A cell whose
  combination was folded into another has no value.

## Going further

- [Running a SWAT+ project](../swat-plus/): the project Hubbard Brook, its water balance and its plausibility checks,
  and [[swat.load]], which reads a TxtInOut folder or a zip as a value that [[swat.set]] can change.
- [Streams, watersheds and HAND](../streams-watersheds-and-hand/): the division of a terrain that QSWAT+ builds on.
- **Many runs** and **Calibration** in the application: vary `cn2_change` and `awc_change` against observed flow.
- In Python, the same model is written by `terraflux python` as a script of landschaft: `ls.hydro.swat.watershed`,
  `from_tables`, `run`, `hru_map`, `channel_map` and `reach_discharge`.
- The operators' reference: [[swat.watershed]], [[swat.from_tables]], [[swat.wgn]], [[swat.set]], [[swat.run]],
  [[swat.hru_map]], [[swat.channel_map]], [[swat.reach_discharge]].

## References

- Arnold, J. G., Srinivasan, R., Muttiah, R. S., & Williams, J. R. (1998). Large area hydrologic modeling and assessment part I: Model development. *Journal of the American Water Resources Association*, 34(1), 73–89. https://doi.org/10.1111/j.1752-1688.1998.tb05961.x
- Bieger, K., Arnold, J. G., Rathjens, H., White, M. J., Bosch, D. D., Allen, P. M., Volk, M., & Srinivasan, R. (2017). Introduction to SWAT+, a completely restructured version of the Soil and Water Assessment Tool. *Journal of the American Water Resources Association*, 53(1), 115–130. https://doi.org/10.1111/1752-1688.12482
- Dile, Y. T., Daggupati, P., George, C., Srinivasan, R., & Arnold, J. (2016). Introducing a new open source GIS user interface for the SWAT model. *Environmental Modelling & Software*, 85, 129–138. https://doi.org/10.1016/j.envsoft.2016.08.004
- Moriasi, D. N., Arnold, J. G., Van Liew, M. W., Bingner, R. L., Harmel, R. D., & Veith, T. L. (2007). Model evaluation guidelines for systematic quantification of accuracy in watershed simulations. *Transactions of the ASABE*, 50(3), 885–900. https://doi.org/10.13031/2013.23153
