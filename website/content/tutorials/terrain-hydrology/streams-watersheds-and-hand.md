---
title: Streams, watersheds and HAND
short: Streams and watersheds
description: Extract a stream network from the flow accumulation, split it into links with their Strahler order, delineate the watershed of a gauge, and map how high every cell stands above the nearest stream.
order: 20
level: intermediate
time: 50 min
ops: [hydro.streams, hydro.stream_links, hydro.watersheds, hydro.hand, hydro.distance_to_channel, vector.read, map.calc]
series: Terrain hydrology
part: 2
---

In this part you turn the flow accumulation of [part 1](../depressions-and-flow/) into the things ecologists
actually map: a stream network, its links with their Strahler order and length, the watershed that drains to a
sampling point, the subbasin of every reach, and two measures of where each cell stands relative to the water:
its height above the nearest stream (HAND) and the distance its water travels to reach one. At the end you will
know how to choose the threshold that makes a stream, how to place a gauge so that its watershed is right, and
how HAND separates valley floors from slopes and plateaus in a way elevation cannot.

## The idea

A flow accumulation counts, for every cell, the cells upstream of it. Streams begin where enough area drains
through a cell for water to flow in a channel most of the time. The simplest rule, and still the most used,
is a **threshold**: every cell whose accumulation is at least, say, 500 cells is a stream. With cells of 30 m,
500 cells are 45 ha, so every stream drains at least 45 ha of hillslope.

The cells that pass the threshold form a branching network. Hydrologists cut it into **links**: the stretches
between a source (where a stream starts), a junction (where two streams meet) and an outlet (where the network
leaves the map, or a point you choose, such as a gauge). Each link has a place in the hierarchy of the network,
its **Strahler order** (Strahler, 1957).

!!! science "Key idea: Strahler order"
    A link that starts at a source has order 1. Where two links of the same order \( n \) meet, the link below
    has order \( n + 1 \); where links of different orders meet, the link below keeps the higher one. So two
    first-order streams make a second-order stream, but a first-order stream joining a second-order one leaves
    it second order. The order measures how many levels of branching lie upstream: headwater streams are order
    1, large rivers 6 to 10 or more. It depends on the threshold and the resolution of the map, so it is a
    property of *your network*, not of the river.

From the network come the **watersheds** (or *catchments*, *drainage basins*): the area that drains to a point.
On a D8 grid this is easy: follow the flow direction of every cell downhill until it reaches the point or not.
Every cell whose path passes through the point belongs to its watershed. Done for every link at once, it gives
the **subbasins**: each cell labelled with the first link its water reaches.

The last idea comes from the Amazon. Rennó et al. (2008) and Nobre et al. (2011), at Brazil's National
Institute for Space Research (INPE), wanted a terrain variable that tells where the water table is near the
surface, in a forest so dense that the ground cannot be seen. Elevation does not: a valley floor in the
mountains and a plateau in the lowlands can have the same elevation. They proposed **HAND**, the *Height Above
the Nearest Drainage*.

!!! science "Key idea: height above the nearest drainage"
    For each cell, follow its D8 flow path down to the first stream cell it reaches. HAND is the difference
    in elevation between the two:

    \[ \mathrm{HAND}_i = z_i - z_{s(i)} \]

    where \( z_i \) is the elevation of cell \( i \) and \( z_{s(i)} \) that of the stream cell \( s(i) \) where
    its water joins the drainage. HAND is 0 on the streams, small on floodplains and valley floors (the water
    table is near the surface; they flood), and large on hilltops and plateaus (the water table is deep). It
    *normalises* the terrain by the drainage network: two cells 3 m above their streams have similar soil water
    conditions, whatever their elevation.

Rennó et al. (2008) used HAND to map the environments of terra-firme rainforest near Manaus, from waterlogged
valley bottoms to plateaus, and Schietti et al. (2014) showed that the vertical distance from the drainage, not
elevation, drives the change of floristic composition across an Amazonian rainforest. HAND is now used to map
flood-prone land, riparian zones and the soils and vegetation that follow the water table.

## When to use it

- **Choosing and describing sampling sites on streams.** The watershed of each site gives its catchment area,
  land use and slope; the Strahler order and link length describe its position in the network.
- **Stream networks where none is mapped**, or where the map misses small headwater streams, which are often
  the richest in endemic invertebrates and amphibians and the least protected.
- **Riparian zones and floodplains.** Cells with a small HAND (a few metres) are the valley floors that flood
  and stay wet; they make a better riparian buffer than a fixed distance from a mapped river.
- **Plant and soil gradients** in forests, especially where a vertical distance to water matters more than
  elevation (Schietti et al., 2014).
- **Subbasins for a model.** SWAT+ ([part 4](../swat-plus/)) and other watershed models divide the land into
  subbasins exactly this way.

What it is *not*: an extracted stream is where the terrain *would* concentrate water, not where a channel
exists. Headwater streams start at very different contributing areas in a wet and a dry climate, on permeable and
impermeable rock. HAND is not a flood model: it says which land is low relative to the drainage, not how often
water reaches it.

## What you need

- **The filled DEM of [part 1](../depressions-and-flow/)**: `filled.tif`, the Oregon SRTM elevation
  reprojected to UTM 10N in cells of 30 m and filled, saved by part 1's `save_filled` node (as 64-bit numbers,
  so the values are exactly those part 1 computed). Put it in the folder of this part's model.
- **An outlet point** for the watershed of a gauge: a vector file with one point (GeoJSON, GeoPackage,
  shapefile...). Save this text as `outlets.geojson` next to the model; it is a point in longitude and latitude
  near a tributary, where a stream gauge could be:

    ```json
    {"type": "FeatureCollection", "features": [{"type": "Feature", "properties": {"name": "gauge"},
     "geometry": {"type": "Point", "coordinates": [-122.942408, 43.055098]}}]}
    ```

  Your own gauges, sampling sites or dams work the same way; their coordinate system does not matter, Terraflux
  moves them onto the grid of the DEM.

## Build the model

### Streams

<div class="steps" markdown>

1. Start a new model. Add a [[raster.read]] (subject **IO**) with **path** `filled.tif` and rename it
   `filled`.
2. With `filled` selected, click [[hydro.flow_accumulation]] in the library's subject **Terrain hydrology**
   (D8, in cells, as in part 1) and rename it `accumulation`.
3. Select `accumulation` and click [[hydro.streams]]. Rename it `streams` and set **threshold** to `500`.
4. Run to `streams` (++ctrl+enter++): 1 on the streams, 0 elsewhere.

</div>

```tfgraph
#! caption: Streams where at least 500 cells (45 ha) drain through a cell.
#! highlight: streams
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "filled"
op = "raster.read"
params = { path = "filled.tif" }
note = "The filled DEM of part 1."

[[nodes]]
id = "accumulation"
op = "hydro.flow_accumulation"
inputs = { dem = "filled.raster" }

[[nodes]]
id = "streams"
op = "hydro.streams"
inputs = { accumulation = "accumulation.map" }
params = { threshold = 500 }
note = "1 where at least 500 cells drain through."
```

The **threshold** is in the units of the accumulation: here cells (500 cells of 900 m² are 45 ha); with an
accumulation in `area` units, give square metres. Nothing in the terrain alone tells you the right value, and
it decides almost everything that follows: the number of streams, their order, the size of the subbasins, the
HAND of every cell. Move the slider to see the networks of six thresholds, each link coloured by its Strahler
order:

![Legend of the Strahler orders](/img/tutorials/terrain-hydrology/streams-watersheds-and-hand/order-legend.png "Strahler orders of the links, from 1 (headwaters, light blue) up.")

```tfslider
label: Threshold (hectares)
caption: "Stream networks for thresholds of 100 to 5,000 cells (9 to 450 ha), each link coloured by its Strahler order, drawn two cells wide. A small threshold draws a stream in every hollow; a large one keeps the main rivers. The numbers come from the table of hydro.stream_links: the links, their total length, and the drainage density (stream length per area of the map, 89.7 km²)."
frames:
  - { value: "9 ha", src: /img/tutorials/terrain-hydrology/streams-watersheds-and-hand/streams_100.png, stats: { Links: "520", "Length": "182 km", "Drainage density": "2.03 km/km²", "Highest order": "5" } }
  - { value: "22.5 ha", src: /img/tutorials/terrain-hydrology/streams-watersheds-and-hand/streams_250.png, stats: { Links: "206", "Length": "110 km", "Drainage density": "1.23 km/km²", "Highest order": "4" } }
  - { value: "45 ha", src: /img/tutorials/terrain-hydrology/streams-watersheds-and-hand/streams_500.png, stats: { Links: "106", "Length": "75 km", "Drainage density": "0.84 km/km²", "Highest order": "3" } }
  - { value: "90 ha", src: /img/tutorials/terrain-hydrology/streams-watersheds-and-hand/streams_1000.png, stats: { Links: "38", "Length": "50 km", "Drainage density": "0.56 km/km²", "Highest order": "3" } }
  - { value: "225 ha", src: /img/tutorials/terrain-hydrology/streams-watersheds-and-hand/streams_2500.png, stats: { Links: "16", "Length": "32 km", "Drainage density": "0.36 km/km²", "Highest order": "2" } }
  - { value: "450 ha", src: /img/tutorials/terrain-hydrology/streams-watersheds-and-hand/streams_5000.png, stats: { Links: "8", "Length": "21 km", "Drainage density": "0.23 km/km²", "Highest order": "2" } }
start: 2
chart:
  type: line
  data: /data/tutorials/terrain-hydrology/streams-watersheds-and-hand/density.csv
  x: threshold_ha
  y: ["Drainage density (km/km²)"]
  colors: { "Drainage density (km/km²)": "#2f6f9f" }
  xlabel: Threshold (ha)
  height: 240
```

How to choose it:

- **From a map of real channel heads.** The best answer: walk (or find in a survey, or see on lidar) where
  channels begin in a few hollows, read their accumulation, and use a typical value.
- **From a stream map at another scale.** Choose the threshold whose drainage density matches that of a
  trusted map of the same area, for the streams that map shows.
- **From the network's own structure.** Tarboton, Bras & Rodriguez-Iturbe (1991) proposed the smallest
  threshold at which the extracted network still behaves like real networks in the way the drops of streams
  of each Strahler order compare.
- **From the purpose.** For a model of subbasins (SWAT+), coarser is fine; for the habitat of headwater
  species, finer.

!!! note "Streams as InVEST traces them"
    InVEST's sediment delivery ratio model extracts streams from its multiple-flow (`mfd`) accumulation, which
    spreads water so that the cells above a threshold do not always join into lines. Give [[hydro.streams]] the
    filled DEM in its `dem` input and a **trace** (InVEST uses `0.7`): the cells above the threshold are traced
    up from the outlets along the `mfd` directions, and tributaries are kept down to 0.7 times the threshold
    where they lead to such a stream. Use it, with an `mfd` accumulation, when your streams must match InVEST's
    ([Ecosystem services](../../ecosystem-services/)). With the same threshold of 500 cells, the D8 network has
    2,113 stream cells and the traced `mfd` one 3,034.

```tfcompare
caption: "Drag the handle: streams of 500 cells from the D8 accumulation (left) and traced from the mfd accumulation with trace 0.7, as InVEST does (right). The traced network reaches further up the hollows, because the multiple-flow accumulation gathers the water of the hillslopes into the hollows sooner and tributaries are kept down to 70 % of the threshold."
before: { src: /img/tutorials/terrain-hydrology/streams-watersheds-and-hand/streams_d8.png, label: "D8, 500 cells" }
after: { src: /img/tutorials/terrain-hydrology/streams-watersheds-and-hand/streams_traced.png, label: "mfd traced, 500 cells" }
```

### Links, their table and an outlet

<div class="steps" markdown>

1. In the subject **IO**, click [[vector.read]] and set **path** to `outlets.geojson`. Rename it `outlets`.
2. Add [[hydro.stream_links]] and rename it `links`. Connect `filled`'s raster to its `dem`, `streams`'s map to its
   `streams` and `outlets`'s `vector` to its `outlets`.
3. Run to `links`. It has two outputs: a map (`map`), the number of the link of each stream cell, 0 elsewhere;
   and a table (`table`), a row per link. To see the table as a chart, connect it to a [[view.plot]]; to keep it,
   connect it to a [[table.write]] with **path** `links.csv`.

</div>

```tfgraph
#! caption: The stream links, cut at the gauge, with their table.
#! highlight: outlets, links, save
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "filled"
op = "raster.read"
params = { path = "filled.tif" }

[[nodes]]
id = "accumulation"
op = "hydro.flow_accumulation"
inputs = { dem = "filled.raster" }

[[nodes]]
id = "streams"
op = "hydro.streams"
inputs = { accumulation = "accumulation.map" }
params = { threshold = 500 }

[[nodes]]
id = "outlets"
op = "vector.read"
params = { path = "outlets.geojson" }
note = "A gauge, as a point."

[[nodes]]
id = "links"
op = "hydro.stream_links"
inputs = { dem = "filled.raster", streams = "streams.map", outlets = "outlets.vector" }
params = { snap = 300 }
note = "Links 1, 2... and their table."

[[nodes]]
id = "save"
op = "table.write"
inputs = { table = "links.table" }
params = { path = "links.csv" }
```

[[hydro.stream_links]] follows the D8 directions of the DEM it is given (so give it the same filled DEM), and
numbers the links 1, 2, ... in the order of their first cells, row by row from the top of the map. The table has
one row per link, with these columns:

| column | meaning |
|---|---|
| `link` | the link's number, as on the map |
| `downstream` | the link its water flows into; 0 if it leaves the map |
| `outlet` | 1 if the link ends at one of the outlet points |
| `start_row`, `start_col`, `end_row`, `end_col` | its first and last cells (rows from the top, columns from the left, from 0) |
| `cells` | its number of cells |
| `length` | its length along the D8 path, from its first cell to the cell below its last, in metres (diagonal steps count √2 cells) |
| `z_start`, `z_end` | the elevation of its first cell and of the cell below its last |
| `order` | its Strahler order |

The first rows of the table of this map:

```tftable
data: /data/tutorials/terrain-hydrology/streams-watersheds-and-hand/links-head.csv
digits: 1
caption: "The first six links of the 500-cell network. Links 1 and 2 are single cells on the top edge of the map: streams that come from outside it. The difference z_start − z_end divided by the length is a link's mean gradient."
```

The **outlets** are points where a link must end even if no tributary joins there: a gauge, a dam, a sampling
site, the outlet of the watershed you study. A point is rarely exactly on the stream cell (a GPS position, a
point on a map at another scale, a stream that the DEM puts a cell aside), so each one is **snapped** to the
nearest stream cell within **snap** map units (300 m by default, the snap threshold of QSWAT+; 3 cells for a
map without projection). A point farther than that from every stream is an error, so you notice it. The gauge
here was 46 m from the stream and snapped to the cell at row 166, column 154; the link ending there is link
**44**, the one row of the table with `outlet` = 1.

### The watershed of the gauge, and the subbasins

[[hydro.watersheds]] labels every cell with the first *seed* its water reaches along the D8 directions. Seeds
are cells with a positive whole number, 0 elsewhere. Two uses:

<div class="steps" markdown>

1. **The watershed of the gauge.** Add a [[map.calc]] named `seed` with input `l` from `links`'s map and the
   expression `if l == 44 then 1 else 0`: link 44, the link ending at the gauge, is the only seed. Add
   [[hydro.watersheds]] named `watershed`, its `dem` from `filled` and its `seeds` from `seed`. Every cell whose
   water passes through link 44 gets 1: the whole watershed above the gauge, since the links above it are not
   seeds and their water reaches link 44.
2. **The subbasins.** Add another [[hydro.watersheds]] named `subbasins` with its `seeds` straight from
   `links`'s map: each cell gets the number of the first link its water reaches.

</div>

```tfgraph
#! caption: The watershed of the gauge (from its link) and the subbasin of every link.
#! highlight: seed, watershed, subbasins
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "filled"
op = "raster.read"
params = { path = "filled.tif" }

[[nodes]]
id = "accumulation"
op = "hydro.flow_accumulation"
inputs = { dem = "filled.raster" }

[[nodes]]
id = "streams"
op = "hydro.streams"
inputs = { accumulation = "accumulation.map" }
params = { threshold = 500 }

[[nodes]]
id = "outlets"
op = "vector.read"
params = { path = "outlets.geojson" }

[[nodes]]
id = "links"
op = "hydro.stream_links"
inputs = { dem = "filled.raster", streams = "streams.map", outlets = "outlets.vector" }

[[nodes]]
id = "seed"
op = "map.calc"
inputs = { l = "links.map" }
params = { expression = "if l == 44 then 1 else 0" }
note = "Link 44 ends at the gauge (outlet = 1 in the table)."

[[nodes]]
id = "watershed"
op = "hydro.watersheds"
inputs = { dem = "filled.raster", seeds = "seed.map" }

[[nodes]]
id = "subbasins"
op = "hydro.watersheds"
inputs = { dem = "filled.raster", seeds = "links.map" }
```

The number 44 comes from the table: run the model once, find the row with `outlet` = 1, and write its `link`
in the expression. With several gauges, several rows have `outlet` = 1; give each its own label
(`if l == 44 then 1 else if l == 71 then 2 else 0`) and one [[hydro.watersheds]] labels all their watersheds.
Nested gauges are handled as they should be: the watershed of the upper gauge stops at the upper gauge, and
the lower one gets the area between them.

```tfgallery
columns: 2
caption: "Left: the watershed of the gauge, 6,568 cells (5.91 km²), with the point as given (white circle) and the stream cell it was snapped to (orange square). Right: the subbasins of the 106 links of the network, each in a colour; the cells near the edges whose water leaves the map before reaching a stream (5 % of the map) have none."
images:
  - { src: /img/tutorials/terrain-hydrology/streams-watersheds-and-hand/watershed.png, caption: "The watershed of the gauge" }
  - { src: /img/tutorials/terrain-hydrology/streams-watersheds-and-hand/subbasins.png, caption: "The subbasins of every link" }
```

### HAND and the distance to the streams

<div class="steps" markdown>

1. Add [[hydro.hand]] named `hand`: its `dem` from `filled`, its `streams` from `streams`.
2. Add [[hydro.distance_to_channel]] named `distance`: its `dem` from `filled`, its `streams` from `streams`.
3. Run each and look at the maps.

</div>

```tfgraph
#! caption: The height of each cell above the nearest stream, and the distance its water travels to one.
#! highlight: hand, distance
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "filled"
op = "raster.read"
params = { path = "filled.tif" }

[[nodes]]
id = "accumulation"
op = "hydro.flow_accumulation"
inputs = { dem = "filled.raster" }

[[nodes]]
id = "streams"
op = "hydro.streams"
inputs = { accumulation = "accumulation.map" }
params = { threshold = 500 }

[[nodes]]
id = "hand"
op = "hydro.hand"
inputs = { dem = "filled.raster", streams = "streams.map" }
note = "Metres above the stream its water reaches (D8)."

[[nodes]]
id = "distance"
op = "hydro.distance_to_channel"
inputs = { dem = "filled.raster", streams = "streams.map" }
note = "Mean flow distance (mfd), in cell lengths."
```

[[hydro.hand]] takes the drainage either as the `streams` input (any non-zero cell is drainage, so a map of
mapped rivers and lakes works too) or, without it, from its **threshold**: the cells of D8 accumulation at least
that many cells. It follows the D8 directions (with flat areas drained, **flats**), so give it a filled DEM:
every cell must reach a stream. HAND is in the units of the DEM, metres here. The results are WhiteboxTools'
`ElevationAboveStream` cell for cell.

[[hydro.distance_to_channel]] is the horizontal counterpart: the mean distance water travels from a cell to a
stream along InVEST's multiple flow directions, the *downslope* part of InVEST's sediment delivery model
(part 3). The distance is in cell lengths (a step across a side is 1, across a corner √2; multiply by 30 for
metres here), or in the units of the optional `weights`, a map of the length of each step onto a cell. It is 0
on the streams and has no value where no flow reaches one. Note that its `streams` input must be 1 on the
streams.

## Read the results

**The network.** At 500 cells, the map has 106 links and 75 km of streams, a drainage density of 0.84 km per
km². 61 links are of order 1, 39 of order 2 and 6 of order 3, and the gradient falls with the order, as in real
networks:

```tftable
data: /data/tutorials/terrain-hydrology/streams-watersheds-and-hand/orders.csv
labels: { order: "Strahler order", links: "Links", length_km: "Length (km)", mean_length_m: "Mean length (m)", mean_gradient_pct: "Mean gradient (%)" }
digits: 2
caption: "The links of the 500-cell network by Strahler order, from the table of hydro.stream_links. First-order streams fall 13.8 m per 100 m, third-order ones 2.3 m."
```

The highest order is only 3 because the map is small and its edges cut the network into 16 separate pieces
(the rows whose `downstream` is 0): the main river of the south-east corner continues outside the map. For a
true Strahler order of a river, the DEM must cover its whole basin.

![The links of the network by Strahler order](/img/tutorials/terrain-hydrology/streams-watersheds-and-hand/orders.png "The 106 links of the 500-cell network, coloured by Strahler order, drawn two cells wide.")

**The watershed.** The gauge drains 6,568 cells, 5.91 km². For a sampling site, report this area and
summarise any map over it (land use, slope, the open land of part 1) with the watershed as a mask. Check it
against what you know: does its boundary follow the ridges on the hillshade? Does its area agree with the one
published for the gauge, if there is one? A watershed far too small is the classic sign of an outlet that was
not snapped (see the pitfalls).

**HAND.** Half the map stands more than 106 m above its nearest stream; 10 % less than 17 m. The valley floors
form narrow dark ribbons: a steep, deeply dissected landscape with almost no floodplain. Elevation and HAND
share little: across the map their correlation is only 0.34, because valley floors and ridges occur at every
elevation (the chart below). HAND and the flow distance to the streams are more alike
(correlation 0.79), but not the same thing: a cell on a steep slope can be close to a stream horizontally and
far above it.

```tfgallery
columns: 2
caption: "HAND, in metres above the stream each cell drains to, and the mean distance water travels to a stream along the multiple flow directions, in metres. Both are 0 on the streams; HAND grows quickly on the steep valley sides, the distance more evenly. Cells whose water leaves the map before reaching a stream have no value."
images:
  - { src: /img/tutorials/terrain-hydrology/streams-watersheds-and-hand/hand.png, caption: "HAND (m)" }
  - { src: /img/tutorials/terrain-hydrology/streams-watersheds-and-hand/distance.png, caption: "Distance to the streams (m)" }
```

```tfchart
type: scatter
data: /data/tutorials/terrain-hydrology/streams-watersheds-and-hand/hand-elevation.csv
x: elevation
y: [HAND]
colors: { HAND: "#2f6f9f" }
xlabel: Elevation (m)
ylabel: HAND (m)
height: 300
caption: "HAND against elevation for 600 cells drawn at random. At every elevation, HAND ranges from 0 (a valley floor) to a few hundred metres (a ridge): elevation alone says little about where the water is."
```

The cells with a small HAND are the land a riparian or floodplain study cares about. Move the slider to see
how much of the map lies within a given height of its stream:

```tfslider
label: HAND below (m)
caption: "The cells less than a given height above their nearest stream (500-cell network). The valley floors are narrow: 4.5 % of the map is within 5 m of its stream, a quarter within 50 m."
frames:
  - { value: 2, src: /img/tutorials/terrain-hydrology/streams-watersheds-and-hand/hand_below_2.png, stats: { "Share of the map": "3.3%" } }
  - { value: 5, src: /img/tutorials/terrain-hydrology/streams-watersheds-and-hand/hand_below_5.png, stats: { "Share of the map": "4.5%" } }
  - { value: 10, src: /img/tutorials/terrain-hydrology/streams-watersheds-and-hand/hand_below_10.png, stats: { "Share of the map": "6.8%" } }
  - { value: 20, src: /img/tutorials/terrain-hydrology/streams-watersheds-and-hand/hand_below_20.png, stats: { "Share of the map": "11.2%" } }
  - { value: 50, src: /img/tutorials/terrain-hydrology/streams-watersheds-and-hand/hand_below_50.png, stats: { "Share of the map": "24.5%" } }
```

Classes of HAND are local: the heights that separate waterlogged valley bottoms from slopes in the flat
Amazonian lowlands studied by Rennó et al. (2008) and Nobre et al. (2011) are not those of a steep mountain
range. Calibrate the thresholds with field data (soil saturation, the edge of riparian vegetation, a flood of
known extent) before mapping classes.

!!! paper "In the literature"
    Report the DEM, its conditioning, the flow method (D8 for streams, links, watersheds and HAND), the
    threshold of the streams in cells and in hectares and how it was chosen, and how the outlets were snapped
    (distance). For HAND, give the stream network it was measured from: the same DEM with another threshold
    gives other heights.

## Settings that change the answer

The threshold of the streams changes HAND more than anything else, because HAND is measured to the nearest
stream: with fewer streams, every cell is farther above one.

```tfslider
label: Streams from (hectares)
caption: "HAND measured to the networks of six thresholds (the same colours, 0 to 300 m). With streams from 9 ha, every hollow has a stream and the median HAND is 60 m; with streams from 450 ha, only the main valleys count and it is 205 m."
frames:
  - { value: "9 ha", src: /img/tutorials/terrain-hydrology/streams-watersheds-and-hand/hand_100.png, stats: { "Median HAND": "59.6 m", "90th percentile": "158 m", "Below 5 m": "8.5%" } }
  - { value: "22.5 ha", src: /img/tutorials/terrain-hydrology/streams-watersheds-and-hand/hand_250.png, stats: { "Median HAND": "83.5 m", "90th percentile": "202 m", "Below 5 m": "6.0%" } }
  - { value: "45 ha", src: /img/tutorials/terrain-hydrology/streams-watersheds-and-hand/hand_500.png, stats: { "Median HAND": "106.2 m", "90th percentile": "235 m", "Below 5 m": "4.5%" } }
  - { value: "90 ha", src: /img/tutorials/terrain-hydrology/streams-watersheds-and-hand/hand_1000.png, stats: { "Median HAND": "135.9 m", "90th percentile": "279 m", "Below 5 m": "3.5%" } }
  - { value: "225 ha", src: /img/tutorials/terrain-hydrology/streams-watersheds-and-hand/hand_2500.png, stats: { "Median HAND": "177.9 m", "90th percentile": "330 m", "Below 5 m": "2.6%" } }
  - { value: "450 ha", src: /img/tutorials/terrain-hydrology/streams-watersheds-and-hand/hand_5000.png, stats: { "Median HAND": "204.6 m", "90th percentile": "370 m", "Below 5 m": "2.1%" } }
start: 2
```

So choose the threshold for the question. For the soil water of a hillslope, the drainage is every channel
that actually carries water, including small ones (a small threshold); for the floodplain of a river, the
drainage is the river (a large threshold, or a map of the river given as the `streams` input). The same holds
for the Strahler order: with streams from 9 ha the main river is of order 5, with streams from 450 ha of order
2. Never compare orders, drainage densities or HAND computed with different thresholds or resolutions.

The other settings matter less: **snap** only matters when points are far from the streams, and **flats**
should stay on (with a filled DEM it is what gives the filled areas a direction).

## Common pitfalls

- **An outlet not on the stream.** [[hydro.watersheds]] takes the seeds as they are. Burn the gauge's point
  straight onto the grid ([[vector.rasterize]]) and use it as a seed, and here its watershed is 14 cells
  (1.3 ha), because the point lies 46 m off the stream, on the valley side. Snap outlets first, through
  [[hydro.stream_links]] as above, and check the area of every watershed.
- **Different DEMs or directions.** Streams, links, watersheds and HAND must all come from the same filled DEM;
  directions from another DEM (or a breached one for some, a filled one for others) give links that do not
  follow the streams.
- **Watersheds cut by the edge.** If the watershed of a gauge touches the edge of the map, part of it is
  missing. Use a DEM that covers the whole basin.
- **Cells with no stream.** Near the edges, cells whose water leaves the map before reaching a stream have no
  HAND, no distance and no subbasin (0). They are not errors, but they are not part of any catchment you
  studied.
- **Taking extracted streams for real ones.** Compare the network with a map, aerial images or the field.
  Under forest canopy, an SRTM network can be shifted by a cell or two; on flat land, streams can follow
  artefacts.
- **HAND thresholds from another region.** See above: calibrate them.
- **Units of the threshold.** A threshold of 500 is 500 cells with `units = cells` but 500 m² with
  `units = area`: a stream in every cell.

## Going further

- [Part 1: Depressions, flow directions and flow accumulation](../depressions-and-flow/).
- [Part 3: Wetness, stream power and sediment connectivity](../wetness-and-connectivity/): indices of soil
  moisture and erosion, and the index of connectivity that uses the distance to the streams.
- [Part 4: Watershed simulation with SWAT+](../swat-plus/): subbasins, channels and daily water budgets.
- [Ecosystem services](../../ecosystem-services/): InVEST's sediment and water models, which use the same
  routing.
- The operators' reference: [[hydro.streams]], [[hydro.stream_links]], [[hydro.watersheds]], [[hydro.hand]],
  [[hydro.distance_to_channel]].

## References

- Nobre, A. D., Cuartas, L. A., Hodnett, M., Rennó, C. D., Rodrigues, G., Silveira, A., Waterloo, M., & Saleska, S. (2011). Height Above the Nearest Drainage – a hydrologically relevant new terrain model. *Journal of Hydrology*, 404(1–2), 13–29. https://doi.org/10.1016/j.jhydrol.2011.03.051
- Rennó, C. D., Nobre, A. D., Cuartas, L. A., Soares, J. V., Hodnett, M. G., Tomasella, J., & Waterloo, M. J. (2008). HAND, a new terrain descriptor using SRTM-DEM: Mapping terra-firme rainforest environments in Amazonia. *Remote Sensing of Environment*, 112(9), 3469–3481. https://doi.org/10.1016/j.rse.2008.03.018
- Schietti, J., Emilio, T., Rennó, C. D., Drucker, D. P., Costa, F. R. C., Nogueira, A., Baccaro, F. B., Figueiredo, F., Castilho, C. V., Kinupp, V., Guillaumet, J.-L., Garcia, A. R. M., Lima, A. P., & Magnusson, W. E. (2014). Vertical distance from drainage drives floristic composition changes in an Amazonian rainforest. *Plant Ecology & Diversity*, 7(1–2), 241–253. https://doi.org/10.1080/17550874.2013.783642
- Strahler, A. N. (1957). Quantitative analysis of watershed geomorphology. *Eos, Transactions American Geophysical Union*, 38(6), 913–920. https://doi.org/10.1029/TR038i006p00913
- Tarboton, D. G., Bras, R. L., & Rodriguez-Iturbe, I. (1991). On the extraction of channel networks from digital elevation data. *Hydrological Processes*, 5(1), 81–100. https://doi.org/10.1002/hyp.3360050107
