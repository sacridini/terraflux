---
title: Watershed simulation with SWAT+
short: SWAT+
description: Run a SWAT+ project day by day in Terraflux, read its water balance, streamflow and soil loss, and learn which settings and checks decide whether the numbers can be trusted.
order: 40
level: advanced
time: 50 min
ops: [swat.run, table.write, view.plot]
---

The first three parts of this series described where water *would* go on a terrain. A watershed model says how
much goes there, and when: how much of the rain and snow of each day runs off at once, how much the soil keeps,
how much the vegetation sends back to the air, how much reaches the aquifers and the stream, and how much soil
the runoff carries away. In this tutorial you run SWAT+, one of the most used watershed models, on a small
forested watershed of the Hubbard Brook Experimental Forest in New Hampshire, read its yearly, monthly and daily
results, compare the soil loss of its land units with two cover factors, and learn to check a model run before
you believe it, because this one, as you will see, has a problem worth finding.

## The idea

**SWAT**, the *Soil and Water Assessment Tool*, was developed by the US Department of Agriculture's Agricultural
Research Service and Texas A&M University to predict the effect of land management on water, sediment and
farm chemicals in large basins over long periods (Arnold et al., 1998). It has been applied in thousands of
studies, to water supply, erosion, nutrients, land use and climate change (Gassman et al., 2007). **SWAT+**
(Bieger et al., 2017) is a complete restructuring of SWAT: the same processes, but the watershed is built from
*objects* connected by flows, which makes it possible to represent how landscape units are linked (a hillslope
draining onto a floodplain, an aquifer feeding a stream).

The objects of a SWAT+ project:

- **HRUs**, *hydrologic response units*: the land, divided into areas of one land use, one soil and one slope
  class. An HRU is not one patch on the map: all the cells of a subbasin with that combination, wherever they
  are, form one HRU. Each day, SWAT+ computes the water, snow, plant growth, nutrients and erosion of each HRU.
- **Routing units**: groups of HRUs (often a subbasin) whose outflows are added and sent on.
- **Aquifers**: a shallow aquifer, which returns water to the channels as baseflow, and a deep one, which water
  leaves the system through.
- **Channels**: the streams, which route water and sediment from one to the next down to the outlet.
- **Weather**: stations with measured series (precipitation, temperature...) and a *weather generator* that
  makes up the variables and days without measurements from monthly statistics.

![The objects of the example project](/img/tutorials/terrain-hydrology/swat-plus/objects.png "The objects of the Hubbard Brook project and how its connection files join them: one weather station, ten HRUs in one routing unit, which sends its surface and lateral flow to the channel and its percolation to the shallow aquifer; the aquifer returns baseflow to the channel and recharges a deep aquifer.")

!!! science "Key idea: the daily water balance of an HRU"
    Every day, for every HRU, SWAT+ updates the water stored in the soil profile:

    \[ SW_t = SW_{t-1} + R - Q_{surf} - E_a - w_{perc} - Q_{lat} \]

    where \( SW \) is the soil water (mm), \( R \) the rain and snowmelt reaching the ground, \( Q_{surf} \) the
    surface runoff, computed with the US Soil Conservation Service's *curve number* (a number from about 30 to
    100 for how readily a land use on a soil produces runoff), \( E_a \) the actual evapotranspiration (from the
    canopy, the plants and the soil), \( w_{perc} \) the water percolating below the soil to the aquifer, and
    \( Q_{lat} \) the lateral flow through the soil to the stream. Precipitation falls as snow below a
    temperature threshold and melts later.

The soil loss of an HRU comes from the *modified universal soil loss equation*, MUSLE (Williams, 1975), which
replaces the rainfall factor of the USLE by the runoff of the day:

\[ \mathrm{sed} = 11.8 \, (Q_{surf} \, q_{peak} \, \mathrm{area})^{0.56} \, K \, C \, P \, LS \, \mathrm{CFRG} \]

with \( Q_{surf} \) the surface runoff (mm), \( q_{peak} \) the peak runoff rate (m³/s), the area of the HRU
(ha), \( K \) the soil's erodibility, \( C \) the cover and management factor (how much the vegetation and its
litter protect the soil), \( P \) the support practice factor, \( LS \) the length–slope factor (the sediment
transport index of [part 3](../wetness-and-connectivity/) is the same idea) and CFRG a factor for coarse
fragments; the result is in tonnes per day.

**In Terraflux**, [[swat.run]] runs a SWAT+ project through landschaft's port of SWAT+ 62.0.1 to C++. For what
the port covers (measured and generated weather; HRUs with snow, evapotranspiration, curve-number runoff,
MUSLE erosion, percolation, lateral flow, nutrients, plant growth and management; routing units; aquifers;
SWAT-DEG channels) it gives SWAT+'s own results, value for value: landschaft checks it against SWAT+ itself,
every printed field of every day, on official examples including this one. What it does not cover
(reservoirs, wetlands, pesticides, salts and other constituents, Green-Ampt infiltration, the CENTURY carbon
model, water allocation) is refused with an error rather than skipped.

!!! warning "What Terraflux does not do (yet)"
    Terraflux **runs existing SWAT+ projects**. It does not build one from rasters: the division of a DEM, a
    land use map and a soil map into subbasins, channels and HRUs, and the writing of the project's files, are
    done by QSWAT+ (a QGIS plugin) and the SWAT+ Editor, or in Python by landschaft
    (`ls.hydro.swat.from_rasters`). The streams, links and subbasins of [part 2](../streams-watersheds-and-hand/)
    are the same kind of division QSWAT+ makes, but [[swat.run]] does not take them as input.

## When to use it

- **Water and sediment budgets** of a watershed: how much water leaves it in a year, a month, a day; which land
  units produce the runoff and the soil loss.
- **Scenarios**: what changes in streamflow, erosion or nutrients if a forest is cut, a pasture reforested,
  the climate drier. Run the same project with edited files (land use, weather) and compare.
- **Ecological questions that depend on flow**: low-flow periods for fish and invertebrates, sediment reaching
  spawning gravels, the water a wetland or riparian forest receives.

What it is *not*: a model that works out of the box. A SWAT+ project has thousands of parameters, most of them
defaults from databases. Before its numbers mean anything for a real watershed it must be **calibrated and
validated** against measured streamflow (and sediment or nutrients if you report them), and its results should
be judged with standard statistics such as the Nash–Sutcliffe efficiency (Nash & Sutcliffe, 1970) against
published criteria (Moriasi et al., 2007). Terraflux runs the model; calibration is up to you. For questions
about the shape of the terrain only (where water converges, which slopes connect to streams), the indices of
parts 1 to 3 are simpler and need no parameters.

## What you need

- **A SWAT+ project**: a *TxtInOut* folder, the folder of text files that QSWAT+ and the SWAT+ Editor write and
  that SWAT+ reads (`file.cio` lists all the others), or a zip of it. Outputs of earlier runs left in the folder
  (`.txt`, `.out`, `.csv` files...) are ignored.
- **Here**: `hubbard_brook.zip`, the example landschaft ships (in Terraflux's repository,
  `extern/landschaft/src/landschaft/hydro/swat/data/`), one of the official SWAT+ examples
  (SWATPlus-datasets, LGPL-2.1). It models watershed 2 of the Hubbard Brook Experimental Forest, the watershed
  whose forest was cut in the deforestation experiment of Likens et al. (1970): 16.3 ha, 10 HRUs (young forest,
  mature forest and two small barren units), one routing unit, one channel, a shallow and a deep aquifer, 79
  files. The simulation runs from 2000 to 2002 with daily precipitation and temperature from local gauges and
  the other weather from the generator; its `print.prt` skips the first year. Save it next to your model.

!!! note "A project written for an earlier SWAT+"
    The fifth value of `parameters.bsn` is read by SWAT+ 62 as the *peak rate factor* (484 in projects written
    by the SWAT+ Editor 4). Files written for earlier versions, including the original official Hubbard Brook
    example, have 1 there, and SWAT+ 62 then computes peak rates, and erosion, hundreds of times too low
    without a word. landschaft's copy has 484. [[swat.run]] warns when it finds a value below 10.

## Build the model

### The yearly water balance

<div class="steps" markdown>

1. In the library's subject **Terrain hydrology**, click [[swat.run]]. In the Node pane, set **path** with
   **Choose...** (a zip) or **A folder...** (a TxtInOut folder); in the model file the path is relative to the
   model's folder: `hubbard_brook.zip`. Rename the node `swat`.
2. Leave **step** at `year`. Run the node (++ctrl+enter++). Its outputs are tables, shown under the canvas,
   where you can sort and filter them.
3. To keep a table, add a [[table.write]] (subject **IO**) and connect the output to its `table`, for example
   `swat`'s `basin_wb` with **path** `basin_wb.csv`.

</div>

```tfgraph
#! caption: SWAT+ run on the Hubbard Brook project, its basin's yearly water balance and its HRUs' written to CSV files.
#! highlight: swat
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "swat"
op = "swat.run"
params = { path = "hubbard_brook.zip", step = "year" }
note = "Hubbard Brook W2, 2000-2002."

[[nodes]]
id = "basin"
op = "table.write"
inputs = { table = "swat.basin_wb" }
params = { path = "basin_wb.csv" }

[[nodes]]
id = "hrus"
op = "table.write"
inputs = { table = "swat.hru_wb" }
params = { path = "hru_wb.csv" }
```

[[swat.run]] has no inputs: everything is in the project. Its outputs are the tables SWAT+ prints, with SWAT+'s
own column names and units:

| output | what | a row per |
|---|---|---|
| `basin_wb`, `hru_wb` | water balance (mm): precip, snofall, snomlt, surq_gen, latq, wateryld, perc, et, esoil, eplant, cn, sw_ave, pet... | date (basin), date and HRU |
| `basin_nb`, `hru_nb` | nutrient balance (kg/ha): fertiliser, fixation, denitrification, uptake... | date, date and HRU |
| `basin_ls`, `hru_ls` | losses: sedyld (t/ha), usle, nitrogen and phosphorus with sediment and runoff (kg/ha)... | date, date and HRU |
| `basin_pw`, `hru_pw` | plants and weather: lai, bioms, yield, residue, stresses, temperatures, radiation... | date, date and HRU |
| `channel_sd` | the channels: flo_in and flo_out (m³/s), sed_in and sed_out, nutrients, water temperature... | date and channel |
| `aquifer` | the aquifers: flo (to the channels, mm), stor, rchrg, seep, dep_wt (depth of the water table, m)... | date and aquifer |
| `weather` | the daily weather of each station: precip, tmax, tmin, solrad, rhum, windsp, pet... | year, day and station |

Every table starts with `year` and `day` (the day of the year at the end of the period: 365 for a year, the
last day of the month for a month), then the object's number (`hru`, `channel`, `aquifer`, `station`), then
the values. The `weather` table is always daily, for the whole period and every station of the project (here
19 stations × 1,096 days).

The settings:

- **step**: `day`, `month`, `year` (the default) or `aa`, the average annual over the printed years (one row).
- **tables**: which tables to print, a list such as `["basin_wb", "channel_sd"]` (default: all). A table not
  chosen has no value, and a node connected to it fails with "has no value", so choose what you connect.
  Printing fewer tables is faster for long runs.
- **nyskip**: years simulated before printing, a *spin-up* (warm-up) for the soil water, the aquifers and the
  plants. Without it, the project's own value in `print.prt` applies (1 here).
- **c_factor**: the cover factor \( C \) of MUSLE, `swatplus` (the default) or `plant` (see below).

### Daily streamflow and the HRUs on average

<div class="steps" markdown>

1. Add a second [[swat.run]] named `daily`, the same **path**, **step** `day`, **tables**
   `["basin_wb", "channel_sd"]` and **nyskip** `0`, to see all three years.
2. Add a third named `average`, **step** `aa`, **tables** `["hru_wb", "hru_ls", "hru_pw"]`.
3. Write their tables with [[table.write]], or look at them with [[view.plot]] (its `table` input; **chart**
   `lines`, **x** and **y** the columns to draw).

</div>

```tfgraph
#! caption: The same project, by day (streamflow) and as an average year (the HRUs).
#! highlight: daily, average
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "daily"
op = "swat.run"
params = { path = "hubbard_brook.zip", step = "day", tables = ["basin_wb", "channel_sd"], nyskip = 0 }

[[nodes]]
id = "flow"
op = "table.write"
inputs = { table = "daily.channel_sd" }
params = { path = "channel_day.csv" }

[[nodes]]
id = "rain"
op = "table.write"
inputs = { table = "daily.basin_wb" }
params = { path = "basin_day.csv" }

[[nodes]]
id = "average"
op = "swat.run"
params = { path = "hubbard_brook.zip", step = "aa", tables = ["hru_wb", "hru_ls", "hru_pw"] }

[[nodes]]
id = "hru_water"
op = "table.write"
inputs = { table = "average.hru_wb" }
params = { path = "hru_wb_aa.csv" }

[[nodes]]
id = "hru_loss"
op = "table.write"
inputs = { table = "average.hru_ls" }
params = { path = "hru_ls_aa.csv" }
```

Each [[swat.run]] node simulates the whole period; the three years take a fraction of a second here. For a
scenario, copy the project, edit its files (a land use in `hru-data.hru`, the weather files) and add another
[[swat.run]] node that reads the copy.

=== "Application"

    Click a [[swat.run]] node and press ++ctrl+enter++: its tables show under the canvas, where you can sort
    and filter them. A table connected to a [[view.plot]] is drawn as a chart in a tab of its own.

=== "Command line"

    Save the model as `swat.toml` beside `hubbard_brook.zip` and run it; the [[table.write]] nodes write the
    CSV files beside the model:

    ```bash
    terraflux check swat.toml
    terraflux run swat.toml --out output
    ```

## Read the results

**The water balance.** All values are in millimetres of water over the basin (1 mm over 16.3 ha is 163 m³):

```tftable
data: /data/tutorials/terrain-hydrology/swat-plus/basin-year.csv
labels: { year: "Year", precip: "Precipitation", snofall: "Snowfall", snomlt: "Snowmelt", et: "Evapotranspiration", surq_gen: "Surface runoff", latq: "Lateral flow", perc: "Percolation", wateryld: "Water yield" }
digits: 1
caption: "The yearly water balance of the basin (basin_wb, mm), for the two printed years and their average (step aa). The water yield is what reaches the channel from the land on the same day (surface runoff plus lateral flow); percolation reaches it later, as baseflow from the aquifer."
```

About a quarter of the precipitation falls as snow. Of 1,157 mm a year, 442 mm run off the surface, 31 mm flow
laterally through the soil, 563 mm percolate to the aquifer (which returns most of it to the channel as
baseflow) and only 127 mm evaporate.

**Is that plausible?** Here is the check every SWAT+ run needs. Potential evapotranspiration is 680 mm a year,
and a temperate forest in leaf transpires much of it, yet the model evaporates 127 mm, all of it from the soil:
the plants transpire nothing (`eplant` is 0). The monthly results show why:

```tfchart
type: bar
data: /data/tutorials/terrain-hydrology/swat-plus/basin-month.csv
x: month
y: ["Evapotranspiration", "Surface runoff", "Lateral flow", "Percolation"]
colors: { "Evapotranspiration": "#3b8a5a", "Surface runoff": "#e8901f", "Lateral flow": "#a7c957", "Percolation": "#2f6f9f" }
stacked: true
ylabel: mm per month
height: 320
caption: "Where the water goes, month by month, 2000 to 2002 (nyskip 0, so the spin-up year is shown). In the summer of 2000 the forest evaporates up to 95 mm a month; from 2001 on, never more than 22."
```

```tfchart
type: line
data: /data/tutorials/terrain-hydrology/swat-plus/basin-month.csv
x: month
y: ["Leaf area index", "Residue (t/ha)"]
colors: { "Leaf area index": "#3b8a5a", "Residue (t/ha)": "#8c564b" }
ylabel: "LAI (m²/m²), residue (t/ha)"
height: 240
caption: "The basin's mean leaf area index and litter (basin_pw). The forest starts with leaves (LAI 2) and loses them in the autumn of 2000, as it should; but it never leafs out again, and the litter decays to nothing."
```

In this project the forest drops its leaves in the autumn of 2000 and never grows them back. Without leaves
there is no transpiration, without litter the soil is bare, and the water that the trees should return to the
air runs off and percolates instead. Since landschaft's port reproduces SWAT+ 62 exactly on this project, this
is what SWAT+ itself computes: the cause lies in the project's plant settings, not in the program. It is an
example made to test software, not a calibrated model of Hubbard Brook, and this is exactly the kind of error
a calibration against measured streamflow would expose. Always look at `lai`, `bioms` and `et` (in `hru_pw`,
`basin_pw` and `basin_wb`) before you look at anything else.

**Streamflow.** The daily discharge of the channel (`flo_out` of `channel_sd`, m³/s) against the daily
precipitation and snowmelt:

![Daily precipitation, snowmelt and discharge](/img/tutorials/terrain-hydrology/swat-plus/hydrograph.png "Top: daily precipitation (dark blue) and snowmelt (light blue), mm, drawn downwards. Bottom: the daily discharge leaving the channel, m³/s. The peaks follow rain and the spring snowmelt; the largest, 0.09 m³/s, on 13 April 2002, is snowmelt with rain. In the summer of 2000 the stream dries for weeks; 230 of the 1,096 days have no flow.")

The mean discharge is 0.0047 m³/s (4.7 litres a second), highest in April (snowmelt) and lowest in January
and February, when precipitation is stored as snow, and in August. For a study of stream organisms, the daily series is the raw
material for the variables of the flow regime: the number of days without flow, the size and timing of floods,
the duration of low flows.

**The HRUs.** The average annual balance and soil loss of each land unit (`hru_wb`, `hru_ls`, step `aa`):

```tftable
data: /data/tutorials/terrain-hydrology/swat-plus/hru-aa.csv
digits: 2
caption: "The ten HRUs on average over 2001–2002: their land use, area and slope (from the project's files), their water balance (mm/yr) and their soil loss (t/ha/yr) with SWAT+ 62's cover factor and with the plants' minimum C. Two mature-forest HRUs cover 86 % of the watershed."
```

Water first: the barren units evaporate most (493 mm, from a bare, wet soil), the leafless forests least. Then
soil loss, with a warning: with SWAT+ 62's default cover factor, the mature forest on 47 % slopes loses 274 t/ha
a year, and the basin 179 t/ha. These numbers are as wrong as the plants are: they are the soil loss of bare,
steep ground.

!!! paper "In the literature"
    Report the SWAT+ version (here SWAT+ 62.0.1 through landschaft), how the project was built (QSWAT+ and
    editor versions, DEM, land use and soil maps, HRU thresholds), the weather data, the simulation period and
    the spin-up years, the calibration (parameters, method, period) and the validation statistics (NSE, percent
    bias...) on a period not used for calibration. Give water balances in mm, discharge in m³/s, soil loss in
    t/ha per year, and say which cover factor method was used.

## Settings that change the answer

**The cover factor.** SWAT+ 62 computes the \( C \) of MUSLE from the cover of the litter times the cover of the
growing plants, a method taken from the APEX model in which a tall canopy protects the soil little (raindrops
falling from the canopy hit the ground hard); a forest is protected mainly by its litter. Earlier versions of
SWAT+ (and SWAT) used, for each plant, the minimum \( C \) of its database (0.001 for a forest), raised as the
cover thinned. **c_factor** = `plant` chooses that earlier method. Only sediment and what travels with it
change; water, plants and nutrients in the soil stay the same.

On this project the difference is extreme: the basin's soil loss is 179 t/ha a year with `swatplus` and 0.23
t/ha with `plant`, because the forests have lost both leaves and litter and the two methods disagree most on
bare ground under a "forest". On a project whose plants grow, the gap is smaller but can still be large for
forests. Choose the method on purpose, say which you used, and calibrate sediment against measurements if you
report it.

**The spin-up.** The model starts from initial soil water, aquifer storage and plant biomass that are guesses.
**nyskip** simulates years before printing so that these stores settle. Here, with **nyskip** = 0:

```tftable
data: /data/tutorials/terrain-hydrology/swat-plus/spin-up.csv
columns: [year, precip, et, surq_gen, perc, wateryld, aquifer_flo]
labels: { year: "Year", precip: "Precipitation", et: "Evapotranspiration", surq_gen: "Surface runoff", perc: "Percolation", wateryld: "Water yield", aquifer_flo: "Baseflow (mm)", aquifer_stor: "Aquifer storage (mm)" }
digits: 1
caption: "The basin by year with nyskip = 0 (basin_wb, and aquifer 1 of the aquifer table). The first year is different from the others in its evapotranspiration and its baseflow, which rises from 290 to 490 and 548 mm as the aquifer fills."
```

The year 2000 behaves differently: the aquifer is still filling (its baseflow 290 mm against 490 and 548 later)
and, here, the forest still has leaves. A spin-up of one year is the minimum; for slow stores (deep aquifers,
soil carbon) use several. Never report the spin-up years.

**The step** changes only how the results are summarised, not the simulation: the yearly table is the sum of the
daily one. Print daily only what you need daily (streamflow for a hydrograph), and averages for the rest.

## Common pitfalls

- **Believing an uncalibrated model.** The example above shows how far default parameters can be from a real
  forest. Check the water balance (is evapotranspiration plausible for the climate and vegetation?), the plants
  (`lai`, `bioms`) and the streamflow against measurements before anything else (Moriasi et al., 2007).
- **Old project files.** A `parameters.bsn` written for an earlier SWAT+ makes peak rates and erosion hundreds
  of times too small; [[swat.run]] warns. Update the project with the SWAT+ Editor.
- **Unsupported features.** A project with reservoirs, wetlands, pesticides or Green-Ampt infiltration is
  refused with an error. Use SWAT+ itself for those.
- **Connecting a table that was not printed.** With **tables** set, the other outputs have no value and a node
  using them fails. Add the table to the list.
- **Units.** Water in mm over the HRU or the basin; channel flows in m³/s; soil loss in t/ha; aquifer depth in
  m. Multiply mm by the area to get volumes.
- **The day column.** It is the day of the year at the end of the period, not a date: day 59 of 2001 is the
  end of February. Build dates from `year` and `day` before plotting.
- **Paths.** The **path** is relative to the model's folder. Moving the model without the project breaks it.
- **Expecting Terraflux to build the project.** It does not (yet): use QSWAT+ and the SWAT+ Editor, or
  landschaft in Python, and give [[swat.run]] the TxtInOut folder they write.

## Going further

- [Part 1](../depressions-and-flow/), [part 2](../streams-watersheds-and-hand/) and
  [part 3](../wetness-and-connectivity/) of *Terrain hydrology*: the terrain analysis behind the division of a
  watershed.
- [Ecosystem services](../../ecosystem-services/): InVEST's annual water yield and sediment delivery models,
  simpler, yearly alternatives to SWAT+ for mapping services.
- [Uncertainty](../../uncertainty/): run a model many times with varied inputs.
- The operator's reference: [[swat.run]].

## References

- Arnold, J. G., Srinivasan, R., Muttiah, R. S., & Williams, J. R. (1998). Large area hydrologic modeling and assessment part I: Model development. *Journal of the American Water Resources Association*, 34(1), 73–89. https://doi.org/10.1111/j.1752-1688.1998.tb05961.x
- Bieger, K., Arnold, J. G., Rathjens, H., White, M. J., Bosch, D. D., Allen, P. M., Volk, M., & Srinivasan, R. (2017). Introduction to SWAT+, a completely restructured version of the Soil and Water Assessment Tool. *Journal of the American Water Resources Association*, 53(1), 115–130. https://doi.org/10.1111/1752-1688.12482
- Gassman, P. W., Reyes, M. R., Green, C. H., & Arnold, J. G. (2007). The Soil and Water Assessment Tool: Historical development, applications, and future research directions. *Transactions of the ASABE*, 50(4), 1211–1250. https://doi.org/10.13031/2013.23637
- Likens, G. E., Bormann, F. H., Johnson, N. M., Fisher, D. W., & Pierce, R. S. (1970). Effects of forest cutting and herbicide treatment on nutrient budgets in the Hubbard Brook watershed-ecosystem. *Ecological Monographs*, 40(1), 23–47. https://doi.org/10.2307/1942440
- Moriasi, D. N., Arnold, J. G., Van Liew, M. W., Bingner, R. L., Harmel, R. D., & Veith, T. L. (2007). Model evaluation guidelines for systematic quantification of accuracy in watershed simulations. *Transactions of the ASABE*, 50(3), 885–900. https://doi.org/10.13031/2013.23153
- Nash, J. E., & Sutcliffe, J. V. (1970). River flow forecasting through conceptual models part I — A discussion of principles. *Journal of Hydrology*, 10(3), 282–290. https://doi.org/10.1016/0022-1694(70)90255-6
- Williams, J. R. (1975). Sediment-yield prediction with universal equation using runoff energy factor. In *Present and Prospective Technology for Predicting Sediment Yields and Sources* (ARS-S-40, pp. 244–252). USDA Agricultural Research Service.
