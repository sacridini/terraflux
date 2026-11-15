---
title: Habitat connectivity graphs
description: "Conefor Sensinode and Makurhini in Python: nodes and distances, the indices, importances, ProtConn, SAMC and the conventions."
order: 90
---

The same in the app: [Connectivity as a graph](/tutorials/habitat-networks/connectivity-as-a-graph/), [Connectivity indices](/tutorials/habitat-networks/connectivity-indices/), [Importance of each patch](/tutorials/habitat-networks/importance-of-each-patch/), [Least-cost links](/tutorials/habitat-networks/least-cost-links/) and [ProtConn](/tutorials/habitat-networks/protconn/). The same as a worked tutorial in Python: [Habitat networks in Python](/tutorials/habitat-networks/habitat-networks-in-python/). API: [`tf.HabitatGraph`][terraflux.HabitatGraph] and the `tf.conn` functions, on the [habitat networks](/python/api/networks/) page. The graphs reproduce Makurhini and Conefor Sensinode (see [how each tool is checked](/about/validation/#habitat-graphs-in-detail)).

The graph-based connectivity indices of Saura and colleagues, as computed by
[Conefor Sensinode](http://www.conefor.org) and the R package
[Makurhini](https://github.com/connectscape/Makurhini): habitat patches are the
nodes (with an attribute, usually their area), and the distances (or dispersal
probabilities, or links) between them are the connections.

The first examples use a small network of four patches, their areas (ha) and the distances between them (m):

```python
import numpy as np
import terraflux as tf

areas = [100, 50, 20, 80]
distances = np.array([
    [0,    800,  1500, 5000],
    [800,  0,    600,  4000],
    [1500, 600,  0,    3000],
    [5000, 4000, 3000, 0],
])
patch_ids, total_area = [1, 2, 3, 4], 10_000
LA = total_area
```

```python
# nodes: attribute per patch; connections: n × n distance matrix or (from, to, distance) table
g = tf.HabitatGraph(areas, distances, ids=patch_ids, landscape_area=total_area)

g.overall(distance=[5_000, 10_000], probability=0.5)   # one row per distance
g.node_importance(["PC", "IIC"], distance=10_000)      # dPC, dPCintra, dPCflux, dPCconnector, dIIC...
g.components(distance=10_000)                          # component of every node
g.link_importance("PC", distance=10_000)               # dPC_removal of every link (MK_dPCIIC_links)
new_distances = [(1, 2, 2000)]                         # the pair 1-2 becomes 2 km apart
g.link_change(new_distances, "PC", distance=10_000)    # dPC_change of pairs with new distances
g.node_importance(["BC", "BCIIC", "BCPC"], distance=10_000)  # Conefor's betweenness centralities
g.centrality(10_000)                                   # degree, eigen, close, BWC, cluster (MK_RMCentrality)
g.centrality(10_000, probability=0.5)                  # weighted by p (strength, 1/p as lengths)

# ECA over time (Makurhini's MK_dECA): one graph per time
g_2000, g_2010, g_2020 = g, g, g                       # one graph per date; the same graph here, to run it
tf.conn.eca_change([g_2000, g_2010, g_2020], "PC", distance=10_000, landscape_area=LA)

# nodes that could be added (restoration: 1 existing, 0 to add)
flags = [1, 1, 1, 0]
g_res = tf.HabitatGraph(areas, distances, restoration=flags)
g_res.node_importance(["PC", "IIC"], distance=10_000)  # + dPCres, dIICres of the nodes to add

# Conefor's own files
g.to_conefor("nodes.txt", "distances.txt")
g = tf.HabitatGraph.from_conefor("nodes.txt", "distances.txt", convention="conefor")
```

The next ones start from the patches of a map: the forests of a crop of the CORINE map of the library, as a raster of patch ids, as polygons, and, as in the [habitat networks tutorial](/tutorials/habitat-networks/habitat-networks-in-python/#connectivity-of-protected-areas-protconn), the large forests as protected areas and the map extent as the region:

```python
import geopandas as gpd
import rasterio.features
import shapely
from scipy import ndimage

landcover = tf.load_raster(tf.example_data("clc"))[:800, :800]
tf.write_raster(landcover, "landcover.tif")

forest = np.isin(landcover.values, [23, 24, 25])
ids, n = ndimage.label(forest, structure=np.ones((3, 3)))          # a raster of patch ids
tf.write_raster(landcover.copy(data=ids.astype("int32")), "patches.tif")
sizes = np.bincount(ids.ravel())
large = np.isin(ids, np.flatnonzero(sizes > 1000)) & (ids > 0)     # the larger forests
tf.write_raster(landcover.copy(data=ndimage.label(large)[0].astype("int32")), "protected_ids.tif")
shapes = rasterio.features.shapes(large.astype("uint8"), mask=large, transform=landcover.rio.transform())
protected_areas = gpd.GeoDataFrame(geometry=[shapely.geometry.shape(s) for s, _ in shapes], crs=landcover.rio.crs)
patches_gdf = protected_areas.assign(patch_id=range(1, len(protected_areas) + 1))
region = gpd.GeoDataFrame(geometry=[shapely.box(*landcover.rio.bounds())], crs=landcover.rio.crs)
ecoregions = region.assign(eco_id=1)
forest_2000 = forest_2020 = patches_gdf                            # one set of patches per date

resistance = tf.reclassify(landcover, {23: 1.0, 24: 1.0, 25: 1.0}, others=10.0, dtype="float64")
tf.write_raster(resistance, "resistance.tif")
```

```python
# Protected Connected land (Makurhini's MK_ProtConn, MK_ProtConnMult)
tf.conn.protconn(protected_areas, region, distance=[10_000, 50_000], transboundary=50_000,
                 bound=True, area_unit="ha", delta=True)
tf.conn.protconn_regions(protected_areas, ecoregions, id="eco_id", distance=10_000)
tf.conn.protconn_raster("protected_ids.tif", region, distance=10_000, transboundary=20_000)

# settlement probabilities of a spatial absorbing Markov chain (Makurhini's MK_dPC_SAMC)
g = tf.HabitatGraph.from_samc(patches_gdf, "resistance.tif", dispersal_distance=5_000, area_unit="ha")
g.node_importance("PC", pij_min=0.001)

# focal connectivity (Makurhini's MK_Focal_nodes): every node in its neighbourhood
tf.conn.focal_nodes(patches_gdf, id="patch_id", metric="PC", distance=5_000, search_buffer=10_000)

# grids (Makurhini's make_grid, MK_Connect_grid, MK_dECA_grid)
cells = tf.conn.make_grid(region, cell_area=1e7)            # hexagons of 10 km2
tf.conn.connectivity_grid(protected_areas, region, grid=cells, distance=10_000)
tf.conn.eca_change_grid([forest_2000, forest_2020], region, grid=cells, distance=5_000, metric="PC")

# nodes from a raster of patch ids (or the patches of habitat classes), or from polygons
g = tf.HabitatGraph.from_raster("patches.tif", area_unit="ha", distance="edge", max_distance=20_000)
g = tf.HabitatGraph.from_raster("landcover.tif", habitat=[23, 24, 25], directions=8)
g = tf.HabitatGraph.from_vector(patches_gdf, id="patch_id", area_unit="km2", distance="centroid")
imp = g.node_importance("PC", distance=10_000)
g.to_geodataframe(imp)             # node values with the geometries (from_vector)

g = tf.HabitatGraph.from_raster("patches.tif", area_unit="ha", distance="edge", max_distance=20_000)
imp = g.node_importance("PC", distance=10_000)
g.to_raster(imp, "dPC")            # node values on the patches (from_raster)

g = tf.HabitatGraph.from_raster("patches.tif", distance="least-cost", resistance="resistance.tif")

tf.conn.node_table("patches.tif", area_unit="ha")              # Makurhini's nodesfile
tf.conn.distance_table(patches_gdf, id="patch_id", distance="edge", max_distance=5_000)  # distancefile
```

## Nodes and distances from maps

`node_table` and `distance_table` are Makurhini's `nodesfile` and `distancefile`
(Euclidean distances; the map units must be metres):

* **Raster of patch ids** (NA or `nodata` = no habitat; with `habitat=` the
  `directions`-connected patches of those classes are labelled first): the
  attribute is the number of cells × the cell width² in `area_unit` (or given per
  patch, `weighted` multiplies it by the area). Centroids are the means of the cell
  centres (`raster::xyFromCell`, averaged in 80-bit precision as R's `colMeans`);
  edge distances are the gap between the closest cells of two patches (what GEOS
  measures between the polygons of `terra::as.polygons`), 0 when they touch.
* **Polygons or points** (GeoDataFrame): the attribute is the area as sf's
  `st_area` computes it (GDAL's shoelace sums), or a column. Centroids are those
  of the largest part, or a point on the surface when that centroid falls outside
  (Makurhini's `st_centroid_within_poly`); edge distances come from GEOS.

* **Least-cost distances** (`distance="least-cost"`, `resistance=` a raster; NA cells
  cannot be crossed) are gdistance's `costDistance` as Makurhini calls it: between
  neighbouring cells (`cost_directions` 8 or 4) the conductance is 1 / the resistance of
  the cell of larger index (`cost_function="makurhini"`, its default `1/x[2]`) or
  1 / the mean of both (`"mean"`), divided by the distance between the cell centres;
  a step costs its reciprocal, and the distance between two nodes is the cheapest
  path between the cells of their centroids, in cost units (resistance × map units).
  Pairs without any path (other components of the resistance, nodes outside it or on
  NA cells) get their centroid distance, as in Makurhini (`unreachable="drop"` leaves
  them unconnected). Dijkstra runs from every node in parallel over the raster (each
  pair summed from its later node, as gdistance keeps the lower triangle), stops when
  the nodes before it are reached or at `max_distance`, and uses the same step costs
  (raster centre coordinates, R's x87 `sqrt`, `r_mean`): the distances are gdistance's
  to the last bit.
* **Commute-time distances** (`distance="commute-time"`, the same `resistance`,
  `cost_directions` and `cost_function`) are gdistance's `commuteDistance`: the expected
  time of a random walk over the same conductances from one node's cell to the other's
  and back, i.e. their effective resistance (as in Circuitscape) times the total
  conductance. The Laplacian of every connected part of the conductance graph, grounded
  at one node cell, is factorised once (SciPy's SuperLU) and solved for the other node
  cells in blocks; the distances agree with gdistance (CHOLMOD) to 1e-11. As in
  gdistance, nodes on NA cells or outside the resistance are left out; pairs in
  separate parts (where gdistance fails) get their centroid distance, or none with
  `unreachable="drop"`.

`max_distance` keeps the pairs up to that distance (in `distance_unit`), which is
what Makurhini's `threshold` does in pair tables (in its matrices it drops the
pairs up to it instead). Raster edge distances search the boundary cells of each
patch row by row, from the nearest row outwards, and stop at the gap of the
bounding boxes; with `max_distance`, only patches whose boxes come that close are
compared.

## Indices

| index | meaning | node importance |
|---|---|---|
| `NC`, `NL` | number of components and of links | `dNC`, `dNL` |
| `H` | Harary index: sum of 1 / (links between two nodes) | `dH` |
| `CCP`, `LCP` (`LCPnum`) | sum of the squared component attributes over the total attribute² / LA² | `dCCP`, `dLCP` |
| `IIC` (`IICnum`, `EC(IIC)`) | integral index of connectivity: ΣΣ a_i a_j / (1 + nl_ij) / LA² | `dIIC`, `dIICintra`, `dIICflux`, `dIICconnector` |
| `F`, `AWF` | flux and area-weighted flux: ΣΣ p_ij, ΣΣ a_i a_j p_ij | `dF`, `dAWF` |
| `PC` (`PCnum`, `EC(PC)`) | probability of connectivity: ΣΣ a_i a_j p*_ij / LA² (p* = most probable path) | `dPC`, `dPCintra`, `dPCflux`, `dPCconnector`, `varPC` |
| `A` | total attribute | `dA` |

`overall` also gives Makurhini's split of IIC and PC into intra-patch, direct and
stepping-stone parts (`PCintra(%)`, `PCdirect(%)`, `PCstep(%)`). A node's importance is
the loss of the index without it, in % (`varPC` is the loss of PC itself); dI =
intra + flux + connector. Without `landscape_area`, `IIC`, `PC` and `LCP` are NaN (only
their numerators, as in Makurhini) under the default convention, and use LA = 1 (as
Conefor) with `convention="conefor"`.

## Centrality

* `node_importance(["BC", "BCIIC", "BCPC"], ...)` (asked for by name, not in the
  default list) gives Conefor's betweenness centralities (Bodin & Saura 2010): `BC`,
  Freeman's betweenness of the links normalised by (n − 1)(n − 2) / 2; `varBC_IIC`, over
  ordered pairs the share of their shortest paths through the node times
  a_i a_j / (1 + nl_ij), and `dBC_IIC` in % of IICnum; `varBC_PC`, a_i a_j p*_ij of the
  pairs whose most probable path runs through the node, and `dBC_PC` in % of PCnum.
  They come from the same searches as the other indices (Brandes' accumulation).
* `centrality(distance, probability=None, binary=True)` is Makurhini's
  `MK_RMCentrality` (igraph): binary, the graph of the links, with `degree`; weighted
  (`binary=False`, or a `probability`), every pair with its probability p, 1 / p as
  the length of the edge and the weight of `strength` and eigenvector centrality.
  `eigen` is the leading eigenvector (the largest 1), `close` igraph's closeness
  (1 / the sum of the distances to the reached nodes; weighted, normalised: times
  their number), `BWC` igraph's betweenness (paths equal within 1e-10 all count),
  `cluster` the component. With python-igraph installed, `memb_rw` holds the walktrap
  communities (weighted by p, as in Makurhini); Makurhini's Louvain communities
  depend on a random order and are left out.

## Focal nodes

`tf.conn.focal_nodes(polygons, distance=, search_buffer=, metric="IIC")` is Makurhini's
`MK_Focal_nodes` (Latorre-Cárdenas et al. 2023). Around every node (the focal node),
the polygons intersecting its buffer of `search_buffer` (sf's 30 segments per quarter
circle; one buffer per distance, or one for all) form a graph: distances between their
points on surface, the landscape area the area of their bounding box. With three or more
nodes it gives the neighbourhood's `EC(IIC)` and `IIC` (or PC), the focal node's `dIIC`
and its intra/flux/connector fractions (Makurhini's `MK_dPCIIC` conventions) and the
composite connectivity index `IComp` = IIC × dIIC; with fewer, NaN. The attribute is the
area in `area_unit` (Makurhini: always ha), a column or values. Makurhini simplifies the
focal polygon with mapshaper before buffering it, which only matters for nodes at the
very edge of the buffer; terraflux buffers the polygon itself. Makurhini's
`fragmentation` statistics are not included.

## Spatial absorbing Markov chains (SAMC)

`HabitatGraph.from_samc(polygons, resistance, dispersal_distance=)` is Makurhini's
`MK_dPC_SAMC` (Fletcher et al. 2023, the samc package): the patches are connected by
the probability of settling in one when starting from the other, under a random walk
over the resistance raster. The cells with a resistance are the transient states; from
a cell the walk steps to a neighbour (`directions` 4 or 8, samc's order) with probability
(1 − absorption) · w / Σw, w = 1 / the mean resistance of both cells (samc's default;
`cost_function="makurhini"`: 1 / the neighbour's) over √2 for diagonal steps, and settles
with the cell's absorption: by default 0 in the patches and exp(−1.247 − 1.788 ·
log(dispersal distance / cell size)) elsewhere, or an `absorption` raster. With
F = (I − Q)⁻¹ (one sparse LU factorisation, solved for the vertex cells), settling in cell
d from cell o has probability F[o, d] · absorption[d]; between two patches it is the
`settle` (`"sum"` or `"mean"`) of those values from the boundary vertices of the later
patch to those of the earlier one (`boundary_vertices`: Makurhini's `SR_vertices`,
`vertices` 10 by default or a `vertex_fraction`, picked systematically). The graph has
`kind="probability"`; `node_importance("PC", pij_min=0.001)` gives `MK_dPC_SAMC`'s
values. `tf.conn.settlement_probabilities` returns the matrix itself. Makurhini's default
absorption rasterizes its own example data (`habitat_nodes`) instead of the given
patches; terraflux uses the given patches. Random vertex sampling and mapshaper's
simplification (`keep`) are not ported.

## Links, changes and nodes to add

* `link_importance(metric)` is Makurhini's `MK_dPCIIC_links` with `removal`: the loss
  of IIC or PC (in %) without each link (the links of the IIC graph, the edges above
  `pij_min` of the PC graph). Other pairs change nothing; Makurhini lists them too, with
  0 (PC) or, for IIC, the same share of its unconnected pairs as the links carry.
* `link_change(changes, metric)` is its `change`: for each pair whose value differs
  in `changes` (a pair table or a matrix), `100 (I - I') / I` (negative for a gain).
  For IIC a pair that comes closer than the distance gains a link and anything else
  counts 0; for PC the pair's edge takes the probability of its new value, without the
  `pij_min` cut, as in Makurhini.
* `restoration=` (1 existing, 0 to add, as Makurhini's column) rates the nodes to add:
  with the default convention `node_importance` adds `dPCres` / `dIICres`, the gain of
  each node on its own over the existing ones in % of the graph with it (Makurhini's
  code misaligns the attributes unless the nodes to add come last; terraflux uses
  the definition). With `convention="conefor"` it is Conefor's "adding nodes" mode:
  `overall` and the removal importances are those of the existing nodes, and every
  index gets `d...add` = its gain in % of the existing graph (`dAadd` ... `dPCadd`).

Removing a link repairs, in every source's shortest-path tree, the subtree below the
link (as for a node). Changes that shorten a pair and nodes to add use the lengths
between all nodes: the new length of a pair is `min(D_ij, D_ia + w + D_bj, ...)`, or
through the node to add. A longer pair is removed and added back with its new length.

## ProtConn

`tf.conn.protconn(protected_areas, region, distance=...)` is Makurhini's `MK_ProtConn`
(Saura et al. 2017, 2018), one row per distance: `ECA`, `PC`, `LA` (the region's area),
`Protected_surface`, `Prot`, `ProtConn`, `ProtUnconn`, `RelConn`, `Unprotected`, the
fractions `ProtConn_Prot`, `_Trans`, `_Unprot`, `_Within`, `_Contig` and their `_land`
shares, and with `bound=True` `ProtUnconn_Design` and `ProtConn_Bound`. The nodes
(`protconn_nodes`) are Makurhini's: the protected areas that touch the region, cut to
it, dissolved and split into separate polygons (their area in the region); with
`transboundary` the parts of them outside the region and the protected areas within
that distance (`transboundary_type="nodes"`: of the region's areas; `"region"`: of the
region), cut to that zone, are transboundary nodes (attribute 0); the region itself
joins them with `bound` (only next to transboundary nodes, as in Makurhini). Distances
run between polygon edges (GEOS) or are least-cost over a `resistance`, probabilities are
`exp(d log(p) / t)` without a cut, and `protconn_indicators` applies
`get_protconn_grid`'s formulas to any prepared nodes and distances. Makurhini weighs the
pairs with a transboundary node 1 instead of a_i a_j (`transboundary_weight=1`, the
default; 0 makes them stepping stones only). `delta=True` also rates each protected area
(`dProt`, `dProtConn`, `varProtConn` once removed; Makurhini weighs the transboundary
pairs 0 in the region's ECA but 1 without the area, terraflux uses one weight).
`protconn_regions` runs every region of a GeoDataFrame (`MK_ProtConnMult`). A region
with one dissolved node gets MK_ProtConn's fixed values for it.

Makurhini simplifies the region and the zones' polygons with mapshaper before
buffering them (Visvalingam, 5 % of the vertices); terraflux buffers the exact
polygons (30 segments per quarter circle, as sf), so both select the same areas when
the simplification keeps the shapes (as for the reference polygons).

## ProtConn from rasters and on grids

* `protconn_raster(protected_ids, region, ...)` is `MK_ProtConn_raster`: every
  protected-area id inside the region (rasterized on the protected areas' grid, cells
  whose centre is inside) is a node with the area of its cells there; transboundary
  nodes are the ids outside the region within `transboundary` (Makurhini keeps them only
  when there are two or more); distances run between the nodes' centroids. As in
  Makurhini, ProtConn[Within] sums the attributes of every node. (Makurhini pairs the
  attributes, sorted by id, with the centroids in the order the ids appear in the
  raster, and fails without two transboundary nodes; terraflux keeps each id with its
  centroid.)
* `make_grid(region, cell_area)` gives Makurhini's grid: sf's `st_make_grid(square =
  FALSE)` hexagons (the same coordinates) or raster squares over the region's extent,
  the cells touching the region (`clip`, `grid_boundary` as in Makurhini).
* `connectivity_grid(protected_areas, region, grid=|cell_area=, distance=)` is
  `MK_Connect_grid`: ProtConn of every cell (the cell as the region, the areas selected
  once for the whole region, the other cells' areas transboundary), or with
  `protconn_index=False` PC, ECA and normalised ECA of the patches dissolved in each
  cell. Makurhini's default `pij_min` becomes infinity there and cuts every link (each
  node alone): `pij_min=np.inf` reproduces it, the default keeps every link.
* `eca_change_grid(times, region, ...)` is `MK_dECA_grid`: the ECA change between
  consecutive times in each cell, with Makurhini's rules for cells where the habitat
  appears or disappears.

## ECA over time

`tf.conn.eca_change(graphs, metric, distance=, landscape_area=)` is Makurhini's `MK_dECA`:
for one graph per time (or scenario) the habitat area (the node areas of graphs made
by `from_raster` / `from_vector`, summed as R does, else the sum of the attributes),
the ECA (EC(PC) or EC(IIC), capped at LA), both in % of LA and of the habitat area,
`dA` and `dECA` (change from the previous time, the first from LA), `rECA = dECA / dA`
and Makurhini's reading of them ("+ Connectivity loss", "+ Habitat loss", ...). A graph
may hold a single node (its ECA is its attribute). Makurhini rounds some columns to
3 decimals (which ones depends on the number of times); terraflux does not round.

## Connections and conventions

`connections` holds distances (default), `kind="probability"` direct dispersal
probabilities, or `kind="link"` 0/1 links, as Conefor's connection types. A square
matrix is read from its upper triangle (NaN = not connected); in a pair table,
missing pairs are not connected. The probability of a distance d is the negative
exponential kernel `exp(d log(probability) / distance)`.

| | `convention="makurhini"` (default) | `convention="conefor"` |
|---|---|---|
| links (IIC, H, NC...) | pairs closer than `distance` (d < t) | pairs up to `distance` (d ≤ t) |
| `pij_min` | 0.01: probabilities below it are dropped | 0: Conefor's `minProb` heuristic (links at or below it dropped, paths no more probable not extended) |
| unconnected pairs in IIC | count as 16443701 links apart (MK_dPCIIC's value for them) in the whole graph, not in the graphs without a node | add nothing |
| without `landscape_area` | indices NaN, numerators reported | LA = 1 |
| `PC > 1`, `EC > LA` | capped, as MK_dPCIIC | not capped |

Makurhini's default distance (the median distance, `distance=None`) is also the
default here. Makurhini rounds its node importances to 7 decimals and its overall
fractions to 5 (sharing the rounding error out so they add to 100); terraflux
returns the unrounded values.

## How it works

Each shortest path is found once per source node, in parallel over the sources:
breadth-first search for links, Dijkstra over −log p for probabilities. The node
importances (dI) need the index of the graph without each node. Conefor and
Makurhini recompute the whole graph once per node; here, removing a node k only
changes the paths that ran through it, which are the subtree of k in each source's
shortest-path tree. Their new lengths come from a Dijkstra restricted to that
subtree, seeded from untouched neighbours. For links, most subtree nodes keep their
level through another neighbour and are skipped. For probabilities, the four best
ways into every node, kept while the tree is built, give most seeds without
scanning the neighbours. The per-node contributions are merged in source order, so
the results do not depend on the number of threads.

## References

- Pascual-Hortal, L., & Saura, S. (2006). Comparison and development of new graph-based landscape connectivity indices: towards the priorization of habitat patches and corridors for conservation. *Landscape Ecology*, 21(7), 959–967. https://doi.org/10.1007/s10980-006-0013-z
- Saura, S., & Pascual-Hortal, L. (2007). A new habitat availability index to integrate connectivity in landscape conservation planning: comparison with existing indices and application to a case study. *Landscape and Urban Planning*, 83(2–3), 91–103. https://doi.org/10.1016/j.landurbplan.2007.03.005
- Saura, S., & Rubio, L. (2010). A common currency for the different ways in which patches and links can contribute to habitat availability and connectivity in the landscape. *Ecography*, 33(3), 523–537. https://doi.org/10.1111/j.1600-0587.2009.05760.x
- Saura, S., & Torné, J. (2009). Conefor Sensinode 2.2: a software package for quantifying the importance of habitat patches for landscape connectivity. *Environmental Modelling & Software*, 24(1), 135–139. https://doi.org/10.1016/j.envsoft.2008.05.005
- Godínez-Gómez, O., Correa-Ayram, C., Goicolea, T., & Saura, S. (2026). Makurhini: an R package for comprehensive analysis of landscape fragmentation and connectivity. *Environmental Modelling & Software*, 201, 106981. https://doi.org/10.1016/j.envsoft.2026.106981

More references, with how the studies used each method, are at the end of the application's tutorials of the same subject (linked at the top of this page).
