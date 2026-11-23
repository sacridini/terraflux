---
title: "Vector analysis, part 2: shapes, lines, Voronoi cells and clusters"
short: Shapes, lines, clusters
description: Measure and reshape features one at a time, work along lines, divide an area into the catchments of points, group records by distance, and smooth or simplify polygons without opening gaps between them.
order: 20
level: intermediate
time: 60 min
ops: [vector.geometry, vector.centroid, vector.hull, vector.simplify, vector.points_along, vector.locate, vector.voronoi, vector.cluster, vector.smooth, vector.coverage_simplify, vector.coverage_validate]
series: Vector analysis
part: 2
---

[Part 1](../select-join-overlay/) compared layers with each other. This part is about the **shape of a single
feature** and about what a set of features makes together: the centroid and the hull of a patch, the points along a
road and where each record falls along it, the area closer to one monitoring station than to any other, the groups
that a scatter of records forms, and polygons simplified or smoothed so that neighbours still fit. At the end you
will have catchments for six stations with the records in each, the colonies that the records make, and a
polygon map simplified with no gap between its polygons.

## The idea: what a geometry is, and what a set of them makes

A geometry is a list of coordinates, and many questions are about the list rather than the thing it stands for.
Four ideas carry this part.

**Measures along a line.** A line has a length, and every point on it has a *measure*: the distance along the line
from its start. With a measure you can place a point (a station every 2 km), cut a piece (the middle half), or ask
the opposite question: for a record near a road, how far along the road is the nearest point? This is **linear
referencing**, the way roads and rivers have always been given addresses (kilometre 12 of the highway).

**Catchments by distance.** Given a set of points (stations, nests, wells), the **Voronoi polygon** of each is the
area that is closer to it than to any other point (Okabe et al., 2000). It is the simplest catchment: the stations'
service areas if everyone goes to the nearest. Its dual, the **Delaunay triangulation**, joins points that have
neighbouring cells into triangles of the most even shape possible (Delaunay, 1934); its edges are the network of
neighbours.

**Groups by distance.** **DBSCAN** (Ester et al., 1996) finds groups in points without being told how many: a point
with at least `min_points` neighbours within a distance is a *core*; cores that are close to one another make a
cluster, points close to a core join it, and the rest is **noise**. Nests of a colony, a cluster of records, a
hotspot of fires: the distance and the minimum size are the two numbers to choose.

**Simplifying without breaking.** A coastline drawn with a million vertices is too much for a map at 1:100,000.
**Douglas and Peucker's** (1973) method drops the vertices that deviate less than a tolerance; **Visvalingam and
Whyatt's** (1993) drops the ones that make the smallest triangles. Applied to each polygon alone, either one moves
the shared border of two neighbours differently on each side and opens **gaps and overlaps** between them. A set of
polygons that meet along shared edges, with no gap and no overlap, is a **coverage** (a map of municipalities, a
polygonized land cover), and it is simplified *together* so that the shared edges move once, for both.

!!! science "Key idea: smoothing is not simplifying"
    **Chaikin's** (1974) corner cutting replaces every segment by two points, at a quarter and three quarters of
    it: the corners are cut off and a line of stair steps becomes a curve. It makes more vertices, not fewer, and
    moves the line inwards, so it is for drawing (the stair steps of a polygonized map), not for measuring.

![Voronoi cells and a coverage simplified](/img/tutorials/vector-analysis/shapes-lines-clusters/voronoi-coverage.png "Left: the Voronoi catchments of six stations, cut to the 20 km study area, with the simulated records coloured by the catchment that holds them. Middle and right: the same patch of a land cover coverage simplified with a 60 m tolerance, polygon by polygon (red: gaps, black: overlaps) and together.")

## When to use it

- **Where is the middle of this patch, for a label?** [[vector.centroid]], with `inside` for a point that always
  lies on the feature.
- **What is the extent of a species' records or a group?** [[vector.hull]], convex or concave, one per group with
  `by`.
- **Where along a road is each record? Stations every 2 km?** [[vector.locate]], [[vector.points_along]],
  [[vector.substring]] and [[vector.split_lines]].
- **Which station is each record nearest to? How large is each catchment?** [[vector.voronoi]], then a join.
- **Which records form a group?** [[vector.cluster]].
- **A map of polygons is too detailed or too jagged.** [[vector.simplify]] for a single layer of separate
  features, [[vector.coverage_simplify]] when they share edges, [[vector.smooth]] to draw them.

It is not for what a *raster* needs: distance maps from features are [[vector.distance]]; clusters of cells are a
raster task. Its relations between two layers are in [part 1](../select-join-overlay/), and the table of
equivalents with PostGIS, GDAL and QGIS is [there too](../select-join-overlay/#the-equivalence-table).

## What you need

The layers of [part 1](../select-join-overlay/#what-you-need), in this part's own folder, and four more. The records
are simulated; the forest, water and towns are cut from a CORINE Land Cover window; the stations stand in six forest
patches; the study area is the square of the window.

| File | Features | What it is |
|---|---|---|
| [records.gpkg](/data/tutorials/vector-analysis/shapes-lines-clusters/records.gpkg) | 355 points | the simulated records |
| [forest.gpkg](/data/tutorials/vector-analysis/shapes-lines-clusters/forest.gpkg) | 71 polygons | the forest patches |
| [road.gpkg](/data/tutorials/vector-analysis/shapes-lines-clusters/road.gpkg) | 1 line | the planned road, 23.3 km |
| [stations.gpkg](/data/tutorials/vector-analysis/shapes-lines-clusters/stations.gpkg) | 6 points | monitoring stations `S1` to `S6`, each on one of the six largest forest patches |
| [window.gpkg](/data/tutorials/vector-analysis/shapes-lines-clusters/window.gpkg) | 1 polygon | the study area, 20 by 20 km |
| [cover.gpkg](/data/tutorials/vector-analysis/shapes-lines-clusters/cover.gpkg) | 19 polygons | the land cover of a 5 km square as patches that share their edges: a coverage |
| [cover_raw.gpkg](/data/tutorials/vector-analysis/shapes-lines-clusters/cover_raw.gpkg) | 19 polygons | the same patches as a polygonized map gives them, with no shared vertices |

Everything is in EPSG:3035 (metres), so every distance and tolerance below is in metres.

## Shapes: one feature at a time

These operations take a layer and give back a layer, a feature for each feature, with its attributes. They keep
the coordinate system.

| Node | What it gives | Settings that matter |
|---|---|---|
| [[vector.geometry]] | the columns `area`, `perimeter`, `length`, `x` and `y` | `geodesic`: areas in m² on the ellipsoid, for layers in degrees |
| [[vector.centroid]] | each feature as a point | `inside`: a point guaranteed to lie on the feature |
| [[vector.hull]] | each feature's convex or concave hull | `kind`, `ratio` (1 is the convex hull, 0 the tightest), `by` (one hull per value of an attribute), `dissolve` (one hull of everything) |
| [[vector.simplify]] | each geometry with fewer vertices | `tolerance`, in the layer's units; `preserve_topology` |
| [[vector.envelope]] | the bounding box | |
| [[vector.make_valid]] | invalid polygons repaired (a bow-tie becomes two polygons) | |
| [[vector.explode]] | one feature for each part of a multi-part feature | |
| [[vector.boundary]] | the rings of a polygon as lines | |

The **centroid** of a concave polygon (a C, an L, a river bend) can fall *outside* it. For a label or a sample
point, use `inside`. A **hull** is the outline that wraps the points as a rubber band (convex), or hugs them more
tightly (concave); with `by` it is the extent of each group, as a minimum convex polygon is the extent of an
animal's records (see [Animal movement](/tutorials/animal-movement/) for home ranges proper).

### In Python

```python
import terraflux as tf

v = tf.vector
records = tf.load_vector("records.gpkg")       # the 355 simulated records
forest = tf.load_vector("forest.gpkg")         # the 71 forest patches
stations = tf.load_vector("stations.gpkg")     # 6 monitoring stations, one in each of the six largest patches
window = tf.load_vector("window.gpkg")         # the study area, a square of 20 km
road = tf.load_vector("road.gpkg")             # the planned road

m = v.measures(forest)                         # area, perimeter, length, x and y, in metres
print(m[["patch", "type", "area_ha", "area", "perimeter"]].head(3).round(0).to_string())
```

```text
   patch        type  area_ha        area  perimeter
0      1  coniferous   2118.0  21181250.0    78400.0
1      2  coniferous   1914.0  19138750.0   102900.0
2      3  coniferous    600.0   5996250.0    31250.0
```

The centroids of the forest patches are on the patch itself for 61 of 71; the other 10 fall outside their own patch
(in a clearing, or a bend). The point on the surface never does:

```python
centre = v.centroid(forest)                    # the centroid of every patch
label = v.centroid(forest, inside=True)        # a point that lies on it
misses = lambda pts: sum(not p.within(g) for p, g in zip(pts.geometry, forest.geometry))
print(misses(centre), misses(label))
```

```text
10 0
```

Simplification, repair and the splitting of multi-part features:

```python
import shapely

nv = lambda g: sum(len(shapely.get_coordinates(x)) for x in g.geometry)    # vertices of a layer
simple = v.simplify(forest, 50)                # Douglas-Peucker, 50 m, topology kept
print(nv(forest), nv(simple), v.make_valid(forest).geometry.is_valid.all())
types = v.dissolve(forest, by="type")          # one multi-part feature for each type
print(len(types), len(v.explode(types)), sorted(set(v.boundary(forest).geom_type)))
```

```text
15399 2056 True
3 71 ['LineString', 'MultiLineString']
```

A tolerance of 50 m takes the patches from 15,399 to 2,056 vertices; the patches come from a grid of 25 m cells, so
a tolerance of two cells removes the stair steps but keeps the outline. `explode` gives back the 71 single parts of
the three types.

## Lines and linear referencing

A **measure** is the distance along a line from its start, in the units of the layer. These nodes work with
measures:

| Node | What it gives |
|---|---|
| [[vector.points_along]] | points at a `spacing` (or `n` in all) along each line, with their `part` and `measure` |
| [[vector.substring]] | the piece of each line between two measures (`normalized`: as fractions of its length) |
| [[vector.split_lines]] | each line cut into pieces of at most `length` |
| [[vector.locate]] | for each feature of a layer, the nearest line of another, the `measure` along it of the closest point, and the `distance` |
| [[vector.shortest_line]] | the line from each feature to its nearest feature in another layer, with that layer's attributes and the `distance` |
| [[vector.closest_point]] | each point moved onto the nearest point of the other layer (snapped), with `moved` and `snapped`; `max_distance` leaves the far ones alone |

A multi-part line is taken part by part, and the measures start from 0 on each part (`part` tells them apart). The
last piece of a split is the remainder, shorter than `length`.

```tfgraph
#! caption: Stations every 2 km along the road, the road's middle half, and where each record falls along it.
#! highlight: stops, middle, along
format = "terraflux/1"

[[nodes]]
id = "road"
op = "vector.read"
params = { path = "road.gpkg" }

[[nodes]]
id = "records"
op = "vector.read"
params = { path = "records.gpkg" }

[[nodes]]
id = "stops"
op = "vector.points_along"
inputs = { vector = "road.vector" }
params = { spacing = 2000 }
note = "A point every 2 km, with its measure."

[[nodes]]
id = "middle"
op = "vector.substring"
inputs = { vector = "road.vector" }
params = { start = 0.25, end = 0.75, normalized = true }
note = "The middle half of the road."

[[nodes]]
id = "along"
op = "vector.locate"
inputs = { vector = "records.vector", lines = "road.vector" }
note = "line, measure and distance for every record."
```

### In Python

```python
stops = v.points_along(road, spacing=2000)    # a point every 2 km from the start
print(len(stops), stops[["part", "measure"]].tail(2).to_string())
middle = v.substring(road, 0.25, 0.75, normalized=True)    # the middle half
pieces = v.split_lines(road, 5000)             # pieces of at most 5 km
print(round(road.length.iloc[0]), round(middle.length.iloc[0]), pieces.length.round().tolist())
```

```text
12     part  measure
10     0  20000.0
11     0  22000.0
23291 11646 [5000.0, 5000.0, 5000.0, 5000.0, 3291.0]
```

The road is 23,291 m long, so there are 12 stops (0 to 22 km) and the last of five pieces is 3,291 m. Where along the
road are the records that are within 500 m of it, and how far is each record that lies in no forest from the
nearest forest patch?

```python
along = v.locate(records, road)                # the nearest line, the measure along it, the distance
print(along[["record", "line", "measure", "distance"]].head(3).round(0).to_string())
close = along[along["distance"] <= 500]
print(len(close), round(close["measure"].min()), round(close["measure"].max()))
lone = v.disjoint(records, forest)             # the records in no forest patch
link = v.shortest_line(lone, forest)           # a line from each to the nearest patch
print(len(link), round(link["distance"].median()), round(link["distance"].max()))
onto = v.closest_point(records, road, max_distance=500)    # the records within 500 m, moved onto the road
print(int(onto["snapped"].sum()), round(onto["moved"].max()))
```

```text
   record  line  measure  distance
0       1     0   9131.0    3121.0
1       2     0  19359.0    5565.0
2       3     0  17427.0    5896.0
26 543 17845
100 161 2577
26 482
```

The 26 records near the road are spread from kilometre 0.5 to kilometre 17.8 of it; the 100 records in no forest
are at a median of 161 m from the nearest patch and at most 2.6 km. `line` is the position in the layer of lines
(always 0 here, there is one road): join it back with [[vector.nearest]] when you need the line's attributes.

## Voronoi catchments and Delaunay triangles

[[vector.voronoi]] gives each point the polygon of the area nearest to it, in the order of the points and with
their attributes. Without a limit the cells of the points at the edge of the set run off to infinity, so GEOS
(the geometry library the application uses) cuts them at an arbitrary margin around the points: those cells are not
catchments of anything. Connect the study area to the optional `clip` input, or give `clip` in Python, and every
cell is cut to it. A point whose cell is cut away entirely keeps an empty one.

[[vector.delaunay]] triangulates all the vertices of a layer: triangles by default, the edges as lines with `edges`.
It takes no attributes, only the geometry.

```tfgraph
#! caption: The catchment of each station within the study area, and the number of records and birds in each.
#! highlight: cells, per_cell
format = "terraflux/1"

[[nodes]]
id = "stations"
op = "vector.read"
params = { path = "stations.gpkg" }

[[nodes]]
id = "window"
op = "vector.read"
params = { path = "window.gpkg" }

[[nodes]]
id = "records"
op = "vector.read"
params = { path = "records.gpkg" }

[[nodes]]
id = "cells"
op = "vector.voronoi"
inputs = { vector = "stations.vector", clip = "window.vector" }
note = "One polygon for every station, cut to the study area."

[[nodes]]
id = "per_cell"
op = "vector.spatial_join"
inputs = { vector = "cells.vector", other = "records.vector" }
params = { predicate = "contains", mode = "summary", fields = ["birds"] }
note = "Records and birds in every catchment."
```

### In Python

```python
cells = v.voronoi(stations, clip=window)       # one polygon per station, cut to the study area
loose = v.voronoi(stations)                    # not cut: GEOS's own margin
print((cells.geometry.area / 1e4).round().tolist())
print((loose.geometry.area / 1e4).round().tolist())
home = v.join(records, cells[["station", "geometry"]], "within", how="first")
print(home["station"].value_counts(dropna=False).to_string())
tin = v.delaunay(stations)
print(len(tin), round(v.delaunay(stations, edges=True).length.sum()))
```

```text
[8737.0, 7100.0, 4478.0, 7870.0, 4866.0, 6949.0]
[63831.0, 28002.0, 53125.0, 40552.0, 4866.0, 59847.0]
station
S1    123
S2     89
S6     81
S5     33
S3     22
S4      7
5 85304
```

The six catchments add up to 40,000 ha, the study area. Without `clip` five of the six cells are four to twelve
times larger (the largest, 63,831 ha, is 638 km² in a study area of 400): that margin is not a catchment. Only the
inner cell, S5, is the same in both. The 355 records are shared among the stations as the table shows (all of them
are inside the study area, so none is lost), and the triangulation of six points has five triangles.

## Clusters of records

[[vector.cluster]] adds a column of cluster numbers (`cluster` by default) to a layer: DBSCAN on the *distance
between geometries*, so the features may be points, lines or polygons. Its two settings are the
`distance` (the neighbourhood, in the layer's units) and `min_points` (how many features, itself included, make a
core). The clusters are numbered 0, 1, 2... by their lowest-numbered feature, and the noise is `-1`. With
`min_points = 1` there is no noise: every chain of features within the distance of each other is a group.

To draw each group, keep the clustered features with [[vector.filter]] (`cluster >= 0`) and give [[vector.hull]] `by =
"cluster"`.

```tfgraph
#! caption: Group the records that lie within 600 m of at least four others, leave the isolated ones out, and wrap each group in a concave hull.
#! highlight: groups, colonies
format = "terraflux/1"

[[nodes]]
id = "records"
op = "vector.read"
params = { path = "records.gpkg" }

[[nodes]]
id = "groups"
op = "vector.cluster"
inputs = { vector = "records.vector" }
params = { distance = 600, min_points = 5 }
note = "A cluster number for every record; -1 for noise."

[[nodes]]
id = "kept"
op = "vector.filter"
inputs = { vector = "groups.vector" }
params = { expression = "cluster >= 0" }
note = "Without the noise."

[[nodes]]
id = "colonies"
op = "vector.hull"
inputs = { vector = "kept.vector" }
params = { kind = "concave", ratio = 0.4, by = "cluster" }
note = "The extent of every group, and how many records it holds."
```

### In Python

```python
groups = v.cluster(records, 600, min_points=5)     # DBSCAN: 600 m, at least 5 records (itself included)
print(groups["cluster"].value_counts().sort_index().to_string())
colonies = v.hull(groups[groups["cluster"] >= 0], by="cluster", kind="concave", ratio=0.4)
print(colonies.assign(ha=colonies.geometry.area / 1e4)[["cluster", "count", "ha"]].round(0).to_string())
```

```text
cluster
-1     59
 0     54
 1    111
 2     66
 3     12
 4     24
 5     15
 6     14
   cluster  count     ha
0        0     54  135.0
1        1    111  189.0
2        2     66  567.0
3        3     12   40.0
4        4     24   78.0
5        5     15   99.0
6        6     14   87.0
```

Seven groups and 59 records of noise. The largest group, with 111 records, spreads over 189 ha; the group with 66
records over 567 ha. A concave hull with `ratio` 0.4 follows the points more closely than their convex hull would,
and needs GEOS 3.11 or later.

## Settings that change the answer

The distance and the minimum size decide what a cluster is. The same 355 records, with four distances and three
sizes of core:

```tftable
csv: |
  distance_m,min_points,clusters,noise
  300,1,104,0
  300,5,7,143
  300,10,3,194
  600,1,57,0
  600,5,7,59
  600,10,7,91
  900,1,44,0
  900,5,6,48
  900,10,6,52
  1200,1,37,0
  1200,5,6,43
  1200,10,6,44
digits: 0
labels: { distance_m: "Distance (m)", min_points: "Core (records)", clusters: "Clusters", noise: "Noise (records)" }
caption: "DBSCAN on the 355 records. With a core of 1 every record is in a group (chains of records); a larger core leaves isolated records as noise and cuts the thin bridges between groups."
```

Read it along the rows of `min_points = 5`: from 300 to 1,200 m the number of groups hardly changes (6 or 7), but
the noise falls from 143 to 43 records, so what the distance decides is how many records are admitted to a group,
not how many groups there are. A core of 1 is another question (how many chains of nearby records are there?), and
its 37 to 104 groups are no clusters in the ecological sense. Choose the distance from what the animal does (the
distance it moves in a day, the territory's radius) and report it.

## Smoothing and coverages

[[vector.smooth]] applies Chaikin's corner cutting to lines and polygons, `iterations` times (1 to 10), keeping the
ends of a line with `preserve_ends`. [[vector.coverage_validate]] and [[vector.coverage_simplify]] work on a layer
of polygons that must form a **valid coverage**: no overlap, and every shared edge matching its neighbour's *vertex
by vertex*. [[vector.coverage_simplify]] moves each shared edge once, for both neighbours, with a `tolerance`, and
`preserve_boundary` leaves the outer outline of the whole as it is. [[vector.coverage_validate]] gives each feature
with a column `valid` and, as its geometry, the edges that are not (empty when the feature is valid); `gap_width`
also flags edges closer than that to a neighbour.

!!! warning "A polygonized map is not a coverage as it comes"
    The polygons that [[vector.polygonize]] (and GDAL's polygonize) make have only their own corners. Where a
    neighbour's edge meets the junction of two other patches, that edge has no vertex there, and the coverage
    check calls the edges unmatched. In `cover_raw.gpkg`, 14 of 19 patches fail it; in `cover.gpkg`, the same
    patches with their boundaries noded (cut at every junction), all 19 pass. Always check with
    [[vector.coverage_validate]]; the application has no node that nodes a layer yet, so in Python it takes three
    lines of shapely (below).

```tfgraph
#! caption: Check that the polygons form a coverage, simplify them together with a 60 m tolerance, and smooth a copy to draw it.
#! highlight: valid, together, drawn
format = "terraflux/1"

[[nodes]]
id = "cover"
op = "vector.read"
params = { path = "cover.gpkg" }

[[nodes]]
id = "valid"
op = "vector.coverage_validate"
inputs = { vector = "cover.vector" }
note = "valid is 1 where a feature's edges match its neighbours'."

[[nodes]]
id = "together"
op = "vector.coverage_simplify"
inputs = { vector = "cover.vector" }
params = { tolerance = 60 }
note = "Shared edges simplified once, for both neighbours."

[[nodes]]
id = "drawn"
op = "vector.smooth"
inputs = { vector = "together.vector" }
params = { iterations = 2 }
note = "For drawing, not for measuring."
```

### In Python

```python
cover_raw = tf.load_vector("cover_raw.gpkg")   # 19 patches as a polygonized map gives them
cover = tf.load_vector("cover.gpkg")           # the same patches with shared vertices (noded)
print(len(cover_raw), int(v.coverage_validate(cover_raw)["valid"].sum()), int(v.coverage_validate(cover)["valid"].sum()))
alone = v.simplify(cover, 60)                  # each polygon alone
together = v.coverage_simplify(cover, 60)      # the polygons together
print(int(v.coverage_validate(alone)["valid"].sum()), int(v.coverage_validate(together)["valid"].sum()))
gaps = lambda g: round(cover.area.sum() - v.union(g).geometry.area.iloc[0])     # area no polygon holds
print(gaps(alone), gaps(together))
smooth = v.smooth(cover_raw, iterations=2)
print(round(cover_raw.length.sum()), round(smooth.length.sum()))
```

```text
19 5 19
1 19
290151 0
146650 127693
```

Simplified one by one, the 19 polygons of 25 km² leave 290,151 m² (1.2%) that no polygon holds, and overlap where
neighbours moved the other way; only one polygon of the 19 still has valid edges. Simplified together, none
is lost and all 19 are valid. Smoothing two times takes 19 km off the 147 km of outlines, and its polygons no longer
fit one another either: it is for the picture.

The noding of a polygonized layer, in shapely (this is how `cover.gpkg` was made from `cover_raw.gpkg`):

<!-- no-run -->
```python
import numpy as np, geopandas as gpd, shapely

lines = shapely.node(shapely.union_all(shapely.boundary(cover_raw.geometry.values)))       # cut at every junction
pieces = shapely.get_parts(shapely.polygonize(shapely.get_parts(lines)))
ip, ic = shapely.STRtree(cover_raw.geometry.values).query(shapely.point_on_surface(pieces), predicate="within")
classes = np.zeros(len(pieces), dtype=int)
classes[ip] = cover_raw["class"].values[ic]                                                # the class under each piece
noded = gpd.GeoDataFrame({"class": classes}, geometry=pieces, crs=cover_raw.crs)
```

## Common pitfalls

- **Metres, not degrees.** A tolerance, a spacing, a clustering distance or a buffer is in the units of the layer.
  Reproject to a metric system first (the tips of every one of these nodes say so).
- **Voronoi without a clip.** The cells at the edge of the points reach GEOS's margin and have no meaning as
  catchments: always give the study area, and compare the areas as above.
- **Duplicate points.** Two points at the same place share one Voronoi cell, and every duplicate row gets that same
  cell. A set with a single distinct place gives empty polygons.
- **Delaunay and equal choices.** When four points lie on one circle (the corners of a square, a regular grid), both
  diagonals make a valid Delaunay triangulation, and the geometry library chooses one: different versions of GEOS
  can choose differently, and the triangles come in the order GEOS gives them. Do not rely on one diagonal (the
  example is below the list).
- **The concave hull and the coverage tools need new GEOS.** The concave hull needs GEOS 3.11 or later;
  `coverage_simplify` and `coverage_validate` need GEOS 3.12 or later (shapely 2.1). An older one refuses with a
  message that says so.
- **`simplify` on neighbours.** Use it for a layer of separate features; for a coverage it opens gaps and overlaps,
  as shown above. Use [[vector.coverage_simplify]] and validate the layer first.
- **A centroid is not on the feature.** For a concave polygon use `inside`; for a patch with several parts, the
  centroid of the whole can be in none of them.
- **DBSCAN counts the point itself.** `min_points = 5` means the point and four others. And `-1` is noise: do not
  take it for a group when counting clusters.
- **Linear referencing on multi-part lines.** Measures restart at 0 on each part; a `locate` on a multi-line
  measures in sequence along its parts, as GEOS does.
- **Smoothing before measuring.** It shrinks polygons and doubles the vertices at each iteration; measure first.

The cocircular case is four points at the corners of a square. The triangulation has two triangles, and which
diagonal splits them is up to the library; what you read below is what this computer's GEOS gives:

```python
import geopandas as gpd
from shapely.geometry import Point

square = gpd.GeoSeries([Point(0, 0), Point(1, 0), Point(1, 1), Point(0, 1)])    # four points on one circle
print(len(v.delaunay(square)))
print(v.delaunay(square).geometry.to_wkt().tolist())
```

<!-- varies -->
```text
2
['POLYGON ((0 1, 0 0, 1 0, 0 1))', 'POLYGON ((0 1, 1 0, 1 1, 0 1))']
```

## Going further

- [Part 1](../select-join-overlay/) and its [equivalence table](../select-join-overlay/#the-equivalence-table):
  every operation of the two parts, with its name in PostGIS, GDAL, QGIS and geopandas.
- [Vector data](../../getting-started/vector-data/) and the [vector calculator](/calculators/vector/).
- [Animal movement](/tutorials/animal-movement/): home ranges, which are hulls with a probability behind them.
- The reference of [`tf.vector`](/python/api/vector/), and the operators [[vector.voronoi]], [[vector.cluster]],
  [[vector.coverage_simplify]], [[vector.locate]].

## References

- Chaikin, G. M. (1974). An algorithm for high-speed curve generation. *Computer Graphics and Image Processing*, 3(4), 346-349. https://doi.org/10.1016/0146-664X(74)90028-8
- Delaunay, B. (1934). Sur la sphere vide. *Izvestia Akademii Nauk SSSR, Otdelenie Matematicheskikh i Estestvennykh Nauk*, 7, 793-800.
- Douglas, D. H., & Peucker, T. K. (1973). Algorithms for the reduction of the number of points required to represent a digitized line or its caricature. *Cartographica*, 10(2), 112-122. https://doi.org/10.3138/FM57-6770-U75U-7727
- Ester, M., Kriegel, H.-P., Sander, J., & Xu, X. (1996). A density-based algorithm for discovering clusters in large spatial databases with noise. In *Proceedings of the Second International Conference on Knowledge Discovery and Data Mining (KDD-96)*, 226-231. AAAI Press.
- Okabe, A., Boots, B., Sugihara, K., & Chiu, S. N. (2000). *Spatial tessellations: Concepts and applications of Voronoi diagrams* (2nd ed.). Wiley. https://doi.org/10.1002/9780470317013
- Visvalingam, M., & Whyatt, J. D. (1993). Line generalisation by repeated elimination of points. *The Cartographic Journal*, 30(1), 46-51. https://doi.org/10.1179/caj.1993.30.1.46
