---
title: Protected-area connectivity (ProtConn)
short: ProtConn
description: Measure how much of a region is both protected and connected with Saura's ProtConn, read its fractions, rank the reserves by their contribution, and test whether protecting a few stepping stones would do more than protecting more area.
order: 60
level: intermediate
time: 45 min
ops: [conn.protconn, conn.protconn_raster, conn.protconn_regions, vector.read, vector.write, conn.nodes, conn.to_map, raster.calc]
series: Habitat networks
part: 6
---

International targets ask countries to protect a share of their land in systems of protected areas that are
"well connected". The area protected is easy to measure; how well it is connected was not, until Saura and
colleagues defined **ProtConn**, the percentage of a region that is protected *and* reachable from the other
protected areas. In this last part of the series you use it on the landscape around Hannover: you make a network
of hypothetical reserves from its largest forests, compute ProtConn and its fractions at five dispersal
distances, find which reserves matter most, and test a plan that adds small stepping stones instead of large
areas. At the end you can compute and report ProtConn for any region and any set of protected areas.

## The idea

### Protected and connected

In 2010 the Convention on Biological Diversity set Aichi Target 11: by 2020, at least 17% of land conserved
through "ecologically representative and well-connected systems of protected areas". Its successor, the
Kunming-Montreal framework of 2022, raised the area to 30% and kept the words "well-connected". Saura et al.
(2017) proposed the indicator that is now used to follow that second half of the target, **ProtConn**, and
computed it for every terrestrial ecoregion; Saura et al. (2018) did the same for every country and found large
gaps, in many countries, between the area protected and the area protected and connected, and Saura et al. (2019)
followed its change from 2010 to 2018.

ProtConn is the equivalent connected area of [part 2](../connectivity-indices/), with the protected areas as the
nodes, expressed as a share of the region:

!!! science "Key idea: ProtConn"
    With the protected areas \(i\) (areas \(a_i\)) of a region of area \(A_L\), and \(p^*_{ij}\) the
    probability of the best path between them for a species of median dispersal distance \(d\)
    (\(p = 0.5\) at \(d\)),

    \[ \mathrm{ProtConn} = 100 \times \frac{\sqrt{\sum_i \sum_j a_i\, a_j\, p^*_{ij}}}{A_L} = 100 \times \frac{\mathrm{ECA}}{A_L} \]

    - \(\mathrm{Prot} = 100 \times \sum_i a_i / A_L\) is the share of the region protected. ProtConn can never
      exceed it.
    - \(\mathrm{ProtUnconn} = \mathrm{Prot} - \mathrm{ProtConn}\) is the protected share that is *not*
      connected: the shortfall due to isolation.
    - \(\mathrm{RelConn} = 100 \times \mathrm{ProtConn} / \mathrm{Prot}\) is the percentage of the protected
      land that is connected.

    The paths may go through **unprotected land** (a gap of farmland), through **other protected areas**, and
    through protected areas **outside the region** (*transboundary*, within a distance of it).

ProtConn also splits into fractions that sum to 100%: **ProtConn_Prot**, the connectivity through protected
land only (itself split into *Within*, inside each protected area, and *Contig*, between touching protected
areas), **ProtConn_Unprot**, the part that depends on crossing unprotected land, and **ProtConn_Trans**, the part
that depends on protected areas outside the region. A high Unprot fraction says that the network works only if
the land *between* the reserves stays permeable: a target for land use policy, not for new reserves.

## When to use it

- **Reporting on area-based targets**: ProtConn is the indicator of the CBD's framework for "well-connected"
  systems of protected areas, computed for countries, ecoregions and provinces.
- **Comparing regions** with [[conn.protconn_regions]], which gives a row per region of a layer.
- **Evaluating a plan**: ProtConn before and after adding (or degazetting) protected areas, and the contribution
  of each protected area (dProtConn), to choose where to protect next.
- **Mapping it** over a grid of hexagons with [[conn.connectivity_grid]].

ProtConn is about **protected areas**, not habitat: a reserve counts with all its area, forest or not. For the
connectivity of a habitat, use [parts 1 to 4](../connectivity-as-a-graph/).

## What you need

- **The region**, as a polygon (or several, dissolved), in a projected coordinate system in metres. Here,
  [`region.geojson`](/data/tutorials/habitat-networks/protconn/region.geojson): the 50 × 50 km square of the
  series, in EPSG:3035.
- **The protected areas**, as polygons in the same coordinate system. Real studies use the World Database on
  Protected Areas or a national register. Here, to keep the example in one landscape, the reserves are
  hypothetical: **the 18 forests of 500 ha or more**, made polygons in
  [`reserves.gpkg`](/data/tutorials/habitat-networks/protconn/reserves.gpkg) (217 KB, 39,964 ha, 16.0% of the
  region). They are not the real protected areas of Lower Saxony.
- **One or more median dispersal distances.** Saura et al. used 10 km as the reference, with other distances to
  cover species that move less or more. Here 1, 2, 5, 10 and 20 km.

![The hypothetical reserves and the stepping stones added in the second scenario](/img/tutorials/habitat-networks/protconn/reserves.png "The reserves of the example: the 18 forests of 500 ha or more (dark green). Orange: the 17 forests of 40 to 500 ha whose stepping-stone role (dPCconnector at 2 km, part 3) is 0.5% or more, protected in the second scenario. Pale green: the other forests.")

## Build the model

### Making the reserves (optional)

If your protected areas are already polygons, skip this. The reserves of the example were made from the forest
map: [[conn.nodes]] gives every patch's area, [[conn.to_map]] paints the areas and the patch numbers on the
patches, and [[raster.calc]] keeps the patches of 500 ha or more (`None` elsewhere: no value). The result is a map
of reserves, which [[conn.protconn_raster]] reads directly.

```tfgraph
#! caption: Reserves made from the forest map (every forest of 500 ha or more), and ProtConn computed on the raster, with the patches' centroids as nodes.
#! numbered: true
#! highlight: reserves, protconn
format = "terraflux/1"

[[nodes]]
id = "land"
op = "raster.read"
params = { path = "clc_100m.tif" }
note = "CORINE Land Cover, cells of 100 m."

[[nodes]]
id = "graph"
op = "conn.graph_from_raster"
inputs = { map = "land.raster" }
params = { habitat = [23, 24, 25], area_unit = "ha" }

[[nodes]]
id = "patches"
op = "conn.nodes"
inputs = { map = "land.raster" }
params = { habitat = [23, 24, 25], area_unit = "ha" }
note = "Every forest patch and its area (ha)."

[[nodes]]
id = "area"
op = "conn.to_map"
inputs = { graph = "graph.graph", table = "patches.table" }
params = { column = "attribute" }

[[nodes]]
id = "patch"
op = "conn.to_map"
inputs = { graph = "graph.graph", table = "patches.table" }
params = { column = "id" }

[[nodes]]
id = "reserves"
op = "raster.calc"
inputs = { a = "area.map", p = "patch.map" }
params = { expression = "p if a >= 500 else None" }
note = "The forests of 500 ha or more, each with its number."

[[nodes]]
id = "region"
op = "raster.calc"
inputs = { l = "land.raster" }
params = { expression = "1" }
note = "The region: every cell of the map."

[[nodes]]
id = "protconn"
op = "conn.protconn_raster"
inputs = { protected = "reserves.map", region_map = "region.map" }
params = { distance = [1000, 2000, 5000, 10000, 20000], area_unit = "ha" }
```

The raster version is quick, but its nodes are the reserves' **centroids**, so the distances between reserves
are measured centre to centre. ProtConn's definition uses the distance between the reserves' **edges**, which
the vector version below computes. On this example the raster version gives ProtConn = 6.5% at 5 km, the vector
version 9.3%: large reserves are far apart centre to centre and close edge to edge. Use the vector version for
anything you report.

### ProtConn of the protected areas

<div class="steps" markdown>

1. **Read the two layers** with [[vector.read]]: `reserves.gpkg` and `region.geojson`.
2. **ProtConn** ([[conn.protconn]], library subject **Habitat networks**). Connect the region to `region` and
   the reserves to `protected`. Set `distance` to the median dispersal distances, in the layers' units
   (metres): `[1000, 2000, 5000, 10000, 20000]`. `probability` (0.5) is the probability at that distance.
   `area_unit = "ha"` gives the ECA in hectares.
3. **The contribution of each reserve**: set `delta = true`. The output `delta` is then the reserves clipped to
   the region, with `dProt`, `dProtConn` and `varProtConn` per distance. Write it with [[vector.write]] to
   open it in a GIS.

</div>

```tfgraph
#! caption: ProtConn of a set of protected areas in a region at five median dispersal distances, with every protected area's contribution written to a file.
#! numbered: true
#! highlight: protconn
format = "terraflux/1"

[[nodes]]
id = "reserves"
op = "vector.read"
params = { path = "reserves.gpkg" }
note = "The protected areas (polygons, EPSG:3035)."

[[nodes]]
id = "region"
op = "vector.read"
params = { path = "region.geojson" }
note = "The region (a polygon)."

[[nodes]]
id = "protconn"
op = "conn.protconn"
inputs = { region = "region.vector", protected = "reserves.vector" }
params = { distance = [1000, 2000, 5000, 10000, 20000], area_unit = "ha", delta = true }
note = "A row per distance; delta: each reserve's importance."

[[nodes]]
id = "save_delta"
op = "vector.write"
inputs = { vector = "protconn.delta" }
params = { path = "delta.geojson" }
```

The other settings of [[conn.protconn]]:

- `transboundary`: protected areas outside the region within this distance of it (or of the nodes,
  `transboundary_type`) take part as stepping stones. Here the
  layer has only the region's reserves, so the Trans fraction is 0.
- `bound = true`: the region's own parts as nodes, for ProtConn_Bound (the connectivity a region of its shape
  could reach at most).
- `distance_type = "least-cost"` with a `resistance` map: least-cost distances between reserves, as in
  [part 4](../least-cost-links/).

## Read the results

### ProtConn at five distances

```tfchart
type: bar
data: /data/tutorials/habitat-networks/protconn/protconn.csv
x: distance
y: [ProtConn, ProtUnconn]
labels: { ProtConn: "ProtConn (protected and connected)", ProtUnconn: "ProtUnconn (protected, not connected)" }
colors: { ProtConn: "#3b8a5a", ProtUnconn: "#d9a441" }
stacked: true
xlabel: Median dispersal distance
ylabel: "% of the region"
height: 300
caption: "ProtConn and ProtUnconn of the 18 reserves at five distances; together they make Prot, the 16.0% of the region protected. At 10 km, the reference distance of the global assessments, 11.5% of the region is protected and connected and 4.5% protected but isolated."
```

| Median dispersal distance | Prot (%) | ProtConn (%) | ProtUnconn (%) | RelConn (%) | ECA (ha) |
|---|---|---|---|---|---|
| 1 km | 16.0 | 6.2 | 9.8 | 38.8 | 15,525 |
| 2 km | 16.0 | 7.1 | 8.9 | 44.6 | 17,836 |
| 5 km | 16.0 | 9.3 | 6.7 | 58.4 | 23,329 |
| 10 km | 16.0 | 11.5 | 4.5 | 71.9 | 28,712 |
| 20 km | 16.0 | 13.3 | 2.7 | 83.3 | 33,283 |

For a species of 10 km, 72% of the protected land is connected (RelConn); for a species of 1 km, 39%: the
reserves are close enough to each other for wide-ranging mammals and birds, not for most forest invertebrates or
small mammals. A target of 17% "well connected" would be missed at every distance even though Prot (16%) is close
to it.

### Through protected or unprotected land

```tfchart
type: bar
data: /data/tutorials/habitat-networks/protconn/fractions.csv
x: distance
y: [Prot, Unprot, Trans]
labels: { Prot: "ProtConn_Prot (through protected land)", Unprot: "ProtConn_Unprot (across unprotected land)", Trans: "ProtConn_Trans (through reserves outside)" }
colors: { Prot: "#3b8a5a", Unprot: "#e8901f", Trans: "#0072b2" }
stacked: true
percent: true
xlabel: Median dispersal distance
ylabel: "% of ProtConn"
height: 280
caption: "ProtConn's fractions. At 1 km, 87% of the connectivity is inside the reserves themselves; at 10 km and beyond, more than half depends on crossing the unprotected farmland between them."
```

At short distances ProtConn is mostly the reserves' own area: there is little movement between them. At 10 km
and more, most of it **depends on the unprotected land** between the reserves. That land is the network's weak
point: if it became harder to cross (more towns, roads, intensive crops), ProtConn would fall without a hectare of
reserve lost.

### Which reserves matter most

![The contribution of each reserve to ProtConn at 5 km](/img/tutorials/habitat-networks/protconn/dprotconn.png "dProtConn at 5 km: the share of ProtConn lost without each reserve. The large southern forest (9,151 ha) carries 22.9%; the forest of 4,599 ha in the south-west 13.3% and the north-eastern one (7,071 ha) 10.9%.")

The `delta` output ranks the reserves. At 5 km the largest southern forest (9,151 ha) carries 22.9% of ProtConn,
the south-western forest of 4,599 ha 13.3%, ahead of the larger north-eastern one (7,071 ha, 10.9%), probably
because it is within reach of the southern forest while the north-eastern one is farther from the other large
reserves.
As in part 3, importance follows area first and position second.

### A plan: stepping stones or more area?

The reserves leave a gap across the north and the centre. The second scenario protects, in addition, the 17
forests of 40 to 500 ha that [part 3](../importance-of-each-patch/) found to be stepping stones
(dPCconnector of 0.5% or more at 2 km): 3,133 ha more, raising Prot from 16.0% to 17.2%.

```tfchart
type: line
data: /data/tutorials/habitat-networks/protconn/protconn-added.csv
x: distance
y: [ProtConn, ProtConn2]
labels: { ProtConn: "18 reserves (Prot 16.0%)", ProtConn2: "with 17 stepping stones (Prot 17.2%)" }
colors: { ProtConn: "#3b8a5a", ProtConn2: "#e8901f" }
xlabel: Median dispersal distance
ylabel: "ProtConn (% of the region)"
height: 280
caption: "ProtConn with and without the stepping stones. 1.2 points of extra protected area raise ProtConn by 1.8 points at 5 km and 2.0 at 10 km: more than their own area, because they connect reserves that were apart."
```

At 5 km, ProtConn rises from 9.3% to 11.2% (+1.8 points) and RelConn from 58% to 65%; at 10 km, from 11.5% to
13.5%. The stepping stones add **more connected area than their own area**: they bring reserves that were
isolated into reach of each other. At 1 km they add only 0.4 points: for a species that moves little, small
reserves far from others count only for themselves. This is the reasoning behind connectivity-aware
protected-area planning: the next hectare protected is worth most where it joins what is already there.

!!! paper "In the literature"
    Report Prot, ProtConn, ProtUnconn and RelConn at each distance, the fractions (Prot, Unprot, Trans), the
    data of the protected areas (source, date, which designations), how overlapping areas were treated (they
    are dissolved), and whether transboundary areas were included. Saura et al. (2017, 2018) are the models,
    and their methods are implemented by Makurhini (Godínez-Gómez et al., 2026), whose numbers Terraflux
    reproduces.

## Settings that change the answer

- **The median dispersal distance** changes ProtConn more than anything else (6.2% to 13.3% here). Give it at
  several distances, or at the reference distance and one shorter.
- **Edge or centroid distances** (vector or raster versions): 9.3% against 6.5% at 5 km.
- **Transboundary areas.** Ignoring the protected areas just outside the region underestimates ProtConn,
  especially for small regions; include them with `transboundary` when the data allow.
- **Least-cost distances** lower ProtConn where the land between reserves is hard to cross, and are closer to
  the target's spirit (connected for species), at the price of a resistance map to justify.
- **The protected areas included**: strict reserves only, or all designations. Say which.

## Common pitfalls

- **Overlapping protected areas.** A national park that overlaps a Natura 2000 site must not count twice:
  [[conn.protconn]] dissolves the protected areas clipped to the region, as Makurhini does.
- **Geographic coordinates.** Areas and distances need a projected, equal-area system: reproject the layers
  first.
- **A region larger than the data.** Protected areas missing from the layer count as unprotected land.
- **Comparing ProtConn with habitat ECA.** ProtConn is about protected land, whatever its habitat; a reserve of
  farmland counts as much as one of forest.

## Going further

- [Part 2](../connectivity-indices/): ECA and the probability of connectivity.
- [Part 3](../importance-of-each-patch/): the importance of each patch, used here to choose the stepping stones.
- [Spatial prioritization](/tutorials/spatial-prioritization/systematic-conservation-planning/): choosing new
  protected areas for many features at once.
- The operators: [[conn.protconn]], [[conn.protconn_raster]], [[conn.protconn_regions]],
  [[conn.connectivity_grid]], [[conn.make_grid]].

## References

- Godínez-Gómez, O., Correa-Ayram, C., Goicolea, T., & Saura, S. (2026). Makurhini: an R package for comprehensive analysis of landscape fragmentation and connectivity. *Environmental Modelling & Software*, 201, 106981. https://doi.org/10.1016/j.envsoft.2026.106981
- Saura, S., Bastin, L., Battistella, L., Mandrici, A., & Dubois, G. (2017). Protected areas in the world's ecoregions: how well connected are they? *Ecological Indicators*, 76, 144–158. https://doi.org/10.1016/j.ecolind.2016.12.047
- Saura, S., Bertzky, B., Bastin, L., Battistella, L., Mandrici, A., & Dubois, G. (2018). Protected area connectivity: shortfalls in global targets and country-level priorities. *Biological Conservation*, 219, 53–67. https://doi.org/10.1016/j.biocon.2017.12.020
- Saura, S., Bertzky, B., Bastin, L., Battistella, L., Mandrici, A., & Dubois, G. (2019). Global trends in protected area connectivity from 2010 to 2018. *Biological Conservation*, 238, 108183. https://doi.org/10.1016/j.biocon.2019.07.028
