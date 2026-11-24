---
title: "Vector analysis, part 1: select, join and overlay"
short: Select, join, overlay
description: Ask which features lie inside, near or beside others, join their attributes, find the nearest ones, and cut layers with each other and with lines, with the same names as PostGIS, GDAL and QGIS; with the table that says which is which.
order: 10
level: intermediate
time: 60 min
ops: [vector.select_by_location, vector.spatial_join, vector.nearest, vector.overlay, vector.split, vector.buffer, vector.clip, vector.dissolve, vector.calc, vector.filter]
series: Vector analysis
part: 1
---

Most questions about a landscape that start from vectors are questions about **where features are in relation to
other features**: which records fall inside a reserve, which forest patches a road would cut, how far each nest is
from the nearest river, how much forest is left once a road and its verge are taken out. In this part you will
answer those, with the operations that spatial databases (PostGIS), GDAL and QGIS share, called by the same names
in the application and in Python. At the end you will have counted the records in each reserve, found the forest a
planned road would touch, and read a table that says which tool of the other programs each operation is.

## The idea: relations between features

A **feature** is an object with a geometry (a point, a line or a polygon) and a row of attributes; a **layer** is a
set of features of one kind (see [Vector data](../../getting-started/vector-data/)). Two features can be in many
relations: one inside the other, side by side, crossing, overlapping, apart. Egenhofer and Franzosa (1991) showed
that these relations can be told from how the **interiors**, the **boundaries** and the **exteriors** of the two
geometries meet, and Clementini and colleagues (1993) reduced them to a small set that people use without thinking
about the theory. The Simple Features standard of the Open Geospatial Consortium (Herring, 2011) fixed their names,
and every spatial database and library has used them since.

!!! science "Key idea: a predicate is a yes-or-no question about two geometries"
    Each relation is a **predicate**: a function that gives true or false for a pair of geometries. Their order
    matters: `within(a, b)` asks whether *a* lies inside *b*, and `contains(b, a)` is the same question asked from
    the other side.

    | Predicate | True when | Example |
    |---|---|---|
    | `intersects` | the two share at least one point | a record in or on the edge of a reserve |
    | `within` | *a* lies inside *b*: no point of *a* is outside, and their interiors meet | a record inside a reserve |
    | `contains` | *a* holds all of *b* (`within` seen from the other side) | a reserve and the records inside it |
    | `covered_by`, `covers` | as `within` and `contains`, but a point on the edge counts | a record on the fence |
    | `contains_properly` | *b* lies in the interior of *a*, touching none of its edge | a patch well inside a reserve |
    | `touches` | they share boundary points and no interior points | two parcels that share a fence |
    | `crosses` | they share interior points, in fewer dimensions than both | a road through a reserve |
    | `overlaps` | same kind, partly shared, neither inside the other | two reserves that overlap |
    | `equals` | the same set of points, whatever the order of the vertices | the same patch drawn twice |
    | `disjoint` | they share no point | a record that is in no reserve |
    | `dwithin` | the distance between them is at most a given one | a record within 500 m of a road |

The same few questions come back in every kind of study, in two forms. **Keep** the features that meet a condition
(selection by location). **Take** from the other layer what meets them: its attributes (a join), the nearest one
(a nearest-neighbour join), or the pieces where the two overlap (an overlay). This part is those operations, one
after the other, on one small study area.

## When to use it

- **Which records are inside protected areas?** [[vector.select_by_location]] with `within`.
- **How many records and how many birds does each reserve hold?** [[vector.spatial_join]] with `summary`.
- **How far is each nest from the nearest river or road?** [[vector.nearest]]; with `k`, the three nearest.
- **How isolated is each forest patch from the next one?** [[vector.nearest]] with no second layer: the distance
  to the nearest *other* patch of the same layer.
- **How much forest would a road, with a 500 m verge, affect?** [[vector.buffer]] and [[vector.overlay]].
- **What part of the forest lies in reserves, by type?** [[vector.clip]] and [[vector.dissolve]].
- **Into how many fragments would the road cut the forest?** [[vector.split]]: each patch cut by the road, one
  feature per piece.
- **Which patches are near a road and hold records?** A script of [[vector.calc]] or [[vector.filter]] that reads
  another layer, with `dwithin(road, 500)` and `count(records, "contains")`.

It is not for questions about a *surface* (the forest in every cell of a map is [[vector.zonal]] and
[[raster.zonal]], see [Vector data](../../getting-started/vector-data/)), nor for geometry of one feature on its own
(its centroid, its hull, its smoothed outline): that is [part 2](../shapes-lines-clusters/), together with lines,
Voronoi cells and clusters.

## What you need

Six layers of one study area, a window of 20 by 20 km of a CORINE Land Cover map in the LAEA Europe system
(EPSG:3035, in metres): forest patches and water are cut from the map, the reserves, the road and the records are
made for this tutorial. **The records are simulated** (a random generator with a fixed seed, groups of points in
forests and some scattered in the open): they stand for the records of a forest bird. Every file is a GeoPackage.

| File | Features | What it is |
|---|---|---|
| [records.gpkg](/data/tutorials/vector-analysis/select-join-overlay/records.gpkg) | 355 points | simulated records: `record`, `year`, `birds` (how many seen) |
| [forest.gpkg](/data/tutorials/vector-analysis/select-join-overlay/forest.gpkg) | 71 polygons | forest patches: `patch`, `type` (broad-leaved, coniferous, mixed), `area_ha` |
| [reserves.gpkg](/data/tutorials/vector-analysis/select-join-overlay/reserves.gpkg) | 3 polygons | `reserve` (North, East, South) and `year` |
| [water.gpkg](/data/tutorials/vector-analysis/select-join-overlay/water.gpkg) | 6 polygons | five lakes and a river: `kind`, `area_ha` |
| [road.gpkg](/data/tutorials/vector-analysis/select-join-overlay/road.gpkg) | 1 line | a planned road, 23.3 km: `name`, `lanes` |
| [towns.gpkg](/data/tutorials/vector-analysis/select-join-overlay/towns.gpkg) | 18 polygons | urban areas of 20 ha or more (used in part 2) |

![The study area](/img/tutorials/vector-analysis/select-join-overlay/map.png "The study area: forest patches (green), lakes and a river (blue), towns (pink), the three reserves (orange outline), the planned road with the 500 m buffer around it (red) and the 355 simulated records, those inside a reserve with a white centre.")

!!! warning "Distances are in the units of the layer"
    The layers are in a **projected** system whose unit is the metre, so a distance of 500 is 500 m and an area is
    in m². If yours are in longitude and latitude (EPSG:4326), a distance of 500 means 500 *degrees*. Reproject them
    first with [[vector.reproject]] (or `tf.reproject`): see [Coordinate systems](../../getting-started/coordinate-systems/).
    When the two layers of an operation are in different systems, the second is reprojected to the first's.

All the operations below take layers as the application's nodes do: two inputs, `vector` (the features you start
from) and `other` (what you compare them to), and one output with the features of the first layer. In Python the
same operations are in the module `tf.vector`: functions with the PostGIS names that take a path, a GeoDataFrame
or a GeoSeries, and give back a GeoDataFrame. Their reference is [the page of `tf.vector`](/python/api/vector/).

## Select by location

[[vector.select_by_location]] keeps the features of the first layer for which a predicate holds **with at least one
feature** of the second. Which records are in a reserve?

```tfgraph
#! caption: Read the records and the reserves and keep the records inside a reserve.
#! highlight: inside
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "records"
op = "vector.read"
params = { path = "records.gpkg" }

[[nodes]]
id = "reserves"
op = "vector.read"
params = { path = "reserves.gpkg" }

[[nodes]]
id = "inside"
op = "vector.select_by_location"
inputs = { vector = "records.vector", other = "reserves.vector" }
params = { predicate = "within" }
note = "The records of 'vector' inside a reserve."
```

=== "Application"

    Add the two layers with [[vector.read]] (the Node pane's `path`), then [[vector.select_by_location]] from the
    library's **Vector analysis** subject (or ++slash++ to search). Drag the output of the records to its `vector`
    input and the reserves to `other`, and choose the `predicate` in the Node pane. Turn on `invert` to keep the
    records that do *not* meet. Run to the node with ++ctrl+enter++: the features and their table are its outputs.

=== "Command line"

    ```bash
    terraflux run select.toml     # the model above, saved as a file
    ```

=== "Model file"

    The graph above is the model file: its **Model file (TOML)** toggle shows it.

Of the 355 records, 196 are inside a reserve. The other 159 are outside, which `invert` gives, and 100 are in no
forest patch at all, which `disjoint` gives. The distance form, `dwithin`, needs a `distance` in map units: 26
records are within 500 m of the planned road.

!!! tip "A point on the edge is not within"
    `within` is false for a point exactly on the border of a polygon, because no interior point of the polygon
    holds it; `covered_by` and `intersects` are true. A record on a fence line is the case. When the edge should
    count, choose `covered_by`.

### In Python

The library has one function for each predicate, with the PostGIS name, and [`tf.vector.select`][terraflux.vector.select]
for any of them with `invert` and a distance. The first block loads the layers; later blocks use them.

```python
import terraflux as tf

v = tf.vector                                  # the module of these two parts
records = tf.load_vector("records.gpkg")       # 355 simulated records (points)
forest = tf.load_vector("forest.gpkg")         # 71 forest patches (polygons)
reserves = tf.load_vector("reserves.gpkg")     # 3 reserves (polygons)
water = tf.load_vector("water.gpkg")           # 6 lakes and rivers (polygons)
road = tf.load_vector("road.gpkg")             # the planned road (a line)
print(len(records), len(forest), len(reserves), records.crs.to_epsg())
```

```text
355 71 3 3035
```

```python
inside = v.within(records, reserves)                      # the records inside a reserve
outside = v.select(records, reserves, "within", invert=True)
no_forest = v.disjoint(records, forest)                   # in no forest patch
near_road = v.dwithin(records, road, 500)                 # within 500 m of the road
print(len(inside), len(outside), len(no_forest), len(near_road))
```

```text
196 159 100 26
```

Each returns the features of the first layer, with their attributes, as a GeoDataFrame in the same system. The
edge case is quickest to see with three points and a square of 10 by 10:

```python
import geopandas as gpd
from shapely.geometry import Point, box

pts = gpd.GeoSeries([Point(5, 5), Point(10, 5), Point(15, 5)])   # inside, on the edge, outside
park = box(0, 0, 10, 10)
print(len(v.within(pts, park)), len(v.covered_by(pts, park)), len(v.intersects(pts, park)), len(v.touches(pts, park)))
```

```text
1 2 2 1
```

When a yes or no for every feature is more useful than a selection, [`tf.vector.relate`][terraflux.vector.relate]
gives a `bool` array (what `select` keeps), [`tf.vector.count`][terraflux.vector.count] the number of features of
the other layer that meet each one, and [`tf.vector.distance`][terraflux.vector.distance] the distance to the
nearest. They are what the calculator's spatial functions (below) compute.

```python
hit = v.relate(records, road, "dwithin", distance=500)          # one True or False per record
n_in = v.count(reserves, records, "contains")                    # records in each reserve
d_road = v.distance(records, road)                               # metres to the road
print(int(hit.sum()), n_in, round(d_road.min()), round(d_road.mean()))
```

```text
26 [112  72  12] 9 4174
```

## Join: the attributes of what meets, and the nearest

A selection only keeps features. A **join** brings the *attributes* of the other layer onto the first.
[[vector.spatial_join]] has three modes, chosen with `mode`:

| `mode` | One row per... | Use it to |
|---|---|---|
| `attributes` (the default) | feature of the first layer, with the attributes of the **first** feature of the other that meets it | give each record its reserve |
| `all` | pair that meets (a feature that meets nothing stays once, with empty values) | keep every match, as a database's left join does |
| `summary` | feature of the first layer: `count`, and the sum and mean of the other's attributes in `fields` | count and total the records in each reserve |

The predicate is any of the list above (`predicate`, `intersects` by default), with `distance` for `dwithin`.
With `attributes` a feature that meets several gets the first of them, so a record on the border of two reserves
takes one; use `all` to see both.

[[vector.nearest]] joins each feature to the **nearest** feature of the other layer instead, whatever the
distance, and adds its `distance` (in map units, 0 where they touch). With `max_distance`, a feature with nothing
that near keeps empty values.

Two settings widen it. `k` asks for the **k nearest** features instead of the nearest: with `k` above 1 there is one
row for each pair, the nearest first, and a column `rank` (1 to `k`) says which neighbour it is. And the `other`
input may stay **unconnected**: the layer is then compared with itself, and a feature is never its own neighbour.
For forest patches that is the distance from each patch to the nearest *other* patch, which landscape ecology calls
the **Euclidean nearest-neighbour distance** (ENN, in FRAGSTATS: McGarigal and Marks, 1995), the usual measure of
how isolated a patch is.

```tfgraph
#! caption: Give each record the name of its reserve, count the records and birds in each reserve, find the nearest water within 2 km and the three nearest water features, and the distance from each forest patch to the nearest other patch.
#! highlight: home, per_reserve, near_water, three_water, enn
format = "terraflux/1"

[[nodes]]
id = "records"
op = "vector.read"
params = { path = "records.gpkg" }

[[nodes]]
id = "reserves"
op = "vector.read"
params = { path = "reserves.gpkg" }

[[nodes]]
id = "water"
op = "vector.read"
params = { path = "water.gpkg" }

[[nodes]]
id = "forest"
op = "vector.read"
params = { path = "forest.gpkg" }

[[nodes]]
id = "home"
op = "vector.spatial_join"
inputs = { vector = "records.vector", other = "reserves.vector" }
params = { predicate = "within", mode = "attributes" }
note = "Each record with the reserve and year of the reserve that holds it."

[[nodes]]
id = "per_reserve"
op = "vector.spatial_join"
inputs = { vector = "reserves.vector", other = "records.vector" }
params = { predicate = "contains", mode = "summary", fields = ["birds"] }
note = "count, birds_sum and birds_mean for every reserve."

[[nodes]]
id = "near_water"
op = "vector.nearest"
inputs = { vector = "records.vector", other = "water.vector" }
params = { max_distance = 2000 }
note = "The nearest lake or river and the distance to it."

[[nodes]]
id = "three_water"
op = "vector.nearest"
inputs = { vector = "records.vector", other = "water.vector" }
params = { k = 3 }
note = "Three rows for every record, with rank 1, 2 and 3."

[[nodes]]
id = "enn"
op = "vector.nearest"
inputs = { vector = "forest.vector" }
note = "No 'other': each patch and the nearest other patch, with the distance (ENN)."
```

The order of the layers decides what is joined to what. The first graph node starts from the records and takes
the reserve's attributes; the second starts from the reserves and counts the records: the same predicate, seen from
the other side, and the same pairs.

### In Python

```python
home = v.join(records, reserves, "within", how="first")       # a reserve for each record
print(home["reserve"].value_counts(dropna=False).to_string())
per = v.join(reserves, records, "contains", how="summary", fields="birds")
print(per[["reserve", "count", "birds_sum", "birds_mean"]].round(2).to_string())
```

```text
reserve
None     159
North    112
East      72
South     12
  reserve  count  birds_sum  birds_mean
0   North    112      307.0        2.74
1    East     72      224.0        3.11
2   South     12       32.0        2.67
```

The 159 records outside every reserve have no value in `reserve`. The nearest feature, with the distance of at most
2 km:

```python
near = v.nearest(records, water, max_distance=2000)
print(near["kind"].value_counts(dropna=False).to_string())
print(near["distance"].describe().round(0).to_string())
```

```text
kind
None     342
lake      11
river      2
count      13.0
mean      748.0
std       636.0
min         0.0
25%        57.0
50%       886.0
75%      1257.0
max      1786.0
```

Only 13 of the 355 records have water within 2 km; the distance is `NaN` for the others, so the statistics are of
those 13. Write `near["distance"]` and not `near.distance`: a GeoDataFrame has a *method* called `distance`.

With `k`, the three nearest water features of each record, one row each; with no second layer, the distance
between forest patches:

```python
three = v.nearest(records, water, k=3)                     # one row for each pair, the nearest first
print(len(three))
print(three[["record", "kind", "distance", "rank"]].head(6).round(0).to_string())
enn = v.nearest(forest)                                    # no second layer: the nearest other patch (ENN)
print(enn[["patch", "patch_2", "distance"]].head(3).round(0).to_string())
print(round(enn["distance"].mean()), round(enn["distance"].median()), round(enn["distance"].max()))
print(int((enn["distance"] == 0).sum()), round(enn.loc[enn["distance"] > 0, "distance"].mean()))
```

```text
1065
   record   kind  distance  rank
0       1   lake    4838.0     1
1       1  river   11581.0     2
2       1   lake   11686.0     3
3       2   lake    6192.0     1
4       2   lake   14766.0     2
5       2  river   15786.0     3
   patch  patch_2  distance
0      1       19       0.0
1      2       11       0.0
2      3       20       0.0
189 0 4453
50 638
```

The 355 records give 1,065 rows (3 times 355), and `rank` is 1 for the nearest water. The columns of the neighbour
have the suffix `_2` (`patch_2` is the number of the nearest other patch). The **mean ENN of the forest patches is
189 m**, but the median is 0: 50 of the 71 patches *touch* another patch (the patches are cut from the map by class,
so a coniferous patch next to a broad-leaved one has distance 0 to it, and so do two patches that meet at a corner).
The other 21 are on average 638 m from the nearest one, and at most 4,453 m. To measure the isolation of patches of
*forest* and not of classes, [[vector.dissolve]] the patches first, or work with one `type`.

## Overlay: cut layers with each other

An **overlay** makes new geometries from two layers of polygons. [[vector.overlay]] has five modes (`how`):

| `how` | The result |
|---|---|
| `intersection` (the default) | a piece wherever a feature of each layer meets, with the attributes of both |
| `union` | those pieces, and the parts of each layer the other does not cover, with only their own attributes |
| `difference` | each feature of the first layer less what the second covers |
| `symmetric_difference` | what is in one layer but not in both: the union without the pieces |
| `identity` | the first layer, cut where the second meets it, with the second's attributes there |

The second layer must be polygons. The first may also be **lines or points** for `intersection`, `difference` and
`identity`: the pieces are the parts of the lines inside the polygons, or the points inside them. A piece that is
not of the first layer's kind (two polygons that only share an edge give a line) is dropped.

Four neighbours of overlay work on one layer or with a mask: [[vector.buffer]] (the area within a distance of every
feature, `dissolve` merging them), [[vector.clip]] (the features cut to the area of a mask), [[vector.dissolve]]
(features merged into one, or one per value of an attribute with `by`) and [[vector.reproject]].

How much forest would the planned road affect, with a verge of 500 m on each side? Buffer the road, then intersect
the forest with the buffer; the difference is the forest left.

```tfgraph
#! caption: The forest inside a 500 m buffer around the road, the forest left, the forest in reserves and the forest by type.
#! highlight: affected, left, protected, by_type
format = "terraflux/1"

[[nodes]]
id = "forest"
op = "vector.read"
params = { path = "forest.gpkg" }

[[nodes]]
id = "reserves"
op = "vector.read"
params = { path = "reserves.gpkg" }

[[nodes]]
id = "road"
op = "vector.read"
params = { path = "road.gpkg" }

[[nodes]]
id = "verge"
op = "vector.buffer"
inputs = { vector = "road.vector" }
params = { distance = 500 }
note = "The area within 500 m of the road."

[[nodes]]
id = "affected"
op = "vector.overlay"
inputs = { vector = "forest.vector", other = "verge.vector" }
params = { how = "intersection" }
note = "The pieces of forest inside the verge."

[[nodes]]
id = "left"
op = "vector.overlay"
inputs = { vector = "forest.vector", other = "verge.vector" }
params = { how = "difference" }
note = "The forest less the verge."

[[nodes]]
id = "protected"
op = "vector.clip"
inputs = { vector = "forest.vector", mask = "reserves.vector" }
note = "The forest inside the reserves."

[[nodes]]
id = "by_type"
op = "vector.dissolve"
inputs = { vector = "forest.vector" }
params = { by = "type" }
note = "One feature for each forest type, and count."
```

The overlay of a layer with a **line** shows the most common use of the line forms: where the road runs through
forest, and how long a stretch. [[vector.overlay]] with the road as the first layer and the forest as the second
gives the road cut at every forest patch, with the patch's attributes.

### In Python

```python
verge = v.buffer(road, 500)                         # a polygon around the road
cut = v.overlay(forest, verge, "intersection")      # the forest inside it
rest = v.difference(forest, verge)                  # the forest outside it
protected = v.clip(forest, reserves)                # the forest inside the reserves
print(len(cut), round(cut.geometry.area.sum() / 1e4), round(forest.geometry.area.sum() / 1e4))
print(len(rest), round(rest.geometry.area.sum() / 1e4), len(protected), round(protected.geometry.area.sum() / 1e4))
```

```text
14 662 9624
71 8962 14 4940
```

Of 9,624 ha of forest, 662 ha (6.9%) lie within 500 m of the road, and 4,940 ha (51%) are in a reserve. The
forest left has the same 71 patches as before, because the road's verge only trims or cuts them. By type, and the
road through the forest:

```python
by_type = cut.assign(ha=cut.geometry.area / 1e4).groupby("type")["ha"].sum()   # the pieces keep the forest's attributes
print(by_type.round(0).to_string())
types = v.dissolve(forest, by="type")                  # one feature per type, and count
print(types.assign(ha=types.geometry.area / 1e4)[["type", "count", "ha"]].round(0).to_string())
inside_forest = v.overlay(road, forest, "intersection")     # the road cut at the forest patches
print(len(inside_forest), round(inside_forest.length.sum()), round(road.length.sum()))
```

```text
type
broad-leaved     66.0
coniferous      525.0
mixed            71.0
           type  count      ha
0    coniferous     38  7937.0
1         mixed     17   953.0
2  broad-leaved     16   734.0
10 7563 23291
```

Most of the forest in the verge is coniferous, which is also most of the forest (7,937 of 9,624 ha). The road
runs through forest in 10 stretches that add up to 7,563 m of its 23,291 m, about a third. The overlay's rows come
in the application's order (the pieces pair by pair, then the remainders), not in `geopandas.overlay`'s: the same
rows, sorted differently.

### Cut features with lines: split

An overlay needs polygons as its second layer and keeps the pieces where the two meet. Sometimes the question is the
opposite: **cut** a feature along a line and keep *every* piece. A road through a forest patch divides it into
fragments, a fence divides a parcel, a river divides a floodplain. [[vector.split]] takes the features to cut
(`vector`: polygons or lines) and the blades (`lines`: lines, or polygons, whose boundaries cut), and gives one
feature for each piece, with the attributes of the feature it came from and two columns: `part`, the rank of the
piece (1 is the **largest**, by area for polygons and by length for lines), and `parts`, how many pieces the feature
became. A feature that nothing cuts stays whole, with `part` and `parts` equal to 1.

```tfgraph
#! caption: Cut the forest patches along the planned road, one feature for each piece.
#! highlight: pieces
format = "terraflux/1"

[[nodes]]
id = "forest"
op = "vector.read"
params = { path = "forest.gpkg" }

[[nodes]]
id = "road"
op = "vector.read"
params = { path = "road.gpkg" }

[[nodes]]
id = "pieces"
op = "vector.split"
inputs = { vector = "forest.vector", lines = "road.vector" }
note = "The patches cut along the road: part 1 is the largest piece of each."
```

![Forest patches cut by the planned road](/img/tutorials/vector-analysis/select-join-overlay/split-road.png "The planned road cuts 10 of the 71 forest patches into 28 pieces. The largest piece of each cut patch is dark green, the others orange; the pale patches are not touched by the road.")

The number of pieces says how much *fragmentation* the road causes, and the area of the largest piece how much of a
patch stays together:

```python
cut = v.split(forest, road)                           # one row for each piece
cut["ha"] = cut.geometry.area / 1e4
hit = cut[cut["parts"] > 1]                           # the patches the road cuts
print(len(forest), len(cut), hit["patch"].nunique(), len(hit))
main = hit[hit["part"] == 1]                          # the largest piece of each cut patch
print(main[["patch", "parts", "area_ha", "ha"]].head(4).round(0).to_string())
print(int((hit["ha"] < 1).sum()), round(hit.loc[hit["part"] > 1, "ha"].median(), 1))
```

```text
71 89 10 28
    patch  parts  area_ha      ha
0       1      3   2118.0  2115.0
4       3      2    600.0   473.0
7       5      8    483.0   304.0
17      8      2    175.0   174.0
8 2.5
```

The road cuts 10 of the 71 patches into 28 pieces: the layer grows from 71 to 89 features, the 61 patches it does not
touch and the 28 pieces. Patch 3 (600 ha) keeps 473 ha in its largest piece; patch 5 (483 ha) falls into 8 pieces, the
largest of 304 ha. Patch 1, the largest of all, is cut in three, but only 3 ha come off it. Eight of the 28 pieces
are under 1 ha, and the median of the 18 pieces that are not the largest of their patch is 2.5 ha: most of what a road
cuts off a patch is a sliver or a fragment too small to hold the species of the forest. That is why the number of
pieces overstates the loss, and the largest piece is worth reading beside it.

Blades can also be polygons, whose boundaries cut: `v.split(forest, reserves)` cuts the forest along the reserves'
borders (10 patches into 27 pieces), a layer to report by reserve from, and unlike an overlay it keeps the forest that
is outside the reserves, as pieces of its own.

## The calculator reads other layers

The vector calculator ([[vector.calc]], [[vector.filter]]; see [its page](/calculators/vector/)) can ask these
questions inside a script, a value for every feature. A vector connected to its `+ new input` becomes a **layer** the
script names by the port, and these functions take it as their first value:

| Function | Value for each feature |
|---|---|
| `within(parks)`, `contains(parks)`, `intersects(parks)`, `touches`, `crosses`, `overlaps`, `covers`, `covered_by`, `contains_properly`, `equals`, `disjoint` | 1 if the predicate holds with at least one feature of the layer, else 0 |
| `dwithin(rivers, 500)` | 1 if some feature of the layer is within 500 map units |
| `count(trees, "contains")` | how many features of the layer meet it by the predicate (`intersects` if none is given) |
| `distance(roads)` | the distance to the nearest feature of the layer |

```tfgraph
#! caption: A script over the forest patches that reads three other layers: the reserves, the road and the records.
#! highlight: patches
format = "terraflux/1"

[[nodes]]
id = "forest"
op = "vector.read"
params = { path = "forest.gpkg" }

[[nodes]]
id = "reserves"
op = "vector.read"
params = { path = "reserves.gpkg" }

[[nodes]]
id = "road"
op = "vector.read"
params = { path = "road.gpkg" }

[[nodes]]
id = "records"
op = "vector.read"
params = { path = "records.gpkg" }

[[nodes]]
id = "patches"
op = "vector.calc"
inputs = { vector = "forest.vector", reserves = "reserves.vector", road = "road.vector", records = "records.vector" }
params = { expression = """
out in_reserve = intersects(reserves)
out near_road = dwithin(road, 500)
out n_records = count(records, "contains")
out road_km = distance(road) / 1000
""" }
note = "Four new attributes, one value per patch."
```

A script can combine them with the geometry's measures, so that `within(reserves) and geometry.area > 1e6` is the
patches of more than 100 ha inside a reserve; [[vector.filter]] keeps them. The page of the calculator has the rest
of the rules, and says why the playground there cannot run them: it has no second layer to read.

### In Python

The three functions are those of the block above, and give the same values:

```python
patches = forest.copy()
patches["in_reserve"] = v.relate(forest, reserves, "intersects")
patches["near_road"] = v.relate(forest, road, "dwithin", distance=500)
patches["n_records"] = v.count(forest, records, "contains")
patches["road_km"] = v.distance(forest, road) / 1000
print(patches[["in_reserve", "near_road"]].sum().to_string())
print(patches["n_records"].max(), round(patches["road_km"].min(), 2), round(patches["road_km"].max(), 1))
```

```text
in_reserve    14
near_road     14
91 0.0 12.9
```

Fourteen of the 71 patches touch a reserve and fourteen lie within 500 m of the road (not the same fourteen: eight
patches are in both, the largest of them with 91 records); the most records in a patch are 91, and the farthest
patch is 12.9 km from the road.

## The equivalence table

The operations of this series, with their names in the programs that spatial analysts already know. A dash means
that the program has no ready-made tool for it, or none that is documented as one. GDAL's `ogr2ogr` runs SQL in the
`SQLite` dialect, which has SpatiaLite's `ST_` functions when GDAL is built with SpatiaLite; its own OGR SQL has no
spatial predicates. OGR's layer methods (`Intersection`, `Union`, `SymDifference`, `Identity`, `Erase`, `Clip`) are
in its C and Python APIs. QGIS names are those of Processing's tools. Rows of [part 2](../shapes-lines-clusters/)
are included, so that this is the one table.

| Operation | PostGIS | GDAL / OGR | QGIS | geopandas / shapely | `tf.vector` | Application |
|---|---|---|---|---|---|---|
| Keep the features that meet others | `WHERE EXISTS (SELECT 1 FROM b WHERE ST_Within(a.geom, b.geom))`, also `ST_Contains`, `ST_Intersects`, `ST_Touches`, `ST_Crosses`, `ST_Overlaps`, `ST_Covers`, `ST_CoveredBy`, `ST_ContainsProperly`, `ST_Equals` | `-dialect SQLite -sql` with `ST_Within(a.geometry, b.geometry)` (SpatiaLite) | Extract by location, Select by location | `a.loc[a.sjoin(b, predicate="within").index.unique()]` | `within(a, b)`, `contains`, `intersects`, `touches`, `crosses`, `overlaps`, `covers`, `covered_by`, `contains_properly`, `equals`, `select(a, b, predicate)`, `relate` | [[vector.select_by_location]] |
| Keep the features that meet none | `NOT EXISTS (... ST_Intersects ...)`, `ST_Disjoint` | `NOT EXISTS` in the SQL | Extract by location, `disjoint` | `a[~a.index.isin(a.sjoin(b).index)]` | `disjoint(a, b)`, `select(..., invert=True)` | [[vector.select_by_location]], `disjoint` or `invert` |
| Within a distance | `ST_DWithin(a.geom, b.geom, 500)` | `ST_Distance(a.geometry, b.geometry) <= 500` in the SQL | Buffer, then Extract by location | `a.sjoin(b, predicate="dwithin", distance=500)`, `shapely.dwithin` | `dwithin(a, b, 500)` | [[vector.select_by_location]], `dwithin` |
| Join the attributes of the first match or of every match | `a LEFT JOIN b ON ST_Within(a.geom, b.geom)` | `LEFT JOIN ... ON ST_Within(...)` in the SQL | Join attributes by location | `a.sjoin(b, how="left", predicate=...)` | `join(a, b, predicate, how="first")`, `how="all"` | [[vector.spatial_join]], `mode` attributes or all |
| Count and total what meets each feature | `GROUP BY` over the join, `count(*)`, `sum()` | `GROUP BY` in the SQL | Join attributes by location (summary), Count points in polygon | `sjoin`, then `groupby` | `join(a, b, predicate, how="summary", fields=...)`, `count(a, b)` | [[vector.spatial_join]], `mode` summary |
| The nearest feature and its distance | `ORDER BY a.geom <-> b.geom LIMIT 1` in a lateral join | — | Join attributes by nearest | `a.sjoin_nearest(b, max_distance=...)` | `nearest(a, b, max_distance)` | [[vector.nearest]] |
| The k nearest features, with their rank | `ORDER BY a.geom <-> b.geom LIMIT 3` in a lateral join | — | Join attributes by nearest (Maximum nearest neighbors) | `sjoin_nearest` has no `k` (it gives all ties at the nearest distance): `shapely.STRtree.nearest` in a loop | `nearest(a, b, k=3)` | [[vector.nearest]], `k` |
| Distance to the nearest other feature of the same layer (ENN) | `ORDER BY a.geom <-> b.geom LIMIT 1` with `b.id <> a.id` in a lateral join | — | — | `a.sjoin_nearest(a, exclusive=True, distance_col="distance")` | `nearest(a)` | [[vector.nearest]], no `other` |
| Distance to the nearest feature | `min(ST_Distance(a.geom, b.geom))` | — | Distance to nearest hub (points) | `a.distance(b.union_all())` | `distance(a, b)` | `distance(layer)` in [[vector.calc]]; `distance` of [[vector.nearest]] |
| A yes, a count or a distance as an attribute | `EXISTS (...)`, `count(...)` in the select list | the same in the SQL | `overlay_within('layer')`, `overlay_intersects`, `overlay_nearest` in the field calculator | a column of `a.within(...)`, `a.intersects(...)` | `relate(a, b, predicate)`, `count`, `distance` | `within(layer)`, `dwithin(layer, 500)`, `count(layer, "contains")`, `distance(layer)` in [[vector.calc]] and [[vector.filter]] |
| Intersection (pieces with both attributes) | `ST_Intersection(a.geom, b.geom)` over the pairs that `ST_Intersects` | `Layer.Intersection` | Intersection | `overlay(a, b, how="intersection")` | `intersection(a, b)`, `overlay(a, b, "intersection")` | [[vector.overlay]], `intersection` |
| Union of two layers | `ST_Union(a.geom, b.geom)` per pair, and the parts each layer keeps | `Layer.Union` | Union | `overlay(a, b, how="union")` | `union(a, b)`, `overlay(a, b, "union")` | [[vector.overlay]], `union` |
| Difference | `ST_Difference(a.geom, ST_Union(b.geom))` | `Layer.Erase` | Difference | `overlay(a, b, how="difference")` | `difference(a, b)` | [[vector.overlay]], `difference` |
| Symmetric difference | `ST_SymDifference(a.geom, b.geom)` | `Layer.SymDifference` | Symmetrical difference | `overlay(a, b, how="symmetric_difference")` | `symmetric_difference(a, b)` | [[vector.overlay]], `symmetric_difference` |
| Identity | — | `Layer.Identity` | — | `overlay(a, b, how="identity")` | `identity(a, b)` | [[vector.overlay]], `identity` |
| Cut features by lines, one row for each piece | `ST_Split(a.geom, line)` (then `ST_Dump`) | — | Split with lines | `shapely.ops.split(geom, line)` | `split(a, lines)` | [[vector.split]] |
| Clip to a mask | `ST_Intersection(a.geom, ST_Union(mask.geom))` | `ogr2ogr -clipsrc mask.gpkg`, `Layer.Clip` | Clip | `geopandas.clip(a, mask)` | `clip(a, mask)` | [[vector.clip]] |
| Merge features, all or by an attribute | `SELECT by, ST_Union(geom) ... GROUP BY by` | `ST_Union(geometry) ... GROUP BY` in the SQL | Dissolve | `a.dissolve(by=...)` | `dissolve(a, by=...)`, `union(a)` | [[vector.dissolve]] |
| Buffer | `ST_Buffer(geom, 500)` | `ST_Buffer(geometry, 500)` in the SQL | Buffer | `a.buffer(500)` | `buffer(a, 500, dissolve=...)` | [[vector.buffer]] |
| Centroid | `ST_Centroid(geom)` | `ST_Centroid(geometry)` in the SQL | Centroids | `a.centroid` | `centroid(a)` | [[vector.centroid]] |
| A point that lies on the feature | `ST_PointOnSurface(geom)` | `ST_PointOnSurface(geometry)` in the SQL | Point on surface | `a.representative_point()` | `centroid(a, inside=True)` | [[vector.centroid]], `inside` |
| Convex hull | `ST_ConvexHull(geom)` | `ST_ConvexHull(geometry)` in the SQL | Convex hull | `a.convex_hull` | `hull(a)`, `hull(a, by=...)` | [[vector.hull]] |
| Concave hull | `ST_ConcaveHull(geom, 0.3)` | — | Concave hull (k-nearest neighbor) | `a.concave_hull(ratio=0.3)` | `hull(a, kind="concave", ratio=0.3)` | [[vector.hull]], `kind` concave |
| Simplify a geometry | `ST_SimplifyPreserveTopology(geom, 50)` | `ogr2ogr -simplify 50` | Simplify | `a.simplify(50)` | `simplify(a, 50)` | [[vector.simplify]] |
| Bounding box | `ST_Envelope(geom)` | `ST_Envelope(geometry)` in the SQL | Bounding boxes | `a.envelope` | `envelope(a)` | [[vector.envelope]] |
| Repair an invalid geometry | `ST_MakeValid(geom)` | `ogr2ogr -makevalid` | Fix geometries | `a.make_valid()` | `make_valid(a)` | [[vector.make_valid]] |
| Split multi-part features | `ST_Dump(geom)` | `ogr2ogr -explodecollections` | Multipart to singleparts | `a.explode()` | `explode(a)` | [[vector.explode]] |
| Boundary | `ST_Boundary(geom)` | `ST_Boundary(geometry)` in the SQL | Boundary | `a.boundary` | `boundary(a)` | [[vector.boundary]] |
| Largest inscribed circle (the depth of a patch) | `ST_MaximumInscribedCircle(geom)` | — | Pole of inaccessibility (the centre and `dist_pole`, its distance to the edge) | `a.maximum_inscribed_circle()`, `shapely.maximum_inscribed_circle` | `shape(a)`, column `inscribed_radius`; `shape(a, geometry="inscribed")` for the circles | [[vector.shape]], `inscribed_radius`, `geometry` inscribed |
| Smallest enclosing circle | `ST_MinimumBoundingCircle(geom)`, `ST_MinimumBoundingRadius(geom)` | — | Minimum bounding geometry (Minimum enclosing circle) | `a.minimum_bounding_circle()`, `a.minimum_bounding_radius()` | `shape(a)`, column `bounding_radius`; `geometry="bounding"` | [[vector.shape]], `bounding_radius`, `geometry` bounding |
| Minimum rotated rectangle, its sides and direction | `ST_OrientedEnvelope(geom)` | — | Minimum bounding geometry (Minimum oriented rectangle) | `a.minimum_rotated_rectangle()`, `shapely.oriented_envelope` | `shape(a)`, columns `rect_length`, `rect_width`, `elongation`, `orientation`; `geometry="rectangle"` | [[vector.shape]], `geometry` rectangle |
| Related circumscribing circle (FRAGSTATS CIRCLE) | — | — | — | `1 - a.area / (pi * shapely.minimum_bounding_radius(a) ** 2)` | `shape(a)`, column `circle` | [[vector.shape]], `circle` |
| Area, perimeter, length, centroid | `ST_Area`, `ST_Perimeter`, `ST_Length`, `ST_X(ST_Centroid())` | `OGR_GEOM_AREA` in OGR SQL | Add geometry attributes | `a.area`, `a.length`, `a.centroid.x` | `measures(a, geodesic=False)` | [[vector.geometry]] |
| Points along a line | `ST_LineInterpolatePoints(geom, 0.1)` | — | Points along geometry | `shapely.line_interpolate_point` | `points_along(a, spacing=1000)` | [[vector.points_along]] |
| A piece of a line | `ST_LineSubstring(geom, 0.25, 0.75)` | — | Line substring | `shapely.ops.substring` | `substring(a, 0.25, 0.75, normalized=True)` | [[vector.substring]] |
| Cut a line into pieces | `ST_LineSubstring` over `generate_series` | — | Split lines by maximum length | `shapely.ops.substring` in a loop | `split_lines(a, 5000)` | [[vector.split_lines]] |
| Measure along a line of the nearest point | `ST_LineLocatePoint(line, point)` | — | `line_locate_point()` in the field calculator | `line.project(point)` | `locate(a, lines)` | [[vector.locate]] |
| The line between two features | `ST_ShortestLine(a.geom, b.geom)` | — | Shortest line between features | `shapely.shortest_line(a, b)` | `shortest_line(a, b)` | [[vector.shortest_line]] |
| Move a point to the nearest line | `ST_ClosestPoint(line, point)` | — | Snap geometries to layer | `shapely.ops.nearest_points` | `closest_point(a, b, max_distance)` | [[vector.closest_point]] |
| Voronoi polygons | `ST_VoronoiPolygons(geom)` | — | Voronoi polygons | `a.voronoi_polygons()`, `shapely.voronoi_polygons` | `voronoi(a, clip=...)` | [[vector.voronoi]] |
| Delaunay triangles | `ST_DelaunayTriangles(geom)` | — | Delaunay triangulation | `shapely.delaunay_triangles` | `delaunay(a, edges=False)` | [[vector.delaunay]] |
| Clusters by distance (DBSCAN) | `ST_ClusterDBSCAN(geom, 600, 5) OVER ()`, `ST_ClusterWithin` | — | DBSCAN clustering | `sklearn.cluster.DBSCAN` on the coordinates | `cluster(a, 600, min_points=5)` | [[vector.cluster]] |
| Smooth lines and polygons | `ST_ChaikinSmoothing(geom, 2)` | — | Smooth | — | `smooth(a, iterations=2)` | [[vector.smooth]] |
| Simplify polygons that share edges | `ST_CoverageSimplify(geom, 60) OVER ()` | — | — | `shapely.coverage_simplify` | `coverage_simplify(a, 60)` | [[vector.coverage_simplify]] |
| Find the edges that overlap or leave gaps | `ST_CoverageInvalidEdges(geom) OVER ()` | — | — | `shapely.coverage_invalid_edges` | `coverage_validate(a, gap_width=0)` | [[vector.coverage_validate]] |

!!! note "Where a name is not the whole story"
    The same name can mean a slightly different thing. PostGIS's `ST_Union(a, b)` unites two geometries, while
    the table's union of two layers makes the pieces of an overlay; PostGIS's functions work on the pairs a query
    gives them, while the application's and the library's work on whole layers with a spatial index. A QGIS tool
    keeps its own options. The names come from each program's documentation: they were not compared by running
    the other programs, so read the options of your version before you rely on a likeness.

## Common pitfalls

- **Distances in degrees.** `dwithin`, `nearest`, a buffer or a clustering distance are in the units of the first
  layer. Reproject to a metric system first.
- **The order of the layers.** `within(a, b)` is not `within(b, a)`; a join keeps the geometry of the *first*
  layer and its order. Say what you start from.
- **"At least one."** A selection keeps a feature when the predicate holds with *any* feature of the other layer,
  and `disjoint` when it holds with none. It never says *which* one: join for that.
- **The edge.** A point on a border is not `within` but is `covered_by`; a polygon that only shares an edge with
  another `touches` it and does not `overlap` it.
- **One match or all.** A join with `attributes` gives each feature the **first** of several matches (the lowest
  index of the other layer). The count of matches is `summary`.
- **Same names in both layers.** A column of the second layer that the first already has gets the suffix `_2`
  (`suffix` changes it); the nearest's `distance` too, if there is already a `distance`.
- **Invalid polygons.** An overlay, a dissolve or a clip can fail on a self-intersecting polygon; repair it first
  with [[vector.make_valid]].
- **Empty geometries.** An empty or missing geometry never satisfies a predicate, `disjoint` included, and gives
  `NaN` for a distance.
- **A blade that stops short does not cut.** [[vector.split]] cuts only where the line crosses a feature from one
  side to the other. A road that ends inside a patch, or stops a few metres before its edge, leaves the patch in one
  piece (`parts` is 1): a square cut by a line through it gives 2 pieces, by the same line shortened to end inside it
  gives 1. Extend the line past the edge first, and compare the number of pieces with what you
  expect.
- **Slivers.** A line that clips the corner of a patch cuts off a piece of a few square metres. Count the pieces
  below a size that matters (1 ha in the example above) before reporting a number of fragments.
- **A nearest-neighbour distance of 0.** With no `other` layer, a patch that touches another one (at an edge or a
  corner) is at distance 0 from it, and a layer cut by class has many such pairs. The mean ENN of 189 m above hides
  a median of 0: read both, and decide whether touching patches of different classes are neighbours or one patch.
- **`k` makes rows, not columns.** With `k` above 1 a feature has up to `k` rows, so a count of rows is no longer
  a count of features: group by the feature's key, or keep `rank` equal to 1.
- **Lines and points in an overlay.** The second layer is always polygons; `union` and `symmetric_difference` need
  polygons in both, and a layer that mixes points, lines and polygons is refused.

## Going further

- [Part 2](../shapes-lines-clusters/): centroids, hulls and simplification, the depth and shape of a patch
  (inscribed circle, rectangle, CIRCLE), lines and linear referencing, Voronoi catchments, clusters, smoothing and
  polygon coverages.
- [The vector calculator](/calculators/vector/): scripts over attributes and the geometry's measures, now with
  spatial functions of other layers.
- [Vector data](../../getting-started/vector-data/): burn polygons into a grid, statistics of a map in each polygon,
  values at points.
- The operators: [[vector.select_by_location]], [[vector.spatial_join]], [[vector.nearest]], [[vector.overlay]],
  [[vector.split]], [[vector.buffer]], [[vector.clip]], [[vector.dissolve]]. The library's reference: [`tf.vector`](/python/api/vector/).

## References

- Clementini, E., Di Felice, P., & van Oosterom, P. (1993). A small set of formal topological relationships suitable for end-user interaction. In *Advances in Spatial Databases (SSD 1993)*, Lecture Notes in Computer Science 692, 277-295. https://doi.org/10.1007/3-540-56869-7_16
- Egenhofer, M. J., & Franzosa, R. D. (1991). Point-set topological spatial relations. *International Journal of Geographical Information Systems*, 5(2), 161-174. https://doi.org/10.1080/02693799108927841
- Herring, J. R. (Ed.). (2011). *OpenGIS implementation specification for geographic information: Simple feature access, part 1: Common architecture* (OGC 06-103r4). Open Geospatial Consortium.
- McGarigal, K., & Marks, B. J. (1995). *FRAGSTATS: spatial pattern analysis program for quantifying landscape structure* (General Technical Report PNW-GTR-351). USDA Forest Service, Pacific Northwest Research Station. https://doi.org/10.2737/PNW-GTR-351
