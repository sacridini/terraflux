---
title: Distance to habitat
short: Distance to habitat
description: Measure how far every cell of a landscape lies from a forest and from its edge, read what those distances say about edge effects and natural regeneration, and split the open land into the zones of the forests nearest to it.
order: 10
level: beginner
time: 40 min
ops: [distance.euclidean, distance.proximity, distance.influence_zones, raster.reproject, map.calc, raster.read]
series: Distance and restoration
part: 1
---

In this first part of the series you measure distances in a real forest landscape of south-western Oregon. You
turn a satellite image of greenness into a forest map, measure how far every forest cell lies from the forest's
edge and how far every open cell lies from the nearest forest, and divide the open land among the large forests
nearest to it. At the end you will have three maps and two curves that answer everyday questions of restoration
and conservation: how much of the forest is exposed to edge effects, how much of the cleared land is close enough
to the forest to regrow by itself, and which forest each part of the matrix "belongs" to.

## The idea

Distance is the simplest spatial variable there is, and one of the most useful in ecology. Many processes fade
with distance from a habitat's edge:

- **Inside a forest**, light, wind, temperature and humidity change near the edge, and so do the plants and
  animals that live there. These *edge effects* reach tens of metres for microclimate and up to several hundred
  metres for some species (Murcia, 1995). The distance of a forest cell to the edge says how deep in the
  interior it lies.
- **Outside a forest**, most seeds fall close to their parent trees or are carried short distances by birds and
  bats. The chance that cleared land regrows naturally falls quickly with distance from the forest, so land near
  forest can often be restored by letting it regrow, at a fraction of the cost of planting (Chazdon & Guariguata,
  2016). The distance of an open cell to the nearest forest says how close it is to a seed source.

A *raster* is a map made of a grid of square *cells*, each holding one number. A **distance map** is a raster in
which each cell holds the straight-line (Euclidean) distance from its centre to the nearest cell of some kind.
GuidosToolbox, the image analysis software of the European Commission's Joint Research Centre (Vogt & Riitters,
2017), computes it on both sides of an edge at once and gives it a sign: positive inside the habitat, negative
outside it.

![A toy map of 8 by 10 cells: forest in green, open land in beige, and in each cell its signed distance to the edge in cells](/img/tutorials/distance-restoration/distance-to-habitat/toy.png "The signed distance cell by cell. Each forest cell (green) holds its distance to the nearest open cell; each open cell (beige) the distance to the nearest forest cell, with a minus sign. Cells next to the edge are at 1; a diagonal neighbour is at √2 ≈ 1.4. Multiply by the cell's side (30 m here) for metres.")

!!! science "Key idea"
    For a cell \(i\) of the habitat \(H\), the signed distance is

    \[ d_i = \min_{j \notin H} \lVert c_i - c_j \rVert \]

    and for a cell outside it \( d_i = -\min_{j \in H} \lVert c_i - c_j \rVert \), where \(c_i\) is the centre
    of cell \(i\) (in cells) and \(\lVert \cdot \rVert\) the straight-line distance. A cell on the edge is at 1
    (its neighbour across the edge is one cell away); multiplied by the cell's side it is in metres.

Two related tools answer "which forest is nearest?". A **zone of influence** is the part of the open land nearer
to one forest object than to any other, the raster version of a Voronoi diagram (or of Thiessen polygons around
points). GuidosToolbox draws the zones as the basins of the distance map: the lines between zones run where two
forests are equally far.

## When to use it

- **Edge exposure.** The share of a forest within 50 or 100 m of its edge is a direct, easily explained
  indicator of edge effects, and the distance map is a covariate for species that avoid or seek edges.
- **Natural regeneration potential.** Studies of the Atlantic Forest and other tropical regions map land within
  a few hundred metres of forest as the cheapest to restore (Chazdon & Guariguata, 2016). The distance to the
  nearest forest is the first layer of such a map.
- **Drivers of land change.** Distance to roads, rivers, towns or existing farmland is among the most common
  explanatory variables of deforestation and land change models; [[distance.proximity]] computes it for any
  set of values.
- **Buffer zones and catchments.** The zones of influence assign every part of the matrix to its nearest
  reserve, for planning buffer zones or for sharing the management of the land between them.

What it is *not* for: a straight line ignores what lies between two places. When the land in between is easier
or harder to cross (a river, a highway, a pasture), use an accumulated cost instead: the
[next part](../costs-and-corridors/) does, and so do the [grid distances](/tutorials/#grid-distances) and
[circuit connectivity](/tutorials/#circuit-connectivity) tutorials. Distances also say nothing about how
connected the forest is as a whole: for that see [habitat networks](/tutorials/#habitat-networks).

## What you need

- **A habitat map.** Here, `ndvi.tif`: summer greenness (the normalised difference vegetation index, NDVI, from
  0 for bare ground to about 0.9 for dense canopy) of 371 × 371 cells of about 30 m in south-western Oregon,
  a composite of the summers of 2000 to 2016 from landschaft's example data. Your own map can be a land cover
  classification, a forest mask from Global Forest Watch, or any map of the habitat of a species.
- **A projected coordinate system.** A *coordinate reference system* (CRS) says what the numbers of the grid
  mean on the ground. `ndvi.tif` is in degrees of latitude and longitude (EPSG:4326), and a degree of longitude
  is shorter than a degree of latitude away from the equator: at 43° N its cells are about 31 m tall and 23 m
  wide. Distances need square cells of a known size in metres, so the first step reprojects the map to UTM zone
  10 N (EPSG:32610) with cells of 30 m.

!!! app "In the application"
    The operators of this series are under **Distance & restoration** in the library (the left pane). Click one
    to add it to the graph, drag from an output port (right side of a node) to an input port (left side) to
    connect them, and set its settings in the **Node** pane. ++ctrl+enter++ runs the model up to the selected
    node, and a **view.plot** node shows a map in a tab of its own.

## Build the model

### Read and project the map

Start with a [[raster.read]] node whose `path` is `ndvi.tif`, then a [[raster.reproject]] node. Connect
`ndvi.raster` to `projected.raster` and set `crs` to `EPSG:32610` and `resolution` to `30`. Reprojection
resamples the values to the new grid; for a continuous map like NDVI Terraflux uses bilinear interpolation, a
weighted mean of the four nearest cells (for a map of classes it would take the nearest cell instead).

### Define the forest

GuidosToolbox's operators read maps coded **0 missing, 1 background, 2 foreground**. The *foreground* is the
habitat whose distances you measure, the *background* everything else on the map, and *missing* the cells
without data (outside the study area, clouds). A [[map.calc]] node makes that code from the NDVI:

```text
if isnull(n) then 0 else if n >= 0.7 then 2 else 1
```

Its input `n` is `projected.raster`. Cells without a value (the corners that the reprojection leaves empty)
become 0, cells with an NDVI of 0.7 or more forest (2), the rest open land (1). The threshold of 0.7 is a choice,
not a fact: a later section shows how much it matters.

```tfgraph
#! caption: Read the NDVI, project it to metres, and code it as GuidosToolbox expects: 0 missing, 1 open land, 2 forest.
#! highlight: projected, forest
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "ndvi"
op = "raster.read"
params = { path = "ndvi.tif" }
note = "Summer NDVI of south-western Oregon, cells of about 30 m, in degrees."

[[nodes]]
id = "projected"
op = "raster.reproject"
inputs = { raster = "ndvi.raster" }
params = { crs = "EPSG:32610", resolution = 30 }
note = "UTM zone 10 N: square cells of 30 m."

[[nodes]]
id = "forest"
op = "map.calc"
inputs = { n = "projected.raster" }
params = { expression = "if isnull(n) then 0 else if n >= 0.7 then 2 else 1" }
note = "0 missing, 1 open land, 2 forest."
```

```tfgallery
columns: 2
caption: The NDVI after projection (left) and the forest it defines with a threshold of 0.7 (right). Forest covers 41% of the cells with data, in 568 separate pieces.
images:
  - { src: /img/tutorials/distance-restoration/distance-to-habitat/ndvi.png, caption: "Summer NDVI, projected" }
  - { src: /img/tutorials/distance-restoration/distance-to-habitat/forest.png, caption: "NDVI ≥ 0.7: forest" }
```

### The signed distance to the edge

Add [[distance.euclidean]] and connect `forest.map` to its `map` input. With no `foreground` setting it takes the
map's own codes (2 the foreground); for a land cover map with other codes, list the habitat's classes in
`foreground` (for example `[1, 3]`). It has three outputs:

- `map`: the signed distance in cells (positive in the forest, negative outside).
- `display`: GuidosToolbox's byte version of it (130 plus the distance inside, 130 minus it outside, up to 120
  cells), for viewing in GuidosToolbox.
- `stats`: per side (1 the foreground, 0 the background) the number of cells and the mean and largest distance.

### The distance to the nearest forest, in metres

[[distance.proximity]] is GDAL's proximity tool: the distance from every cell to the nearest cell holding some
values, here `values = [2]`. Connect `forest.map` to its `raster` input. Its distances are in the map's units
(metres, after the projection) unless `units = "cells"`, and `max_distance` leaves the cells beyond it without a
value. Unlike [[distance.euclidean]] it works on the file in blocks, whatever its size, so it is the one to use
for a country-wide map of the distance to roads or to forest.

### Zones of influence

Add [[distance.influence_zones]] with `forest.map` as its `map`. It labels the forest's *objects* (groups of
forest cells touching by a side or a corner, holes filled), keeps those of at least `min_area` cells, and divides
the rest of the map among them. GuidosToolbox's default `min_area` of 5,000 cells suits large maps; this one has
about 100,000 cells, so set `min_area = 1000` (90 ha at 30 m). The outputs are `map` (GuidosToolbox's codes),
`ids` (the object of every cell), `zones` (the zone of every cell, 0 on the lines between them) and `objects`
(the area of each object).

```tfgraph
#! caption: The full model: the signed distance to the edge, the distance to the nearest forest in metres, and the zones of influence of the forests of 1,000 cells or more.
#! highlight: distance, to_forest, zones
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "ndvi"
op = "raster.read"
params = { path = "ndvi.tif" }

[[nodes]]
id = "projected"
op = "raster.reproject"
inputs = { raster = "ndvi.raster" }
params = { crs = "EPSG:32610", resolution = 30 }

[[nodes]]
id = "forest"
op = "map.calc"
inputs = { n = "projected.raster" }
params = { expression = "if isnull(n) then 0 else if n >= 0.7 then 2 else 1" }

[[nodes]]
id = "distance"
op = "distance.euclidean"
inputs = { map = "forest.map" }
note = "Signed distance to the edge, in cells."

[[nodes]]
id = "to_forest"
op = "distance.proximity"
inputs = { raster = "forest.map" }
params = { values = [2] }
note = "Metres to the nearest forest cell."

[[nodes]]
id = "zones"
op = "distance.influence_zones"
inputs = { map = "forest.map" }
params = { min_area = 1000 }

[[nodes]]
id = "plot_distance"
op = "view.plot"
inputs = { value = "distance.map" }
params = { title = "Signed distance (cells)" }

[[nodes]]
id = "plot_zones"
op = "view.plot"
inputs = { value = "zones.map" }
params = { title = "Zones of influence" }
```

From the command line, save the model as `distance.toml` next to `ndvi.tif` and run it. To keep a map, add a
[[raster.write]] node, or make a table an output of the model so that `terraflux run` writes it as a CSV:

```toml
[outputs.distance_stats]
from = "distance.stats"
```

```bash
terraflux run distance.toml --out results
```

## Read the results

### Inside and outside the forest

![The signed distance to the forest's edge in metres: green inside the forest, pink outside](/img/tutorials/distance-restoration/distance-to-habitat/signed.png "The signed distance in metres (the map's cells times 30). Green: inside the forest, darker deeper in it; pink: open land, darker further from any forest. White: the edge.")

The map is mostly pale: this is a fine-grained landscape where almost nothing is far from an edge. The deepest
forest cell lies 13 cells (390 m) from open land, and the most remote open cell 13.3 cells (400 m) from forest.
The `stats` table gives the means: forest cells lie on average 2.1 cells (64 m) from the edge, open cells 2.7
cells (81 m) from the forest.

![The distance from every open cell to the nearest forest cell, in metres](/img/tutorials/distance-restoration/distance-to-habitat/to_forest.png "distance.proximity: metres from every cell to the nearest forest cell (0 in the forest). The darkest patches are the open land furthest from any seed source.")

The two maps agree (the proximity map is the negative half of the signed one, in metres), but the proximity map
is the one to give a regeneration model or to threshold ("open land within 100 m of forest").

The most useful summary is a curve: for every distance, the share of each side within it.

```tfchart
type: line
data: /data/tutorials/distance-restoration/distance-to-habitat/reach.csv
x: distance
y: [open, forest]
labels: { open: "Open land within this distance of forest", forest: "Forest within this distance of the edge" }
colors: { open: "#d9a441", forest: "#3b8a5a" }
xlabel: Distance (m)
ylabel: "% of the cells"
percent: true
caption: Cumulative distances from the real run. Half of the open land lies within 60 m of a forest and 96% within 210 m; 64% of the forest lies within 60 m of its edge.
```

Read the curves with an ecological threshold in mind. If edge effects on the microclimate reach about 60 m, then
64% of this forest is edge habitat and only about a third is interior. If natural regeneration is likely within
about 100 m of a seed source, then about three quarters of the open land could regrow without planting (the
real distances depend on the species and the region: take them from the local literature).

!!! paper "In the literature"
    Haddad et al. (2015) computed the distance of every forest point on Earth to the nearest edge and found that
    70% of the world's remaining forest lies within 1 km of an edge, where fragmentation's effects on species and
    ecosystems accumulate. The same distance, on your map, is what [[distance.euclidean]] computes.

### Zones of influence

![The zones of influence of the eight forest objects of 1,000 cells or more](/img/tutorials/distance-restoration/distance-to-habitat/zones.png "Zones of influence: each colour is the part of the map nearer to one of the eight forest objects of 1,000 cells or more (dark green) than to any other; black lines separate them. Smaller forest (pale green) belongs to the zone it lies in.")

Eight forest objects reach 1,000 cells; the largest has 9,508 cells (8.6 km²), the smallest of the eight 1,067.
Each gets the land nearest to it. Large objects next to small ones get large zones, and a zone's line falls
halfway between two objects. Use the `zones` map as you would use Thiessen polygons: to sum the open land, the
people or the pressure that each forest has around it.

!!! warning "Experimental"
    The zones of influence follow GuidosToolbox's method, but its distance transform is not yet reproduced
    exactly, so the lines can differ in detail from GuidosToolbox's own maps (the operator's description says
    so).

### How to report it

Report the habitat definition (the data, the threshold, the cell size and the CRS), then the distances in metres:
the mean and largest distance on each side, and the share of each side within the thresholds that matter for your
question (for example "64% of the forest lies within 60 m of an edge"). The curve above is a good figure for a
paper, and the proximity map a good layer for the next steps of a restoration plan.

## Settings that change the answer

### The definition of the habitat

The threshold that turns NDVI into forest is rarely given by the ecology, and the distances depend on it more
than on anything else. Move the slider: each frame is a real run of the model with another threshold.

```tfslider
label: NDVI threshold of the forest
caption: The signed distance in metres (green inside the forest, pink outside; the colours are fixed from -600 to 600 m) for four thresholds. At 0.6 the forest covers almost everything and its interior is deep; at 0.75 a few scattered stands remain and open land lies up to 1.7 km from them.
frames:
  - { value: 0.60, src: /img/tutorials/distance-restoration/distance-to-habitat/signed_0.6.png, stats: { "Forest": "92%", "Open land, mean distance to forest": "50 m", "Forest, mean distance to edge": "222 m" } }
  - { value: 0.65, src: /img/tutorials/distance-restoration/distance-to-habitat/signed_0.65.png, stats: { "Forest": "80%", "Open land, mean distance to forest": "51 m", "Forest, mean distance to edge": "117 m" } }
  - { value: 0.70, src: /img/tutorials/distance-restoration/distance-to-habitat/signed_0.7.png, stats: { "Forest": "41%", "Open land, mean distance to forest": "81 m", "Forest, mean distance to edge": "64 m" } }
  - { value: 0.75, src: /img/tutorials/distance-restoration/distance-to-habitat/signed_0.75.png, stats: { "Forest": "8%", "Open land, mean distance to forest": "255 m", "Forest, mean distance to edge": "42 m" } }
start: 2
```

Between 0.65 and 0.70 the forest falls from 80% to 41% of the map, because most cells have an NDVI between those
two values: the threshold cuts through the bulk of the distribution. When a result depends this much on a
threshold, report it for two or three plausible values, or use a definition of habitat from field data.

### The least object of the zones

`min_area` decides which forests get a zone. With a larger value fewer, larger zones remain; at 0.75 no object
reaches 1,000 cells and the operator stops with the message that no object is kept. Choose it from the ecology
(the smallest forest that holds a population, or the smallest reserve that counts) rather than from the map.

## Common pitfalls

- **Degrees are not metres.** Distances on a map in latitude and longitude are in degrees, and the cells are not
  square. Project the map first (here to UTM), or give the cells' size where an operator asks for it.
- **Cells, not metres.** [[distance.euclidean]] gives distances in cells; [[distance.proximity]] in the map's
  units. Multiply the first by the cell's side before you compare them or report them.
- **The edge of the map is not an edge of the forest.** A forest cut by the map's border looks smaller and its
  interior shallower than it is, and the open land near the border may be close to a forest just outside. Give
  the analysis a margin around the study area, or code the outside as missing (0), as here.
- **Missing cells.** Clouds or gaps coded as open land create false edges. Code them 0 (missing) in the
  GuidosToolbox coding, as the expression above does for empty cells.
- **Resolution.** At 30 m a hedge or a narrow riparian strip may be forest; at 250 m it disappears. Distances
  to the nearest forest grow as the cells grow, so compare landscapes at the same resolution.

## Going further

- Distances that account for what lies between: [Costs, paths and corridors](../costs-and-corridors/), the next
  part, finds the cheapest route between two forests and what restoring it would gain.
- The edges and cores of the forest as classes: [habitat patterns (MSPA)](/tutorials/#habitat-patterns).
- How fragmented the forest is at many scales: [fragmentation](/tutorials/#fragmentation).
- The operators: [[distance.euclidean]], [[distance.proximity]], [[distance.influence_zones]] and
  [[distance.objects_proximity]], which measures the gap between neighbouring objects along the lines between
  their zones.

## References

- Chazdon, R. L., & Guariguata, M. R. (2016). Natural regeneration as a tool for large-scale forest restoration in the tropics: prospects and challenges. *Biotropica*, 48(6), 716–730. https://doi.org/10.1111/btp.12381
- Haddad, N. M., Brudvig, L. A., Clobert, J., Davies, K. F., Gonzalez, A., Holt, R. D., et al. (2015). Habitat fragmentation and its lasting impact on Earth's ecosystems. *Science Advances*, 1(2), e1500052. https://doi.org/10.1126/sciadv.1500052
- Murcia, C. (1995). Edge effects in fragmented forests: implications for conservation. *Trends in Ecology & Evolution*, 10(2), 58–62. https://doi.org/10.1016/S0169-5347(00)88977-6
- Vogt, P., & Riitters, K. (2017). GuidosToolbox: universal digital image object analysis. *European Journal of Remote Sensing*, 50(1), 352–361. https://doi.org/10.1080/22797254.2017.1330650
