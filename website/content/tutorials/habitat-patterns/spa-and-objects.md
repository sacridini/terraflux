---
title: Simplified patterns, objects and distances
short: SPA, objects, distances
description: "GuidosToolbox's companions of MSPA on a real forest: SPA's 2 to 6 classes, the forest's objects by size (Accounting), parcellation and the effective mesh size, contortion, the distance to the forest's edge, and the zones of influence of large forests."
order: 30
level: intermediate
time: 50 min
ops: [landscape.spa, landscape.accounting, landscape.parcellation, landscape.contortion, distance.euclidean, distance.influence_zones, distance.objects_proximity]
series: Habitat patterns with GuidosToolbox
part: 3
---

MSPA's seven classes are not always what a question needs. A map for a planning meeting may need three classes,
not twenty codes. A forest report asks how much of the forest is in large blocks and how much in small woods. A
study of edge effects wants the distance of every tree to the nearest field, not a class at one edge width. This
part covers the tools GuidosToolbox offers around MSPA, on the same forest map as
[part 2](/tutorials/habitat-patterns/mspa/): the simplified pattern analysis (SPA), the objects by size
(Accounting), the parcellation of the landscape, the contortion of the patches, the Euclidean distance to the
forest's edge, and the zones of influence and proximity of the large forests. All of them read the same binary
map, so they hang off one [[raster.read]] node side by side.

## The idea

All the tools of this page look at the same thing, a map of foreground (forest) and background, but each asks a
different question of it:

| Question | Tool | What it gives |
|---|---|---|
| Interior, edge and fragments, in fewer classes? | [[landscape.spa]] | 2, 3, 5 or 6 classes, porosity |
| How is the forest shared between small and large pieces? | [[landscape.accounting]] | objects in size classes |
| How finely is the landscape divided? | [[landscape.parcellation]] | parcels, mean sizes, division |
| How convoluted is each piece's outline? | [[landscape.contortion]] | a value per patch |
| How far is every cell from the forest's edge? | [[distance.euclidean]] | a signed distance |
| Which part of the land is closest to which large forest? | [[distance.influence_zones]] | zones and the lines between them |
| Where are two large forests closest? | [[distance.objects_proximity]] | gaps and connected area gains |

Three ideas run through them.

**An object** is a group of foreground cells connected to each other (by their sides, or also by their corners
with 8-connectivity): what an ecologist calls a *patch*. Counting objects and measuring them is the oldest way to
describe fragmentation, and still one of the most useful: is most of the forest in a few large blocks or spread
over many small woods?

**Division.** Jaeger (2000) proposed to measure how divided a landscape is by a probability: pick two points at
random; how likely is it that they are *not* in the same patch?

!!! science "Key idea: division and the effective mesh size"
    For patches of areas \(a_1, a_2, \ldots, a_n\) in a region of area \(A\):

    \[ \text{DIVISION} = 1 - \sum_{i=1}^{n} \left( \frac{a_i}{A} \right)^2 \qquad m_\text{eff} = \frac{1}{A} \sum_{i=1}^{n} a_i^2 \]

    DIVISION is the probability that two random points of the region fall in different patches. The *effective
    mesh size* \(m_\text{eff}\) is the size of the patch a random point falls in, on average: the area of the
    patches if the region were cut into equal ones with the same DIVISION. It grows when patches merge and shrinks
    when they are cut, and it is additive across regions, which made it popular in national monitoring of
    fragmentation by roads and settlements (Jaeger, 2000).

**Distance.** The edge width of MSPA is a threshold on a distance: a forest cell is core when it is further than
the edge width from the background. Measuring that distance for every cell gives, in one map, the answer for every
edge width at once.

## When to use it

- **SPA** when the audience is not specialist, or when only interior versus the rest matters. SPA5 and SPA6 are
  close to MSPA without the bridge, loop and branch distinction; SPA2 and SPA3 are the bluntest summaries.
- **Accounting** to report the forest by patch size classes. FAO's global analysis of forest fragmentation for
  *The State of the World's Forests 2020* reported the world's forest in six size classes, from under 100 ha to
  over 1,000,000 ha, with exactly this method, alongside the forest area density of the
  [fragmentation tutorials](/tutorials/fragmentation/) (Vogt et al., 2019; FAO & UNEP, 2020). Thresholds can come
  from species' area requirements: the minimum area for a territory, or for a viable population.
- **Parcellation** to compare how finely divided different landscapes, classes or dates are.
- **Contortion** to find the patches with the most convoluted outlines: much edge for their area, more exposed to
  edge effects, and often shaped by human use.
- **The Euclidean distance** when edge effects decline gradually with distance rather than stopping at a depth, or
  to model a response (bird density, temperature, carbon stock) as a function of the distance to the edge;
  Chaplin-Kramer et al. (2015) found the biomass of tropical forests 25% lower within 500 m of an
  edge than in their interiors, and still 10% lower 1.5 km in.
- **Influence zones and proximity** to see which large forest every part of the land is closest to, and where the
  gaps between large forests are narrowest: a first look for restoring links.

## What you need

GuidosToolbox's sample forest map, [`input.tif`](/data/tutorials/habitat-patterns/mspa/input.tif), as in part 2
(0 missing, 1 background, 2 forest). In the application, the template **Patterns and objects**
(**Templates > Patterns (GuidosToolbox)**) has SPA and Accounting on a forest drawn by chance; replace its first
node by a [[raster.read]] of your map.

## Build the model

```tfgraph
#! caption: One forest map read once, and six of GuidosToolbox's tools on it side by side.
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "forest"
op = "raster.read"
params = { path = "input.tif" }
note = "0 missing, 1 background, 2 forest."

[[nodes]]
id = "spa"
op = "landscape.spa"
inputs = { map = "forest.raster" }
params = { variant = 6 }
note = "Core, islet, edge, perforation, margin."

[[nodes]]
id = "objects"
op = "landscape.accounting"
inputs = { map = "forest.raster" }
params = { thresholds = [50, 250, 1000, 5000, 20000], highlight_largest = true }
note = "The forest's objects in six size classes."

[[nodes]]
id = "parcels"
op = "landscape.parcellation"
inputs = { map = "forest.raster" }
note = "Parcels of the forest and of the background."

[[nodes]]
id = "contortion"
op = "landscape.contortion"
inputs = { map = "forest.raster" }
params = { min_area = 100 }

[[nodes]]
id = "distance"
op = "distance.euclidean"
inputs = { map = "forest.raster" }

[[nodes]]
id = "zones"
op = "distance.influence_zones"
inputs = { map = "forest.raster" }
params = { min_area = 5000 }
```

Every tool takes the same input, `forest.raster`: connect node 1's output `map` to the input `map` of each. They do not
depend on each other, so Terraflux runs them in parallel. The tables they give (`stats`, `objects`, `summary`) show
in the Viewer when the node is selected, and in a model run from the command line they are written as CSV
files when you make them outputs of the model:

```toml
[outputs.objects_by_size]
from = "objects.stats"
```

Each tool also takes `foreground` (the classes of the forest in a map of many classes), like MSPA, except
parcellation, which works on every class of the map at once.

## SPA: the pattern in fewer classes

[[landscape.spa]] is a simplified MSPA of the 8-connected foreground with edges one cell wide, in four variants
(`variant`):

- **SPA2**: *contiguous* foreground (17, the objects that have a core) and *small linear features* (1, the rest).
- **SPA3**: core (17), core-opening (100) and *margin* (1, everything that is not core).
- **SPA5**: core, edge (3), perforation (5) and margin (1, bridges, loops and branches together, and islets).
- **SPA6**: SPA5 with the islets (9) apart from the margin.

```tfgallery
columns: 4
caption: "The zoom of part 2 in SPA's four variants (core green, margin orange, edge black, perforation blue, islet brown, core-opening dark grey). From SPA2 to SPA6 the forest is told apart in more and more detail; the core is the same in SPA3, 5 and 6, and within a few cells of MSPA's."
images:
  - { src: /img/tutorials/habitat-patterns/spa-and-objects/spa2.png, caption: "SPA2" }
  - { src: /img/tutorials/habitat-patterns/spa-and-objects/spa3.png, caption: "SPA3" }
  - { src: /img/tutorials/habitat-patterns/spa-and-objects/spa5.png, caption: "SPA5" }
  - { src: /img/tutorials/habitat-patterns/spa-and-objects/spa6.png, caption: "SPA6" }
```

On the whole map, SPA6 gives: core 75.1% of the forest, edge 13.5%, margin 6.0%, islet 3.3% and perforation 2.1%.
SPA2 puts 90.8% of the forest in contiguous objects and 9.2% in small linear features. SPA also reports the
**porosity**, \(100 - 100 \times \text{contiguous} / (\text{contiguous} + \text{core-openings})\): the share of holes in
the forest's interior, here 1.4% (SPA5 and SPA6). A forest pierced by clearings has a high porosity.

The `objects` table of SPA counts the objects of each class as GuidosToolbox does: here 2,850 forest objects, of
which 2,443 islets.

### SPA in Python

[`tf.spa`][terraflux.spa] takes the map and the variant (2, 3, 5 or 6; the default is 5). The map is the part's
`input.tif`, shipped as example data:

```python
import terraflux as tf

forest = tf.load_raster(tf.example_data("input"))   # 0 missing, 1 background, 2 forest
spa = tf.spa(forest, variant=6)
print(spa.stats[["class", "code", "area", "pct_fg"]].round(1).to_string(index=False))
print(f"porosity {spa.porosity:.1f}%")
print(spa.counts)
```

```text
      class  code   area  pct_fg
       Core    17 321767    75.1
      Islet     9  14062     3.3
       Edge     3  58008    13.5
Perforation     5   9124     2.1
     Margin     1  25529     6.0
porosity 1.4%
{'foreground': 2850, 'core': 1196, 'margin': 4396, 'core_opening': 707, 'edge': 870, 'perforation': 422, 'islet': 2443, 'contiguous': 847}
```

`spa.map` is the class map (`tf.SPA_CLASSES` names its codes), `spa.stats` the areas, `spa.porosity` the
porosity, `spa.counts` the number of objects of each class (2,850 forest objects, 2,443 of them islets) and
`spa.areas` the areas in cells, with the openings. `print(spa.summary())` is a full printable report;
`tf.spa(forest, variant=2).stats` has the two classes of SPA2 (90.8% contiguous, 9.2% small linear features).

## Accounting: the forest's objects by size

[[landscape.accounting]] labels every object of the forest, measures it in cells and puts it in one of up to six
size classes. The `thresholds` are the upper bounds of the classes in cells: with the default
`[50, 250, 1000, 5000, 20000]` (GuidosToolbox's batch), class 1 holds the objects of up to 50 cells, class 2 those
of 51 to 250, and so on, class 6 those above 20,000. `thresholds = "auto"` proposes bounds at 0.1, 1, 10, 50 and
90% of the range of the objects' areas, as GuidosToolbox does. `highlight_largest` paints the three largest objects
apart.

![The forest's objects by size](/img/tutorials/habitat-patterns/spa-and-objects/accounting.png "The forest's objects in six size classes (cells), in GuidosToolbox's colours, the three largest in magenta. Those three are all of class 6 here, which is why green, class 6's colour, does not show.")

```tfchart
type: bar
data: /data/tutorials/habitat-patterns/spa-and-objects/accounting.csv
x: class
y: [objects, area]
labels: { objects: "% of the objects", area: "% of the forest's area" }
colors: { objects: "#a03c00", area: "#3b8a5a" }
xlabel: Object size (cells)
ylabel: "%"
caption: "The signature of a fragmented forest: 93.6% of the objects are of 50 cells or fewer, yet they hold 4.7% of the forest; the three objects above 20,000 cells hold 64.1% of it."
```

The `stats` table reads:

| Size class | Cells | Objects | Forest cells | % of the objects | % of the forest |
|---|---|---|---|---|---|
| 1 | 1–50 | 2,668 | 19,953 | 93.6 | 4.7 |
| 2 | 51–250 | 129 | 13,094 | 4.5 | 3.1 |
| 3 | 251–1,000 | 30 | 13,867 | 1.1 | 3.2 |
| 4 | 1,001–5,000 | 13 | 30,322 | 0.5 | 7.1 |
| 5 | 5,001–20,000 | 7 | 76,569 | 0.2 | 17.9 |
| 6 | over 20,000 | 3 | 274,685 | 0.1 | 64.1 |

This is how most forests in farmed landscapes look: very many small woods and a few large blocks holding most of
the area. The two halves of the table answer different questions. The large objects hold the interior habitat and
most of the forest's connectivity; the small ones are numerous stepping stones, and their number tells you how
scattered the forest outside the blocks is. Report both columns.

!!! paper "In the literature"
    For *The State of the World's Forests 2020*, FAO and the Joint Research Centre ran Accounting on a global forest
    map at 100 m with patches defined by 4-connectivity, in six classes with upper bounds of 100, 1,000, 10,000,
    100,000 and 1,000,000 hectares (Vogt et al., 2019). To use classes in hectares, divide by the area of a cell:
    with cells of 100 m (1 ha) the thresholds are the hectares themselves; with cells of 30 m (0.09 ha), 100 ha is
    1,111 cells.

### Accounting in Python

```python
acc = tf.accounting(forest, thresholds=(50, 250, 1000, 5000, 20000), highlight_largest=True)
print(acc.stats[["size_class", "range", "objects", "area", "pct_objects", "pct_area"]].round(1).to_string(index=False))
```

```text
 size_class        range  objects   area  pct_objects  pct_area
          1       1 - 50     2668  19953         93.6       4.7
          2     51 - 250      129  13094          4.5       3.1
          3   251 - 1000       30  13867          1.1       3.2
          4  1001 - 5000       13  30322          0.5       7.1
          5 5001 - 20000        7  76569          0.2      17.9
          6    20001 ->         3 274685          0.1      64.1
```

This is the part's table. The thresholds are the upper bounds of the classes in cells (one to five of them);
`thresholds="auto"` proposes them from the range of the objects' areas, as GuidosToolbox does. `connectivity` is 8
(4 for the FAO report of the paragraph above), and `pixel_resolution` (metres) sets the hectares of the report.
The result also holds the maps `acc.classes` (the size class of each cell, with the colours of
`tf.accounting_colormap()`), `acc.ids` (the object numbers) and `acc.area` (the area of the object under each
cell), `acc.object_area` (the area of every object), `acc.largest` (the three largest) and `acc.summary()`, a
printable report in hectares. `tf.plot(acc)` draws the size classes, their chart and the numbers.

## Parcellation: how divided is the landscape?

[[landscape.parcellation]] works on every class of the map: each connected region of one value is a *parcel*.
Here, the forest's parcels and the background's. Its `stats` table gives, per class:

| | Forest (2) | Background (1) | Whole map |
|---|---|---|---|
| Parcels | 2,850 | 1,170 | 4,020 |
| Average parcel size, APS (cells) | 150 | 488 | 249 |
| Area-weighted APS, AWAPS (cells) | 114,286 | 458,001 | 310,683 |
| AWAPS over the area with data (cells) | 48,984 | 261,699 | |
| Division | 0.733 | 0.198 | 0.689 |
| PARC (%) | 9.6 | 1.6 | 8.5 |

How to read it:

- The **APS** (the average parcel size) of the forest is small, 150 cells, because the many small woods weigh as
  much as the large blocks. The **AWAPS** weights each parcel by its own area: it is the size of the parcel that a
  forest cell picked at random belongs to, on average. 114,286 cells: a forest cell is usually in a large block.
- **AWAPS over the area with data** divides by the whole map rather than by the forest: this is Jaeger's
  (2000) effective mesh size of the forest, 48,984 cells.
- **Division** 0.733 for the forest: two forest cells picked at random are in different parcels three times in
  four. The background (fields, villages) is much less divided (0.198): it is one large connected matrix.
- **PARC**, GuidosToolbox's degree of parcellation, summarises the same thing on a logarithmic scale: higher is more
  finely parcelled.

Parcellation is most useful to compare: two dates, two regions, two classes. The effective mesh size and the
division of the forest are also among the class metrics of the [landscape metrics](/tutorials/landscape-metrics/).

### Parcellation in Python

```python
parcels = tf.parcellation(forest)
print(parcels.stats.to_string(index=False))
print({k: round(v, 3) for k, v in parcels.global_stats.items()})
```

```text
 Class  Count  Area[pixels]    APS     AWAPS  AWAPS/data  DIVISION  PARC[%]
     1   1170        571240 488.24 458001.44   261699.40    0.1982     1.60
     2   2850        428490 150.35 114286.18    48983.71    0.7333     9.57
{'total_parcels': 4020, 'data_area': 999730, 'aps': 248.689, 'awaps': 310683.114, 'division': 0.689, 'parc': 8.459}
```

`Class` 2 is the forest and 1 the background, as in the part's table (`AWAPS/data` is the area-weighted size over
the area with data: the effective mesh size). `parcels.global_stats` holds the values of the whole map, and
`tf.parcellation(forest, return_labels=True).parcel_labels` is a map of the parcels. `connectivity` is 8 by
default (4: only by a side). The map must be coded 1 and up, with 0 missing: a forest coded 0 and 1 loses its
background (see the pitfalls), so [`tf.reclassify`][terraflux.reclassify] it first.

## Contortion: how convoluted are the outlines?

[[landscape.contortion]] measures how much the outline of each patch turns, relative to its size, for the patches
with a core and of `min_area` cells at least (here 100). Compact patches score low, elongated and dendritic ones
high. Its `display` output scales the values to 3–255 over the map's range for drawing; its `map` keeps
GuidosToolbox's values (0 missing, 1 background, 2 forest left out, from 3 the contortion plus one).

![Contortion of the forest's patches](/img/tutorials/habitat-patterns/spa-and-objects/contortion-zoom.png "Contortion in the zoom of part 2, on GuidosToolbox's scale from the least (red) to the most contorted (green) patch of the map. Dark grey: forest left out (no core, or under 100 cells); light grey: background. The small compact woods are the least contorted; the large, branching forests more.")

Contortion depends on the resolution (a finer map draws a more ragged outline) and on the size of the patch; compare
patches of similar size, and maps of the same resolution.

### Contortion in Python

```python
contortion = tf.contortion(forest, min_area=100)
print(int(contortion.max()), int((contortion == 2).sum()))
shown = tf.contortion(forest, min_area=100, return_display=True)   # 3-255, to draw
```

```text
42 25318
```

[`tf.contortion`][terraflux.contortion] returns the map with GuidosToolbox's values: 0 missing, 1 background, 2
forest left out (under `min_area` cells, or without a core: here 25,318 cells) and from 3 the contortion plus one,
so 42 is the most contorted patch. `return_display=True` scales the values to 3 to 255, the byte scale of
`tf.contortion_colormap()`.

## The distance to the forest's edge

[[distance.euclidean]] gives every cell its straight-line distance, in cells, to the forest's edge: positive inside
the forest, negative outside it. Its `display` output is GuidosToolbox's byte version (130 plus the distance inside,
130 minus it outside, up to 120 cells), and its `stats` table the mean and largest distance on each side: here a
forest cell is on average 6.0 cells from the edge, and the deepest is 37 cells in; a non-forest cell is on average
5.0 cells from the forest, and the furthest 33.5 cells.

![Distance to the forest's edge](/img/tutorials/habitat-patterns/spa-and-objects/distance-zoom.png "The signed distance to the forest's edge in the zoom of part 2: green inside the forest, the deeper the darker; pink outside it, the further the darker. The colour scale stops at 40 cells.")

The distance map holds every MSPA core at once. The share of the forest at least \(w + 1\) cells from the background
is MSPA's core at an edge width \(w\): we compared it with the MSPA runs of part 2 and the two differ by fewer than
20 cells in 200,000 for every width from 1 to 6. So one map gives the whole curve:

```tfchart
type: area
data: /data/tutorials/habitat-patterns/spa-and-objects/distance-deeper.csv
x: edge_width
y: [forest]
labels: { forest: "Core, % of the forest" }
colors: { forest: "#00c800" }
xlabel: Edge width (cells)
ylabel: "% of the forest"
caption: "The core of the forest for every edge width from 0 to 30 cells, read from the distance map. Half the forest is core at an edge of about 4 cells; a tenth at 13; almost none beyond 25."
```

This is a good figure for a paper: it shows how the amount of interior habitat depends on the depth of edge
effects, without committing to one depth.

### The distance in Python

```python
dist = tf.euclidean_distance(forest, pixel_size=1)   # distances in cells
print(dist.stats[["class", "pixel_count", "mean_distance_units", "max_distance_units"]].to_string(index=False))

# the core of every edge width, from the one distance map: forest cells at least w + 1 from the background
inside = forest.values == 2
for w in (1, 2, 6):
    core = ((dist.signed_distance.values >= w + 1) & inside).sum() / inside.sum() * 100
    print(f"edge_width={w}: {core:.1f}% of the forest")
```

```text
              class  pixel_count  mean_distance_units  max_distance_units
Foreground (Forest)       428490                 6.01               37.00
Background (Matrix)       571240                 5.03               33.54
edge_width=1: 75.1% of the forest
edge_width=2: 61.8% of the forest
edge_width=6: 33.7% of the forest
```

[`tf.euclidean_distance`][terraflux.euclidean_distance] gives `dist.signed_distance` (positive inside the forest,
negative outside, in cells; `pixel_size` makes the table's `_units` columns metres), `dist.byte_map` (the byte
version, 130 plus the distance inside and 130 minus it outside) and the means and maxima as attributes
(`dist.mean_fg_distance`, `dist.max_bg_distance`...). The last lines are the claim of the chart above in four
lines: these are MSPA's core shares for the same edge widths (75.1, 61.8 and 33.7% in [part 2](/tutorials/habitat-patterns/mspa/#the-settings-in-python)) to
within 0.1 point, so one distance map holds the whole curve.

## Zones of influence and proximity

[[distance.influence_zones]] takes the large objects of the forest (8-connected, holes filled, of `min_area` cells
or more: GuidosToolbox's default is 5,000) and divides the whole map among them: every cell goes to the object it is
closest to (the *watershed* of the distance to the objects). The lines between the zones are where two objects are
equally far.

![Zones of influence](/img/tutorials/habitat-patterns/spa-and-objects/zones.png "The ten forest objects of 5,000 cells or more (dark green, holes filled) and their zones of influence (one colour each). The black lines are where two zones meet. The large southern forest claims the whole south of the map.")

[[distance.objects_proximity]] follows those lines and measures, along them, the gap between the two objects and
the connected area gain (CAG) of joining them. On this map it finds 18 pairs of neighbouring objects, and its `lines` table
lists, for every cell of the lines, the two objects, their areas, the connector length and the CAG; sorted by the
connector length, the narrowest gaps come first, here a few cells wide. Such short gaps between large forests are the first places to look when planning to
restore a link, before the more detailed tools of the
[restoration planner](/tutorials/distance-restoration/).

!!! warning "Experimental"
    Influence zones and proximity are marked experimental in terraflux-py, the library Terraflux uses: GuidosToolbox's
    distance transform is not yet reproduced exactly, so lines may differ from GuidosToolbox's by a cell here and
    there.

### Zones of influence and proximity in Python

```python
zones = tf.influence_zones(forest, min_area=5000)
print(zones.n_objects, zones.object_area)

prox = tf.proximity(forest, min_area=5000)
lines = prox.lines.sort_values("clength")      # one row per cell of the lines between the zones
two = lines[lines["nbr"] == 2]                  # nbr: how many objects meet in the cell
print(len(lines), "cells on the lines,", len({tuple(i) for i in two["ids"]}), "pairs of objects")
print(lines.head(3).to_string())
```

```text
10 [214811   7927   7398  13850  33508  13303  26366  14983   6471  12637]
1426 cells on the lines, 18 pairs of objects
         x    y  nbr  clength    cag      ids           areas
1343  1000   77    2        2  25814  [7, 10]  [26366, 12637]
1342   999   77    2        2  25814  [7, 10]  [26366, 12637]
344    851  549    2        4  22266   [3, 5]   [7398, 33508]
```

[`tf.influence_zones`][terraflux.influence_zones] gives `zones.zones` (every cell in the zone of its nearest object,
the lines between the zones apart), `zones.ids` and `zones.object_area`: the ten objects of 5,000 cells or more of the
part (the other, smaller forest is not an object). [`tf.proximity`][terraflux.proximity] gives the same and the
`lines` table: for every cell of a line, the two objects (`ids`) and their areas, the connector length `clength`
and the connected area gain `cag`; sorted by `clength` the narrowest gaps come first, here two cells wide between
objects 7 and 10. `core_buffer` reduces the objects to their cores first, `buffer` draws a background buffer around
them, and `prox_max` marks the lines of a proximity up to that value. Both are experimental, as the warning above
says.

## Settings that change the answer

- **Connectivity.** Accounting, parcellation and SPA's object counts depend on it: with 4 neighbours, woods that
  touch at a corner are separate objects, and the counts of small objects grow.
- **Thresholds of Accounting.** Classes in cells change meaning with the resolution; set them in hectares and
  convert.
- **`min_area`** of contortion and of the zones: which patches count as large enough.
- **The map's extent.** Every object cut by the map's border is smaller than it really is; the largest objects of
  a map are the most likely to be cut.

## Common pitfalls

- **Comparing APS between maps.** The average patch size is dominated by the number of small patches, which
  depends on the map's detail; prefer AWAPS or the effective mesh size.
- **Cells versus hectares.** All these tools count cells. Multiply by the area of a cell, and on a map in degrees
  (like this sample) project first: cells of degrees have different areas at different latitudes.
- **Parcellation of a binary map coded 0 and 1.** Parcellation reads the classes 1 to 255 and takes 0 as missing:
  a forest map coded 0 and 1 would lose its background. Reclassify first.
- **Distances in cells.** The Euclidean distance is in cells; multiply by the cell size for metres.

## Going further

- [Part 4](/tutorials/habitat-patterns/networks-and-change/): MSPA's cores and bridges as a network, change
  between two dates, maps larger than memory.
- [Landscape metrics](/tutorials/landscape-metrics/): patch, class and landscape metrics, many of them close
  relatives of these (core area, mesh size, shape).
- [Distance and restoration](/tutorials/distance-restoration/): the restoration planner, least-cost corridors and
  isochrones.
- The operator reference: [[landscape.spa]], [[landscape.accounting]], [[landscape.parcellation]],
  [[landscape.contortion]], [[distance.euclidean]], [[distance.influence_zones]],
  [[distance.objects_proximity]].

## References

- Chaplin-Kramer, R., Ramler, I., Sharp, R., Haddad, N. M., Gerber, J. S., West, P. C., Mandle, L., et al. (2015). Degradation in carbon stocks near tropical forest edges. *Nature Communications*, 6, 10158. https://doi.org/10.1038/ncomms10158
- FAO & UNEP. (2020). *The State of the World's Forests 2020: Forests, biodiversity and people*. FAO. https://doi.org/10.4060/ca8642en
- Jaeger, J. A. G. (2000). Landscape division, splitting index, and effective mesh size: new measures of landscape fragmentation. *Landscape Ecology*, 15(2), 115–130. https://doi.org/10.1023/A:1008129329289
- Soille, P., & Vogt, P. (2009). Morphological segmentation of binary patterns. *Pattern Recognition Letters*, 30(4), 456–459. https://doi.org/10.1016/j.patrec.2008.10.015
- Vogt, P., & Riitters, K. (2017). GuidosToolbox: universal digital image object analysis. *European Journal of Remote Sensing*, 50(1), 352–361. https://doi.org/10.1080/22797254.2017.1330650
- Vogt, P., Riitters, K. H., Caudullo, G., & Eckhardt, B. (2019). *FAO – State of the World's Forests: Forest fragmentation* (EUR 29972 EN). Publications Office of the European Union. https://doi.org/10.2760/145325
- Vogt, P., Riitters, K., Rambaud, P., d'Annunzio, R., Lindquist, E., & Pekkarinen, A. (2022). GuidosToolbox Workbench: spatial analysis of raster maps for ecological applications. *Ecography*, 2022(3), e05864. https://doi.org/10.1111/ecog.05864
