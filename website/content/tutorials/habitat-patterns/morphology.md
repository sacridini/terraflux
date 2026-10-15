---
title: The idea of morphological pattern analysis
short: The idea of MSPA
description: "What MSPA sees in a forest map, cell by cell, on a small drawn forest. Foreground and background, the structuring element, the edge width, 4 and 8 neighbours, and the seven classes."
order: 10
level: beginner
time: 35 min
ops: [landscape.mspa, raster.reclassify, nlm.random_cluster, view.plot]
series: Habitat patterns with GuidosToolbox
part: 1
---

This first part of the series explains what Morphological Spatial Pattern Analysis (MSPA) does to a habitat map,
before we run it on real data. We use a forest drawn by hand on a grid of 26 by 44 cells, small enough to follow
cell by cell. Terraflux runs MSPA on it, and we look at why each cell got its class. By the end you will know the
words MSPA uses (foreground, background, structuring element, edge width, connectivity), the seven classes and
what they say about a species, and you will have run your first MSPA model in the application.

## The idea

### Why the shape of habitat matters

Take two forest patches of 100 hectares each. One is a square, 1 km on a side. The other is a strip 100 m wide
and 10 km long. On a map of forest and non-forest they are the same: 100 ha of forest. For a forest bird they are
not. Near a forest edge there is more light, more wind, drier air, more nest predators and more parasites coming in
from the fields. These changes are called *edge effects*. They reach tens to hundreds of metres into a forest,
depending on the process and the ecosystem (Murcia, 1995; Ries et al., 2004). The square has an interior far from
any edge. The strip has none: no point in it is more than 50 m from a field.

The strip has something the square does not, though. It may join two larger forests and let animals move between
them. Shape and position decide which *role* a piece of habitat plays: interior, edge, corridor, dead end,
isolated fragment.

Laurance and Yensen (1991) put the first idea in a simple model, the *core-area model*: if edge effects reach a
depth \(d\), only the part of a patch further than \(d\) from its boundary is *core*, the habitat with interior
conditions. For a square patch of side \(L\) the core is a square of side \(L - 2d\).

!!! science "Key idea: the core area"
    For a square patch of side \(L\) and edge effects that reach a depth \(d\) into it,

    \[ \text{core share} = \frac{(L - 2d)^2}{L^2} \]

    where both \(L\) and \(d\) are in metres. A patch narrower than \(2d\) has no core at all. Small patches and
    elongated or convoluted patches lose their interior first.

![The core share of square patches](/img/tutorials/habitat-patterns/morphology/core-area.png "The core-area model of Laurance and Yensen (1991): the share of a square patch that is core, for edge effects reaching 25, 50, 100 and 200 m. A patch 400 m wide is 56% core when edge effects reach 50 m, and has no core at all when they reach 200 m.")

### From the core to every cell's role

Morphological Spatial Pattern Analysis takes this idea to every cell of a map. It was developed at the European
Commission's Joint Research Centre by Peter Vogt, Pierre Soille and colleagues, with Kurt Riitters of the US
Forest Service. Vogt et al. (2007a) described it for landscape ecologists as a way to map spatial patterns with
image processing, and Soille and Vogt (2009) gave its exact definition. The same group showed how it finds
corridors (Vogt et al., 2007b) and how its classes behave on random maps (Riitters et al., 2007). Since then it has
become a standard tool for mapping green infrastructure and forest connectors (Wickham et al., 2010; Saura et al.,
2011). It ships in the free software GuidosToolbox (Vogt & Riitters, 2017; Vogt et al., 2022), and its code was
released as open source in 2022 (Soille & Vogt, 2022).

MSPA works on a **binary map**: every cell is either **foreground** (the habitat you study, here forest) or
**background** (everything else). Cells without data are **missing**. GuidosToolbox codes such a map
**0 missing, 1 background, 2 foreground**, and Terraflux reads the same coding (or any map, if you tell it which
classes are the foreground).

It then gives every foreground cell one of seven classes:

| Class | What it is, geometrically | What it may mean for a species |
|---|---|---|
| **Core** | further from the background than the edge width | interior habitat |
| **Edge** | the band between a core and the background outside it | habitat exposed to the matrix |
| **Perforation** | the band between a core and a hole inside it | an inner edge: around a clearing, a lake, a village |
| **Bridge** | a strip without core joining two or more different cores | a structural corridor |
| **Loop** | a strip without core joining a core to itself | a shortcut, an alternative route |
| **Branch** | a strip without core attached at one end only | a dead end, a spur |
| **Islet** | a piece of foreground too small or thin to hold any core | an isolated fragment, a stepping stone |

The figure below is the MSPA of our drawn forest, computed by Terraflux with the defaults (8 neighbours, an edge
one cell wide).

![The drawn forest](/img/tutorials/habitat-patterns/morphology/toy-forest.png "The drawn forest: two blocks (A on the left, B on the right), a strip joining them, a clearing inside B with a few trees in it, a strip that leaves B and comes back, a dead-end strip from A, a small fragment, a long thin strip, and two little groups of cells touching only by their corners.")

![Its MSPA](/img/tutorials/habitat-patterns/morphology/toy-mspa.png "Its MSPA, in GuidosToolbox's colours, with an edge one cell wide and 8 neighbours. Every structure got its own class: the strip between A and B is a bridge, the strip that leaves B and returns is a loop, the dead ends are branches, the ring around the clearing is perforation, and the small fragment and the long thin strip are islets.")

Look at the long thin strip at the top. It covers 14 cells, more than the small fragment at the bottom left (4
cells), yet both are islets: the strip is never wider than one cell, so it holds no core. Size alone would not tell
you that. Look also at the strip on the right of B: it leaves B and comes back to it, so it is a loop, not a
bridge. A bridge must join two *different* cores.

!!! warning "Structure is not function"
    MSPA describes geometry. A bridge is a strip that *could* serve as a corridor; whether an animal uses it
    depends on its width compared with what the species needs, on what lies along it, on roads that cut it and on
    the species' behaviour. Treat the classes as hypotheses about function, to be checked with what you know of
    the species (Vogt et al., 2007b).

## How MSPA finds the classes

MSPA is built from the operations of *mathematical morphology*, a branch of image analysis that studies shapes by
probing them with a small shape of reference, the **structuring element** (Serra, 1982; Soille, 2004). Two
operations are enough to follow it:

- **Erosion** keeps a foreground cell only if the structuring element, centred on it, fits entirely inside the
  foreground. It peels a layer off every patch and removes patches thinner than the element.
- **Dilation** does the opposite: it adds every cell the element touches when it is centred on a foreground cell.
  It grows every patch by a layer.

![Four steps of MSPA on the drawn forest](/img/tutorials/habitat-patterns/morphology/steps.png "The steps behind the classes. 1: the structuring element, a 3 × 3 square, fits around the cell in block A (blue: the cell is core) but not around the cell in the thin strip (red). 2: the erosion keeps the cells where it fits: the core. 3: the core grown back by one cell, within the forest, gives the edges and the perforation (black). 4: what is left (orange) are the strips and fragments, which MSPA then sorts by the cores they touch. A simplified view: where a strip meets a core, MSPA gives the cells of the edge it crosses to the strip.")

Step by step:

<div class="steps" markdown>

1. **Core.** The core is the erosion of the foreground by a square of side \(2w + 1\) cells, where \(w\) is the
   edge width. With the default \(w = 1\) the square is 3 × 3: a cell is core when its eight neighbours are all
   foreground. With \(w = 2\) it is 5 × 5: a cell is core when all the cells up to two steps away are foreground.
   In Terraflux you can check that the core of [[landscape.mspa]] is exactly this erosion; our figure script does.
2. **Edge and perforation.** The core is grown back by \(w\) cells, within the foreground. The band it covers is
   edge where it faces the background outside, and perforation where it faces a hole inside the core.
3. **The rest.** Foreground cells reached neither by the core nor by its band are strips and fragments. A
   fragment that touches no core is an **islet**. A strip that touches two different cores is a **bridge**, one
   that touches the same core at both ends a **loop**, and one attached at one end only a **branch**.

</div>

!!! science "Key idea: the edge width is a distance"
    The edge width \(w\) is the depth of edge effects, counted in cells:

    \[ w = \frac{\text{depth of edge effects (m)}}{\text{cell size (m)}} \]

    Edge effects reaching 100 m into a forest mapped with cells of 25 m give \(w = 4\). A patch is core-free, and so
    a bridge, loop, branch or islet, when it is no wider than \(2w\) cells.

### The background around and inside the forest

MSPA also names some background. With **internal features** on (the default), a hole inside a core, such as the
clearing in block B, is a **core-opening**. Background enclosed by the forest but not by a core, such as the space
between B and its loop, is a **border-opening**. The trees inside the clearing are an islet, but an islet
*inside a core-opening*: MSPA gives such features 100 more than their usual code (109 instead of 9), so you can
tell the trees of a clearing from the fragments in the open fields. In GuidosToolbox, and so in Terraflux, the
perforation around a core-opening also carries its internal code (105) when internal features are on. Turn
`internal` off and the openings are plain background and every class has its plain code.

The codes of the classes are GuidosToolbox's:

| Code | Class | Code | Class |
|---|---|---|---|
| 17 | Core | 33 | Bridge |
| 9 | Islet | 35, 37 | Bridge in edge, in perforation |
| 5 | Perforation | 65 | Loop |
| 3 | Edge | 67, 69 | Loop in edge, in perforation |
| 1 | Branch | 100 | Core-opening |
| 0 | Background | 220 | Border-opening |
| 129 | Missing | +100 | a class inside a core-opening |

The codes 35, 37, 67 and 69 mark the cells where a bridge or a loop crosses an edge or a perforation to reach its
core (GuidosToolbox calls them transitions). GuidosToolbox draws them in the colour of the bridge or loop, and so
do the figures here.

### 4 or 8 neighbours

A last choice decides which cells belong together. With **8 neighbours** (connectivity 8, the default), cells
touching only at a corner are connected. With **4 neighbours**, only cells sharing a side are.

![4 and 8 neighbours](/img/tutorials/habitat-patterns/morphology/connectivity.png "Left: the 4 and the 8 neighbours of a cell. Right: the spur below block B, cells touching only by their corners. With 8 neighbours (connectivity = 8) the spur is attached to B: a branch. With 4 neighbours its cells are cut from B and from each other: three islets.")

Ecologically the question is: can the species cross where two pieces of habitat touch only at a corner? For a
bird, almost certainly; for a ground beetle in a hedgerow network mapped at 2 m, perhaps not. Percolation theory,
brought to landscape ecology by Gardner et al. (1987), shows that the choice can change much: a random map of a square grid becomes connected from side to side at about
59% habitat with 4 neighbours, but at about 41% with 8. On real maps the two answers are
usually close for the large forests and differ in the fragmented parts, as the next part shows.

## When to use it

MSPA answers questions about the *structure* of habitat:

- **Where is the interior habitat, and how much is there?** The core, at an edge width set from the depth of edge
  effects for your species or process.
- **Which strips connect which patches?** Bridges are candidate structural corridors. Vogt et al. (2007b) used them
  to map landscape corridors; Saura et al. (2011) combined MSPA's cores and bridges with network analysis to find
  the forest connectors most important for connectivity in Spain.
- **How much of the habitat is exposed?** Edge and perforation, and how they change between two dates.
- **Green infrastructure.** Wickham et al. (2010) mapped the green infrastructure of the conterminous United States
  as MSPA's hubs (cores) and links (bridges) and measured its change.
- **Forest monitoring and reporting.** GuidosToolbox's methods, of which MSPA is the best known, feed national and
  global reports on forests (Vogt et al., 2019, 2022); part 2 and the
  [fragmentation tutorials](/tutorials/fragmentation/) show the measures used there.

What MSPA is **not** for:

- It does not measure *functional* connectivity, the movement of a given species through the landscape. For that,
  give the landscape a resistance and use least-cost distances ([Grid distances](/tutorials/grid-distances/)) or
  circuit theory ([Circuit connectivity](/tutorials/circuit-connectivity/)), or build a habitat network with a
  dispersal distance ([Habitat networks](/tutorials/habitat-networks/)).
- It sees only one habitat at a time, as foreground. For maps of many classes use
  [landscape metrics](/tutorials/landscape-metrics/), or the landscape mosaic of the
  [fragmentation tutorials](/tutorials/fragmentation/).
- It counts cells, not metres: an edge width means different distances at different resolutions (see the
  pitfalls below).
- How much habitat surrounds each cell at a given scale is a question for
  [forest area density](/tutorials/fragmentation/), not for MSPA.

## What you need

A raster map of one habitat: forest and non-forest, wetland and dry land, natural vegetation and the rest. It can be
a land cover map (you will reclassify it), a forest mask from satellite imagery, or a map of habitat suitability
cut at a threshold. In a raster every cell is a square of the same size, so it is worth knowing its *resolution*,
the side of a cell in metres: the edge width is counted in cells. Prefer a projected *coordinate reference system*
(CRS), one in metres, so that cells are squares on the ground.

To start, nothing at all: the application's template **MSPA of a forest** draws its own forest.

## Build the model

The template has four nodes. In the application, open **Templates > Patterns (GuidosToolbox) > MSPA of a forest**:
it runs as soon as it opens.

```tfgraph
#! caption: The template MSPA of a forest: a landscape drawn by chance, its forest made MSPA's foreground, MSPA, and a plot of the result.
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "landscape"
op = "nlm.random_cluster"
params = { rows = 300, columns = 300, p = 0.55, weights = [0.3, 0.15, 0.55], seed = 7 }
note = "A map of three classes drawn by chance (1 farming, 2 water, 3 forest): in its place, a raster.read of a map of yours."

[[nodes]]
id = "forest"
op = "raster.reclassify"
inputs = { map = "landscape.map" }
params = { rules = "3 -> 2\n* -> 1" }
note = "MSPA reads 2 as the foreground and 1 as the background: the forest (3) becomes 2, every other class 1."

[[nodes]]
id = "mspa"
op = "landscape.mspa"
inputs = { map = "forest.map" }
params = { edge_width = 1, connectivity = 8 }
note = "Core, islet, perforation, edge, loop, bridge and branch, as GuidosToolbox computes them."

[[nodes]]
id = "plot"
op = "view.plot"
inputs = { value = "mspa.map" }
params = { title = "MSPA" }
note = "Its map in a tab of its own, in GuidosToolbox's colours, refreshed at every run."
```

**Node 1, the landscape** ([[nlm.random_cluster]]). A *neutral landscape model* draws a map by chance: here 300 ×
300 cells in three classes, 1 farming, 2 water and 3 forest, grown as clusters. It stands in for a map of yours;
part 2 replaces it with a [[raster.read]] of a real land cover map. Its output `map` is the land cover.

**Node 2, the foreground** ([[raster.reclassify]]). MSPA needs a binary map. The rules `3 -> 2` and `* -> 1` turn
the forest into 2 (foreground) and every other class (`*`) into 1 (background). Its input `map` is connected to
node 1's output `map`: drag from the small circle on the right of node 1 (an *output port*) to the circle on the
left of node 2 (an *input port*). Why a separate node rather than a setting of MSPA? Because the binary map is
worth seeing: it is the moment where you decide what counts as habitat, and every class you include or leave out
changes the result.

**Node 3, MSPA** ([[landscape.mspa]]). Its input `map` is node 2's output. Its settings:

- `edge_width` (1 to 255, default 1): the depth of edge effects in cells.
- `connectivity` (8 or 4, default 8): whether cells touching at a corner are connected.
- `internal` (default on): openings and internal features told apart.
- `foreground`: the classes of the foreground. Given, it replaces node 2: MSPA would read the land cover directly
  with `foreground = 3`, every other class background and cells without a value missing.

Its output `map` holds the classes, in GuidosToolbox's codes.

**Node 4, the plot** ([[view.plot]]). It shows a map in a tab of its own, refreshed at every run, without writing a
file. Terraflux knows the map comes from MSPA and draws it in GuidosToolbox's colours with the class names.

```tfgallery
columns: 2
caption: "What the template shows. Left, the landscape drawn by node 1 (54% forest). Right, its MSPA: mostly core, with perforations around the farmland and water inside the forest, and few bridges, because random clusters seldom make strips."
images:
  - { src: /img/tutorials/habitat-patterns/morphology/template-landscape.png, caption: "Node 1: the landscape" }
  - { src: /img/tutorials/habitat-patterns/morphology/template-mspa.png, caption: "Node 3: its MSPA" }
```

!!! app "In the application"
    The operators are in the library on the left, by subject: MSPA is under **Habitat patterns**. Click an
    operator and it is added connected to the node selected; or press `/` anywhere, type `mspa` and press
    ++enter++. Select a node to see its settings in the **Node** pane; change one and press ++ctrl+enter++ to run
    the model up to the selected node. Only what the change touches runs again, so moving the edge width from 1 to
    4 shows its effect at once. ++f5++ runs the whole model.

=== "Command line"

    Save the model as `mspa.toml` and add a `raster.write` node to keep the result as a GeoTIFF, then:

    ```bash
    terraflux run mspa.toml
    ```

=== "Model file"

    A model is a text file in TOML: one `[[nodes]]` block per node, `inputs` naming the output each input reads
    (`"forest.map"`: the output `map` of node `forest`) and `params` the settings. The application saves and opens
    the same file; the "Model file (TOML)" toggle under each graph on this site shows it.

## Read the results

On the drawn forest Terraflux gives these shares of the forest (the foreground) for three edge widths:

| Edge width | Core | Edge | Perforation | Bridge | Loop | Branch | Islet |
|---|---|---|---|---|---|---|---|
| 1 | 45.3% | 24.4% | 8.3% | 6.1% | 4.4% | 3.6% | 7.8% |
| 2 | 16.1% | 26.9% | 0% | 44.4% | 0% | 4.7% | 7.8% |
| 3 | 6.9% | 20.6% | 0% | 0% | 59.4% | 5.3% | 7.8% |

The islets do not change: they had no core even at an edge width of 1. Everything else does, and in an
instructive way:

```tfgallery
columns: 3
caption: "The drawn forest with edges 1, 2 and 3 cells wide (colours as above). At 2, block B keeps only a thin core at its bottom; the rest of B, too narrow around the clearing, becomes bridge between that core and the core of A. At 3, B has no core left: all of it is a loop attached to A's small core, and the clearing, with no core around it, is now a border-opening."
images:
  - { src: /img/tutorials/habitat-patterns/morphology/edge-1.png, caption: "edge_width = 1" }
  - { src: /img/tutorials/habitat-patterns/morphology/edge-2.png, caption: "edge_width = 2" }
  - { src: /img/tutorials/habitat-patterns/morphology/edge-3.png, caption: "edge_width = 3" }
```

This is the most important thing to understand about MSPA: **the classes are relative to the edge width**. Block
B is core habitat for a species whose edge effects reach one cell, a corridor for a species sensitive to two cells
of edge, and a loop of A for one sensitive to three. None of these is the "true" class; each answers the question
for one species or process. Report the edge width with every MSPA result, and its value in metres.

How to read an MSPA map, as a habit:

1. **Core first.** Where is it, how much of the habitat is it (the share of core is a measure of how intact the
   habitat is), in how many separate pieces?
2. **Then the connectors.** Bridges join cores: follow them and see which cores they join. Many loops and few
   bridges means a habitat in large connected pieces with internal shortcuts; many islets means scattered
   fragments.
3. **Then the exposure.** Edge and perforation: a forest with a lot of perforation is pierced by clearings,
   settlements or water bodies.

## Settings that change the answer

- **Edge width.** The setting that matters most, as above. Part 2 moves it from 1 to 6 on a real map with a slider.
- **Connectivity.** 8 or 4 neighbours: it changes the fragments and strips, never the core.
- **What is foreground.** Which classes count as habitat (young forest? plantations? shrubland?) can change the
  result more than any setting of MSPA.
- **The map's resolution.** The same forest mapped at 10 m and at 100 m does not give the same classes, even with
  the edge width converted to the same distance in metres: thin strips disappear at coarse resolutions and small
  clearings fill in. Ostapowicz et al. (2008) studied how the cell size and the edge width together change MSPA's
  classes of forest: compare results only at the same resolution and edge width.

## Common pitfalls

- **Wrong coding.** MSPA reads 1 as background and 2 as foreground. A forest mask coded 0 and 1 is read as
  missing and background: no foreground at all. Reclassify it (`1 -> 2`, `0 -> 1`) or give `foreground = 1`.
- **Missing cells are not background.** Cells coded 0 (or without a value) are left out. Use missing for areas
  outside the study region or under clouds, not for non-habitat.
- **The edge width in metres.** An edge width of 3 is 90 m on a 30 m Landsat map and 30 m on a 10 m Sentinel-2 map.
  Convert, and report both.
- **Geographic coordinates.** A map in degrees of longitude and latitude has cells that are not square on the
  ground away from the equator, so an edge width of one cell is a different distance north-south and east-west.
  Reproject to a CRS in metres first ([[raster.reproject]]).
- **The map's border.** MSPA cannot see the forest beyond the edge of the map: a patch cut by the border may be
  classed as a branch or an islet when it is part of a large forest outside. Leave a margin around the study area,
  or mark the outside as missing.

## Going further

- Part 2, [MSPA step by step on a real forest map](/tutorials/habitat-patterns/mspa/), runs MSPA on two real maps
  and moves its settings with sliders.
- Part 3, [Simplified patterns and objects](/tutorials/habitat-patterns/spa-and-objects/), covers SPA, the objects
  by size, parcellation, contortion and distances.
- Part 4, [From patterns to networks and change](/tutorials/habitat-patterns/networks-and-change/), turns cores and
  bridges into a network and compares two dates.
- The operator reference: [[landscape.mspa]], [[raster.reclassify]], [[nlm.random_cluster]].

## References

- Gardner, R. H., Milne, B. T., Turner, M. G., & O'Neill, R. V. (1987). Neutral models for the analysis of broad-scale landscape pattern. *Landscape Ecology*, 1(1), 19–28. https://doi.org/10.1007/BF02275262
- Laurance, W. F., & Yensen, E. (1991). Predicting the impacts of edge effects in fragmented habitats. *Biological Conservation*, 55(1), 77–92. https://doi.org/10.1016/0006-3207(91)90006-U
- Murcia, C. (1995). Edge effects in fragmented forests: implications for conservation. *Trends in Ecology & Evolution*, 10(2), 58–62. https://doi.org/10.1016/S0169-5347(00)88977-6
- Ostapowicz, K., Vogt, P., Riitters, K. H., Kozak, J., & Estreguil, C. (2008). Impact of scale on morphological spatial pattern of forest. *Landscape Ecology*, 23(9), 1107–1117. https://doi.org/10.1007/s10980-008-9271-2
- Ries, L., Fletcher, R. J., Jr., Battin, J., & Sisk, T. D. (2004). Ecological responses to habitat edges: mechanisms, models, and variability explained. *Annual Review of Ecology, Evolution, and Systematics*, 35, 491–522. https://doi.org/10.1146/annurev.ecolsys.35.112202.130148
- Riitters, K., Vogt, P., Soille, P., Kozak, J., & Estreguil, C. (2007). Neutral model analysis of landscape patterns from mathematical morphology. *Landscape Ecology*, 22(7), 1033–1043. https://doi.org/10.1007/s10980-007-9089-3
- Saura, S., Vogt, P., Velázquez, J., Hernando, A., & Tejera, R. (2011). Key structural forest connectors can be identified by combining landscape spatial pattern and network analyses. *Forest Ecology and Management*, 262(2), 150–160. https://doi.org/10.1016/j.foreco.2011.03.017
- Serra, J. (1982). *Image analysis and mathematical morphology*. Academic Press.
- Soille, P. (2004). *Morphological image analysis: Principles and applications* (2nd ed.). Springer. https://doi.org/10.1007/978-3-662-05088-0
- Soille, P., & Vogt, P. (2009). Morphological segmentation of binary patterns. *Pattern Recognition Letters*, 30(4), 456–459. https://doi.org/10.1016/j.patrec.2008.10.015
- Soille, P., & Vogt, P. (2022). Morphological spatial pattern analysis: open source release. *The International Archives of the Photogrammetry, Remote Sensing and Spatial Information Sciences*, XLVIII-4/W1-2022, 427–433. https://doi.org/10.5194/isprs-archives-XLVIII-4-W1-2022-427-2022
- Vogt, P., Riitters, K. H., Estreguil, C., Kozak, J., Wade, T. G., & Wickham, J. D. (2007a). Mapping spatial patterns with morphological image processing. *Landscape Ecology*, 22(2), 171–177. https://doi.org/10.1007/s10980-006-9013-2
- Vogt, P., Riitters, K. H., Iwanowski, M., Estreguil, C., Kozak, J., & Soille, P. (2007b). Mapping landscape corridors. *Ecological Indicators*, 7(2), 481–488. https://doi.org/10.1016/j.ecolind.2006.11.001
- Vogt, P., & Riitters, K. (2017). GuidosToolbox: universal digital image object analysis. *European Journal of Remote Sensing*, 50(1), 352–361. https://doi.org/10.1080/22797254.2017.1330650
- Vogt, P., Riitters, K. H., Caudullo, G., & Eckhardt, B. (2019). *FAO – State of the World's Forests: Forest fragmentation* (EUR 29972 EN). Publications Office of the European Union. https://doi.org/10.2760/145325
- Vogt, P., Riitters, K., Rambaud, P., d'Annunzio, R., Lindquist, E., & Pekkarinen, A. (2022). GuidosToolbox Workbench: spatial analysis of raster maps for ecological applications. *Ecography*, 2022(3), e05864. https://doi.org/10.1111/ecog.05864
- Wickham, J. D., Riitters, K. H., Wade, T. G., & Vogt, P. (2010). A national assessment of green infrastructure and change for the conterminous United States using morphological image processing. *Landscape and Urban Planning*, 94(3–4), 186–195. https://doi.org/10.1016/j.landurbplan.2009.10.003
