---
title: Habitat quality and rarity
short: 3. Habitat quality
description: InVEST's habitat quality model in Terraflux - threats and how their impact fades with distance, the sensitivity of each habitat, the half-saturation constant, protected areas as access, a future scenario, and the rarity of each class against a baseline.
order: 30
level: intermediate
time: 50 min
ops: [services.habitat_quality, stack.of, table.read, vector.read, raster.read]
series: Ecosystem services with InVEST
part: 3
---

Where is the forest of a landscape in good shape, and where is it worn down by the crops, roads and towns around
it? InVEST's habitat quality model answers with a map of **degradation** and a map of **quality**, built only from
the land use map, maps of the threats and two small tables. In this tutorial you will build it in Terraflux on
Ribeirão Claro: you will describe three threats, see how each one degrades the habitat around it, choose the one
constant that sets the scale of the quality, give a park its protection, compare the scenario of 2040 with 2020,
and measure how rare each class has become since 1985. At the end you will know what the maps mean, how to set
the model's parameters, and how not to over-read them.

## The idea

Habitat loss is easy to map: the forest that is no longer there. **Habitat degradation** is harder: the forest
that remains but is hunted from the nearby road, invaded from the nearby pasture, burnt from the nearby fields,
noisier and drier at its edges. The habitat quality model uses land use as a proxy for both. It asks three
questions of every cell:

1. **Is it habitat, and how good?** Each class has a habitat suitability \(H_j\) from 0 (not habitat) to 1 (the
   best habitat), for the species or the biodiversity in general you have in mind.
2. **Which threats are near, and how strong are they here?** Each threat (crops, roads, a town) has a map, a
   maximum distance of influence, a relative weight, and a way its impact fades with distance: linearly or
   exponentially.
3. **How sensitive is this habitat to each threat, and how protected is the cell?** Each class has a
   sensitivity to each threat (0 to 1), and each cell an *access* (0 to 1) that lowers every threat where legal,
   social or physical barriers keep it out, as in a strictly protected park.

The model is one of InVEST's, described in its user's guide (Natural Capital Alliance, 2026). Terrado et al.
(2016) adapted it to river basins and their freshwater habitats and compared it with field data: the quality it
gave tracked the observed richness of vascular plants (R² = 0.76) and, less closely, the diversity of aquatic
macroinvertebrates. Sallustio et al. (2017) mapped habitat quality over all of Italy at 20 m, with threats ranked
by experts, and compared it with the network of protected areas.

!!! science "Key idea: degradation from distance, weight, sensitivity and access"
    The impact of threat \(r\) at cell \(x\) depends on the distance \(d_{rx}\) from \(x\) to the nearest cell
    of the threat, and on its maximum distance \(d_{r,\max}\):

    \[ i_{rx} = 1 - \frac{d_{rx}}{d_{r,\max}} \;\;\text{(linear)} \qquad
       i_{rx} = \exp\!\left(-\frac{2.99}{d_{r,\max}}\, d_{rx}\right) \;\;\text{(exponential)} \]

    and 0 beyond \(d_{r,\max}\). With the constant 2.99 the exponential impact falls to 5% at the maximum
    distance. The **total degradation** of a cell of class \(j\) sums the threats:

    \[ D_{xj} = \sum_r \frac{w_r}{\sum_r w_r}\; i_{rx}\; \beta_x\; S_{jr} \]

    where \(w_r\) is the weight of threat \(r\) (divided by the sum of the weights, so that the weights are
    relative), \(\beta_x\) the access of the cell (1 where none is given) and \(S_{jr}\) the sensitivity of class
    \(j\) to threat \(r\). The **quality** is

    \[ Q_{xj} = H_j \left(1 - \frac{D_{xj}^{z}}{D_{xj}^{z} + k^{z}}\right), \qquad z = 2.5, \]

    a curve that keeps the quality near \(H_j\) while the degradation is small, halves it when \(D = k\) (the
    **half-saturation constant**), and brings it towards 0 when the degradation is much larger than \(k\).

!!! note "The threat maps in InVEST 3.20"
    InVEST's user's guide writes the formula with an intensity \(r_y\) of each threat cell, from 0 to 1. Since
    version 3.12, InVEST takes the distance to the nearest cell of the threat instead, and InVEST 3.20 (so
    Terraflux too) counts a cell as the threat wherever its threat map is not 0: values between 0 and 1 are
    accepted (and checked) but a cell of 0.2 counts as fully as a cell of 1. Give the threat maps as 1 where the
    threat is and 0 elsewhere.

A worked cell: a forest cell (sensitivity 0.6 to crops, 0.8 to roads) lies 500 m from the nearest cropland and
300 m from the nearest road; the town is beyond its reach. With this tutorial's threats (crops linear, 800 m,
weight 0.7; roads exponential, 1,500 m, weight 0.8; urban exponential, 2,500 m, weight 1; the weights sum to 2.5),
crops add \(0.28 \times (1 - 500/800) \times 0.6 = 0.063\) and roads
\(0.32 \times e^{-2.99 \times 300/1500} \times 0.8 = 0.32 \times 0.550 \times 0.8 = 0.141\): \(D = 0.204\). With
\(k = 0.35\) its quality is \(1 - 0.204^{2.5}/(0.204^{2.5} + 0.35^{2.5}) = 0.79\); with \(k = 0.05\) it would be
0.03.

![How the impact of the three threats fades with distance](/img/tutorials/ecosystem-services/habitat-quality/decay.png "The impact of each threat of this tutorial with the distance to its nearest cell: crops fade linearly to 0 at 800 m; roads and the town fade exponentially, to 5% of their impact at 1,500 m and 2,500 m, and to 0 beyond.")

### Rarity

With a **base** map (a land use of the past, "when intensive landscape management was relatively rare" in
InVEST's words), the model
also measures the **rarity** of each class: \(R_j = 1 - N_j / (N_{j,\text{base}} + N_j)\), where \(N_j\) is the
area of class \(j\) on the current (or future) map and \(N_{j,\text{base}}\) its area on the base map. A class
whose area has not changed has rarity 0.5; one that has shrunk has more than 0.5 (it has become rarer); one that
has grown has less; a class absent from the base map has 0.

## When to use it

- As a **proxy for biodiversity** beside the other services, when there are no good maps of species: the
  scenarios of a plan compared for carbon, water and habitat at once.
- To find the **remnants most at risk**: habitat of high suitability and high degradation.
- To evaluate **protection**: how much the access of a park lowers the degradation of the habitat inside it.
- To track **change**: the quality of 2020 against a scenario, or against the past with the base map.

What it is **not**: it is not a model of any species' distribution, abundance or persistence. Its numbers are
relative, set by the tables and by \(k\), and comparable between scenarios run with the same parameters, not
between studies. It knows nothing about the size of patches or the connections between them; for those, see
[Fragmentation](/tutorials/fragmentation/fragmentation-and-habitat-loss/),
[MSPA](/tutorials/habitat-patterns/mspa/) and [habitat networks](/tutorials/habitat-networks/connectivity-as-a-graph/).
For one species, a [species distribution model](/tutorials/species-distributions/) is the better tool.

## What you need

- **The current land use map** (`lulc_2020.tif`), projected in metres, integer codes.
- **A map of each threat** on the same grid, 1 where the threat is and 0 elsewhere: `crops_2020.tif`,
  `roads_2020.tif`, `urban_2020.tif` here. A threat can be a land use class (crops, urban) or something the land
  use map does not show (roads, mines, a railway).

![The three threat maps](/img/tutorials/ecosystem-services/habitat-quality/threat-maps.png "The threats of 2020: the cropland of the land use map, the roads (one cell wide) and the town.")

- **The threats**, as a list in the operator's settings (below) or as InVEST's threats table, a CSV with the
  columns `threat`, `max_dist` (metres), `weight` (0 to 1) and `decay` (`linear` or `exponential`):

```text
threat,max_dist,weight,decay
crops,800,0.7,linear
roads,1500,0.8,exponential
urban,2500,1.0,exponential
```

- **The sensitivity table**, a CSV by class with the columns `lucode`, `habitat` (\(H_j\), 0 to 1) and one column
  per threat, named as the threat, with the sensitivity of the class to it. Here forest is the best habitat
  (1), pasture and plantations poor habitat (0.3 and 0.2), the rest none:

| lucode | class | habitat | crops | roads | urban |
|---|---|---|---|---|---|
| 1 | Forest | 1.0 | 0.6 | 0.8 | 0.9 |
| 2 | Pasture | 0.3 | 0.4 | 0.5 | 0.6 |
| 3 | Cropland | 0 | 0 | 0 | 0 |
| 4 | Eucalyptus | 0.2 | 0.3 | 0.4 | 0.5 |
| 5 | Urban | 0 | 0 | 0 | 0 |
| 6 | Water | 0 | 0 | 0 | 0 |

- Optional: **access** polygons with an `access` attribute (here a park, 0, and its buffer zone, 0.5), a future
  map and its threat maps, a base map.

The values are illustrative. In a real study they come from the literature on the threats in your region and
from experts (Sallustio et al. (2017) had experts rank the threats of each land use for all of Italy); document
where each number comes from.

## Build the model

### Step 1: the threat maps as one stack

The operator takes the threat maps as a **stack**: several maps on one grid, each layer named. The names are
what connects a map to its threat and to its column of the sensitivity table, so they must be the same in all
three places.

<div class="steps" markdown>

1. Add three [[raster.read]] nodes and set their `path`s to `crops_2020.tif`, `roads_2020.tif` and
   `urban_2020.tif`. Rename them `crops`, `roads` and `urban` (Node pane, *Rename*).
2. Add a [[stack.of]] node (library subject *Building models*). It takes any number of maps: drag from each
   reader's output to the stack node and name the input `crops`, `roads`, `urban`. The layers take the names of
   the inputs.

</div>

```tfgraph
#! caption: Three threat maps made into one stack whose layers are named crops, roads and urban.
#! highlight: threat_maps
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "crops"
op = "raster.read"
params = { path = "crops_2020.tif" }

[[nodes]]
id = "roads"
op = "raster.read"
params = { path = "roads_2020.tif" }

[[nodes]]
id = "urban"
op = "raster.read"
params = { path = "urban_2020.tif" }

[[nodes]]
id = "threat_maps"
op = "stack.of"
inputs = { crops = "crops.raster", roads = "roads.raster", urban = "urban.raster" }
note = "Layers named as the threats."
```

### Step 2: the model

<div class="steps" markdown>

1. Add [[raster.read]] for `lulc_2020.tif` and [[table.read]] for `sensitivity.csv`.
2. Add [[services.habitat_quality]] (subject *Ecosystem services*) and connect: the land use to `lulc`, the table
   to `sensitivity`, the stack to `threat_maps`.
3. In the Node pane, set `half_saturation` (\(k\)) to 0.05 for a first run (InVEST's default) and fill
   `threats`.

</div>

The setting `threats` takes the list of threats, each with its four values. In the model file it reads:

```tfgraph
#! caption: Habitat quality of 2020: the land use, the sensitivity table, the threat maps and the threats in the settings.
#! highlight: habitat
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "lulc"
op = "raster.read"
params = { path = "lulc_2020.tif" }

[[nodes]]
id = "sensitivity"
op = "table.read"
params = { path = "sensitivity.csv" }
note = "lucode, habitat, crops, roads, urban."

[[nodes]]
id = "crops"
op = "raster.read"
params = { path = "crops_2020.tif" }

[[nodes]]
id = "roads"
op = "raster.read"
params = { path = "roads_2020.tif" }

[[nodes]]
id = "urban"
op = "raster.read"
params = { path = "urban_2020.tif" }

[[nodes]]
id = "threat_maps"
op = "stack.of"
inputs = { crops = "crops.raster", roads = "roads.raster", urban = "urban.raster" }

[[nodes]]
id = "habitat"
op = "services.habitat_quality"
inputs = { lulc = "lulc.raster", sensitivity = "sensitivity.table", threat_maps = "threat_maps.stack" }
params = { half_saturation = 0.05, threats = [
  { threat = "crops", max_dist = 800, weight = 0.7, decay = "linear" },
  { threat = "roads", max_dist = 1500, weight = 0.8, decay = "exponential" },
  { threat = "urban", max_dist = 2500, weight = 1.0, decay = "exponential" },
] }
```

Instead of the list, `threats` can be the name of InVEST's threats table: `threats = "threats.csv"`. It gives the
same maps (checked for this tutorial). Like the `path` of [[raster.read]], the file name is taken from the model's
folder, so the model and its table can move together.

Run it (++ctrl+enter++ on the node). Its outputs:

- `degradation`: the total degradation \(D\) of each cell;
- `threats`: a stack with each threat's share of it, its layers named as the threats;
- `quality`: the habitat quality \(Q\);
- with a future or a base (step 4): `degradation_future`, `quality_future`, `rarity`, `rarity_future`,
  `rarity_table`, and with the base's threat maps `degradation_base` and `quality_base`.

### Step 3: protection as access

The park in the north is strictly protected and its buffer zone partly. Read `access.gpkg` with [[vector.read]]
and connect its `vector` to the input `access`. Cells outside the polygons get access 1 (no protection). The
input also takes a raster of access from 0 to 1, if you have one.

```tfgraph
#! caption: Access polygons connected to the model; k set to 0.35 (see Settings that change the answer).
#! highlight: access, habitat
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "lulc"
op = "raster.read"
params = { path = "lulc_2020.tif" }

[[nodes]]
id = "sensitivity"
op = "table.read"
params = { path = "sensitivity.csv" }

[[nodes]]
id = "crops"
op = "raster.read"
params = { path = "crops_2020.tif" }

[[nodes]]
id = "roads"
op = "raster.read"
params = { path = "roads_2020.tif" }

[[nodes]]
id = "urban"
op = "raster.read"
params = { path = "urban_2020.tif" }

[[nodes]]
id = "threat_maps"
op = "stack.of"
inputs = { crops = "crops.raster", roads = "roads.raster", urban = "urban.raster" }

[[nodes]]
id = "access"
op = "vector.read"
params = { path = "access.gpkg" }
note = "A park (access 0) and its buffer zone (0.5)."

[[nodes]]
id = "habitat"
op = "services.habitat_quality"
inputs = { lulc = "lulc.raster", sensitivity = "sensitivity.table", threat_maps = "threat_maps.stack", access = "access.vector" }
params = { half_saturation = 0.35, threats = [
  { threat = "crops", max_dist = 800, weight = 0.7, decay = "linear" },
  { threat = "roads", max_dist = 1500, weight = 0.8, decay = "exponential" },
  { threat = "urban", max_dist = 2500, weight = 1.0, decay = "exponential" },
] }
```

### Step 4: the future and the past

For the scenario of 2040, read `lulc_2040.tif` and the three threat maps of 2040 into a second stack, and
connect them to `future` and `future_threat_maps`; for the rarity, read `lulc_1985.tif` and connect it to `base`.
The future needs its threat maps (the node says so if they are missing); the base does not (without
`base_threat_maps` it only gives the rarity).

```tfgraph
#! caption: The full model: 2020, the scenario of 2040 with its own threats, and 1985 as the base for rarity. The threat readers are hidden in two stacks to keep the drawing small, and the threats come from InVEST's table (`threats.csv`, beside the model).
#! highlight: habitat
#! numbered: true
#! fragment: true
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
id = "base"
op = "raster.read"
params = { path = "lulc_1985.tif" }

[[nodes]]
id = "sensitivity"
op = "table.read"
params = { path = "sensitivity.csv" }

[[nodes]]
id = "threats_2020"
op = "stack.of"
note = "crops_2020, roads_2020, urban_2020"

[[nodes]]
id = "threats_2040"
op = "stack.of"
note = "crops_2040, roads_2040, urban_2040"

[[nodes]]
id = "habitat"
op = "services.habitat_quality"
inputs = { lulc = "lulc.raster", sensitivity = "sensitivity.table", threat_maps = "threats_2020.stack", future = "future.raster", future_threat_maps = "threats_2040.stack", base = "base.raster" }
params = { half_saturation = 0.35, threats = "threats.csv" }
```

## Read the results

### Degradation, threat by threat

```tfgallery
columns: 3
caption: "The degradation each threat causes (output threats), on one scale. Crops touch most of the forest a little; the roads cut a band through the middle; the town degrades the south. Classes that are not habitat (cropland, the town, water) have sensitivity 0 and no degradation."
images:
  - { src: /img/tutorials/ecosystem-services/habitat-quality/deg_crops.png, caption: "crops" }
  - { src: /img/tutorials/ecosystem-services/habitat-quality/deg_roads.png, caption: "roads" }
  - { src: /img/tutorials/ecosystem-services/habitat-quality/deg_urban.png, caption: "urban" }
```

![Total degradation](/img/tutorials/ecosystem-services/habitat-quality/degradation.png "The total degradation D of 2020: the sum of the three. It reaches 0.70 in the forest next to the town and is lowest in the forest of the north-west, far from the roads and the town (a mean of 0.13 against 0.40 in the forest of the south centre). Cropland, the town and water are pale: they are not sensitive to any threat.")

The degradation map is often the more useful of the two for management: it says **which threat** to act on and
**where**. Its values have no units; they matter relative to each other and to \(k\).

### Quality

![Habitat quality with k = 0.05](/img/tutorials/ecosystem-services/habitat-quality/quality_k0.05.png "Habitat quality with InVEST's default k = 0.05: almost the whole forest falls near 0, because most of it has a degradation well above 0.05. Only the least degraded cores keep some quality.")

With the default \(k = 0.05\), nearly every forest cell is "degraded": the map says little. The reason is in the
curve: \(k\) is the degradation at which quality is halved, and in this landscape the degradation of most forest
is several times 0.05. InVEST's guide advises setting \(k\) to **half of the highest degradation** in the
landscape: run once, read the largest value of `degradation`, and run again. Here the highest degradation is 0.70,
so \(k = 0.35\).

![How k shapes the quality](/img/tutorials/ecosystem-services/habitat-quality/saturation.png "The quality of a class with H = 1 against the total degradation, for five values of k. Each curve halves the quality at D = k. The dotted line marks the highest degradation of Ribeirão Claro, 0.70.")

![Habitat quality with k = 0.35](/img/tutorials/ecosystem-services/habitat-quality/quality_k0.35.png "Habitat quality with k = 0.35: the forest far from threats near 1, its edges near crops and the forest along the roads and around the town lower. Pasture reaches at most 0.3, its habitat suitability; cropland, the town and water are 0.")

### Protection

```tfcompare
caption: "Habitat quality (k = 0.35) without access (left) and with the park (solid outline, access 0) and its buffer zone (dashed, access 0.5) on the right. Inside the park no threat degrades the habitat and the quality equals the habitat suitability; in the buffer zone each threat counts half."
before: { src: /img/tutorials/ecosystem-services/habitat-quality/access_without.png, label: "No access" }
after: { src: /img/tutorials/ecosystem-services/habitat-quality/access_with.png, label: "Park and buffer zone" }
```

```tftable
data: /data/tutorials/ecosystem-services/habitat-quality/access.csv
labels: { area: "Area", access: "Access", forest_quality_without: "Mean forest quality without access", forest_quality_with: "With access" }
digits: 3
caption: "The mean quality of the forest inside the park and its buffer zone."
```

Access does not move the threats: a park whose forest is surrounded by crops keeps its quality only because you
told the model the threats do not get in. Give access values you can defend (enforcement, fences, terrain), and
show the result with and without them.

### The scenario of 2040

![Habitat quality in 2040](/img/tutorials/ecosystem-services/habitat-quality/quality_2040.png "Habitat quality in the scenario of 2040 (k = 0.35): less forest, more cropland next to it, a larger town and a new road into the western hills.")

![Change of habitat quality](/img/tutorials/ecosystem-services/habitat-quality/quality_change.png "The change of quality from 2020 to 2040: dark red where forest became pasture or cropland (quality lost), lighter red where the forest remains but is more degraded (the new road in the west, the larger town), blue along the rivers, where the restored forest gains quality.")

```tftable
data: /data/tutorials/ecosystem-services/habitat-quality/scenarios.csv
labels: { year: "Map", forest_ha: "Forest (ha)", mean_quality_forest: "Mean quality of the forest", habitat_quality_ha: "Quality-weighted habitat (ha)" }
digits: 3
caption: "The scenario loses 31% of the forest and the remaining forest loses 11% of its mean quality. The quality-weighted habitat (the sum of quality times the area of the cells, over every habitat class) falls by a third."
```

Two numbers describe a scenario better than one: the **area** of habitat and its **mean quality**. The
quality-weighted area (each cell's quality times its area, summed) combines them into one and is a common
summary in InVEST studies.

### Rarity

```tftable
data: /data/tutorials/ecosystem-services/habitat-quality/rarity.csv
labels: { scenario: "Map", class: "Class", rarity: "Rarity against 1985" }
digits: 3
caption: "The output rarity_table: 0.5 means the class has the same area as in 1985, more than 0.5 that it has shrunk, less that it has grown. Forest becomes rarer (0.593 in 2020, 0.679 in 2040), cropland and the town commoner; eucalyptus did not exist in 1985 and gets 0."
```

![Rarity of 2020 against 1985](/img/tutorials/ecosystem-services/habitat-quality/rarity.png "The rarity map of 2020 (output rarity): each cell takes the rarity of its class. The forest, the class that has lost the most since 1985, is the lightest (0.59); the plantations, absent in 1985, are 0.")

Rarity is a property of classes, not of places: every cell of a class has the same value. It is useful as a flag
(which habitats are disappearing) and as a weight beside quality, not as a map of where to act.

!!! paper "Reporting habitat quality"
    Report the threats with their maximum distances, weights and decays, the sensitivity table, the value of
    \(k\) and how you chose it, and the access values. Give the area of habitat, its mean quality and the
    quality-weighted area for each scenario, the maps of degradation and of the change of quality, and say that
    the index is relative.

## Settings that change the answer

### The half-saturation constant

\(k\) does not change the ranking of cells (a more degraded cell always has a lower quality), but it changes
every number you will report:

```tfslider
label: Half-saturation constant k
caption: "Habitat quality of 2020 for five values of k. Small values call almost everything degraded; large ones almost nothing. Keep the same k for every scenario you compare."
frames:
  - { value: 0.05, src: /img/tutorials/ecosystem-services/habitat-quality/quality_k0.05.png, stats: { "Mean forest quality": "0.051", "Forest below 0.5": "100%" } }
  - { value: 0.1, src: /img/tutorials/ecosystem-services/habitat-quality/quality_k0.1.png, stats: { "Mean forest quality": "0.204", "Forest below 0.5": "95.2%" } }
  - { value: 0.2, src: /img/tutorials/ecosystem-services/habitat-quality/quality_k0.2.png, stats: { "Mean forest quality": "0.511", "Forest below 0.5": "45.4%" } }
  - { value: 0.35, src: /img/tutorials/ecosystem-services/habitat-quality/quality_k0.35.png, stats: { "Mean forest quality": "0.757", "Forest below 0.5": "13.0%" } }
  - { value: 0.5, src: /img/tutorials/ecosystem-services/habitat-quality/quality_k0.5.png, stats: { "Mean forest quality": "0.867", "Forest below 0.5": "2.3%" } }
start: 3
```

Choose \(k\) once, from the current map, and keep it for the future and the past: a \(k\) recomputed for each
scenario would hide the change you want to see.

### The threats

The maximum distances and the weights set how far and how strongly each threat reaches. Doubling the reach of
the town (to 5 km) would carry its degradation over most of the map, which is 5.4 km from south to north; making roads linear instead of
exponential would degrade more forest at medium distances. These are the numbers to justify from the literature
(how far does hunting go from a road in your region? how far do edge effects of crops reach?) and to test: run the
model with each changed by, say, half, and see which conclusions survive.

## Common pitfalls

- **Names that do not match.** The threat names must be the same in the threats (list or table), the layers of
  the stack, and the columns of the sensitivity table. A missing column stops the model with "the sensitivity
  table lacks the column(s) ...", a missing layer with "no current map of the threat ...".
- **Threat maps with intensities.** A road map with 0.3 for dirt roads and 1 for highways does not weaken the dirt
  roads in InVEST 3.20: both count fully. Make two threats instead (`highways`, `dirt_roads`) with their own
  weights.
- **Weights out of range.** Weights are from 0 to 1 and relative to each other; the model divides each by their
  sum.
- **A maximum distance shorter than a cell.** The model works in cells; a threat of 20 m on a 30 m grid reaches
  only its own cells.
- **The default k.** 0.05 is rarely right for your landscape; check the highest degradation.
- **Comparing studies.** Quality depends on the tables and on \(k\): a quality of 0.6 in one study and 0.8 in
  another say nothing about which landscape is in better shape.

## Going further

- [Annual water yield](/tutorials/ecosystem-services/water-yield/), the next part.
- [Carbon storage and sequestration](/tutorials/ecosystem-services/carbon/) for the same scenario in tons of
  carbon.
- [Ecological intactness](/tutorials/distance-restoration/ecological-intactness/), another index of how intact a
  landscape is, from the distance to human pressures.
- [Fragmentation](/tutorials/fragmentation/fragmentation-and-habitat-loss/) and
  [MSPA](/tutorials/habitat-patterns/mspa/) for the shape of the habitat that remains.
- The operator's reference: [[services.habitat_quality]], [[stack.of]].

## References

- Natural Capital Alliance. (2026). *InVEST 3.20.2 User's Guide: Habitat quality*. https://storage.googleapis.com/releases.naturalcapitalproject.org/invest-userguide/latest/en/habitat_quality.html
- Sallustio, L., De Toni, A., Strollo, A., Di Febbraro, M., Gissi, E., Casella, L., Geneletti, D., Munafò, M., Vizzarri, M., & Marchetti, M. (2017). Assessing habitat quality in relation to the spatial distribution of protected areas in Italy. *Journal of Environmental Management*, 201, 129–137. https://doi.org/10.1016/j.jenvman.2017.06.031
- Terrado, M., Sabater, S., Chaplin-Kramer, B., Mandle, L., Ziv, G., & Acuña, V. (2016). Model development for the assessment of terrestrial and aquatic habitat quality in conservation planning. *Science of the Total Environment*, 540, 63–70. https://doi.org/10.1016/j.scitotenv.2015.03.064
