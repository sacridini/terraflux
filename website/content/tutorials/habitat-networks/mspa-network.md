---
title: The network of a forest's structure (MSPA)
short: MSPA network
description: Turn MSPA's cores and bridges into GuidosToolbox's habitat network, count its components, measure its equivalent connected area and coherence, and find the bridges and stepping-stone cores whose loss would cut it most.
order: 50
level: intermediate
time: 40 min
template: Network of a forest (MSPA)
ops: [landscape.mspa, networks.gtn_components, networks.gtn_importance, networks.conefor]
series: Habitat networks
part: 5
---

The graphs of parts 1 to 4 joined patches by the distance a species can cross. There is another way to see the
links of a forest network: look for the forest **itself** joining the forest, the hedgerows, riparian strips and
narrow woods that physically run from one block to another. Morphological spatial pattern analysis (MSPA, the
subject of the [habitat patterns](/tutorials/habitat-patterns/mspa/) series) finds them, as *bridges*, and
GuidosToolbox turns its cores and bridges into a network. In this part you run MSPA on the forests around Hannover,
build that **structural network**, measure how coherent it is, and rank its bridges and cores by how much of it
would be cut without them. At the end you will know what a structural network says that a dispersal graph does not,
and the other way round.

## The idea

### Cores as nodes, bridges as links

MSPA classifies every cell of a habitat map by its shape (Vogt et al., 2007; Soille & Vogt, 2009): **cores** are
the interior of the habitat, farther than the edge width from its boundary; **bridges** are narrow strips of
habitat that join two different cores; *loops* join a core to itself, *branches* lead nowhere, *islets* are too
small to hold any core. Vogt et al. (2009) proposed to read this as a network: the cores are the nodes, the
bridges the links. Two cores joined by a bridge belong to the same **component**, and a component with at least
one bridge is a **network**; a core without any bridge is **isolated**.

!!! science "Key idea: the structural network"
    For the components \(c\) of cores joined by bridges, with \(A_c\) the core area of component \(c\),

    \[ \mathrm{ECA} = \sqrt{\textstyle\sum_c A_c^2} \qquad \mathrm{ECA}_{\max} = \textstyle\sum_c A_c \qquad
    \text{coherence} = 100 \times \frac{\mathrm{ECA}}{\mathrm{ECA}_{\max}} \]

    This is the equivalent connected area of part 2 for a species that moves freely within a component (along
    the bridges) and never across open land: every pair of cores in a component has a probability of 1, every
    other pair 0. Coherence is 100% when all the cores form one network.

The **importance of a bridge** is the share of that connectivity lost if it were cut (the component may split in
two); the importance of a **stepping-stone core**, a core with two bridges or more, is the share lost without it.
Saura et al. (2011) combined MSPA with Conefor's dPC in this way to find the key structural connectors of a
forest landscape.

### Structure and function, again

A structural network has no dispersal distance: a bridge exists or it does not. It answers *where the forest
itself provides a corridor*, which matters for species that never leave the woods (many forest plants,
ground-dwelling invertebrates, some small mammals) and for planning green infrastructure, where the bridges are
the things to protect. It ignores everything a species might cross; the dispersal graphs of parts 1 to 4 ignore
the shape of the forest. Used together they tell the corridors that exist from the gaps that are crossed.

## When to use it

- **Mapping green infrastructure**: the cores as hubs and the bridges as links of a regional network, and the
  bridges ranked by importance to protect (Vogt et al., 2009; Saura et al., 2011).
- **Species bound to the habitat**, for which a gap of open land is a barrier at any distance.
- **Following a network over time**: the number of networks, the coherence and the most important bridges at
  two dates.

Not for species that cross open land: for them use the graphs of [part 1](../connectivity-as-a-graph/) to
[part 4](../least-cost-links/). And remember that MSPA can only find the bridges the map draws (the habitat
patterns series shows how a coarse map hides them).

## What you need

- **A habitat map** that MSPA can read: GuidosToolbox's coding (0 missing, 1 background, 2 habitat), or any map
  with `foreground` naming the habitat classes. Here the land cover of the series,
  [`clc_100m.tif`](/data/tutorials/habitat-networks/connectivity-as-a-graph/clc_100m.tif), with the forests (23,
  24, 25) as foreground.
- **An edge width**, the depth of the edge effect in cells. With cells of 100 m, `edge_width = 1` makes every
  forest strip two cells (200 m) wide or less a bridge or a branch, never a core. Choose it as the [MSPA
  tutorial](/tutorials/habitat-patterns/mspa/) explains.

The application's template **Network of a forest (MSPA)** (**Templates > Connectivity**) has the model of this
part on a landscape drawn by chance; replace its first node with a [[raster.read]] of your map.

## Build the model

<div class="steps" markdown>

1. **MSPA** ([[landscape.mspa]], library subject **Habitat patterns**). Connect the land cover's `map`, set
   `foreground = [23, 24, 25]` and `edge_width = 1`. Its `map` holds MSPA's classes in GuidosToolbox's codes
   (17 core, 33 bridge, 3 edge...).
2. **The components** ([[networks.gtn_components]], subject **Habitat networks**). Connect `mspa.map`. Its
   outputs: `map`, the networks in GuidosToolbox's colours (a colour per network, isolated cores apart);
   `components`, the number of the component of every core and bridge cell; `stats`, a row per component (its
   core and bridge cells, whether it has a bridge, its area in hectares); `summary`, the totals (components,
   networks, isolated cores, nodes, links, ECA, ECA_max, coherence); `report`, GuidosToolbox's log. Its setting
   `pixel_size` gives the cell's side in metres when the map's own is not in metres.
3. **The importance** ([[networks.gtn_importance]]). Connect `mspa.map` too. Its outputs: `map`, the cores and
   bridges with the most important links and nodes in GuidosToolbox's colours; `importance`, the dPC of every
   bridge (positive) and stepping-stone core (negative) on its cells; `ids`, the number of each link (positive)
   or node (negative); `links` and `nodes`, the tables (each link's component, cells, dPC, the nodes it joins and
   its importance relative to the largest); `report`, GuidosToolbox's text report.
4. **Conefor's inputs** ([[networks.conefor]]), optional: the same network as Conefor Sensinode's node and
   connection files (cores and bridges both as nodes, a link from each bridge to each core it touches), as
   GuidosToolbox writes them for Conefor.

</div>

```tfgraph
#! caption: MSPA of the forests, then GuidosToolbox's network analysis: the components of cores joined by bridges, and the importance of every bridge and stepping-stone core.
#! numbered: true
#! highlight: components, importance
format = "terraflux/1"

[[nodes]]
id = "land"
op = "raster.read"
params = { path = "clc_100m.tif" }
note = "CORINE Land Cover, cells of 100 m."

[[nodes]]
id = "mspa"
op = "landscape.mspa"
inputs = { map = "land.raster" }
params = { foreground = [23, 24, 25], edge_width = 1 }
note = "Cores, bridges, branches... of the forests, with an edge of 100 m."

[[nodes]]
id = "components"
op = "networks.gtn_components"
inputs = { map = "mspa.map" }
note = "Networks of cores joined by bridges; ECA and coherence in summary."

[[nodes]]
id = "importance"
op = "networks.gtn_importance"
inputs = { map = "mspa.map" }
note = "dPC of every bridge and stepping-stone core."

[[nodes]]
id = "conefor"
op = "networks.conefor"
inputs = { map = "mspa.map" }
note = "The same network as Conefor's input files."

[[plots]]
id = "plot"
title = "The most important links and nodes"
layers = [{ from = "importance.map" }]
```

Both network operators read the MSPA map only: they need its codes, so connect `mspa.map` directly, not a
reclassified copy.

## Read the results

### MSPA of the forest

![MSPA of the forests at 100 m with an edge of one cell](/img/tutorials/habitat-networks/mspa-network/mspa.png "MSPA of the forests (edge of one cell, 100 m). Cores (green) hold 14.1% of the map, edges 4.8%, branches 1.3%; bridges (red) only 0.3% and loops 0.1%. A few bridges join the woods of the north and the north-east; the large southern forests have none.")

The forest at 100 m is mostly core: the map is coarse, and only the strips of forest that survived the
resampling show as bridges. 0.3% of the map, about 750 cells, is bridge.

### The networks

![The components of the structural network](/img/tutorials/habitat-networks/mspa-network/components.png "The components: each network (cores joined by bridges) in a colour of its own, the bridges in black, isolated cores in grey, and the rest of the forest (edges, branches, islets) pale. The large southern forest is an isolated core: no strip of forest joins it to anything.")

[[networks.gtn_components]]'s summary:

| Measure | Value |
|---|---|
| Cores (nodes) | 251 |
| Bridges (links) | 112 |
| Components | 138: 59 networks and 79 isolated cores |
| ECA | 11,277 ha |
| ECA_max (all cores) | 35,286 ha |
| Coherence | 32.0% |

A coherence of 32% means the cores offer as much "structurally reachable" core as a single block of a third of
their area. The comparison with part 2 is instructive. There, for a species that crosses 250 m of open land, the
forest's ECA was 29% of the forest; here, for a species that never leaves it, the cores' coherence is 32% of the
*core*. At 100 m, most of the links between forest blocks are gaps of farmland, not strips of forest, so the
structural network is about as fragmented as the functional one of a poor disperser. A finer map (25 m or 10 m) would show the hedgerows and riparian strips that CORINE merges into farmland.

### The most important bridges and cores

![GuidosToolbox's map of the most important links and nodes](/img/tutorials/habitat-networks/mspa-network/importance.png "networks.gtn_importance's map in GuidosToolbox's colours: cores (green), bridges (dark), and the links and nodes whose importance is within 1, 5 or 10% of the largest. One bridge and one core in the north-east stand out.")

GuidosToolbox's colours mark the links and nodes whose importance is **within 1, 5 and 10% of the most
important**, not the top 1, 5 and 10% of their number: when one bridge is far more important than every other, as
here, it is the only one coloured. The `importance` output has the values themselves; drawn on a zoom of the
north-east:

![The importance of the bridges and stepping-stone cores of the north-east](/img/tutorials/habitat-networks/mspa-network/importance-zoom.png "dPC of each bridge and stepping-stone core of the north-east (logarithmic); grey: bridges and cores whose loss changes the network by less than 0.01%. A bridge of seven cells (yellow) joins the large north-eastern forest to the core below it (orange, a stepping stone of 762 ha with seven bridges).")

| Rank | Link (bridge) | Cells | dPC (%) | | Stepping-stone core | Cells | Bridges | dPC (%) |
|---|---|---|---|---|---|---|---|---|
| 1 | 36 | 7 | 7.33 | | 80 | 762 | 7 | 2.03 |
| 2 | 102 | 7 | 1.40 | | 217 | 1 | 2 | 1.39 |
| 3 | 101 | 6 | 1.39 | | 125 | 106 | 4 | 0.90 |
| 4 | 66 | 9 | 1.03 | | 138 | 3 | 2 | 0.64 |
| 5 | 51 | 3 | 0.69 | | 105 | 82 | 3 | 0.49 |

A single bridge, **link 36**, seven cells of forest (7 ha, a strip a few hundred metres long), carries
7.3% of the network's connectivity: it ties the largest core of the north-east to a chain of cores to the south.
Cut it, and the network falls in two. The importance drops fast after it: the 112 bridges are mostly redundant
or join small cores. Among the 56 stepping-stone cores, core 80 (762 ha, seven bridges) is the most important;
core 217, a single cell of core with two bridges, ranks second: structure, not size, makes a stepping stone.

!!! paper "In the literature"
    Report the habitat map and its resolution, the edge width (cells and metres), the numbers of cores, bridges,
    networks and isolated cores, ECA and coherence, and a map of the most important bridges. Saura et al.
    (2011) and Vogt et al. (2009) are the models; Vogt and Riitters (2017) describe GuidosToolbox, whose numbers
    Terraflux reproduces.

### In Python

The three operators are [`tf.mspa`][terraflux.mspa], [`tf.gtn_components`][terraflux.gtn_components] and
[`tf.gtn_importance`][terraflux.gtn_importance]. [`tf.reclassify`][terraflux.reclassify] with `foreground` turns the
land cover into the habitat map MSPA reads (0 missing, 1 background, 2 forest), here from the library's 25 m
example resampled to the part's 100 m cells:

```python
import terraflux as tf

land = tf.reproject(tf.load_raster(tf.example_data("clc")), res=100, resampling="mode")
forest = tf.reclassify(land, foreground=[23, 24, 25])
morph = tf.mspa(forest, edge_width=1)                    # one cell of 100 m; connectivity=8 by default
print(tf.mspa_stats(morph).round(2).to_string())
```

```text
                pixels  pct_data  pct_foreground
class                                           
Core             35286     14.11           68.42
Islet              153      0.06            0.30
Perforation          0      0.00            0.00
Edge             11884      4.75           23.04
Loop               209      0.08            0.41
Bridge             727      0.29            1.41
Branch            3310      1.32            6.42
Background      197833     79.13             NaN
Core-Opening         0      0.00             NaN
Border-Opening     598      0.24             NaN
Missing              0       NaN             NaN
```

These are the shares the part gives: cores 14.1% of the map, edges 4.8%, branches 1.3%, bridges 0.3% and loops 0.1%.
The network is made from the MSPA map alone:

```python
network = tf.gtn_components(morph)               # the pixel size (100 m) is read from the map
print(network.summary())
print(network.stats.sort_values("core_pixels", ascending=False).head(5).to_string(index=False))
```

```text
Network components: 138
Connected networks: 59
Isolated core components: 79
Total core nodes: 251
Total bridge links: 112
ECA: 11277.36 ha (11277.4 pixels)
ECA_max: 35286.00 ha (35286 pixels)
DOC (Degree of Coherence): 31.96%

 component_id  core_pixels  bridge_pixels  total_pixels  has_bridge  core_ha  total_ha
          113         8098              0          8098       False   8098.0    8098.0
            5         5485             93          5578        True   5485.0    5578.0
          110         3878             13          3891        True   3878.0    3891.0
           49         1819             96          1915        True   1819.0    1915.0
           31         1766             13          1779        True   1766.0    1779.0
```

The numbers are the part's table: 251 cores, 112 bridges, 138 components (59 networks, 79 isolated cores), ECA
11,277 ha, ECA_max 35,286 ha and coherence 32.0% (`network.eca`, `network.eca_max`, `network.doc`). The result also
holds `network.components` (the component of every core and bridge pixel) and `network.colored_map`, the
networks in GuidosToolbox's colours. The importance of the links and stepping-stone cores:

```python
importance = tf.gtn_importance(morph, components=network)    # reuse the components
links = importance.links.sort_values("dpc", ascending=False)
nodes = importance.nodes.sort_values("dpc", ascending=False)
print(len(links), "links,", len(nodes), "stepping-stone cores")
print(links.head(5).round(2).to_string(index=False))
print(nodes.head(5).round(2).to_string(index=False))
```

```text
112 links, 56 stepping-stone cores
 link_id  component_id  pixels  dpc connected_nodes  normalized_importance
      36             5       7 7.33         [6, 80]                 100.00
     102           110       7 1.40      [217, 221]                  19.10
     101           110       6 1.39      [209, 217]                  19.03
      66            62       9 1.03      [114, 125]                  14.06
      51            49       3 0.69      [105, 110]                   9.38
 node_id  component_id  pixels  dpc  degree  normalized_importance
      80             5     762 2.03       7                 100.00
     217           110       1 1.39       2                  68.83
     125            62     106 0.90       4                  44.41
     138            62       3 0.64       2                  31.74
     105            49      82 0.49       3                  24.34
```

Link 36 (7 cells, 7.3%) and core 80 (762 cells, 2.03%) lead, as in the part. `importance.importance_map` has
the dPC on the cells (links positive, cores negative), `importance.node_link_ids` their numbers and
`importance.colored_map` the classes within 1, 5 and 10% of the largest, so rank with the tables, as the pitfall
above says. The network is also written as Conefor's input files:

```python
nodes_table, links_table = tf.conefor_inputs(morph, output_dir=".")     # writes mspa_nodes_mspa.txt and mspa_links_mspa.txt
print(nodes_table.shape, links_table.shape)
tf.save(network, "network")                    # the maps and tables of the components, in a folder
```

```text
(363, 2) (230, 3)
```

[`tf.conefor_inputs`][terraflux.conefor_inputs] makes cores and bridges both nodes, with a link from each bridge to
the cores it touches, as [[networks.conefor]] does. Every result saves its maps and tables in a folder with
`tf.save`, as above.

## Settings that change the answer

- **The edge width.** A wider edge shrinks the cores and turns more strips into bridges: there are fewer, smaller
  cores and more links. Coherence can rise or fall; report the width with every number.
- **The resolution of the map.** It decides which strips exist. At 25 m the same forests have many more bridges;
  at 100 m most hedgerows are gone. Never compare networks made from maps of different resolution.
- **8 or 4 neighbours** (`connectivity` of [[landscape.mspa]]): with 4, bridges that hold by a corner break, and
  cores they joined fall apart.
- **What counts as habitat**: adding transitional woodland (class 29) to the foreground makes new bridges across
  clearings.

## Common pitfalls

- **Expecting functional links.** Two woods 50 m apart across a field are not joined: there is no bridge. For
  species that cross fields, use the dispersal graphs.
- **Reading "top 1%" as the top 1% of links.** GuidosToolbox's classes are relative to the largest importance.
  Sort the `links` table by `dpc` for a ranking.
- **Huge single-cell cores.** At a coarse resolution and edge width 1, one interior cell is already a core
  (core 217). A wider edge or a finer map avoids such artefacts.
- **The border.** Cores cut by the map's edge lose bridges outside it.

## Going further

- [Habitat patterns](/tutorials/habitat-patterns/mspa/): MSPA itself, its classes and settings.
- [Part 3](../importance-of-each-patch/): importance with dispersal, for comparison.
- [Part 6: ProtConn](../protconn/): the connectivity of a network of reserves.
- [Costs and corridors](/tutorials/distance-restoration/costs-and-corridors/): where restoring a corridor
  would join two forest objects.
- The operators: [[landscape.mspa]], [[networks.gtn_components]], [[networks.gtn_importance]],
  [[networks.conefor]].

## References

- Saura, S., Vogt, P., Velázquez, J., Hernando, A., & Tejera, R. (2011). Key structural forest connectors can be identified by combining landscape spatial pattern and network analyses. *Forest Ecology and Management*, 262(2), 150–160. https://doi.org/10.1016/j.foreco.2011.03.017
- Soille, P., & Vogt, P. (2009). Morphological segmentation of binary patterns. *Pattern Recognition Letters*, 30(4), 456–459. https://doi.org/10.1016/j.patrec.2008.10.015
- Vogt, P., Ferrari, J. R., Lookingbill, T. R., Gardner, R. H., Riitters, K. H., & Ostapowicz, K. (2009). Mapping functional connectivity. *Ecological Indicators*, 9(1), 64–71. https://doi.org/10.1016/j.ecolind.2008.01.011
- Vogt, P., Riitters, K. H., Iwanowski, M., Estreguil, C., Kozak, J., & Soille, P. (2007). Mapping landscape corridors. *Ecological Indicators*, 7(2), 481–488. https://doi.org/10.1016/j.ecolind.2006.11.001
- Vogt, P., & Riitters, K. (2017). GuidosToolbox: universal digital image object analysis. *European Journal of Remote Sensing*, 50(1), 352–361. https://doi.org/10.1080/22797254.2017.1330650
