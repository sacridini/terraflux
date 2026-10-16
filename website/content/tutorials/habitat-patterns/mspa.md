---
title: MSPA step by step on a real forest map
short: MSPA on a real map
description: "Run MSPA on two real maps, a land cover map reclassified to forest and GuidosToolbox's sample forest, read its classes and statistics, and see how the edge width and the connectivity change them."
order: 20
level: beginner
time: 50 min
ops: [landscape.mspa, raster.read, raster.reclassify, raster.write, view.plot]
series: Habitat patterns with GuidosToolbox
part: 2
---

In [part 1](/tutorials/habitat-patterns/morphology/) we saw what MSPA does on a forest drawn by hand. Here we run
it on real maps. First a land cover map, CORINE Land Cover around Hannover in Germany, from which we make a forest
map with one reclassification. Then GuidosToolbox's own sample forest map, a forest of central Germany mapped in
fine detail, on which we read every class, compute the statistics you would report in a paper, and move the two
settings that matter, the edge width and the connectivity. At the end you will have a model you can point at your
own map, and you will know how to choose its settings and defend them.

## The idea

MSPA gives every habitat cell a structural role (core, edge, perforation, bridge, loop, branch, islet) from two
things only: the map and the edge width (Vogt et al., 2007a; Soille & Vogt, 2009). Part 1 showed the classes cell
by cell. On a real map two more things come into play.

**The map decides what MSPA can see.** MSPA has no notion of "forest": it sees the cells you mark as foreground. A
narrow riparian strip that the map merged into the fields cannot become a bridge, and a clearing smaller than a
cell cannot become a perforation. The thematic definition of the habitat (which land cover classes) and the
spatial detail of the map (its cell size and its *minimum mapping unit*, the smallest area the mappers drew) shape
the result as much as any setting. Ostapowicz et al. (2008) showed how MSPA's classes of the same forests
change with the cell size, and Wickham and Riitters (2019) how much finer data change the picture of forest
fragmentation.

**The answer is a map and a table.** An MSPA map shows *where* the cores, the corridors and the fragments are; its
statistics (the share of the habitat in each class) summarise *how much*. Both are reported: the map for planning
(which bridge to protect, where to restore), the numbers to compare landscapes or dates.

!!! science "Key idea: an MSPA statistic"
    The share of a class \(k\) in the habitat is

    \[ P_k = 100 \times \frac{n_k}{n_\text{foreground}} \]

    where \(n_k\) is the number of cells of class \(k\) (its transitions and internal features included) and
    \(n_\text{foreground}\) the number of foreground cells. The share of the core, \(P_\text{core}\), is the simplest
    measure of how much interior habitat there is; \(P_\text{bridge}\) and \(P_\text{islet}\) say how much of the
    habitat is in corridors and in isolated fragments. Multiply \(n_k\) by the area of a cell for hectares.

## When to use it

- **Mapping green infrastructure.** Wickham et al. (2010) took MSPA's cores as the *hubs* and its bridges as the
  *links* of the green infrastructure of the United States, from a national land cover map at 30 m, and measured
  what was lost between two dates.
- **Finding corridors and connectors.** Bridges are candidate corridors (Vogt et al., 2007b). Combined with network
  analysis they rank the connectors that matter most for the whole forest (Saura et al., 2011; part 4).
- **Reporting the state of a habitat.** The share of core, of edge and of islets, at a stated edge width, is a
  compact description of a forest that can be compared between regions and dates.
- **Choosing where to look.** An MSPA map is a good first view of a landscape before any species-specific
  analysis: it shows the large cores, the thin links between them and the scattered fragments.

It is not the tool for the amount of habitat around each point at a given scale (that is
[forest area density](/tutorials/fragmentation/)), nor for movement through the matrix
([circuit connectivity](/tutorials/circuit-connectivity/), [grid distances](/tutorials/grid-distances/)).

## What you need

Two real maps, which you can download to follow along:

- [`clc.tif`](/data/tutorials/habitat-patterns/mspa/clc.tif) (290 KB): **CORINE Land Cover**, the European land
  cover map of the Copernicus programme, for a square of 50 × 50 km north-west of Hannover, Germany. 2,000 × 2,000
  cells of 25 m in the European equal-area projection (EPSG:3035). Its values are CORINE's classes numbered 1 to 44:
  1 to 11 artificial surfaces, 12 to 22 agriculture, 23 broad-leaved forest, 24 coniferous forest, 25 mixed forest,
  26 to 34 other natural land, 35 to 39 wetlands, 40 to 44 water. Its minimum mapping unit is 25 ha.
- [`input.tif`](/data/tutorials/habitat-patterns/mspa/input.tif) (70 KB): **GuidosToolbox's sample forest map**,
  1,000 × 1,000 cells of a landscape of central Germany, coded as GuidosToolbox expects: 0 missing (270 cells), 1
  background, 2 forest. Its cells are in degrees of longitude and latitude (0.0003°, about 21 m east-west and 34 m
  north-south): fine for learning, but see the pitfalls below.

Put them in a folder, and save the models of this page in the same folder: paths in a model are relative to the
model's file. In the application, you can start from the template **MSPA of your map**
(**Templates > Patterns (GuidosToolbox)**), which has the four nodes of the first model with an empty path.

## Model 1: from a land cover map to MSPA

```tfgraph
#! caption: Read CORINE Land Cover, make its three forest classes the foreground, run MSPA with an edge of 100 m, show it.
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "landcover"
op = "raster.read"
params = { path = "clc.tif" }
note = "CORINE Land Cover, 25 m cells."

[[nodes]]
id = "forest"
op = "raster.reclassify"
inputs = { map = "landcover.raster" }
params = { rules = "23 24 25 -> 2\n* -> 1" }
note = "Broad-leaved, coniferous and mixed forest become the foreground (2), every other class the background (1)."

[[nodes]]
id = "mspa"
op = "landscape.mspa"
inputs = { map = "forest.map" }
params = { edge_width = 4 }
note = "An edge of 4 cells: 100 m."

[[nodes]]
id = "plot"
op = "view.plot"
inputs = { value = "mspa.map" }
params = { title = "MSPA of CORINE's forest" }
```

<div class="steps" markdown>

1. **Read the map** ([[raster.read]]). Add the node (library, subject **Data preparation**, or press `/` and type
   `read`) and choose `clc.tif` in its `path` (**Node > path > Choose...**). The node reads only the file's header
   at first, so even a large map shows at once. Its output `map` is the first band as a map.
2. **Make the binary map** ([[raster.reclassify]]). Connect `landcover.raster` to its input `map`. In the Node pane the
   rules are a table: the application lists the classes of the map it reads, with their share, and **A rule per
   class** fills the table so that you only type the new values. Here two rules: `23 24 25 -> 2` (the three forest
   classes become the foreground) and `* -> 1` (every other value, the background). The first rule that takes a
   value gives it, so the order matters.
3. **MSPA** ([[landscape.mspa]]). Connect `forest.map` to its input `map`. Set `edge_width` to 4: with cells of
   25 m, an edge of 4 cells is 100 m, a common figure for the depth of edge effects in temperate forests (choose
   yours from the literature on your species, Murcia, 1995; Ries et al., 2004). Leave `connectivity` at 8.
4. **Show it** ([[view.plot]]). Connect `mspa.map`. Press ++f5++ to run.

</div>

!!! tip "Skip the reclassification"
    [[landscape.mspa]] can read the land cover directly: set its `foreground` to `[23, 24, 25]` and connect
    `landcover.raster`. Every other class becomes background. Keep the reclassification when you want to see (and
    show) the forest map itself, or when some classes must be *missing* rather than background (water outside the
    study area, clouds): add a rule `40..44 -> 0`.

```tfgallery
columns: 3
caption: "Model 1. Left, CORINE Land Cover (50 × 50 km around Hannover): half of it arable land, a fifth forest. Middle, the forest made MSPA's foreground (node 2). Right, its MSPA with an edge of 100 m (node 3)."
images:
  - { src: /img/tutorials/habitat-patterns/mspa/clc-landcover.png, caption: "CORINE Land Cover" }
  - { src: /img/tutorials/habitat-patterns/mspa/clc-forest.png, caption: "Node 2: the forest" }
  - { src: /img/tutorials/habitat-patterns/mspa/clc-mspa.png, caption: "Node 3: MSPA, edge 100 m" }
```

The forest covers 20.6% of the map. Its MSPA is almost all core (70.4% of the forest) and edge (22.9%), with a
few branches (4.7%) and bridges (2.0%), almost no islets (0.07%) and no perforation at all. Is this forest really
so compact? No: the map is. CORINE does not draw patches smaller than 25 ha nor strips narrower than about 100 m,
so the hedgerows, riparian strips and small woods that make bridges, branches and islets are simply not in it.
With an edge of 100 m, any strip CORINE draws is wide enough to hold core or just barely too narrow.

This is the first lesson of real data: **MSPA can only see what the map shows**. CORINE is good for the large
cores of a region and for comparing regions across Europe; for corridors, use a map of finer detail, such as a
forest mask from Sentinel-2 or Landsat imagery or a national forest map.

## Model 2: MSPA of a detailed forest map

GuidosToolbox's sample map is such a map. It is coded 0, 1 and 2 already, so MSPA reads it as it is.

```tfgraph
#! caption: MSPA of GuidosToolbox's sample forest map, shown and written to a GeoTIFF.
#! numbered: true
#! highlight: write
format = "terraflux/1"

[[nodes]]
id = "forest"
op = "raster.read"
params = { path = "input.tif" }
note = "0 missing, 1 background, 2 forest: GuidosToolbox's coding."

[[nodes]]
id = "mspa"
op = "landscape.mspa"
inputs = { map = "forest.raster" }
params = { edge_width = 1, connectivity = 8 }
note = "GuidosToolbox's defaults."

[[nodes]]
id = "plot"
op = "view.plot"
inputs = { value = "mspa.map" }
params = { title = "MSPA" }

[[nodes]]
id = "write"
op = "raster.write"
inputs = { value = "mspa.map" }
params = { path = "input_mspa.tif" }
note = "The classes in a GeoTIFF, tagged as an MSPA map."
```

Node 4, [[raster.write]], keeps the result: its input `value` takes `mspa.map`, its `path` names the file. It writes
the smallest cell type that holds the values (a byte here) and tags the file as GuidosToolbox's MSPA product, so
that Terraflux, and GuidosToolbox-aware tools, draw it again with its colours and names. Nothing else needs
connecting: `view.plot` and `raster.write` both read the same output, and a node's output can feed as many inputs
as you like.

=== "Command line"

    ```bash
    terraflux run mspa.toml
    ```

    writes `input_mspa.tif` next to the model.

=== "Application"

    ++f5++ runs the model; the plot opens in a tab of its own. A double click on any node opens its result in a
    tab too, and the viewer writes the row, column and value under the pointer in its corner: point at a red
    strip to read 33 (bridge) or 35 (bridge in edge).

## Read the results

![The forest of GuidosToolbox's sample map](/img/tutorials/habitat-patterns/mspa/forest.png "GuidosToolbox's sample map: forest (2) on 42.9% of the cells with data, a few missing cells (0, in white).")

![Its MSPA](/img/tutorials/habitat-patterns/mspa/mspa.png "Its MSPA with GuidosToolbox's defaults (edge of one cell, 8 neighbours, internal features on). Large cores in the south and the north-east; a dense web of bridges, branches and islets in the settled valley across the middle; border-openings (light grey) where fields are enclosed by forest strips without core.")

Zoom in on the south of the map, and drag the handle to compare the forest with its classes:

```tfcompare
caption: "300 × 300 cells in the south of the map. Left of the handle, the forest (green) and the rest (beige); right of it, MSPA's classes (colours as in the legend above). The red lines that cross the fields are bridges: rows of trees and riparian strips joining cores."
before: { src: /img/tutorials/habitat-patterns/mspa/zoom-forest.png, label: Forest }
after: { src: /img/tutorials/habitat-patterns/mspa/zoom-mspa.png, label: MSPA }
```

The share of each class in the forest:

```tfchart
type: bar
data: /data/tutorials/habitat-patterns/mspa/classes.csv
x: class
y: [forest]
labels: { forest: "% of the forest" }
colors: { forest: "#3b8a5a" }
ylabel: "% of the forest"
height: 300
caption: "MSPA's classes in % of the forest (edge of one cell). Hover for the values; the CSV has the cells and the % of the whole map too."
```

| Class | Cells | % of the forest | % of the map |
|---|---|---|---|
| Core | 321,764 | 75.1 | 32.2 |
| Edge | 52,904 | 12.4 | 5.3 |
| Branch | 16,834 | 3.9 | 1.7 |
| Islet | 13,956 | 3.3 | 1.4 |
| Bridge | 9,509 | 2.2 | 1.0 |
| Perforation | 8,628 | 2.0 | 0.9 |
| Loop | 4,895 | 1.1 | 0.5 |

How to read it:

- **Three quarters of the forest is core** at this edge width. With an edge of one cell (21 to 34 m), "core" only
  means "not on the very boundary"; that number falls fast with wider edges (below).
- **Bridges, loops and branches together hold 7.3%** of the forest. Small as it is, this is the structural
  network that ties the cores together. On the map they concentrate in the valley across the middle: that is where
  the forest's connectivity is fragile.
- **Islets are 3.3% of the forest but very many objects**: the brown dots in the fields. Each is small; together
  they may serve as stepping stones for mobile species.
- **Openings.** With internal features on, 5,927 cells are core-openings (clearings inside cores, mostly in the
  large southern forest) and 51,189 cells border-openings (fields enclosed by forest strips). The perforation
  around the core-openings carries the code 105 and the few features inside them (an islet in a clearing: 109)
  carry 100 more than their class; the statistics above count them with their class.

!!! paper "In the literature: how MSPA results are reported"
    Report the map's source, resolution and projection; what counts as foreground; the edge width in cells *and*
    metres; the connectivity; and the share of each class in the habitat (and, if useful, in hectares). Show the map
    with GuidosToolbox's colours, which readers of the MSPA literature recognise, and a zoom on an area of
    interest. Wickham et al. (2010) is a good model of a national report; Saura et al. (2011) of a study that goes
    on from MSPA to the importance of each connector.

## Settings that change the answer

### The edge width

Move the slider. The map is the zoom above; the numbers beside it are the shares of the classes in the whole forest,
and the chart follows the slider.

```tfslider
label: Edge width (cells)
caption: "The same 300 × 300 cells with edges of 1 to 6 cells (about 25 to 150 m). The cores shrink to the interiors of the large forests; the narrow forest between them turns into bridges and loops; small patches become islets. Numbers: % of the whole forest."
frames:
  - { value: 1, src: /img/tutorials/habitat-patterns/mspa/edge-1.png, caption: "edge_width = 1: an edge 1 cell deep", stats: { Core: "75.1%", Edge: "12.3%", Bridge: "2.2%", Loop: "1.1%", Branch: "3.9%", Islet: "3.3%" } }
  - { value: 2, src: /img/tutorials/habitat-patterns/mspa/edge-2.png, caption: "edge_width = 2: an edge 2 cells deep", stats: { Core: "61.8%", Edge: "13.9%", Bridge: "6.5%", Loop: "4.7%", Branch: "4.8%", Islet: "6.1%" } }
  - { value: 3, src: /img/tutorials/habitat-patterns/mspa/edge-3.png, caption: "edge_width = 3: an edge 3 cells deep", stats: { Core: "53.4%", Edge: "14.2%", Bridge: "10.2%", Loop: "7.4%", Branch: "5.3%", Islet: "7.2%" } }
  - { value: 4, src: /img/tutorials/habitat-patterns/mspa/edge-4.png, caption: "edge_width = 4: an edge 4 cells deep", stats: { Core: "46.6%", Edge: "14.0%", Bridge: "15.0%", Loop: "8.8%", Branch: "5.3%", Islet: "8.2%" } }
  - { value: 5, src: /img/tutorials/habitat-patterns/mspa/edge-5.png, caption: "edge_width = 5: an edge 5 cells deep", stats: { Core: "38.8%", Edge: "12.8%", Bridge: "21.4%", Loop: "10.9%", Branch: "5.4%", Islet: "9.0%" } }
  - { value: 6, src: /img/tutorials/habitat-patterns/mspa/edge-6.png, caption: "edge_width = 6: an edge 6 cells deep", stats: { Core: "33.8%", Edge: "11.5%", Bridge: "28.5%", Loop: "9.2%", Branch: "5.0%", Islet: "10.3%" } }
chart:
  type: line
  data: /data/tutorials/habitat-patterns/mspa/edge-width.csv
  x: edge_width
  y: [Core, Edge, Bridge, Loop, Branch, Islet, Perforation]
  colors: { Core: "#00c800", Edge: "#000000", Bridge: "#ff0000", Loop: "#c9a800", Branch: "#ff8c00", Islet: "#a03c00", Perforation: "#0000ff" }
  xlabel: Edge width (cells)
  ylabel: "% of the forest"
```

From an edge of 1 to 6 cells the core falls from 75% to 34% of the forest, the bridges grow from 2% to 29% and the
islets from 3% to 10%. The edge itself barely changes (11 to 14%): wider edges eat into the cores, but the cores
that vanish no longer have an edge. The pattern is general: with wider edges, connections are made of more
habitat, because more of the forest is too narrow to be core.

How to choose the edge width:

1. **From the process.** The depth to which the edge effect you care about reaches: microclimate, nest predation,
   invasive plants, noise. Reviews give ranges per process (Murcia, 1995; Ries et al., 2004); a study of your
   species is better.
2. **In cells.** Divide by the cell size, and round.
3. **Bracket it.** When the depth is uncertain, run two or three widths that bracket the plausible values and
   report how the classes change; this slider *is* that analysis. In the application, put two MSPA nodes side by
   side reading the same map, as the template **MSPA's edge width** does, or change the setting and press
   ++ctrl+enter++.

### 8 or 4 neighbours

```tfcompare
caption: "200 × 200 cells of the settled valley, the area where the two connectivities differ most. Left of the handle, 8 neighbours; right, 4. With 4 neighbours, branches and bridges that hung by a corner fall apart into islets (brown)."
before: { src: /img/tutorials/habitat-patterns/mspa/conn-8.png, label: "connectivity = 8" }
after: { src: /img/tutorials/habitat-patterns/mspa/conn-4.png, label: "connectivity = 4" }
```

Over the whole forest the core is the same with both (75.1%: the core never depends on the connectivity), but with
4 neighbours the islets grow from 3.3% to 4.3% of the forest, the loops fall from 1.1% to 0.8% and bridges and
branches lose a little. The difference is small for this forest as a whole and large in the fragmented valley.
Choose 4 when the species cannot cross where two pieces touch only at a corner, or when your map draws thin
diagonal lines of cells that are artefacts of rasterization rather than real links; otherwise keep 8.

### Internal features

```tfgallery
columns: 2
caption: "The zoom with internal features on (left, the default) and off (right). On, MSPA tells the fields enclosed by forest strips (border-openings, light grey) and the clearings inside cores (core-openings, dark grey) from the open background. Off, they are all plain background, and all the classes take their plain codes."
images:
  - { src: /img/tutorials/habitat-patterns/mspa/internal-on.png, caption: "internal = true" }
  - { src: /img/tutorials/habitat-patterns/mspa/internal-off.png, caption: "internal = false" }
```

Internal features do not change the foreground's classes, only their codes (+100 inside core-openings) and the
naming of the enclosed background. Keep them on for maps, where a clearing inside a forest and a field outside it
mean different things; turn them off when a later step expects the plain codes.

## Common pitfalls

- **A map in degrees.** GuidosToolbox's sample map is in longitude and latitude: its cells are about 21 m wide and
  34 m tall at 51° N. An edge of one cell is therefore 21 m to the east and west but 34 m to the north and south.
  For real work, reproject the map to a CRS in metres with square cells first ([[raster.reproject]], with
  `resampling = "near"` for classes), then choose the edge width.
- **A class forgotten in the reclassification.** If a forest class is left out of the rule, it becomes background
  silently. Look at the binary map (node 2) before trusting MSPA, and check that the forest share is what you
  expect.
- **Missing versus background.** Water or areas outside the study region set to 1 count as non-forest and create
  edges along lakes and borders; set to 0 they are missing and create none. Decide which is true for your species:
  a lakeshore is a real edge for some.
- **Comparing maps of different sources.** Never compare the MSPA of a CORINE map with that of a Landsat forest
  mask: the differences would come from the maps, not the forests (Model 1 above).
- **The edge of the map.** Classes near the border are unreliable: a large forest cut by the border looks smaller.
  Clip a margin away after MSPA, or analyse a larger area than you report.
- **Huge maps.** MSPA reads the whole map into memory. For maps of hundreds of millions of cells, see
  [[landscape.mspa_tiled]] in [part 4](/tutorials/habitat-patterns/networks-and-change/).

## Going further

- [Part 3](/tutorials/habitat-patterns/spa-and-objects/): fewer classes (SPA), the forest's objects by size,
  parcellation, contortion and distances.
- [Part 4](/tutorials/habitat-patterns/networks-and-change/): MSPA's cores and bridges as a network, the change
  between two dates, maps larger than memory.
- [Measuring fragmentation](/tutorials/fragmentation/): the density of forest around every cell, at several scales.
- The operator reference: [[landscape.mspa]], [[raster.reclassify]], [[raster.write]].

## References

- Murcia, C. (1995). Edge effects in fragmented forests: implications for conservation. *Trends in Ecology & Evolution*, 10(2), 58–62. https://doi.org/10.1016/S0169-5347(00)88977-6
- Ostapowicz, K., Vogt, P., Riitters, K. H., Kozak, J., & Estreguil, C. (2008). Impact of scale on morphological spatial pattern of forest. *Landscape Ecology*, 23(9), 1107–1117. https://doi.org/10.1007/s10980-008-9271-2
- Ries, L., Fletcher, R. J., Jr., Battin, J., & Sisk, T. D. (2004). Ecological responses to habitat edges: mechanisms, models, and variability explained. *Annual Review of Ecology, Evolution, and Systematics*, 35, 491–522. https://doi.org/10.1146/annurev.ecolsys.35.112202.130148
- Saura, S., Vogt, P., Velázquez, J., Hernando, A., & Tejera, R. (2011). Key structural forest connectors can be identified by combining landscape spatial pattern and network analyses. *Forest Ecology and Management*, 262(2), 150–160. https://doi.org/10.1016/j.foreco.2011.03.017
- Soille, P., & Vogt, P. (2009). Morphological segmentation of binary patterns. *Pattern Recognition Letters*, 30(4), 456–459. https://doi.org/10.1016/j.patrec.2008.10.015
- Vogt, P., Riitters, K. H., Estreguil, C., Kozak, J., Wade, T. G., & Wickham, J. D. (2007a). Mapping spatial patterns with morphological image processing. *Landscape Ecology*, 22(2), 171–177. https://doi.org/10.1007/s10980-006-9013-2
- Vogt, P., Riitters, K. H., Iwanowski, M., Estreguil, C., Kozak, J., & Soille, P. (2007b). Mapping landscape corridors. *Ecological Indicators*, 7(2), 481–488. https://doi.org/10.1016/j.ecolind.2006.11.001
- Vogt, P., & Riitters, K. (2017). GuidosToolbox: universal digital image object analysis. *European Journal of Remote Sensing*, 50(1), 352–361. https://doi.org/10.1080/22797254.2017.1330650
- Wickham, J. D., Riitters, K. H., Wade, T. G., & Vogt, P. (2010). A national assessment of green infrastructure and change for the conterminous United States using morphological image processing. *Landscape and Urban Planning*, 94(3–4), 186–195. https://doi.org/10.1016/j.landurbplan.2009.10.003
- Wickham, J., & Riitters, K. H. (2019). Influence of high-resolution data on the assessment of forest fragmentation. *Landscape Ecology*, 34(9), 2169–2182. https://doi.org/10.1007/s10980-019-00820-z
