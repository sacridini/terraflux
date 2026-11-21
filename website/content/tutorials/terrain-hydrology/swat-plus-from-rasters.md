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
(Bieger et al., 2017). Terraflux does both through terraflux-py, which ports their methods one for one:
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

### Divide the watershed in Python

The template's data are made in Python with the same generators and seeds: [`tf.nlm.fbm`][terraflux.nlm.fbm] for the
relief (`dimension` of the node is `fract_dim`), [`tf.nlm.random_cluster`][terraflux.nlm.random_cluster] for the land use,
on a grid of 30 by 30 cells of 100 m in the made-up transverse Mercator. The soil table and the weather of the next
step are the part's downloads, `soil_table.csv` and `weather.csv`:

```python
import numpy as np
import pandas as pd
import xarray as xr
import terraflux as tf

crs = "+proj=tmerc +lat_0=-15 +lon_0=-47 +k=1 +x_0=0 +y_0=0 +datum=WGS84 +units=m"
relief = tf.nlm.fbm(30, 30, resolution=100, fract_dim=1.4, seed=91).rio.write_crs(crs)
dem = 700 + 300 * relief                                         # 700 to 1,000 m
landuse = tf.nlm.random_cluster(30, 30, p=0.5, ai=[0.6, 0.4], seed=92, rescale=False, resolution=100).rio.write_crs(crs)
soils = xr.where(relief > 0.4, 10, 20)                           # 10 the uplands, 20 the lower valley
landuse_lookup = pd.DataFrame({"code": [1, 2], "name": ["FRSD", "PAST"]})
soil_lookup = pd.DataFrame({"code": [10, 20], "name": ["UPLAND_LOAM", "VALLEY_CLAY_LOAM"]})
soil_table = pd.read_csv("soil_table.csv")                       # a row per layer, SWAT+'s soils.sol columns

w = tf.hydro.swat.watershed(dem, landuse, soils, landuse_lookup=landuse_lookup, soil_lookup=soil_lookup,
                            soil_table=soil_table, stream_threshold=0.3)       # km2, as the node
print(w)
print(w.tables["gis_hrus"][["landuse", "soil", "slope", "arsub"]].round(1).to_string())
```

```text
SwatWatershed(9 subbasins, 9 HRUs, 714.0 ha)
  landuse              soil  slope  arsub
0    PAST  VALLEY_CLAY_LOAM   19.2  243.0
1    FRSD  VALLEY_CLAY_LOAM   23.3   35.0
2    PAST       UPLAND_LOAM   24.4   81.0
3    FRSD  VALLEY_CLAY_LOAM   22.9   15.0
4    FRSD  VALLEY_CLAY_LOAM   21.8   49.0
5    FRSD       UPLAND_LOAM   26.7   39.0
6    FRSD       UPLAND_LOAM   19.9   77.0
7    FRSD  VALLEY_CLAY_LOAM   27.4  124.0
8    PAST  VALLEY_CLAY_LOAM   24.6   51.0
```

[`tf.hydro.swat.watershed`][terraflux.hydro.swat.watershed] is the node: nine subbasins, nine channels and an HRU each
(714 ha of the 900), as the part says. The result, a [`SwatWatershed`][terraflux.hydro.swat.SwatWatershed], has the
tables of QSWAT+ (`w.tables`: `gis_subbasins`, `gis_channels`, `gis_hrus`, `gis_routing`...) and the maps of the division
on the DEM's grid (`w.subbasins`, `w.channels`, `w.hrus`). The land use and soil maps may be on any grid (the nearest
cell is taken); a DEM without a coordinate system is refused, and so is a code the lookups do not list. The other
settings are the node's, and the two tables of the part come from them:

```python
def division(**settings):
    return tf.hydro.swat.watershed(dem, landuse, soils, landuse_lookup=landuse_lookup, soil_lookup=soil_lookup,
                                   soil_table=soil_table, **settings)

for threshold in (0.15, 0.3, 0.6, 1.2):
    d = division(stream_threshold=threshold)
    print(threshold, len(d.tables["gis_subbasins"]), len(d.tables["gis_hrus"]))
for name, settings in (("dominant_hru", {}), ("dominant", {"hru_method": "dominant"}),
                       ("filter", {"hru_method": "filter", "thresholds": (10, 10, 10)}),
                       ("filter with slope bands", {"hru_method": "filter", "thresholds": (10, 10, 10), "slope_bands": (15, 35)})):
    print(name, len(division(stream_threshold=0.3, **settings).tables["gis_hrus"]))
```

```text
0.15 28 28
0.3 9 9
0.6 3 3
1.2 2 2
dominant_hru 9
dominant 9
filter 32
filter with slope bands 73
```

The first four lines are the threshold (km²), the subbasins and the HRUs of the first table, the last four the HRUs by
method of the second.

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

### Write the project in Python

The weather of [`tf.hydro.swat.climate`][terraflux.hydro.swat.climate] is a daily `DataFrame` for each of precipitation,
maximum and minimum temperature, with the dates as the index and a column per station, and a table of the stations
(index `name`, with `lat`, `lon` and `elev`). The `year` and `day` columns of the file make the dates:

```python
weather = pd.read_csv("weather.csv")                             # year, day, pcp, tmax, tmin
dates = pd.to_datetime(weather["year"].astype(str)) + pd.to_timedelta(weather["day"] - 1, unit="D")
series = {c: pd.DataFrame({"valley": weather[c].to_numpy()}, index=dates) for c in ("pcp", "tmax", "tmin")}
stations = pd.DataFrame({"lat": [-14.98], "lon": [-46.98], "elev": [850]}, index=pd.Index(["valley"], name="name"))
monthly = dict(pcp_hhr=8, slr_ave=[4, 7, 11, 16, 20, 23, 25, 21, 15, 9, 5, 3], dew_ave=4, wnd_ave=2.5)
wx = tf.hydro.swat.climate(stations, pcp=series["pcp"], tmax=series["tmax"], tmin=series["tmin"], monthly=monthly)
print(wx, wx.period())
print(wx.wgn.loc["valley", ["tmp_max_ave", "pcp_ave", "wet_dry", "wet_wet"]].round(2).head(3))

project = tf.hydro.swat.from_tables(w, wx)
project.watershed = w            # what hru_map and channel_map need
print(project)
folder = project.write("TxtInOut")                                # a folder SWAT+ 62 itself can run
print(len(list(folder.iterdir())))
```

```text
SwatClimate(1 stations, pcp, tmax, tmin) (Timestamp('2001-01-01 00:00:00'), Timestamp('2002-12-31 00:00:00'))
       tmp_max_ave  pcp_ave  wet_dry  wet_wet
month                                        
1            30.03    225.9     0.74     0.64
2            29.60    160.6     0.65     0.67
3            28.52    107.0     0.57     0.47
SwatProject(62 files)
62
```

`climate` writes the weather files and the statistics of the weather generator (`wx.wgn`, by station and month: the node
`wgn`; [`tf.hydro.swat.wgn_from_series`][terraflux.hydro.swat.wgn_from_series] computes them alone). What daily
precipitation and temperature cannot give (the largest half-hour rain `pcp_hhr`, the means of sun, dew point and wind)
comes in `monthly`, as in the node; radiation, humidity and wind series can be given too (`slr`, `hmd`, `wnd`) and a
ready `wgn`. [`tf.hydro.swat.from_tables`][terraflux.hydro.swat.from_tables] writes the project, file for file as the
SWAT+ Editor does (62 files here); `start` and `end` change the period (the days all the series cover), and `codes`
the Editor's defaults. The project made by `from_tables` has no watershed until you set it, as the line above does, which
is what lets the maps put results on the cells.
[`tf.hydro.swat.from_rasters`][terraflux.hydro.swat.from_rasters] does `watershed` and `from_tables` in one call, and
[`tf.hydro.swat.load_qswat_project`][terraflux.hydro.swat.load_qswat_project] reads a QSWAT+ database.

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

### Change a parameter in Python

The library has no function for the node `swat.set`: a project is a set of text files, and SWAT+'s hard calibration is
one more, `calibration.cal`, which SWAT+ applies before it starts and which `file.cio` names on its `chg` line. The
change is a few lines (the application's script does the same):

```python
def set_parameter(project, parameter, change, value, layers=(0, 0)):
    """A copy of the project with a row more in calibration.cal; change: pctchg, abschg or absval."""
    copy = project.copy()
    head = "cal_parm   chg_typ   chg_val   conds   soil_lyr1   soil_lyr2   yr1   yr2   day1   day2   obj_tot"
    rows = [r for r in copy.text("calibration.cal").splitlines()[3:] if r.strip()] if "calibration.cal" in copy else []
    rows.append(f"{parameter}   {change}   {value}   0   {layers[0]}   {layers[1]}   0   0   0   0   0")
    copy.set_text("calibration.cal", "\n".join(["calibration.cal: by hand", str(len(rows)), head, *rows, ""]))
    lines = copy.text("file.cio").split("\n")
    k = next(i for i, line in enumerate(lines) if line.split()[:1] == ["chg"])
    words = lines[k].split()
    words[2] = "calibration.cal"                                  # the second file of the chg line
    lines[k] = " ".join(words)
    copy.set_text("file.cio", "\n".join(lines))
    return copy

curve_number = set_parameter(project, "cn2", "pctchg", -10)       # the curve number 10% lower in every HRU
print(curve_number.text("calibration.cal"))
```

```text
calibration.cal: by hand
1
cal_parm   chg_typ   chg_val   conds   soil_lyr1   soil_lyr2   yr1   yr2   day1   day2   obj_tot
cn2   pctchg   -10   0   0   0   0   0   0   0   0
```

The parameter is one of cal_parms.cal's (`cn2` and `awc` are in the project; the application adds the 69 the port
calibrates), `change` is a percentage, an amount or a value, `layers` are the first and last soil layer (0 and 0: all)
and the HRUs are all when the last column is 0 (`obj_tot`; give a count and their numbers after it for some). Each call
returns a copy and the rows add up in order. Because the percentage is an ordinary argument, anything that varies
inputs varies it: a loop, a sensitivity analysis, a calibration against observed flow.

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

### Run it, and put the results on the map in Python

The model runs the project twice, as the part says: the HRUs' average year and the channels' days. [`tf.hydro.swat.run`][terraflux.hydro.swat.run]
takes the `tables` and the `nyskip` (the project's own is 1: the first year is the spin-up), and the maps take the result
they read with `result=` (without it, each runs the project again):

```python
yearly = tf.hydro.swat.run(project, tables={"hru_wb": "aa", "hru_ls": "aa"})
daily = tf.hydro.swat.run(project, tables={"channel_sd": "day"})
sediment = tf.hydro.swat.hru_map(project, "sedyld", result=yearly)      # t/ha a year
runoff = tf.hydro.swat.hru_map(project, "surq_gen", result=yearly)      # mm a year
flow = tf.hydro.swat.channel_map(project, result=daily)                 # mean m3/s, on the stream cells
q = tf.hydro.swat.reach_discharge(project, result=daily)                # a column per channel
print(sediment.shape, round(float(sediment.mean()), 2), round(float(runoff.mean()), 2), round(float(flow.mean()), 4))
print(q.shape, q.index[0].date(), q.index[-1].date(), int(q.mean().idxmax()), round(float(q.max().max()), 3))
```

```text
(30, 30) 123.25 192.9 0.0146
(365, 9) 2002-01-01 2002-12-31 4 1.34
```

[`tf.hydro.swat.hru_map`][terraflux.hydro.swat.hru_map] paints each cell with the value of its HRU (`how="annual"`, the
column `variable` of `hru_ls`, `hru_wb`...) and gives NaN outside the watershed and on the cells whose combination was
folded into another HRU; [`tf.hydro.swat.channel_map`][terraflux.hydro.swat.channel_map] does the same for the channels
(`column`, `how`). [`tf.hydro.swat.reach_discharge`][terraflux.hydro.swat.reach_discharge] is the hydrograph's table: here
the 365 days of the second year, since the first is skipped (`nyskip=0` prints both). The channel that leaves the valley is
the one with the largest mean, number 4. `tf.hydro.swat.run(..., c_factor="plant")` and the same setting of the maps choose
MUSLE's cover factor, as in [Running a SWAT+ project](../swat-plus/#in-python).

The parameters of the part's last table, the mean over the cells of the soil lost and the runoff, the mean discharge of
the channels and the largest daily discharge of any channel, with the project changed by `set_parameter`:

```python
for parameter, value in (("cn2", -10), ("cn2", 0), ("cn2", 10), ("awc", -20), ("awc", 0), ("awc", 20)):
    changed = set_parameter(project, parameter, "pctchg", value)
    r_year = tf.hydro.swat.run(changed, tables={"hru_wb": "aa", "hru_ls": "aa"})
    r_day = tf.hydro.swat.run(changed, tables={"channel_sd": "day"})
    print(f"{parameter} {value:+4d}%:",
          round(float(tf.hydro.swat.hru_map(changed, "sedyld", result=r_year).mean()), 3),
          round(float(tf.hydro.swat.hru_map(changed, "surq_gen", result=r_year).mean()), 3),
          round(float(tf.hydro.swat.channel_map(changed, result=r_day).mean()), 4),
          round(float(tf.hydro.swat.reach_discharge(changed, result=r_day).max().max()), 3))
```

```text
cn2  -10%: 116.921 192.16 0.0145 1.345
cn2   +0%: 123.254 192.897 0.0146 1.34
cn2  +10%: 133.345 197.518 0.015 1.328
awc  -20%: 131.722 188.564 0.0147 1.318
awc   +0%: 123.254 192.897 0.0146 1.34
awc  +20%: 115.974 198.394 0.0144 1.347
```

These are the part's numbers (soil lost, runoff, mean discharge and largest daily discharge).

### The example landscape and a land use scenario in Python

For a bigger valley than the template's, [`tf.hydro.swat.example_landscape`][terraflux.hydro.swat.example_landscape] gathers
the inputs of a small model from the example data: the Oregon DEM projected to 30 m cells, a land use from the NDVI (forest
above 0.55, pasture below), two soils by elevation, a station with four years of made-up daily weather and the lookups.
The land use names are rows of SWAT+'s plant database (`FRSD`, deciduous forest; `PAST`, pasture), and the soil table has
the columns of `soils.sol` (an ArcSWAT or QSWAT+ *usersoil* table works too). A land use scenario is another land use map
through the same steps, and `run_many` simulates several projects at once, one per thread:

```python
ex = tf.hydro.swat.example_landscape()
dem, landuse, soils = ex["dem"], ex["landuse"], ex["soils"]
print(dem.shape, dem.rio.crs, dem.rio.resolution(), landuse.to_series().value_counts(normalize=True).round(2).to_dict())
settings = dict(landuse_lookup=ex["landuse_lookup"], soil_lookup=ex["soil_lookup"], soil_table=ex["soil_table"],
                stream_threshold=1.0, slope_bands=(15, 35), hru_method="filter", thresholds=(10, 10, 10))
oregon = tf.hydro.swat.watershed(dem, landuse, soils, **settings)
climate = tf.hydro.swat.climate(ex["stations"], pcp=ex["pcp"], tmax=ex["tmax"], tmin=ex["tmin"], monthly=ex["monthly"])
today = tf.hydro.swat.from_tables(oregon, climate)
print(oregon)

cleared = landuse.where(~((landuse == 1) & (soils == 20)), 2)            # the valley forests become pasture
scenario = tf.hydro.swat.from_rasters(dem, cleared, soils, climate=climate, **settings)
now, then = tf.hydro.swat.run_many([today, scenario], tables={"basin_wb": "aa", "basin_ls": "aa"}, c_factor="plant")
for name, r in (("today", now), ("valleys cleared", then)):
    wb, sed = r.table("basin_wb", "aa").iloc[0], r.table("basin_ls", "aa").iloc[0]
    print(name, round(float(wb["surq_gen"]), 1), round(float(wb["wateryld"]), 1), round(float(wb["et"]), 1), round(float(sed["sedyld"]), 3))
```

```text
(371, 273) EPSG:32610 (30.0, -30.0) {1.0: 0.96, 2.0: 0.04}
SwatWatershed(33 subbasins, 128 HRUs, 8075.9 ha)
today 241.7 263.6 503.6 0.623
valleys cleared 227.4 249.3 520.3 3.712
```

The valley soil conducts water three to four times slower than the upland one, layer for layer, which is what makes it
shed 430 mm a year over the surface where the upland loam percolates nearly everything. Pasture on half of the watershed
changes the water little, a little less runoff and more evapotranspiration, but the basin's soil loss grows six times, from
0.62 to 3.71 t/ha a year (with the plants' cover factor): maps like `hru_map(..., "sedyld")` of such scenarios are the
costs and benefits a restoration plan weighs (see [Restoration priorities](/tutorials/distance-restoration/restoration-plan/#in-python)).
The river of this project runs from January to June and dries in the summer: its baseflow comes from the shallow aquifers,
which the editor sets up to release water only while their water table is above 3 m (`flo_min` in `aquifer.aqu`, edited
as any file with `project.text` and `project.set_text` before running).

A model made with QSWAT+ and the SWAT+ Editor runs as it is: [`tf.hydro.swat.load_txtinout`][terraflux.hydro.swat.load_txtinout]
reads its TxtInOut folder or a zip, as in [Running a SWAT+ project](../swat-plus/#in-python).

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
    62.0.1 through terraflux-py), the parameters changed and the calibration. Report the number of subbasins and HRUs.

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
- In Python, the same model is written by `terraflux python` as a script of terraflux-py, and the `### In Python` sections
  above run it step by step.
- The operators' reference: [[swat.watershed]], [[swat.from_tables]], [[swat.wgn]], [[swat.set]], [[swat.run]],
  [[swat.hru_map]], [[swat.channel_map]], [[swat.reach_discharge]].

## References

- Arnold, J. G., Srinivasan, R., Muttiah, R. S., & Williams, J. R. (1998). Large area hydrologic modeling and assessment part I: Model development. *Journal of the American Water Resources Association*, 34(1), 73–89. https://doi.org/10.1111/j.1752-1688.1998.tb05961.x
- Bieger, K., Arnold, J. G., Rathjens, H., White, M. J., Bosch, D. D., Allen, P. M., Volk, M., & Srinivasan, R. (2017). Introduction to SWAT+, a completely restructured version of the Soil and Water Assessment Tool. *Journal of the American Water Resources Association*, 53(1), 115–130. https://doi.org/10.1111/1752-1688.12482
- Dile, Y. T., Daggupati, P., George, C., Srinivasan, R., & Arnold, J. (2016). Introducing a new open source GIS user interface for the SWAT model. *Environmental Modelling & Software*, 85, 129–138. https://doi.org/10.1016/j.envsoft.2016.08.004
- Moriasi, D. N., Arnold, J. G., Van Liew, M. W., Bingner, R. L., Harmel, R. D., & Veith, T. L. (2007). Model evaluation guidelines for systematic quantification of accuracy in watershed simulations. *Transactions of the ASABE*, 50(3), 885–900. https://doi.org/10.13031/2013.23153
