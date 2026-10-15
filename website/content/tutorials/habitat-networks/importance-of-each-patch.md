---
title: The importance of each patch
short: Importance of each patch
description: Rank every patch and every link by how much connectivity would be lost without it, split that loss into the habitat a patch holds, the flows it sends and receives, and its role as a stepping stone, and compare it with network centralities.
order: 30
level: intermediate
time: 45 min
template: Habitat network (Conefor)
ops: [conn.node_importance, conn.link_importance, conn.centrality, conn.to_map, conn.graph_from_raster]
series: Habitat networks
part: 3
---

Parts [1](../connectivity-as-a-graph/) and [2](../connectivity-indices/) measured the connectivity of the whole
forest. Conservation acts on pieces: this wood is to be protected, that one may be cleared for a road, this gap is
where a hedge would be planted. In this part you ask the question planners ask, **which patches and which links
matter most for the connectivity of the whole?**, and answer it the way Conefor does, by removing each patch in
turn and measuring what is lost. You will split each patch's importance into three roles (its own habitat, the
movement it sends and receives, and its role as a stepping stone between others), rank the links between
patches, and compare the result with the centralities of network science. At the end you will have maps and
tables that name the forests a species of 2 km cannot afford to lose, and the small woods that matter far more
than their size suggests.

## The idea

### Importance as the loss without it

The simplest way to know what a patch does for a network is to take it away. Urban and Keitt (2001) removed
patches one at a time from a landscape graph and watched how its connectivity fell;
Pascual-Hortal and Saura (2006) made it the standard way to rank patches, with the indices of part 2: the
importance of patch \(k\) is the **percentage of the index lost** when \(k\) is removed (with its links).

!!! science "Key idea: dPC"
    \[ \mathrm{dPC}_k = 100 \times \frac{\mathrm{PC} - \mathrm{PC}_{\text{without } k}}{\mathrm{PC}} \]

    It is a percentage of the connectivity of the whole network, so patches can be compared and their values
    mean something on their own: a patch with dPC = 10% holds a tenth of the landscape's reachable habitat. The
    same is done for IIC (dIIC), and for links (the loss when one link is cut).

### Three roles of a patch

Saura and Rubio (2010) showed that dPC is a sum of three parts, each a different way a patch serves the
network:

- **dPCintra**: the habitat *inside* the patch, \(a_k^2\), over PC's numerator. Only the patch's area matters; a
  large forest has a large intra fraction even if it is isolated.
- **dPCflux**: the movement *starting or ending* in the patch, \(\sum_{j \ne k} a_k a_j p^*_{kj}\) counted both
  ways. It is large for large patches that are well connected to other large patches.
- **dPCconnector**: what the patch contributes *as a stepping stone* between other patches, that is, the loss of
  the best paths between other pairs that went through \(k\) and must now take a worse route (or none). It
  does not depend on the patch's own area at all: a small wood in the right place can have a large connector
  fraction.

\[ \mathrm{dPC}_k = \mathrm{dPCintra}_k + \mathrm{dPCflux}_k + \mathrm{dPCconnector}_k \]

In the toy landscape of part 2, patch 1 (the largest) has dPC = 60.7%: 33.7% intra, 27.0% flux and no connector
role. Patch 3 (the smallest, 20 ha) has dPC = 13.4%, of which 1.2% as a connector: it relays movement between
patch 4 and the others.

```tfchart
type: bar
data: /data/tutorials/habitat-networks/connectivity-indices/toy-importance.csv
x: patch
y: [intra, flux, connector]
labels: { intra: "dPCintra", flux: "dPCflux", connector: "dPCconnector" }
colors: { intra: "#3b8a5a", flux: "#0072b2", connector: "#e8901f" }
stacked: true
xlabel: Patch of the toy landscape (100, 50, 20 and 80 ha)
ylabel: "% of PC lost without the patch"
height: 260
caption: "The importance of the four patches of part 2's toy landscape and its three fractions, from conn.node_importance. Patch 4 is large but far from the rest: its importance is almost all intra."
```

### Centralities

Network science ranks nodes by their *centrality*: the **degree** (number of links), **betweenness** (how many
shortest paths between other nodes pass through it), **closeness** (how few steps it is from all others) or the
**eigenvector** centrality (connected to well-connected nodes). They are fast and need no areas. Bodin and Saura
(2010) compared them with dPC and its fractions on real landscapes and found that betweenness and the connector
fraction agree on the main stepping stones but not on all: centralities count steps and ignore how much habitat
lies at each end, while dPCconnector weighs paths by probability and area. Terraflux gives both, so that you can
see where they disagree.

## When to use it

- **Prioritising patches for protection**: the patches with the highest dPC are those whose loss would cost the
  most reachable habitat. Many regional plans rank forests or wetlands this way, from Conefor's tables.
- **Finding stepping stones**: patches with a high dPCconnector compared with their area, which an area-based
  ranking would miss.
- **Assessing a project**: the dPC of the patches a road or a mine would destroy is the share of the network's
  connectivity at stake, in a currency comparable between projects.
- **Choosing where to restore a link**: the links with the highest importance, or, for new links,
  [[conn.link_change]] (the gain of adding or improving one).

What it is *not* for: dPC removes one patch at a time, so it does not say what happens if two neighbouring
patches are both lost (they may each look dispensable because the other remains). For sets of patches, remove
them together (build the graph without them and compare its ECA) or use a spatial prioritization
([spatial prioritization](/tutorials/spatial-prioritization/systematic-conservation-planning/)).

## What you need

The habitat graph of part 1 ([`clc_100m.tif`](/data/tutorials/habitat-networks/connectivity-as-a-graph/clc_100m.tif),
forests 23–25, edge-to-edge distances) and **one** dispersal distance: importance is computed for one kernel at a
time (a list of distances gives a row per patch and distance). Here, 2 km with a probability of one half: in
part 2 it is the distance at which the three fractions of the network are equal, a species of the middle range.

## Build the model

### 1. The importance of every patch

Add [[conn.node_importance]] and connect `graph.graph` to its `graph`. Set `distance = 2000` and
`probability = 0.5`. Its `table` has a row per patch: `id`, `distance`, and for each index of part 2 the share
lost without the patch (`dA`, `dNC`, `dNL`, `dH`, `dCCP`, `dLCP`, `dIIC`, `dF`, `dAWF`, `dPC`), the fractions
`dIICintra`, `dIICflux`, `dIICconnector`, `dPCintra`, `dPCflux`, `dPCconnector`, and `varPC`, the variation of
PC itself. `metrics` limits the list (`["A", "PC"]`), and `BC`, `BCIIC`, `BCPC` add Conefor's betweenness
indices.

### 2. Paint them on the map

[[conn.to_map]] paints a column of the table on the patches. Add four, all with `graph.graph` as `graph` and
`importance.table` as `table`, and set their `column` to `dPC`, `dPCintra`, `dPCflux` and `dPCconnector`.

### 3. Links and centralities

[[conn.link_importance]] removes each link in turn: its `table` has `from`, `to`, `distance` and `dPC_removal`,
the share of PC lost. [[conn.centrality]] gives, per patch, `degree`, `eigen`, `close`, `BWC` (betweenness) and
`cluster`, computed on the links shorter than `distance` (here also 2 km).

```tfgraph
#! caption: The importance of every forest patch at 2 km (dPC and its three fractions, two of them painted on the patches), of every link, and the patches' centralities.
#! numbered: true
#! highlight: importance, connector, links, centrality
format = "terraflux/1"

[[nodes]]
id = "land"
op = "raster.read"
params = { path = "clc_100m.tif" }
note = "CORINE Land Cover, cells of 100 m."

[[nodes]]
id = "graph"
op = "conn.graph_from_raster"
inputs = { map = "land.map" }
params = { habitat = [23, 24, 25], area_unit = "ha", distance = "edge", landscape_area = 250000 }
note = "The forest patches and the gaps between them."

[[nodes]]
id = "importance"
op = "conn.node_importance"
inputs = { graph = "graph.graph" }
params = { distance = 2000, probability = 0.5 }
note = "dPC and its fractions, per patch."

[[nodes]]
id = "dpc"
op = "conn.to_map"
inputs = { graph = "graph.graph", table = "importance.table" }
params = { column = "dPC" }

[[nodes]]
id = "connector"
op = "conn.to_map"
inputs = { graph = "graph.graph", table = "importance.table" }
params = { column = "dPCconnector" }
note = "The stepping-stone role only."

[[nodes]]
id = "links"
op = "conn.link_importance"
inputs = { graph = "graph.graph" }
params = { distance = 2000, probability = 0.5 }
note = "The share of PC lost without each link."

[[nodes]]
id = "centrality"
op = "conn.centrality"
inputs = { graph = "graph.graph" }
params = { distance = 2000 }
note = "Degree, betweenness... on the links up to 2 km."
```

In the application, sort a table by a column with a click on its header: sorting `importance` by `dPC` gives
the ranking, by `dPCconnector` the stepping stones. To keep the tables, add outputs to the model and run it from
the command line (`terraflux run importance.toml --out results` writes `results/importance.csv`), or connect a
[[vector.write]] after [[conn.to_vector]] when the patches are polygons.

## Read the results

### Which forests matter most

![The importance of every forest patch at 2 km, with the patches of the text numbered](/img/tutorials/habitat-networks/importance-of-each-patch/dpc.png "dPC at 2 km, on a logarithmic scale from 0.001% to 30%. The large forests (55, 110, 8) dominate, but medium woods in the north (47, 30, 96) and small ones between them (65, 81, 58, 100) rank far above their size.")

The importance of the 154 patches, summed, is 216%: dPC values overlap (the flux between two patches is lost when
either is removed, so it counts in both), and they are compared with each other, never added. The ranking at 2
km:

| Patch | Area (ha) | Rank by area | dPC (%) | intra | flux | connector |
|---|---|---|---|---|---|---|
| 55 | 9,151 | 1 | 28.7 | 16.2 | 12.2 | 0.4 |
| 110 | 7,071 | 2 | 22.6 | 9.7 | 11.9 | 1.0 |
| 8 | 4,599 | 3 | 16.0 | 4.1 | 11.0 | 1.0 |
| 47 | 2,442 | 6 | 15.8 | 1.2 | 9.0 | **5.6** |
| 96 | 2,860 | 5 | 13.6 | 1.6 | 9.4 | 2.5 |
| 30 | 2,884 | 4 | 13.5 | 1.6 | 9.3 | 2.6 |
| 65 | 406 | 21 | 6.6 | 0.0 | 1.7 | **4.9** |
| 100 | 322 | 26 | 4.5 | 0.0 | 1.4 | **3.1** |
| 58 | 357 | 24 | 4.2 | 0.0 | 1.5 | **2.6** |
| 81 | 46 | 92 | 3.3 | 0.0 | 0.2 | **3.1** |

```tfchart
type: bar
data: /data/tutorials/habitat-networks/importance-of-each-patch/top-patches.csv
x: patch
y: [intra, flux, connector]
labels: { intra: "dPCintra", flux: "dPCflux", connector: "dPCconnector" }
colors: { intra: "#3b8a5a", flux: "#0072b2", connector: "#e8901f" }
stacked: true
ylabel: "% of PC lost without the patch"
height: 320
caption: "The twelve most important patches at 2 km and the three parts of their dPC. The large southern forest 55 matters mostly for its own habitat; 47 and the small woods 65, 100 and 58 mostly as stepping stones."
```

Three kinds of patches appear:

- **Large reservoirs** (55, 110, 8): most of their importance is intra and flux. They hold habitat and exchange
  dispersers with their neighbours. Losing one would cost a fifth to a quarter of the network.
- **Hubs** (47, 30, 96): forests of 2,400 to 2,900 ha in the north with a large flux *and* a large connector
  fraction. Patch 47 is the most important connector of the landscape: it ties the north-west to the woods of
  the north-centre.
- **Stepping stones** (65, 100, 58, 81): small woods (46 to 406 ha) whose dPC is almost entirely connector.
  Patch 81, of 46 ha, ranks 92nd by area and 16th by importance. These are the patches an area-based ranking
  would sacrifice first.

```tfgallery
columns: 3
caption: "The three fractions of dPC at 2 km, on the same logarithmic scale. Intra follows area exactly; flux follows area and neighbours; connector picks out a chain of woods across the north."
images:
  - { src: /img/tutorials/habitat-networks/importance-of-each-patch/intra.png, caption: "dPCintra" }
  - { src: /img/tutorials/habitat-networks/importance-of-each-patch/flux.png, caption: "dPCflux" }
  - { src: /img/tutorials/habitat-networks/importance-of-each-patch/connector.png, caption: "dPCconnector" }
```

### Importance against area

![dPC of every patch against its area, coloured by the share of dPC as a connector](/img/tutorials/habitat-networks/importance-of-each-patch/dpc-vs-area.png "Each point is a patch: its area (x) and its dPC (y), both logarithmic; the colour is the share of its dPC that is connector. Most patches lie on a band where importance follows area; the yellow and orange points above the band are stepping stones (numbered), worth ten to a hundred times more than their area alone.")

The rank correlation (Spearman's) between area and dPC is 0.84: area explains most of the ranking, which is why
large patches are a safe first priority. It is 1.00 for dPCintra (it *is* area), 0.90 for dPCflux, but only 0.56
for dPCconnector. The patches off the band are the reason to compute dPC at all.

### Which links matter most

![The forty most important links at 2 km](/img/tutorials/habitat-networks/importance-of-each-patch/links.png "The forty links whose removal would cost the most PC, drawn between the patches' centres (thicker and darker: more important). A single chain carries the network across the north, from the north-west through 30, 47, 58, 65, 77 and 81 to the north-centre.")

Of the 2,457 links (pairs with a probability above `pij_min`), only 286 would cost any PC if cut: the rest are
redundant, as the best path between their patches goes another way. The most important is the link between 30
and 47 (5.6% of PC, a gap of 1.1 km), then 96–100 (3.4%, 300 m), and a run of short links along the northern
chain: 47–58 (2.9%), 65–77 and 58–65 (2.8% each), 77–81 (2.8%). Read with the patches, the result is a
**corridor of small woods** that holds the north together: protecting it means protecting the woods *and* the
gaps between them (hedges, field margins, the farmland a species crosses).

### Betweenness and connector

On the links up to 2 km, betweenness ranks 47 first (2,819), then 30, 65, 110 and 96; the rank correlation with
dPCconnector is 0.72. They agree on the hubs. They disagree on patches such as 77 (47 ha): its betweenness is 0,
because its neighbours 65 and 81 are 1.8 km apart, within 2 km of each other, and the shortest path in *steps*
skips it; but its dPCconnector is 2.8%, because the path through it (gaps of 300 and 361 m) is far *likelier*
than the direct jump (\(0.5^{300/2000} \times 0.5^{361/2000} = 0.79\) against \(0.5^{1800/2000} = 0.54\)).
This is the difference Bodin and Saura (2010) described: when the question is about dispersal, prefer the
probabilities.

!!! paper "In the literature: reporting patch importance"
    Give the dispersal distance and probability, the ranking by dPC with its three fractions (a stacked bar of
    the top patches, as above), and a map. State that dPC values overlap and are not added. When the analysis
    feeds a plan, report the stepping stones separately: their low area makes them easy to overlook. Saura and
    Rubio (2010) and Saura et al. (2011, forest connectors from MSPA) are good models.

## Settings that change the answer

- **The dispersal distance.** At 250 m almost all importance is intra (part 2: the fractions), and the ranking
  is the ranking by area. The connector role appears between 1 and 5 km, and its winners change with the
  distance: compute dPC at two or three distances for a group of species, and look for patches important at all
  of them.
- **The probability.** A larger `probability` at the same `distance` makes every move likelier, so fewer patches
  are needed as stepping stones.
- **The attribute.** With habitat quality as the attribute (`attribute`, `weighted = true` in
  [[conn.graph_from_raster]]), a large but poor patch loses importance to a small, good one.
- **The distance measure.** Least-cost distances change which links are short; [part 4](../least-cost-links/)
  shows that the stepping stones of the north lose most of their role when the farmland between them is costly.

## Common pitfalls

- **Adding dPC values.** The dPC of two patches is not the loss if both are removed. Remove them together.
- **Ranking by dPC without the fractions.** The top of the list is mostly large patches, which everyone already
  values. The connector fraction is where the analysis adds information.
- **One distance for all species.** A stepping stone for a species of 2 km may be irrelevant for one of 500 m.
- **The border.** Patches near the edge of the study area lose neighbours outside it and look less important.
- **Ties at zero.** Many patches have dPCconnector = 0: they relay nothing. A ranking by connector should start
  with those above zero.

## Going further

- [Part 4: least-cost links](../least-cost-links/): the importance of the patches when the matrix is not uniform.
- [Part 5: the MSPA network](../mspa-network/): the importance of the bridges and cores of a forest's structure.
- [Part 6: ProtConn](../protconn/): the importance of each reserve for the connectivity of a protected network.
- [Costs and corridors](/tutorials/distance-restoration/costs-and-corridors/): where to restore a corridor
  between two forests.
- The operators: [[conn.node_importance]], [[conn.link_importance]], [[conn.link_change]], [[conn.centrality]],
  [[conn.focal_nodes]] (importance within a neighbourhood of each patch), [[conn.to_map]], [[conn.to_vector]].

## References

- Bodin, Ö., & Saura, S. (2010). Ranking individual habitat patches as connectivity providers: integrating network analysis and patch removal experiments. *Ecological Modelling*, 221(19), 2393–2405. https://doi.org/10.1016/j.ecolmodel.2010.06.017
- Pascual-Hortal, L., & Saura, S. (2006). Comparison and development of new graph-based landscape connectivity indices: towards the priorization of habitat patches and corridors for conservation. *Landscape Ecology*, 21(7), 959–967. https://doi.org/10.1007/s10980-006-0013-z
- Saura, S., & Rubio, L. (2010). A common currency for the different ways in which patches and links can contribute to habitat availability and connectivity in the landscape. *Ecography*, 33(3), 523–537. https://doi.org/10.1111/j.1600-0587.2009.05760.x
- Saura, S., Vogt, P., Velázquez, J., Hernando, A., & Tejera, R. (2011). Key structural forest connectors can be identified by combining landscape spatial pattern and network analyses. *Forest Ecology and Management*, 262(2), 150–160. https://doi.org/10.1016/j.foreco.2011.03.017
- Urban, D., & Keitt, T. (2001). Landscape connectivity: a graph-theoretic perspective. *Ecology*, 82(5), 1205–1218. https://doi.org/10.1890/0012-9658(2001)082[1205:LCAGTP]2.0.CO;2
