---
title: Nutrient delivery ratio
short: 6. Nutrients
description: InVEST's nutrient delivery ratio model in Terraflux - the nitrogen and phosphorus loads of the land uses, the retention of the vegetation they cross on the way down, the index of connectivity, the export to the streams by surface and subsurface flow, the settings the answer depends on, and what restoring riparian forest does in a land use scenario.
order: 60
level: advanced
time: 60 min
ops: [services.ndr, table.read, vector.read, raster.read]
series: Ecosystem services with InVEST
part: 6
---

Fertilizer on soybean fields, dung on pasture and sewage from towns put nitrogen and phosphorus into the water,
where they feed algae, use up the oxygen fish need and make drinking water harder and more expensive to treat.
Part of these nutrients never reaches a river: on the way down, the soil, the roots and the leaf litter of forest,
grassland and wetlands take them up or hold them. That retention is an ecosystem service, and strips of forest
along the rivers are the classic way to restore it. InVEST's **nutrient delivery ratio** (NDR) model maps both
sides: how much nitrogen and phosphorus each land use releases, and how much of it reaches a stream. In this
tutorial you will build it in Terraflux on Ribeirão Claro, read its maps of retention, delivery and export, learn
which settings move the answer, and see why the riparian restoration of the 2040 scenario cuts the surface export
of nitrogen while the subsurface export still rises.

## The idea

The model follows a nutrient from the cell where it is released to the first stream below it, in three steps.

**How much each cell releases.** Each land use class gets a **load**: kilograms of nitrogen and of phosphorus per
hectare and year. Usually these are *export coefficients*, the loads measured at the edge of fields, pastures or
forests in studies of small catchments. Wetter places wash out more, so the load of each cell is scaled by a
**runoff proxy**: annual rain, or a quickflow index, divided by its mean over the landscape. For nitrogen, part of
the load dissolves and travels down through the soil instead of over it.

!!! science "Key idea: the load of a cell"
    \[ \text{load}_{x} = \text{load}_j \cdot \text{RPI}_x, \qquad \text{RPI}_x = \frac{RP_x}{\overline{RP}} \]

    \[ \text{load}_{\text{surf},x} = (1 - p_{\text{sub},j})\,\text{load}_x, \qquad
       \text{load}_{\text{subs},x} = p_{\text{sub},j}\,\text{load}_x \]

    where \(\text{load}_j\) is the load of the class \(j\) of cell \(x\) (kg / ha / yr), \(RP_x\) the runoff
    proxy of the cell, \(\overline{RP}\) its mean over the cells of the run (so its units do not matter), and
    \(p_{\text{sub},j}\) the share of the class's nitrogen that leaves by subsurface flow (`proportion_subsurface_n`;
    phosphorus has none, all of it moves over the surface). If the table's loads are **application rates** (the
    fertilizer or manure put on the land) rather than measured runoff, the model first multiplies them by
    \(1 - \text{eff}_j\), the class's own retention.

**How much the land below retains.** Every class also has a **retention efficiency** \(\text{eff}_j\) (0 to 1),
the largest share of the nutrients passing through it that it can retain, and a **critical length**
\(\ell_j\) (m), the distance over which it reaches that share. Natural vegetation retains much (InVEST's guide
gives above 0.8 as typical), a road or a town almost nothing. The load of a cell crosses the cells below it on its
way to the stream, and the model accumulates their retention into an **effective retention** \(\text{eff}'\), from
the stream up:

!!! science "Key idea: retention along the flow path"
    \[ \text{eff}'_i = \begin{cases}
         \text{eff}_i\,(1 - s_i) & \text{if the next cell down is a stream} \\
         \text{eff}'_{\text{down}}\,s_i + \text{eff}_i\,(1 - s_i) & \text{if } \text{eff}_i > \text{eff}'_{\text{down}} \\
         \text{eff}'_{\text{down}} & \text{otherwise}
       \end{cases} \]

    \[ s_i = \exp\left(-\frac{5\,\ell_{i}}{\ell_{j}}\right) \]

    where \(\text{eff}'_{\text{down}}\) is the effective retention of the next cell down, \(\ell_i\) the length of
    the step to it (the cell size, or the cell size times \(\sqrt 2\) on a diagonal) and \(\ell_j\) the critical
    length of the class of cell \(i\). After a path as long as the critical length, \(s = e^{-5} = 0.007\): the
    class has reached 99% of its efficiency. With multiple flow directions a cell flows to several neighbours, and
    its \(\text{eff}'\) is the mean of the values above over them, weighted by the share of flow each receives.
    Stream cells have a retention of 0.

The third case is the one to notice: a cell that retains less than the land below it takes the retention of that
land. A soybean field above a strip of forest is credited with the strip's retention, because its nutrients have to
cross the strip.

![Effective retention along two flow paths](/img/tutorials/ecosystem-services/nutrients/retention-path.png "The effective retention of nitrogen along a hillslope of ten cells of 30 m, computed with the formulas above and the table of this tutorial. Left: cropland down to the stream retains 0.15 everywhere (its critical length, 30 m, is one cell). Right: three cells of forest next to the stream (efficiency 0.8, critical length 150 m) build up 0.51, 0.69 and 0.76, and the cropland above takes the 0.76 of the strip.")

**How well a cell is connected to the stream.** As in the [sediment model](/tutorials/ecosystem-services/sediment/),
the share that reaches a stream also depends on the terrain: the **index of connectivity** of Borselli et al.
(2008) is high for cells that drain a large, steep area and lie close to the stream, low for cells far from it
behind long gentle slopes. The model turns it into a delivery ratio with the sigmoid that Vigiak et al. (2012) used
for sediment, scaled down by the effective retention:

!!! science "Key idea: the nutrient delivery ratio"
    \[ IC_x = \log_{10}\frac{D_{\text{up}}}{D_{\text{dn}}}, \qquad
       D_{\text{up}} = \bar{S}\,\sqrt{A}, \qquad
       D_{\text{dn}} = \sum_i \frac{d_i}{S_i} \]

    \[ NDR_x = \frac{1 - \text{eff}'_x}{1 + \exp\left(\dfrac{IC_0 - IC_x}{k}\right)}, \qquad
       IC_0 = \frac{IC_{\max} + IC_{\min}}{2} \]

    where \(A\) is the area draining into the cell (m²), \(\bar{S}\) the mean slope of that area (m/m, kept between
    0.005 and 1), and the sum runs over the cells \(i\) of the path down to the stream, \(d_i\) being the length of
    the path through cell \(i\) and \(S_i\) its slope. Unlike the sediment model's index, this one has no cover
    factor: it depends on the terrain and the streams only. \(IC_0\) is not a setting: it is half way between the
    lowest and the highest index of the run. \(k\) (2 by default) sets how steep the curve is.

![The nutrient delivery ratio against the index of connectivity](/img/tutorials/ecosystem-services/nutrients/ndr-curve.png "The delivery ratio against the index of connectivity for four effective retentions, with k = 2 and the IC_0 of Ribeirão Claro (-1.18, half way between its indices of -3.98 and 1.61). The retention sets the ceiling of the curve; the connectivity moves a cell along it.")

**The subsurface path.** The dissolved nitrogen of the subsurface load is retained by the soil according to the
distance alone, the same for every land use:

!!! science "Key idea: subsurface delivery and export"
    \[ NDR_{\text{subs},x} = 1 - \text{eff}_{\text{sub}}\left(1 - e^{-5\,\ell_x / \ell_{\text{sub}}}\right) \]

    with \(\ell_x\) the distance (m) from the cell down to the stream along the flow path, \(\text{eff}_{\text{sub}}\)
    the largest retention of the soil (`subsurface_efficiency`) and \(\ell_{\text{sub}}\) the distance over which
    it is reached (`subsurface_critical_length`). The **export** of a cell, in kg / ha / yr, is

    \[ x_{\text{exp},x} = \text{load}_{\text{surf},x}\,NDR_x + \text{load}_{\text{subs},x}\,NDR_{\text{subs},x} \]

A worked cell: cropland on rain equal to the mean (\(\text{RPI} = 1\)), 300 m from the stream along the flow (about
the median of Ribeirão Claro), with an index of
connectivity of \(-2\) (about the landscape's mean). Its 20 kg of nitrogen split into 12 kg over the surface and 8 kg
through the soil. Without forest below it, \(\text{eff}' = 0.15\) and \(NDR = 0.85 / (1 + e^{(-1.18 + 2)/2}) =
0.34\): **4.1 kg** reach the stream over the surface. Above the forest strip of the figure, \(\text{eff}' = 0.76\),
\(NDR = 0.096\) and only **1.1 kg** do. Through the soil, \(NDR_{\text{subs}} = 1 - 0.8\,(1 - e^{-5 \times
300/200}) = 0.20\): **1.6 kg**, with or without the strip.

## When to use it

- **Where to restore**: the cells whose vegetation keeps the most nutrients out of the rivers, and the riparian
  strips that would retain most if restored.
- **Scenarios**: how much more nitrogen and phosphorus reach a reservoir if pasture becomes cropland, or less if
  the riparian forest the Brazilian Forest Code requires along rivers is restored.
- **Water funds and payments for watershed services** that target the farms upstream of a water intake.
- **Which classes are the sources**, for a plan of fertilizer management or of livestock.

!!! paper "In the literature"
    Redhead et al. (2018) tested the model on 36 catchments across the United Kingdom against the exports derived
    from measured water chemistry. It ranked the catchments well (Spearman's correlations up to 0.81 for nitrogen
    and 0.88 for phosphorus), but its absolute exports were often far off (median differences of ±64% and ±44% at
    best), and they were sensitive to the loads, the routing and the threshold flow accumulation. Valladares-Castellanos
    et al. (2024) calibrated and validated it on twenty watersheds of Puerto Rico with long-term monitoring data: it
    explained 62.7% of the variance of total nitrogen and 79.3% of total phosphorus between 2000 and 2022. Mayer et
    al. (2007), in a meta-analysis of riparian buffers, found that buffers wider than 50 m removed a large share of
    the nitrogen more consistently than buffers of 0 to 25 m, and that subsurface removal was efficient but not
    related to the width; InVEST's guide draws its suggested subsurface parameters from their review.

What it is **not**: the model works with annual averages of diffuse sources. It has **no in-stream processes** (no
uptake or deposition in the channel, no retention in reservoirs), no **point sources** (the sewage of the town is
not in it unless you add it), no seasons or storms, and no lag of nitrogen in the groundwater, which can take years
or decades to reach a river. It gives loads (kg / yr), not concentrations. InVEST's guide warns that the outputs are
highly sensitive to the inputs, above all to the loads, and that calibration is difficult; use the model to compare
places and scenarios, and the absolute numbers with care. Phosphorus travels mostly bound to soil particles, so read
its export with the [sediment model](/tutorials/ecosystem-services/sediment/) beside it. For daily flows and water
quality with in-stream processes, use a process model such as SWAT+ ([[swat.run]]).

## What you need

All rasters on the grid of the land use map, projected in metres:

| Input | File | Unit |
|---|---|---|
| `dem` | `dem.tif` | m, projected |
| `lulc` | `lulc_2020.tif` | class codes |
| `runoff_proxy` | `precipitation.tif` | any: annual rain or a quickflow index |
| `biophysical` | `biophysical_ndr.csv` | by class: loads, efficiencies, lengths |
| `watersheds` | `watersheds.gpkg` | polygons, with `ws_id` |

The model fills the DEM's depressions and routes the flow over it, and runs only inside the watersheds (polygons
without `ws_id` are numbered 1, 2...).

Ribeirão Claro's files are in [ribeirao-claro.zip](/data/tutorials/ecosystem-services/ideas/ribeirao-claro.zip);
the nutrient table, which the other parts do not use, is in
[ribeirao-claro-ndr.zip](/data/tutorials/ecosystem-services/nutrients/ribeirao-claro-ndr.zip). The runoff proxy
here is the annual rain, as InVEST's guide allows when no quickflow index is at hand; [[services.seasonal_water_yield]]
computes one.

```tfgallery
columns: 2
caption: "The land use of 2020 and the runoff proxy, the annual rain (1,184 to 1,849 mm inside the watersheds, higher on the ridges). Its mean over the watersheds, 1,491 mm (the output runoff_proxy_mean), is what each cell's rain is divided by: a cell on the wettest ridge releases 24% more than its class's load, one in the driest valley 21% less."
images:
  - { src: /img/tutorials/ecosystem-services/nutrients/lulc_2020.png, caption: "lulc, 2020" }
  - { src: /img/tutorials/ecosystem-services/nutrients/runoff_proxy.png, caption: "runoff proxy, mm / yr" }
```

**The biophysical table**, by class (`lucode`): for nitrogen `load_n` (kg / ha / yr), `eff_n` (0 to 1),
`crit_len_n` (m) and `proportion_subsurface_n` (0 to 1); for phosphorus `load_p`, `eff_p` and `crit_len_p`. The
values here are **illustrative**, chosen to be plausible for the land uses of the Cerrado and the Atlantic forest
(fertilized soybean and maize, extensive cattle pasture, native forest, eucalyptus); use measured export
coefficients and retention efficiencies for your region:

| lucode | class | load_n | eff_n | crit_len_n | proportion_subsurface_n |
|---|---|---|---|---|---|
| 1 | Forest | 2.5 | 0.8 | 150 | 0.3 |
| 2 | Pasture | 8 | 0.4 | 60 | 0.3 |
| 3 | Cropland | 20 | 0.15 | 30 | 0.4 |
| 4 | Eucalyptus | 4 | 0.6 | 100 | 0.3 |
| 5 | Urban | 12 | 0.05 | 30 | 0.1 |
| 6 | Water | 0 | 0 | 30 | 0 |

and, in the same table, phosphorus:

| lucode | class | load_p | eff_p | crit_len_p |
|---|---|---|---|---|
| 1 | Forest | 0.1 | 0.8 | 150 |
| 2 | Pasture | 0.5 | 0.45 | 60 |
| 3 | Cropland | 1.5 | 0.2 | 30 |
| 4 | Eucalyptus | 0.25 | 0.6 | 100 |
| 5 | Urban | 1.8 | 0.05 | 30 |
| 6 | Water | 0 | 0 | 30 |

The loads are measured runoff (export coefficients), Terraflux's default. The critical lengths follow InVEST's
guide, which reports 10 to 300 m from the literature on riparian buffers and suggests 150 m as a start (and the
size of a cell where nothing is known); here forest has 150 m, and cropland and the town the size of a cell. In
InVEST the load types are columns of the table (`load_type_n`, `load_type_p`); in Terraflux they are settings of
the node (below). For the subsurface path the tutorial uses the
global averages the guide suggests after Mayer et al. (2007): a critical length of 200 m and an efficiency of 0.8.

## Build the model

<div class="steps" markdown>

1. Add [[raster.read]] nodes for `dem.tif`, `lulc_2020.tif` and `precipitation.tif`, a [[table.read]] for
   `biophysical_ndr.csv` and a [[vector.read]] for `watersheds.gpkg`. Rename each after its content.
2. Add [[services.ndr]] (subject *Ecosystem services*) and connect each to the input of its name; the rain goes to
   `runoff_proxy`.
3. In the Node pane, set `threshold`, the number of cells that must drain into a cell for it to be a **stream**:
   1,000 cells (90 ha), as in the sediment tutorial. Leave `k` at 2.
4. Set `subsurface_critical_length` to 200 and `subsurface_efficiency` to 0.8. They have no defaults: with
   `nitrogen` on (the default), the node asks for them. To model phosphorus only, turn `nitrogen` off and they
   are not needed.
5. Run (++ctrl+enter++).

</div>

```tfgraph
#! caption: The nutrient delivery ratio model of 2020.
#! highlight: nutrients
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "dem"
op = "raster.read"
params = { path = "dem.tif" }

[[nodes]]
id = "lulc"
op = "raster.read"
params = { path = "lulc_2020.tif" }

[[nodes]]
id = "rain"
op = "raster.read"
params = { path = "precipitation.tif" }
note = "The runoff proxy."

[[nodes]]
id = "biophysical"
op = "table.read"
params = { path = "biophysical_ndr.csv" }
note = "Loads, efficiencies, critical lengths."

[[nodes]]
id = "watersheds"
op = "vector.read"
params = { path = "watersheds.gpkg" }

[[nodes]]
id = "nutrients"
op = "services.ndr"
inputs = { dem = "dem.raster", lulc = "lulc.raster", runoff_proxy = "rain.raster", biophysical = "biophysical.table", watersheds = "watersheds.vector" }
params = { threshold = 1000, k = 2, subsurface_critical_length = 200, subsurface_efficiency = 0.8, method = "mfd" }
```

Behind the node, the chain is InVEST's: the DEM's depressions filled, the flow routed by **multiple flow
directions** (`mfd`, the default: each cell's water split among its lower neighbours, as pygeoprocessing does) or
by `d8` (all of it to the steepest neighbour), the flow accumulated, the streams traced, the slope, the index of
connectivity, the effective retention, the delivery ratios and the exports. Its outputs:

| Output | What it is | Unit |
|---|---|---|
| `n_surface_export`, `p_surface_export` | surface load × NDR | kg / ha / yr |
| `n_subsurface_export` | subsurface load × NDR<sub>subs</sub> | kg / ha / yr |
| `n_total_export` | surface plus subsurface nitrogen export | kg / ha / yr |
| `ndr_n`, `ndr_p` | the surface delivery ratios | 0 to 1 |
| `effective_retention_n`, `effective_retention_p` | \(\text{eff}'\) | 0 to 1 |
| `ic`, `stream`, `drains_to_stream` | index of connectivity; the streams (1); 1 where the flow reaches one | |
| `watershed_table` | by watershed: `n_surface_load`, `n_surface_export`, `n_subsurface_export`, `n_total_export`, `n_subsurface_load`, `p_surface_load`, `p_surface_export` | kg / yr |
| `ic_0`, `runoff_proxy_mean` | the numbers the run used | |

Terraflux's node is InVEST 3.20's model, run through landschaft's port of it. landschaft's tests run
InVEST 3.20.2 on synthetic landscapes (with multiple flow directions and D8, nitrogen and phosphorus) and require
every raster of the chain to hold the same 32-bit numbers (on Windows), and the watershed totals to agree to
float32 precision;
Terraflux's tests require its maps to be landschaft's to the bit. One difference is deliberate: InVEST 3.20
measures the distance to the stream of the subsurface delivery in **cells** but compares it with a critical length
in metres; Terraflux, like landschaft, measures it in metres (cells times the cell size). To reproduce a run of
InVEST exactly, give Terraflux InVEST's critical length times the cell size (6,000 for InVEST's 200 on cells of
30 m).

### Loads given as application rates

If your table holds the fertilizer or manure applied rather than measured export, set `load_type_n` (and
`load_type_p`) to `"application-rate"`: the model then multiplies each load by \(1 - \text{eff}\) of its class
before scaling it. Mixed tables are allowed in the model file, with one type per class code:

```tfgraph
#! caption: Cropland's nitrogen as an application rate, the other classes as measured runoff (the inputs as above).
#! highlight: nutrients
#! fragment: true
format = "terraflux/1"

[[nodes]]
id = "nutrients"
op = "services.ndr"
inputs = { dem = "dem.raster", lulc = "lulc.raster", runoff_proxy = "rain.raster", biophysical = "biophysical.table", watersheds = "watersheds.vector" }
params = { threshold = 1000, subsurface_critical_length = 200, subsurface_efficiency = 0.8, load_type_n = { "1" = "measured-runoff", "2" = "measured-runoff", "3" = "application-rate", "4" = "measured-runoff", "5" = "measured-runoff", "6" = "measured-runoff" } }
```

Cropland's 20 kg then count as 17 kg (\(20 \times (1 - 0.15)\)), and the nitrogen export of the two watersheds falls
from 4,134 to 3,843 kg / yr. Every class needs its type when you give them by class.

## Read the results

### Retention and delivery

```tfgallery
columns: 2
caption: "Top: the index of connectivity (highest next to the streams, lowest on the gentle ground far from them) and the effective retention of nitrogen (dark green: 0.8 under the forest and below it; light: cropland and pasture that drain straight to a stream). Bottom: the delivery ratio of nitrogen, which combines both, and the effective retention of phosphorus. White: the streams, where both are missing, as in InVEST, and the land outside the watersheds."
images:
  - { src: /img/tutorials/ecosystem-services/nutrients/ic.png, caption: "ic" }
  - { src: /img/tutorials/ecosystem-services/nutrients/effective_retention_n.png, caption: "effective_retention_n" }
  - { src: /img/tutorials/ecosystem-services/nutrients/ndr_n.png, caption: "ndr_n" }
  - { src: /img/tutorials/ecosystem-services/nutrients/effective_retention_p.png, caption: "effective_retention_p" }
```

The effective retention is the map of the service: high where the land retains, and **downslope of** where it
retains. The forest on the ridges retains what it releases; the forest on the lower slopes also retains what the
fields above it release. Most of the landscape delivers less than 20% of its surface load (mean NDR 0.15); the
cells next to the streams, well connected and with little land below them, deliver up to 73%.

### Export

```tfgallery
columns: 2
caption: "The exports of 2020 on logarithmic scales, kg / ha / yr. Surface nitrogen and phosphorus come from the cropland and pasture close to the streams; the subsurface nitrogen depends only on the load and the distance to a stream, so it follows the land use map more closely. White: the water class, which has no load, the streams (no surface export) and the land outside the watersheds."
images:
  - { src: /img/tutorials/ecosystem-services/nutrients/n_surface_export.png, caption: "n_surface_export" }
  - { src: /img/tutorials/ecosystem-services/nutrients/n_subsurface_export.png, caption: "n_subsurface_export" }
  - { src: /img/tutorials/ecosystem-services/nutrients/n_total_export.png, caption: "n_total_export" }
  - { src: /img/tutorials/ecosystem-services/nutrients/p_surface_export.png, caption: "p_surface_export" }
```

```tftable
data: /data/tutorials/ecosystem-services/nutrients/watersheds.csv
labels: { ws_id: "Watershed", n_surface_load: "N surface load", n_surface_export: "N surface export", n_subsurface_load: "N subsurface load", n_subsurface_export: "N subsurface export", n_total_export: "N total export", p_surface_load: "P load", p_surface_export: "P export" }
digits: 0
caption: "The output watershed_table for 2020, kg per year. Watershed 1 releases 8,872 kg of nitrogen over the surface and 4,677 kg through the soil; 2,334 kg reach the streams. Of its 879 kg of phosphorus, 134 kg (15%) do."
```

One thing to know about the table: its total export is **less** than the sum of the surface and subsurface exports
(in watershed 1, \(1\,396 + 1\,087 = 2\,483\), not \(2\,334\) kg). The stream cells have a subsurface export but no
surface export, so no total either; their subsurface export (248 kg a year in the two watersheds) is in the column
`n_subsurface_export` and not in `n_total_export`. InVEST's own results have the same gap. Report the columns you
used.

Which classes send the nutrients?

```tftable
data: /data/tutorials/ecosystem-services/nutrients/by-class.csv
labels: { class: "Class", area_ha: "Area (ha)", load_n: "N load (kg / ha / yr)", eff_n: "Mean eff' (N)", ndr_n: "Mean NDR (N)", export_n: "N export (kg / yr)", share_n: "N share (%)", export_p: "P export (kg / yr)", share_p: "P share (%)" }
digits: 3
caption: "Retention, delivery and total export by class in the two watersheds, 2020. Cropland covers 18% of the watersheds and sends 47% of the nitrogen and 52% of the phosphorus; pasture, more than twice as large, sends nearly as much. Cropland's mean effective retention (0.54) is far above its own efficiency (0.15): much of it drains through forest."
```

!!! paper "Reporting a nutrient analysis"
    Report the sources of the loads and efficiencies (with the table), whether the loads are measured runoff or
    application rates, the runoff proxy, the DEM and its resolution, the threshold, \(k\), the subsurface critical
    length and efficiency, and the routing. Give the export of each watershed (kg / yr) for each scenario, and
    compare it with loads measured at a gauge where they exist. Say that in-stream processes and point sources are
    not in the model.

### The scenario of 2040

Run the same model with `lulc_2040.tif`. The scenario clears forest edges, turns pasture into cropland, grows the
town, and restores the pasture and cropland along the rivers to forest.

![Change of nitrogen export from 2020 to 2040](/img/tutorials/ecosystem-services/nutrients/n_change.png "The change of total nitrogen export (kg / ha / yr), 2040 minus 2020: more where forest became pasture or pasture became cropland (orange); less on the restored riparian strips and on the slopes above them (purple), which now drain through forest.")

```tfcompare
caption: "The effective retention of nitrogen in 2020 (left) and 2040 (right), same colour scale (0 to 0.8). The restored strips along the rivers raise the retention of the fields above them: the light areas of 2020 next to the rivers turn dark."
before: { src: /img/tutorials/ecosystem-services/nutrients/eff_n_2020.png, label: "2020" }
after: { src: /img/tutorials/ecosystem-services/nutrients/eff_n_2040.png, label: "2040" }
```

```tftable
data: /data/tutorials/ecosystem-services/nutrients/scenario.csv
labels: { quantity: "Quantity (both watersheds)", "2020": "2020", "2040": "2040", change: "Change" }
digits: 1
caption: "The two watersheds in 2020 and 2040, from the watershed tables. The shares retained are 1 - total export / total load."
```

The loads rise by a quarter to two fifths, yet the surface export of nitrogen **falls** by 10% and that of
phosphorus barely moves: the riparian forest retains the extra load and more. The subsurface export of nitrogen
rises with its load (+23%), because the model's soil retains dissolved nitrogen by distance alone, whatever grows
on it. So the total nitrogen export still rises by 8%. Two lessons: a riparian strip in this model works on the
surface path only, and how much nitrogen you assign to the subsurface (`proportion_subsurface_n`) can decide whether
a scenario looks better or worse. The index of connectivity is the same in both years (it does not depend on the
land use); everything that changes comes from the loads and the retention.

## Settings that change the answer

### The stream threshold

The threshold decides where hillslopes end and streams begin. A smaller threshold draws more streams, so every
cell is closer to one, better connected, and has less land below it to retain its load:

```tfslider
label: Stream threshold (cells)
caption: "Total nitrogen export (logarithmic scale) and the streams (blue) for thresholds from 250 to 4,000 cells (22 to 360 ha). The export falls from 4,459 to 3,914 kg / yr as the streams shrink, but not evenly: from 1,000 to 2,000 cells it does not change, because IC_0, half way between the lowest and highest index, moves from -1.18 to -1.56."
frames:
  - { value: 250, src: /img/tutorials/ecosystem-services/nutrients/threshold_250.png, stats: { "Streams": "197 ha", "N export": "4,459 kg / yr", "P export": "275 kg / yr", "IC_0": "-1.23" } }
  - { value: 500, src: /img/tutorials/ecosystem-services/nutrients/threshold_500.png, stats: { "Streams": "126 ha", "N export": "4,303 kg / yr", "P export": "258 kg / yr", "IC_0": "-1.20" } }
  - { value: 1000, src: /img/tutorials/ecosystem-services/nutrients/threshold_1000.png, stats: { "Streams": "81 ha", "N export": "4,134 kg / yr", "P export": "242 kg / yr", "IC_0": "-1.18" } }
  - { value: 2000, src: /img/tutorials/ecosystem-services/nutrients/threshold_2000.png, stats: { "Streams": "44 ha", "N export": "4,138 kg / yr", "P export": "243 kg / yr", "IC_0": "-1.56" } }
  - { value: 4000, src: /img/tutorials/ecosystem-services/nutrients/threshold_4000.png, stats: { "Streams": "16 ha", "N export": "3,914 kg / yr", "P export": "224 kg / yr", "IC_0": "-1.78" } }
start: 2
```

Choose the threshold so that the streams match a map of the real rivers, not to reach a number. With the same
threshold the NDR draws fewer stream cells than the sediment model (81 ha against 99 ha at 1,000 cells here):
InVEST's two models trace the streams from multiple flow directions with different settings, and Terraflux follows
each.

### Borselli's k

```tfchart
type: line
data: /data/tutorials/ecosystem-services/nutrients/k.csv
x: k
y: [n_rel, p_rel]
labels: { n_rel: "Nitrogen, surface export", p_rel: "Phosphorus, export" }
colors: { n_rel: "#2f6f9f", p_rel: "#e8901f" }
xlabel: Borselli's k
ylabel: "% of the export with k = 2"
caption: "The surface exports of the two watersheds for k from 0.5 to 4, relative to the default: from 70% to 108%. The subsurface export does not depend on k."
```

The effect is much smaller than in the sediment model, where \(k\) changed the export by a factor of a thousand.
Here \(IC_0\) sits in the middle of the landscape's indices, so cells lie on both sides of the curve: a larger
\(k\) raises the delivery of the poorly connected cells and lowers that of the well connected ones, and the two
partly cancel. Keep InVEST's default (2) unless measured loads give you a reason to change it.

### The critical lengths

How far the vegetation must extend to reach its efficiency matters less than the efficiency itself. Multiplying
every class's critical length by the same factor:

```tfchart
type: line
data: /data/tutorials/ecosystem-services/nutrients/critical-length.csv
x: factor
y: [n_rel, p_rel]
labels: { n_rel: "Nitrogen, surface export", p_rel: "Phosphorus, export" }
colors: { n_rel: "#2f6f9f", p_rel: "#e8901f" }
xlabel: Critical lengths times
ylabel: "% of the export with the table's lengths"
xlog: true
caption: "The surface exports for the table's critical lengths times 0.25 to 4: 98% to 110%. Longer lengths make a strip of forest reach its efficiency farther from the stream, so the narrow strips retain less."
```

### The subsurface path

```tftable
data: /data/tutorials/ecosystem-services/nutrients/subsurface.csv
labels: { l_sub: "Critical length (m)", eff_sub: "Efficiency", n_subsurface_load: "Subsurface load (kg / yr)", n_subsurface_export: "Subsurface export (kg / yr)", n_total_export: "Total N export (kg / yr)" }
digits: 2
caption: "The subsurface nitrogen of the two watersheds for other subsurface parameters. The efficiency matters most: from 0.95 to 0.5 the total export more than doubles (2,961 to 6,480 kg / yr). The critical length matters less, because half the cells lie more than 300 m from a stream along the flow, where the shorter lengths have long reached their efficiency."
```

These two numbers are among the least known of the model. Without local data, keep the guide's averages, and show
the reader how the answer changes over a plausible range.

### Multiple flow directions or D8

```tftable
data: /data/tutorials/ecosystem-services/nutrients/routing.csv
labels: { method: "Routing", n_surface: "N surface export (kg / yr)", n_total: "N total export (kg / yr)", p_surface: "P export (kg / yr)", ic_0: "IC_0" }
digits: 2
caption: "The exports of the two watersheds with each routing. D8 sends all of a cell's water down one path, which changes the paths, the retention along them and the range of the index (and so IC_0): the surface exports rise by a fifth."
```

## Common pitfalls

- **Loads, their units and their type.** Loads are in kg per hectare and year. Export coefficients are measured
  runoff (the default); fertilizer and manure applied are application rates (`load_type_n`, `load_type_p`). Mixing
  them up changes the load by the factor \(1 - \text{eff}\).
- **Comparing runs of different extents.** Both the runoff proxy's mean and \(IC_0\) are computed over the cells of
  the run. Clip the landscape differently, add a watershed or change the threshold, and every cell's load and
  delivery ratio change. Compare scenarios on the same DEM, watersheds and threshold; to keep the proxy's scaling
  fixed between runs, give `runoff_proxy_mean`.
- **Reading the watershed total as the sum of its parts.** `n_total_export` leaves out the subsurface export of the
  stream cells (above).
- **The streams have no surface export.** The surface load of a stream cell, and the delivery ratio, are missing
  there, as in InVEST: give the water class a load of 0, and check that the streams of the model match the rivers.
- **The subsurface distance.** In metres in Terraflux, in cells in InVEST 3.20 (above): a critical length copied
  from a study that used InVEST may need the cell size.
- **Point sources and in-stream processes.** The town's sewage, a feedlot or a fish farm are not in the land use
  loads; reservoirs and wetlands on the river retain nutrients the model does not see. Add point sources to the
  watershed totals yourself, and do not read the export as the load at the outlet.
- **The resolution.** The connectivity and the retention paths come from the DEM: a 90 m grid has other paths and
  other strips than a 30 m one (InVEST's guide notes that the effect of resolution on the model has not been well
  studied). Narrow riparian strips need fine cells to exist at all.
- **Phosphorus without sediment.** The model routes phosphorus like nitrogen's surface part; most phosphorus leaves
  attached to eroded soil. Compare with the [sediment export](/tutorials/ecosystem-services/sediment/).

## Going further

- [Ecosystem service models: the ideas](/tutorials/ecosystem-services/ideas/) for what the series' models have in
  common, and Ribeirão Claro.
- [Sediment delivery ratio](/tutorials/ecosystem-services/sediment/), the same connectivity applied to soil.
- [Annual water yield](/tutorials/ecosystem-services/water-yield/) on the same watersheds, and
  [Seasonal water yield](/tutorials/ecosystem-services/seasonal-water-yield/), the next part, whose quickflow is the
  runoff proxy InVEST recommends.
- [Monte Carlo uncertainty](/tutorials/uncertainty/monte-carlo/) to carry the uncertainty of the loads and
  efficiencies into the export.
- The operators' reference: [[services.ndr]], [[services.seasonal_water_yield]] (a quickflow index for the runoff
  proxy), [[hydro.watersheds]].

## References

- Borselli, L., Cassi, P., & Torri, D. (2008). Prolegomena to sediment and flow connectivity in the landscape: A GIS and field numerical assessment. *Catena*, 75(3), 268–277. https://doi.org/10.1016/j.catena.2008.07.006
- Mayer, P. M., Reynolds, S. K., McCutchen, M. D., & Canfield, T. J. (2007). Meta-analysis of nitrogen removal in riparian buffers. *Journal of Environmental Quality*, 36(4), 1172–1180. https://doi.org/10.2134/jeq2006.0462
- Natural Capital Alliance. (2026). *InVEST 3.20.2 User's Guide: NDR, nutrient delivery ratio*. https://storage.googleapis.com/releases.naturalcapitalproject.org/invest-userguide/latest/en/ndr.html
- Redhead, J. W., May, L., Oliver, T. H., Hamel, P., Sharp, R., & Bullock, J. M. (2018). National scale evaluation of the InVEST nutrient retention model in the United Kingdom. *Science of the Total Environment*, 610–611, 666–677. https://doi.org/10.1016/j.scitotenv.2017.08.092
- Valladares-Castellanos, M., de Jesús Crespo, R., Xu, Y. J., & Douthat, T. H. (2024). A framework for validating watershed ecosystem service models in the United States using long-term water quality data: Applications with the InVEST Nutrient Delivery (NDR) model in Puerto Rico. *Science of the Total Environment*, 949, 175111. https://doi.org/10.1016/j.scitotenv.2024.175111
- Vigiak, O., Borselli, L., Newham, L. T. H., McInnes, J., & Roberts, A. M. (2012). Comparison of conceptual landscape metrics to define hillslope-scale sediment delivery ratio. *Geomorphology*, 138(1), 74–88. https://doi.org/10.1016/j.geomorph.2011.08.026
