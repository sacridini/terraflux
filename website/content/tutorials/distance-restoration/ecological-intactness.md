---
title: Ecological intactness
short: Ecological intactness
description: Turn a human footprint map into Beyer et al.'s ecological intactness, map where restoring converted land would raise it most, and see what the radius of the neighbourhood changes.
order: 30
level: intermediate
time: 40 min
template: Ecological intactness
ops: [restoration.intactness, restoration.intactness_gradient, raster.read, raster.calc]
series: Distance and restoration
part: 3
---

From this part on, the series follows one region, **Serra Verde**, through the steps of a restoration plan. This
part measures human pressure. You read a map of the human footprint, turn it into the *ecological intactness*
of every cell (how free it and its surroundings are of human pressure), and map where restoring pasture and
cropland would raise the intactness of the whole landscape the most. At the end you have the intactness map, a
map of where restoration pays most for it, and a before-and-after comparison of the landscape with all its
converted land restored.

## The idea

### The human footprint

The **Human Footprint Index** sums eight human pressures in each cell of the world's land on a scale of 0 to 50:
built environments (10 where land is built), croplands (7), pasture (4), population density, night-time lights,
railways, roads and navigable waterways (Sanderson et al., 2002; Venter et al., 2016). A footprint of 0 is
wilderness; a footprint of 4 is the pressure of a pasture alone. Venter et al. (2016) published global maps of it
at 1 km for 1993 and 2009, freely available.

### From footprint to intactness

A cell can be free of pressure and still be degraded by what surrounds it: a patch of forest in the middle of a
city is not intact. Beyer et al. (2020) defined the **ecological intactness** of a cell from the quality of the
cell *and* of its neighbourhood, and used it to show how much intactness the world's ecoregions have lost.
Strassburg et al. (2020) used the same measure as one of the benefits of their global restoration plan, and
Terraflux computes it as they did.

![The quality of habitat against the human footprint: it falls to 0.2 at a footprint of 4](/img/tutorials/distance-restoration/ecological-intactness/quality.png "The quality q = exp(-γh) of a cell with footprint h (green), with γ chosen so that a footprint of 4, a pasture's, leaves a fifth of the quality. Dashed: q to the power z = 0.5, the term each cell contributes to a pair.")

!!! science "Key idea"
    The quality of habitat of a cell with footprint \(h\) is \( q = e^{-\gamma h} \), with
    \(\gamma = -\ln(0.2)/4 \approx 0.40\): 1 without footprint, 0.2 at a footprint of 4. The **intactness** of
    cell \(i\) is

    \[ Q_i = \frac{\sum_j (q_i\, q_j)^{z}\, w_{ij}}{\sum_j w_{ij}}, \qquad w_{ij} = e^{\beta d_{ij}} \]

    over the cells \(j\) within a radius of \(i\) (itself included), where \(d_{ij}\) is their distance in km,
    \(\beta = -0.2\) per km makes nearer cells count more, and \(z = 0.5\). \(Q_i\) runs from 0 (the cell or its
    surroundings fully transformed) to 1 (the cell and all its neighbourhood free of pressure).

### Where restoring helps most

Restoring a share \(x_k\) of cell \(k\) lowers its footprint by a *reduction* \(a_k\) per unit: restoring a
pasture removes a pressure of 4, a cropland 7. The total intactness of the landscape, \(\sum_i Q_i\), then rises,
both in the cell and in every cell whose neighbourhood holds it. The **intactness gradient** is how fast it
rises per unit restored:

\[ \frac{\partial \sum_i Q_i}{\partial x_k} \]

It is the linear benefit of intactness in a restoration plan: high where one restored cell reconnects intact
surroundings, low in the middle of a city, where a restored cell stays surrounded by pressure.

## When to use it

- **Ranking places by human pressure in context**: intactness is a better proxy for the condition of
  biodiversity than the footprint alone, because it accounts for the surroundings.
- **A benefit of restoration** besides species and carbon: the [sixth part](../several-benefits/) adds it to a
  restoration plan, with the gradient as its value per cell.
- **Tracking change**: intactness computed from footprints of two years shows where pressure grew around
  remaining habitat.

What it is *not* for: intactness is not habitat for a given species (that is the [area of
habitat](../original-vegetation-and-habitat/) of the next part), and it says nothing about connectivity for
movement (see the [corridors](../costs-and-corridors/) of the previous part, or
[habitat networks](/tutorials/#habitat-networks)).

## What you need

- **A human footprint map** in a projected CRS: the cells' size in km enters the weights. Serra Verde's
  `footprint.tif` is a footprint index on 60 × 90 cells of 1 km (SIRGAS 2000 / UTM 23S, EPSG:31983).
- **The reduction per unit restored**, here from the land cover: `land_cover.tif` holds the share (0 to 1) of six
  classes in every cell, as six named bands: `forest`, `savanna`, `mangrove`, `pasture`, `cropland` and `urban`.

!!! note "Serra Verde"
    Serra Verde is an *imaginary* coastal region of 60 × 90 km, the example data of landschaft's prioritization
    module: mountains in the north-west, a coastal plain with three cities, a highway and two rivers, and the
    ocean (no data) in the south-east. The data are synthetic, made to behave like real planning data, so that
    every step of a plan can be shown on one small region. Replace them with your own maps of the same kinds.

The application's template **Ecological intactness** (Templates menu, group *Conservation and restoration*)
builds the same model on a footprint drawn by chance.

## Build the model

### The footprint and the reduction

Add a [[raster.read]] node for `footprint.tif`. For the reduction, read the two anthropic bands of the land cover
with two more [[raster.read]] nodes on `land_cover.tif`, setting their `band` to `pasture` and `cropland` (a band
can be chosen by its name). A [[raster.calc]] node with inputs `p` and `c` computes the footprint that restoring one
unit of each cell removes, the mean of 4 (pasture) and 7 (cropland) weighted by their shares:

```text
(4 * p + 7 * c) / (p + c) if p + c > 0 else 0
```

### Intactness and its gradient

[[restoration.intactness]] takes the `footprint` and gives the intactness `map`. Its settings are Beyer et al.'s:
`radius` (3 cells), `gamma` (−ln(0.2)/4), `beta` (−0.2 per km) and `z` (0.5), and `cell_size` (the cells' size
in km, read from a projected grid; give it for a grid without CRS). With a `reduction` and a `restored` map (the
share of each cell restored) it gives the intactness after restoring them.

[[restoration.intactness_gradient]] takes the `footprint` and the `reduction` and gives, for every cell, the gain
in total intactness per unit of the cell restored.

```tfgraph
#! caption: Intactness and where restoring raises it most: the footprint, the reduction a unit restored removes (from the shares of pasture and cropland), the intactness, its gradient, and the intactness with all converted land restored.
#! highlight: intactness, gradient, after
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "footprint"
op = "raster.read"
params = { path = "footprint.tif" }
note = "Human footprint, 0-50, cells of 1 km."

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
note = "Footprint removed per unit restored: 4 pasture, 7 cropland."

[[nodes]]
id = "intactness"
op = "restoration.intactness"
inputs = { footprint = "footprint.raster" }
params = { radius = 3 }

[[nodes]]
id = "gradient"
op = "restoration.intactness_gradient"
inputs = { footprint = "footprint.raster", reduction = "reduction.map" }
params = { radius = 3 }
note = "Gain in total intactness per unit of each cell restored."

[[nodes]]
id = "all_converted"
op = "raster.calc"
inputs = { p = "pasture.raster", c = "cropland.raster" }
params = { expression = "p + c" }
note = "Every share of pasture and cropland restored."

[[nodes]]
id = "after"
op = "restoration.intactness"
inputs = { footprint = "footprint.raster", reduction = "reduction.map", restored = "all_converted.map" }
params = { radius = 3 }
```

## Read the results

### Footprint and intactness

```tfgallery
columns: 2
caption: Left, the human footprint of Serra Verde, from 0 (white) to 23.5 along the highway and in the three cities. Right, the intactness it leaves (radius 3 cells). The mountains of the north-west are intact; the coastal plain is not.
images:
  - { src: /img/tutorials/distance-restoration/ecological-intactness/footprint.png, caption: "Human footprint" }
  - { src: /img/tutorials/distance-restoration/ecological-intactness/intactness.png, caption: "Ecological intactness" }
```

The mean footprint is low (2.0) because most of the region is mountain forest, but the intactness tells a sharper
story: around the highway, the rivers' farms and the cities, intactness falls below 0.3 well beyond the cells
that bear the pressure, because every cell within three cells of them shares it. Over the region, 70% of the cells
have an intactness above 0.5, and the mean is 0.70.

### Where restoration raises it most

```tfgallery
columns: 2
caption: Left, the share of every cell in pasture or cropland, the land restoration can return to nature (916 km² in all). Right, the intactness gradient on that land: the gain in total intactness per unit of the cell restored.
images:
  - { src: /img/tutorials/distance-restoration/ecological-intactness/converted.png, caption: "Converted land" }
  - { src: /img/tutorials/distance-restoration/ecological-intactness/gradient.png, caption: "Intactness gradient" }
```

The gradient is highest (up to 2.3) at the fringe of the converted land, where a restored cell is surrounded by
intact land and lifts its neighbours too. It is lowest along the highway and around the cities: restoring a farm
there removes its own pressure, but the cell stays next to a road or a town and its quality stays low. On average
a unit of converted land restored adds 0.92 to the total intactness.

!!! tip "Restoration at the fringe"
    The gradient rewards restoring land next to intact habitat, the same advice the distance to the forest gave
    in the [first part](../distance-to-habitat/) for natural regeneration. Both point to the edges of what
    remains.

### Before and after

Drag the handle to compare the intactness today with the intactness if every share of pasture and cropland were
restored (the `after` node: the footprint lowered by the reduction times the share restored):

```tfcompare
caption: Ecological intactness now (left) and with all 916 km² of pasture and cropland restored (right), on the same colour scale (0 dark blue, 1 yellow). The mean rises from 0.70 to 0.86, and the cells above 0.5 from 70% to 87%. The highway and the cities, which restoration does not remove, stay.
before: { src: /img/tutorials/distance-restoration/ecological-intactness/compare_now.png, label: Now }
after: { src: /img/tutorials/distance-restoration/ecological-intactness/compare_after.png, label: All converted land restored }
```

### How to report it

Give the footprint data and year, the cell size, the parameters (radius, γ, β, z; say if you used the defaults
of Beyer et al.), the mean intactness and the share of the area above a threshold, before and after a plan. A map
of the gradient shows where restoration pays most for intactness.

## Settings that change the answer

The radius sets the neighbourhood whose pressure a cell shares: a larger radius spreads the effect of a road or a
town further. Each frame is a real run:

```tfslider
label: Radius (cells of 1 km)
caption: Ecological intactness with a radius of 1 to 8 cells. As the radius grows, the low intactness around the highway and the cities widens, and the mean falls slightly. The last value counts how many of the 100 cells of the highest gradient are also among the 100 highest with a radius of 3.
frames:
  - { value: 1, src: /img/tutorials/distance-restoration/ecological-intactness/radius_1.png, stats: { "Mean intactness": "0.707", "Cells above 0.5": "70.5%", "Top 100 shared with radius 3": "89" } }
  - { value: 2, src: /img/tutorials/distance-restoration/ecological-intactness/radius_2.png, stats: { "Mean intactness": "0.705", "Cells above 0.5": "70.3%", "Top 100 shared with radius 3": "92" } }
  - { value: 3, src: /img/tutorials/distance-restoration/ecological-intactness/radius_3.png, stats: { "Mean intactness": "0.702", "Cells above 0.5": "69.8%", "Top 100 shared with radius 3": "100" } }
  - { value: 5, src: /img/tutorials/distance-restoration/ecological-intactness/radius_5.png, stats: { "Mean intactness": "0.694", "Cells above 0.5": "69.0%", "Top 100 shared with radius 3": "83" } }
  - { value: 8, src: /img/tutorials/distance-restoration/ecological-intactness/radius_8.png, stats: { "Mean intactness": "0.685", "Cells above 0.5": "68.4%", "Top 100 shared with radius 3": "59" } }
start: 2
```

The mean barely changes, but the priorities do: with a radius of 8 cells only 59 of the 100 cells where restoring
helps most are the same as with 3. Choose the radius from the distance over which pressure degrades habitat for
the species you care about, and say which you used; Strassburg et al. (2020) used 3 cells.

The other parameters matter too. `gamma` sets how fast quality falls with the footprint (a larger value makes a
pasture more damaging), `beta` how fast the neighbours' weight falls with distance (closer to 0 makes the far
neighbours count as much as the near ones), and `z` how much a single bad neighbour lowers a pair.

## Common pitfalls

- **Degrees.** The weights need distances in km: the footprint must be in a projected CRS, or `cell_size` given
  in km. Terraflux stops with a message on a map in degrees.
- **The reduction is per unit of the cell.** A reduction of 7 and a share restored of 0.5 lower the footprint by
  3.5. Restoring more than the converted share would remove pressures (roads, lights) restoration cannot remove;
  keep `restored` within the share of anthropic land.
- **The edge of the map.** Cells near the edge have a smaller neighbourhood (only the part inside the map), so
  their intactness ignores pressures just outside. Include a margin around the study area.
- **Old footprints.** The global footprint maps lag behind the land; check the year against your land cover.

## Going further

- What converted land was before, and how much habitat each species has lost: [Original vegetation and area of
  habitat](../original-vegetation-and-habitat/), the next part.
- Intactness as a benefit of a restoration plan, weighed against species and carbon:
  [Several benefits](../several-benefits/).
- The operators: [[restoration.intactness]], [[restoration.intactness_gradient]], and the `hfi`, `reduction`
  and `intactness_*` settings of [[restoration.plan]].

## References

- Beyer, H. L., Venter, O., Grantham, H. S., & Watson, J. E. M. (2020). Substantial losses in ecoregion intactness highlight urgency of globally coordinated action. *Conservation Letters*, 13(2), e12692. https://doi.org/10.1111/conl.12692
- Sanderson, E. W., Jaiteh, M., Levy, M. A., Redford, K. H., Wannebo, A. V., & Woolmer, G. (2002). The human footprint and the last of the wild. *BioScience*, 52(10), 891–904. https://doi.org/10.1641/0006-3568(2002)052[0891:THFATL]2.0.CO;2
- Strassburg, B. B. N., Iribarrem, A., Beyer, H. L., Cordeiro, C. L., Crouzeilles, R., Jakovac, C. C., et al. (2020). Global priority areas for ecosystem restoration. *Nature*, 586(7831), 724–729. https://doi.org/10.1038/s41586-020-2784-9
- Venter, O., Sanderson, E. W., Magrach, A., Allan, J. R., Beher, J., Jones, K. R., et al. (2016). Sixteen years of change in the global terrestrial human footprint and implications for biodiversity conservation. *Nature Communications*, 7, 12558. https://doi.org/10.1038/ncomms12558
