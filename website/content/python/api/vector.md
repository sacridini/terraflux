---
title: Vector analysis
description: "Vector analysis as in PostGIS: predicates and selection by location, joins, nearest feature, overlay, clip, dissolve, buffer, geometry, lines and linear referencing, Voronoi, Delaunay, clusters, smoothing and coverages (their shared vertices too)."
order: 25
---

Spatial analysis of vector layers with the names PostGIS and GDAL use: one function for each operation, in the
module `tf.vector`, each with its twin among the application's nodes ([[vector.select_by_location]],
[[vector.overlay]], [[vector.voronoi]]...). The same features come out in the same order, with the same attributes.
Tutorials: [select, join and overlay](/tutorials/vector-analysis/select-join-overlay/) and
[shapes, lines, Voronoi cells and clusters](/tutorials/vector-analysis/shapes-lines-clusters/); the table of
equivalents with PostGIS, GDAL, QGIS and geopandas is in the first.

Conventions, for every function:

- `a` is a path (read with `load_vector`), a GeoDataFrame or a GeoSeries. `b` (or `mask`, `lines`, `clip`) is the
  same, or a shapely geometry, or a list of them (taken to be in the CRS of `a`).
- When both CRSs are known and differ, `b` is reprojected to the CRS of `a`. The result is in the CRS of `a`.
- The result is a GeoDataFrame with the attributes and the geometry of `a`, the index reset (0..n-1), in the order of
  `a`, unless the function says otherwise. `out=` (a path) also writes it with `tf.write_vector`.
- A predicate `p` is evaluated as `p(a_i, b_j)`, the feature of `a` first. A feature of `a` is kept when it holds with
  **at least one** feature of `b`, as `WHERE EXISTS (SELECT 1 FROM b WHERE p(a.geom, b.geom))` does.
- An empty or missing geometry never satisfies a predicate (`disjoint` included).
- Distances, tolerances and buffers are in the units of the CRS: use a projected system for metres.

```python
import geopandas as gpd
import terraflux as tf
from shapely.geometry import LineString, Point, box

v = tf.vector
parks = gpd.GeoDataFrame({"park": ["A", "B"]}, geometry=[box(0, 0, 10, 10), box(20, 0, 30, 10)], crs=32633)
trees = gpd.GeoDataFrame(
    {"tree": [1, 2, 3, 4], "height": [12.0, 8.0, 15.0, 9.0]},
    geometry=[Point(5, 5), Point(10, 5), Point(25, 5), Point(15, 5)], crs=32633)

print(v.within(trees, parks)["tree"].tolist())             # inside a park: the one on the edge is not
print(v.covered_by(trees, parks)["tree"].tolist())         # the edge counts
print(v.select(trees, parks, "within", invert=True)["tree"].tolist())
print(v.dwithin(trees, parks, 5)["tree"].tolist())         # within 5 m of a park
print(v.relate(trees, parks, "within"), v.count(parks, trees, "contains"), v.distance(trees, parks))
```

```text
[1, 3]
[1, 2, 3]
[2, 4]
[1, 2, 3, 4]
[ True False  True False] [1 1] [0. 0. 0. 5.]
```

## Predicates and selection

`select` keeps the features of `a` for which a predicate holds, and the ten functions that are named for a predicate
are its short forms: `within`, `contains`, `intersects`, `touches`, `crosses`, `overlaps`, `covers`, `covered_by`,
`contains_properly` and `equals`, each `tf.vector.<name>(a, b, *, out=None)` and equal to `select(a, b, "<name>")`.
`disjoint` and `dwithin` have their own. `relate`, `count` and `distance` give one value per feature of `a`, as the
vector calculator's spatial functions do.

| Function | Keeps the features of `a` that... | PostGIS |
|---|---|---|
| `within(a, b)` | lie inside a feature of `b` (a point on the edge does not) | `ST_Within` |
| `contains(a, b)` | hold a feature of `b` | `ST_Contains` |
| `intersects(a, b)` | share at least one point with one | `ST_Intersects` |
| `touches(a, b)` | share boundary points and no interior points | `ST_Touches` |
| `crosses(a, b)` | share interior points, in fewer dimensions than both | `ST_Crosses` |
| `overlaps(a, b)` | overlap one: same dimension, partly shared, neither inside the other | `ST_Overlaps` |
| `covers(a, b)` | cover one: none of its points outside (the edge counts) | `ST_Covers` |
| `covered_by(a, b)` | are covered by one (the edge counts) | `ST_CoveredBy` |
| `contains_properly(a, b)` | hold one, which touches only their interior | `ST_ContainsProperly` |
| `equals(a, b)` | are the same set of points as one, whatever the vertices | `ST_Equals` |

::: terraflux.vector.select

::: terraflux.vector.within

::: terraflux.vector.contains

::: terraflux.vector.intersects

::: terraflux.vector.touches

::: terraflux.vector.crosses

::: terraflux.vector.overlaps

::: terraflux.vector.covers

::: terraflux.vector.covered_by

::: terraflux.vector.contains_properly

::: terraflux.vector.equals

::: terraflux.vector.disjoint

::: terraflux.vector.dwithin

::: terraflux.vector.relate

::: terraflux.vector.count

::: terraflux.vector.distance

## Joins and the nearest feature

<!-- no-run -->
```python
v.join(trees, parks, "within")[["tree", "park"]]                  # the park of each tree, None for none
v.join(parks, trees, "contains", how="summary", fields="height")  # count, height_sum, height_mean
v.nearest(trees, parks)[["tree", "park", "distance"]]             # the nearest park and the distance
```

::: terraflux.vector.join

::: terraflux.vector.nearest

## Overlay, clip, dissolve and buffer

`overlay` has five modes; `intersection`, `difference`, `symmetric_difference`, `identity` and `union` are its short
forms. The second layer is polygons; the first may be lines or points for `intersection`, `difference` and
`identity`.

```python
pond = gpd.GeoDataFrame({"water": ["pond"]}, geometry=[box(8, 2, 22, 8)], crs=32633)
for how in ("intersection", "union", "difference", "symmetric_difference", "identity"):
    r = v.overlay(parks, pond, how)
    print(how, len(r), round(r.area.sum()))
print(round(v.clip(parks, pond).area.sum()), v.dissolve(parks).area.iloc[0], round(v.buffer(trees, 2, dissolve=True).area.iloc[0], 1))
```

```text
intersection 2 24
union 5 260
difference 2 176
symmetric_difference 3 236
identity 4 200
24 200.0 50.2
```

::: terraflux.vector.overlay

::: terraflux.vector.intersection

::: terraflux.vector.difference

::: terraflux.vector.symmetric_difference

::: terraflux.vector.identity

::: terraflux.vector.union

::: terraflux.vector.clip

::: terraflux.vector.dissolve

::: terraflux.vector.buffer

## Geometry

One feature in, one feature out (`hull` with `by` or `dissolve` and `explode` excepted), with the attributes kept.

```python
print(v.measures(parks)[["park", "area", "perimeter", "x", "y"]].to_string())
print(v.centroid(parks, inside=True).geometry.to_wkt().tolist(), v.hull(parks, dissolve=True).area.iloc[0])
print(v.simplify(gpd.GeoSeries([LineString([(0, 0), (5, 0.1), (10, 0)])]), 1).geometry.to_wkt().tolist())
```

```text
  park   area  perimeter     x    y
0    A  100.0       40.0   5.0  5.0
1    B  100.0       40.0  25.0  5.0
['POINT (5 5)', 'POINT (25 5)'] 300.0
['LINESTRING (0 0, 10 0)']
```

::: terraflux.vector.measures

::: terraflux.vector.centroid

::: terraflux.vector.hull

::: terraflux.vector.simplify

::: terraflux.vector.envelope

::: terraflux.vector.make_valid

::: terraflux.vector.explode

::: terraflux.vector.boundary

## Lines and linear referencing

A measure is the distance along a line from its start. The arithmetic is fixed (the lengths of the segments summed in
the order of the vertices) so the application and the library agree to the last digit.

```python
road = gpd.GeoDataFrame({"road": ["R1"]}, geometry=[LineString([(0, 0), (30, 0)])], crs=32633)
print(v.points_along(road, spacing=10)[["road", "measure"]].to_string())
print(v.substring(road, 0.25, 0.75, normalized=True).geometry.to_wkt().tolist())
print(v.locate(trees, road)[["tree", "line", "measure", "distance"]].to_string())
```

```text
  road  measure
0   R1      0.0
1   R1     10.0
2   R1     20.0
3   R1     30.0
['LINESTRING (7.5 0, 22.5 0)']
   tree  line  measure  distance
0     1     0       5.0       5.0
1     2     0      10.0       5.0
2     3     0      25.0       5.0
3     4     0      15.0       5.0
```

::: terraflux.vector.points_along

::: terraflux.vector.substring

::: terraflux.vector.split_lines

::: terraflux.vector.locate

::: terraflux.vector.shortest_line

::: terraflux.vector.closest_point

## Voronoi, Delaunay and clusters

```python
cells = v.voronoi(trees, clip=box(0, 0, 30, 10))                     # the cell of every tree, cut to a box
print(cells[["tree"]].assign(area=cells.area).to_string())
pts = gpd.GeoDataFrame(geometry=[Point(0, 0), Point(1, 0), Point(2, 0), Point(10, 0), Point(11, 0), Point(50, 50)], crs=32633)
print(v.cluster(pts, 1.5, min_points=2)["cluster"].tolist(), len(v.delaunay(pts)))
```

```text
   tree   area
0     1   75.0
1     2   50.0
2     3  100.0
3     4   75.0
[0, 0, 0, 1, 1, -1] 4
```

::: terraflux.vector.voronoi

::: terraflux.vector.delaunay

::: terraflux.vector.cluster

## Smoothing and coverages

`coverage_simplify` and `coverage_validate` need shapely 2.1 (GEOS 3.12). A layer made by polygonizing a map usually
fails `coverage_validate` until `coverage_node` adds the vertices that neighbours share (where two patches meet the
straight edge of a third): see
[the second tutorial](/tutorials/vector-analysis/shapes-lines-clusters/#smoothing-and-coverages).

```python
cov = gpd.GeoDataFrame({"id": [1, 2]}, geometry=[box(0, 0, 10, 10), box(10, 0, 20, 10)], crs=32633)
print(v.coverage_validate(cov)["valid"].tolist(), len(v.coverage_simplify(cov, 1)))
print(len(v.smooth(gpd.GeoSeries([LineString([(0, 0), (10, 0), (10, 10)])]), iterations=1).geometry.iloc[0].coords))
```

```text
[True, True] 2
6
```

```python
# a tall cell beside two short ones: its straight edge has no vertex where they meet
tj = gpd.GeoDataFrame({"id": [1, 2, 3]}, geometry=[box(0, 0, 1, 2), box(1, 0, 2, 1), box(1, 1, 2, 2)], crs=32633)
noded = v.coverage_node(tj)
print(noded["added"].tolist(), v.coverage_validate(tj)["valid"].tolist(), v.coverage_validate(noded)["valid"].tolist())
```

```text
[1, 0, 0] [False, False, False] [True, True, True]
```

::: terraflux.vector.smooth

::: terraflux.vector.coverage_node

::: terraflux.vector.coverage_simplify

::: terraflux.vector.coverage_validate
