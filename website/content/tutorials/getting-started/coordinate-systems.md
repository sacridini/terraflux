---
title: Coordinate systems and reprojection
short: Coordinate systems
description: Degrees and metres, why areas and distances need a projected or equal-area system, EPSG codes, how to reproject a raster, and which resampling method fits which data.
order: 40
level: beginner
time: 35 min
ops: [raster.reproject, raster.read, raster.terrain]
---

Every map file says where its cells are with coordinates, and every set of coordinates belongs to a **coordinate
reference system** (CRS). Two maps of the same place in different systems do not line up; a map in the wrong
system gives areas that are off by half. In this tutorial you will see the same landscape in degrees and in
metres, measure a forest's area three ways and find that the naive way overestimates it by 60%, reproject a
raster with [[raster.reproject]], and learn why a land cover map must never be reprojected with the method that
suits an elevation model.

## The idea: from a round Earth to a flat map

A position on Earth can be given in two ways.

- **Geographic coordinates**: longitude and latitude, in **degrees**, on a model of the Earth's shape (an
  ellipsoid and a *datum* that ties it to the ground; WGS 84, used by GPS, is the most common). They cover the
  whole globe, but a degree is not a fixed length: a degree of latitude is always about 111 km, while a degree of
  longitude shrinks from 111 km at the equator to nothing at the poles.
- **Projected coordinates**: x and y, usually in **metres**, on a flat plane. A **map projection** is the
  mathematical rule that flattens the curved surface onto the plane. On a projected map a cell of 30 m is 30 m
  wide everywhere, so areas and distances can be counted directly.

No projection is perfect: flattening a sphere always stretches something. A projection can keep **areas**
(equal-area projections, such as Albers or Lambert azimuthal equal-area), or **shapes and angles** locally
(conformal projections, such as Mercator and the Universal Transverse Mercator, UTM), or distances along some
lines, but never all of them at once. Snyder's manual for the U.S. Geological Survey (Snyder, 1987) remains the
classic description of the projections and their formulas; today they are computed by the PROJ library (PROJ
contributors), which Terraflux uses through GDAL.

```tfgallery
columns: 2
caption: "The same SRTM elevation model of 0.1 x 0.1 degrees in Oregon, at 43° N. Left: in its own CRS (EPSG:4326, longitude and latitude), drawn as a square of 371 x 371 cells. Right: reprojected by Terraflux to UTM zone 10 N (EPSG:32610) with cells of 30 m: 272 x 370 cells, 8.2 km wide and 11.1 km high. At this latitude a degree of longitude is only 74% of a degree of latitude, so the square in degrees was really a tall rectangle."
images:
  - { src: /img/tutorials/getting-started/coordinate-systems/elevation_degrees.png, caption: "In degrees: EPSG:4326" }
  - { src: /img/tutorials/getting-started/coordinate-systems/elevation_utm.png, caption: "In metres: EPSG:32610" }
```

!!! science "Key idea: a degree of longitude shrinks with latitude"
    The east–west length of one degree of longitude at latitude \(\varphi\) is approximately

    \[ L_{\text{lon}} \approx 111.32\ \text{km} \times \cos\varphi \]

    while a degree of latitude stays close to 111 km. So a cell of 1 arc-second (1/3600 of a degree, about
    30 m at the equator) is 30.9 x 30.7 m on the equator, 21.9 x 30.9 m at 45°, and 5.4 x 31.0 m at 80°: its
    area falls from 950 m² to 676 m² and 167 m². On a map in degrees, **cells of the same size in degrees are not
    the same size on the ground**.

```tfchart
type: line
data: /data/tutorials/getting-started/coordinate-systems/cell_area.csv
x: latitude
y: [east_west, north_south]
labels: { east_west: "East–west side (m)", north_south: "North–south side (m)" }
colors: { east_west: "#e8901f", north_south: "#2f6f9f" }
xlabel: Latitude (degrees)
ylabel: Side of a 1 arc-second cell (m)
caption: "The sides of a cell of 1 x 1 arc-second along the latitudes, on the WGS 84 ellipsoid. The north–south side barely changes; the east–west side follows the cosine of the latitude."
```

### Names for coordinate systems: EPSG codes

A CRS is described by a long text (a *WKT*), but most are known by a number in the **EPSG registry**, a public
database of coordinate systems kept by the International Association of Oil & Gas Producers (IOGP). Terraflux accepts
an EPSG code (`EPSG:4326`), a WKT or a PROJ string wherever it asks for a CRS. Some you will meet:

| Code | Name | Units | Keeps | Use it for |
|---|---|---|---|---|
| `EPSG:4326` | WGS 84 | degrees | nothing on a flat map | exchanging data, GPS, GBIF records |
| `EPSG:3857` | WGS 84 / Pseudo-Mercator | metres | local shapes | web basemaps only: areas are badly inflated away from the equator |
| `EPSG:326zz`, `EPSG:327zz` | WGS 84 / UTM zone zz north or south | metres | local shapes, distances within the zone | study areas up to a few hundred km wide |
| `EPSG:3035` | ETRS89-extended / LAEA Europe | metres | areas | anything European (CORINE is in it) |
| `EPSG:5070` | NAD83 / Conus Albers | metres | areas | the conterminous United States |
| `EPSG:5880` | SIRGAS 2000 / Brazil Polyconic | metres | areas along the central meridian | national maps of Brazil |
| `EPSG:6933` | WGS 84 / NSIDC EASE-Grid 2.0 Global | metres | areas | global analyses |

For a study area in one UTM zone, its UTM code is a good default for distances; for areas over larger regions
use the continent's equal-area projection.

## Why areas and distances need a projected CRS

Here is the same question answered three ways: how much forest is on a forest map of 1000 x 1000 cells in
degrees (landschaft's example habitat map; cells of 0.000305°, at about 51.4° N in central Germany), with
428,490 cells of forest?

| Method | Forest area | Error |
|---|---|---|
| Cells x (0.000305° x 111.32 km)², as if a cell were a square of 33.9 m | 492.6 km² | +60% |
| Each cell's true area on the ellipsoid, by its latitude (21.2 x 33.9 m at the centre) | 307.7 km² | reference |
| Reprojected with [[raster.reproject]] to the equal-area EPSG:3035, cells of 25 m, counted | 308.0 km² | +0.1% |

The naive count treats every cell as a square, but at 51° N a cell of 0.000305° is only 21 m wide. Counting
cells on an **equal-area projection** gives the right answer to 0.1%. The same goes for every measure that uses
lengths or areas: patch areas, edge lengths, core areas, the distance thresholds of connectivity analyses, the
dispersal distances of a species. Landscape metrics, MSPA's edge width and distances computed in degrees are
wrong unless the method corrects for it.

!!! warning "Web Mercator is for looking, not for measuring"
    EPSG:3857, the projection of web maps, inflates areas by \(1/\cos^2\varphi\): 2.6 times at 52° N (Hanover),
    4 times at 60°. Never measure areas on it.

## When to reproject

- **To combine layers.** Maps that are combined cell by cell must share one CRS (and one grid: see
  [One grid for all your maps](../one-grid/)). Choose the CRS of your main map, usually the land cover, and bring
  the others onto it.
- **To measure.** Areas on an equal-area projection; local distances on UTM or another conformal projection.
- **To share.** Occurrence records, web maps and many global datasets use EPSG:4326.

Every reprojection of a raster **resamples** it: the new grid's cells do not fall on the old ones, so each new
cell takes its value from the old cells near it. That always loses a little, so reproject each layer once, from
its original file, rather than back and forth.

## What you need

The CRS of each of your files. Open **Metadata** in the viewer's header and read *coordinate system*: Terraflux
shows its name and EPSG code (and the full WKT below). Files that say nothing (*none*) need you to tell
Terraflux what they are; see [A file without a CRS](#a-file-without-a-crs) below.

## Reproject a raster

[[raster.reproject]] warps rasters to another CRS and/or resolution with GDAL's `gdalwarp`. Its settings:

- `crs`: the new CRS (`EPSG:32610`, a WKT, a PROJ string);
- `resolution`: the new cells' side, or `[x, y]`, **in the new CRS's units** (30 for 30 m; about 0.0003 for
  degrees). Without it GDAL picks one that keeps about the same number of cells;
- `resampling`: how new cells get their values (next section);
- `source_crs`: the rasters' CRS, when their files do not say it;
- `extent`: `[xmin, ymin, xmax, ymax]` in the new CRS, to cut while warping;
- `path`: the file to write (else a file in a cache), and `options` for more of `gdalwarp`'s options.

```tfgraph
#! caption: Reproject an elevation model from degrees to UTM zone 10 N, with cells of 30 m.
#! highlight: elevation_utm
format = "terraflux/1"

[[nodes]]
id = "elevation"
op = "raster.read"
params = { path = "orelevation.tif" }
note = "SRTM elevation in EPSG:4326 (degrees), south-western Oregon."

[[nodes]]
id = "elevation_utm"
op = "raster.reproject"
inputs = { raster = "elevation.raster" }
params = { crs = "EPSG:32610", resolution = 30, path = "elevation_utm.tif" }
note = "UTM zone 10 N in metres; values interpolated (bilinear), the default for a continuous map."

[[nodes]]
id = "plot"
op = "view.plot"
inputs = { value = "elevation_utm.raster" }
params = { title = "Elevation (UTM 10 N)" }
```

The input is the `raster` output of [[raster.read]], the file on disk: reprojection is done file to file, whatever
the size of the raster, and the result is again a raster on disk.

!!! app "In the application: Prepare"
    Select a node that gives a raster and open the **Prepare** pane (a tab beside the Node pane). It shows the
    raster's coordinate system, cells and size next to the model's **reference grid**, and what differs. **Reproject
    to**, with a field for the CRS (EPSG:4326 by default), adds a [[raster.reproject]] node with the resampling
    chosen by what the values are; **Put on the reference grid** reprojects, resamples, cuts and aligns in one go.
    Before you click, check **Its values are**: *classes*, *values* or *counts*. Prepare guesses from the data,
    and the guess decides the resampling.

## Choose the resampling method by the data

When a new cell falls between old cells, where does its value come from?

| Method | The new cell takes | For |
|---|---|---|
| `near` | the value of the nearest old cell | **classes** at the same or a finer resolution |
| `mode` | the most common value of the old cells it covers | **classes** on larger cells |
| `bilinear` | a weighted mean of the four nearest cells | **continuous values** (elevation, temperature, NDVI) |
| `cubic`, `cubicspline`, `lanczos` | a smooth curve through 16 or more cells | continuous values, smoother (may overshoot the range) |
| `average` | the mean of the cells it covers | continuous values on larger cells |
| `sum` | the sum of the cells it covers | **counts** (people, individuals) on larger cells, keeping the total |
| `min`, `max`, `med`, `q1`, `q3`, `rms` | that statistic of the cells it covers | special cases |

With `resampling = "auto"` (the default), Terraflux chooses by the data: for classes the nearest cell, or the most
common one when the cells grow; for values bilinear, or the mean when the cells grow.

Why does it matter so much? Because **interpolating class codes invents classes**. Bilinear resampling of a
cell between arable land (12) and coniferous forest (24) gives 18: pastures. Between discontinuous urban fabric
(2) and arable land it gives 4 to 10: roads, ports, airports, dumps. Here is the 15 x 15 km window of the CORINE
map around Hanover reprojected to EPSG:4326, once with `near` and once with `bilinear`:

```tfcompare
caption: "Drag the handle. A corner of about 2 x 2.4 km of the CORINE window around Hanover, reprojected to EPSG:4326. Left: nearest neighbour; only the classes of the original are there (2 urban, 12 arable land, 24 and 25 forests, 40 a river). Right: bilinear; every boundary has grown a fringe of classes that are not in the landscape: black cells are 33 burnt areas, orange 15 vineyards, pale purple 39 intertidal flats."
before: { src: /img/tutorials/getting-started/coordinate-systems/classes_nearest.png, label: "near" }
after: { src: /img/tutorials/getting-started/coordinate-systems/classes_bilinear.png, label: "bilinear" }
```

Over the whole window, bilinear gave 6.7% of the cells another class than nearest, and 4.5% of the cells a class
that does not exist in the window at all: 26 invented classes, including *rice fields*, *olive groves*,
*glaciers and perpetual snow* and *salt marshes*, in Lower Saxony. Landscape metrics computed on such a map would
count thousands of tiny patches along every edge. With `auto` the result is identical to `near`.

!!! tip "The rule of thumb"
    Classes: `near` (or `mode` when cells grow). Measurements: `bilinear` (or `average` when cells grow).
    Counts: `sum`. When in doubt, leave `auto` and check what Prepare thinks the values are.

## Terrain in degrees

Slope, aspect and hillshade compare heights in metres with horizontal distances. On an elevation model in
degrees, the horizontal distances are in degrees too, unless the software corrects for it. Here is the mean slope
of the Oregon elevation model computed by [[raster.terrain]] (GDAL's `gdaldem`) three ways:

| Elevation model | Setting | Mean slope | 90th percentile |
|---|---|---|---|
| Reprojected to UTM 10 N (30 m) | none | 18.0° | 29.3° |
| In degrees (EPSG:4326) | none | 18.4° | 30.2° |
| In degrees (EPSG:4326) | `options = "-s 111120"` | 16.1° | 26.8° |

The GDAL brought with Terraflux (3.12) recognises a surface in degrees and converts its distances by itself, so
the default gives nearly the slope of the projected model. The option `-s 111120` (one degree taken as 111,120 m
in both directions), which older versions of GDAL needed, now *underestimates* the slopes at 43° N by about 10%,
because it treats the shorter degrees of longitude as if they were degrees of latitude. The safest way, whatever
the software, is to reproject the elevation model to a projected CRS in metres first.

## A file without a CRS

Some files have coordinates but do not say in which system. The land use of Sibuyan Island in the Philippines
(see [Your first raster](../first-raster/)) has x from 436,000 to 468,000 and y from 1,356,000 to 1,383,000, but
no CRS in its header. Those numbers fit UTM zone 51 N (EPSG:32651), the zone of Sibuyan at 122.5° E. The setting
`source_crs` says so:

```tfgraph
#! caption: A raster whose file has no CRS: say what it is (source_crs) and reproject it.
#! highlight: wgs84
format = "terraflux/1"

[[nodes]]
id = "landuse"
op = "raster.read"
params = { path = "sibuyan_landuse.tif" }
note = "No CRS in the file: its coordinates are those of UTM zone 51 N."

[[nodes]]
id = "wgs84"
op = "raster.reproject"
inputs = { raster = "landuse.raster" }
params = { crs = "EPSG:4326", source_crs = "EPSG:32651", path = "sibuyan_wgs84.tif" }
note = "Classes: the nearest cell (auto)."
```

The reprojected map falls at 122.41° to 122.71° E and 12.27° to 12.51° N: on Sibuyan. That check, the result
landing where the place really is, is the way to confirm a guessed CRS; a basemap under the map (++b++ in the
viewer) shows it at a glance. [[raster.match]] has the same `source_crs` setting.

## Common pitfalls

- **Layers that do not line up.** Two maps of the same place that appear far apart, or one of them squeezed,
  are in different CRSs. Reproject one onto the other (better: onto the reference grid with [[raster.match]]).
- **Degrees used as metres.** A `resolution` of 30 given with `crs = "EPSG:4326"` asks for cells of 30 *degrees*.
  The resolution is in the new CRS's units.
- **Latitude and longitude swapped.** Occurrence records in a table have `lat` and `lon` columns; swapping them
  sends points to another continent, or into the sea.
- **Measuring on Web Mercator.** Areas inflated 2 to 4 times at mid-latitudes. Use an equal-area projection.
- **A study area across UTM zones.** UTM is accurate within its zone (6° of longitude); far beyond it, distortion
  grows. For large areas use an equal-area projection of the continent.
- **Resampling classes with bilinear or cubic.** Invented classes along every edge. Use `near` or `mode`.
- **Different datums.** WGS 84, ETRS89 and SIRGAS 2000 agree within about a metre; older datums (NAD27, Corrego
  Alegre, SAD69) can be tens of metres off. GDAL converts between them when the CRSs are known; it cannot when
  a file lacks its CRS.

## Going further

- [One grid for all your maps](../one-grid/): resolution, alignment, [[raster.match]] and the reference grid.
- [Your first raster](../first-raster/) and [Vector data](../vector-data/).
- The operators: [[raster.reproject]], [[raster.match]], [[raster.terrain]].

## References

- GDAL/OGR contributors. *GDAL/OGR Geospatial Data Abstraction software Library*. Open Source Geospatial Foundation. https://doi.org/10.5281/zenodo.5884351
- PROJ contributors. *PROJ coordinate transformation software library*. Open Source Geospatial Foundation. https://doi.org/10.5281/zenodo.5884394
- Snyder, J. P. (1987). *Map Projections: A Working Manual* (U.S. Geological Survey Professional Paper 1395). U.S. Government Printing Office. https://doi.org/10.3133/pp1395
