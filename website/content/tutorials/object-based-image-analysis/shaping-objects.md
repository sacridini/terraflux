---
title: "Shaping objects: grow, shrink, open, close, fill, smooth and refine"
short: Shaping objects
description: "Change the outline of objects cell by cell: a shore buffer by an exact distance, thin necks cut and gaps closed, holes filled, spiky outlines rounded, and each border cell sent to the neighbour it looks like. See what each mode does to the cells, the number of objects and the edge, on the Sentinel-2 clip of Lago Paranoá."
order: 170
level: advanced
time: 60 min
ops: [objects.resize, segment.threshold, raster.calc, segment.multiresolution, objects.rules, objects.merge, objects.features, objects.remove_small]
series: Object-based image analysis
part: 17
---

An object's outline comes from the segmenter, and a segmenter draws it along cell edges: stairs, spikes, one-cell necks
that join two blocks, pinholes in a patch. A rule, a shape attribute or an area then reads that outline as if it were the
thing on the ground. In this tutorial you change outlines on purpose, with one operator, [[objects.resize]]. You will
buffer the lake by 40 m with an exact distance, remove specks and cut necks (*open*), close slits and fill holes
(*close*, *fill*), round the outlines (*smooth*), and move each border cell to the neighbouring object it resembles
(*refine*). Every mode is run on real objects and measured: cells, number of objects and length of edge before and after.

## The idea

Seven modes, one operator. The first four are the operations of **mathematical morphology**, the study of shapes by
probing them with a small shape (Serra, 1982; Haralick et al., 1987; Soille, 2004). The probe here is a disc whose radius is
the `distance`.

| Mode | What it does to the chosen objects | The morphology word |
|---|---|---|
| `grow` | Takes the free cells (or the cells of chosen classes) within the distance, each by the nearest object | dilation |
| `shrink` | Gives back the cells within the distance of a cell that is not the object's | erosion |
| `open` | `shrink`, then grow back over the cells the object had: cuts thin necks, drops specks | opening |
| `close` | `grow`, then take back the cells that were added and are near an outside cell: fills slits and small gaps | closing |
| `fill` | Gives each hole (cells of no object, or of chosen classes) that one object encloses to that object | hole filling |
| `smooth` | Gives each cell the most common label in a disc: spikes and notches go, corners round | a mode filter |
| `refine` | Sends each border cell to the neighbouring object whose band means are closest | border optimization |

eCognition users know these as *pixel-based object resizing*, *morphology*, *fill holes* and *border optimization*. They
are the same ideas (what eCognition calls the *candidate* classes of a growth is `into` here).

Two ways to grow are worth telling apart, because they give different shapes.

!!! science "Key idea: exact growth and growth ring by ring"
    **Exact.** With no condition, a free cell takes the nearest object if that is within the distance. Terraflux gets this
    from a Euclidean distance transform of the whole map, with the chosen objects as sources (Felzenszwalb & Huttenlocher,
    2012), in one pass. A single cell grows into a disc.

    **Ring by ring.** With a `mask` (a map: only cells that are not 0 may be taken) or `into` (the classes whose cells may
    be taken), the growth has to follow the condition, so it goes one ring of cells at a time, and a cell takes the object
    with most cells around it (a tie goes to the lower number). The shape now depends on which cells count as neighbours:
    8 neighbours (the cell's corners too) grow a square, 4 neighbours a diamond. A condition cannot be jumped over, which is
    the point: the growth stops at the barrier even when the barrier is thinner than the distance.

## When to use it

- **Before measuring shape.** Area, perimeter, compactness, length and width ([part 4](/tutorials/object-based-image-analysis/describing-objects/))
  read the stairs and the pinholes. If the question is about shape (how compact are the urban blocks, how long is the
  shore), shape the outline first, and say you did.
- **A zone around objects.** A 40 m shore buffer, a 100 m band around forest patches: `grow` by an exact distance, and take
  the difference with the original objects.
- **Specks and necks.** A one-cell false object, two blocks joined by one road cell: `open`.
- **Gaps and holes.** A field split by a track, a patch with pinholes where a tree shades the roof: `close` and `fill`.
- **Borders that sit one or two cells off.** `refine`, after a segmentation whose borders are close to right.

Do not use it:

- to **change a class**. [[objects.resize]] moves cells between objects and keeps each piece's class. A wrong class is
  a rule or a sample problem ([parts 5 and 6](/tutorials/object-based-image-analysis/rules-and-class-trees/));
- to **invent detail the image does not have**. On a 10 m image a 30 m opening is three cells, and shaping does not add
  what the pixels never showed;
- to **hide a bad segmentation**. If objects are wrong over large parts of the image, change the scale or the segmenter
  ([part 3](/tutorials/object-based-image-analysis/choosing-the-scale/)), or edit by hand
  ([part 10](/tutorials/object-based-image-analysis/editing-by-hand/)).

## What you need

- The Sentinel-2 clip of the series (Lago Paranoá, Brasília, 9 August 2024; 580 by 580 cells of 10 m). The models read it
  with [[data.stac]]; for no network, the [clip to download](/data/tutorials/object-based-image-analysis/sentinel2_brasilia.tif)
  has the same five bands.
- **Objects with classes.** The modes `grow`, `close` and `fill` take cells *from* classes (`into`), and every mode can be
  limited to some classes (`classes`). The page makes two sets of objects: the **waters** (an NDWI threshold, as in
  [part 7](/tutorials/object-based-image-analysis/water-by-thresholds/)) and the **patches** (multiresolution objects
  classed by rules and merged by class, as in [parts 6](/tutorials/object-based-image-analysis/rules-and-class-trees/) and
  [8](/tutorials/object-based-image-analysis/levels/)).
- There is no template for this operator: the table above and the models below are the starting points.

## Build the model: water, grow and shrink

### Step 1: objects that leave cells free

A cell can only be *grown into* if it is free: it belongs to no object. A segmentation covers every cell, so there is
nothing free. The way to leave cells free is to give the segmenter cells without a value: [[segment.threshold]] makes no
object of a cell with no value.

Node 4 (`ndwi`) is the water index of part 7. Node 5 (`wet`) keeps it where it is above Otsu's cut, 0.065, and gives
`None` (a missing value) elsewhere. Node 6 (`waters`) cuts it at the same number: each connected piece of cells above it is
an object. The result is 11 objects (the lake and ten small waters), 21,962 cells, and 314,438 free cells, the land.

Node 7 (`buffer`) is [[objects.resize]] with `mode = "grow"` and `distance = 40`: **the distance is in map units**, metres
here. Node 8 (`ones`) is a map of ones, and node 9 (`ring`) grows the same objects with `steps = 4` but with that map as
`mask`. A mask of ones allows every cell, so it changes nothing about *which* cells may be taken, and makes the growth go
ring by ring. Comparing the two nodes isolates the effect of the metric.

```tfgraph
#! caption: The waters (an NDWI above Otsu's cut, nothing elsewhere), grown by an exact distance (node 7) and ring by ring under a mask of ones (node 9).
#! highlight: waters, buffer, ring
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "s2"
op = "data.stac"
params = { url = "https://earth-search.aws.element84.com/v1", collection = "sentinel-2-l2a", assets = ["blue", "green", "red", "nir", "swir16"], datetime = "2024-08-09", extent = [-47.89, -15.89, -47.835, -15.835], cellsize = 0.00009 }
note = "Five bands, one scene."

[[nodes]]
id = "utm"
op = "raster.reproject"
inputs = { raster = "s2.raster" }
params = { crs = "EPSG:31983", resolution = 10, extent = [190520, 8241280, 196320, 8247080] }

[[nodes]]
id = "refl"
op = "raster.calc"
inputs = { s = "utm.raster" }
params = { expression = "out image = s / 10000" }
note = "Reflectance 0 to 1."

[[nodes]]
id = "ndwi"
op = "raster.calc"
inputs = { s = "refl.image" }
params = { expression = "(s.green - s.nir) / (s.green + s.nir)" }
note = "The water index of part 7."

[[nodes]]
id = "wet"
op = "raster.calc"
inputs = { n = "ndwi.map" }
params = { expression = "n if n > 0.065 else None" }
note = "The NDWI where it is above Otsu's cut; no value elsewhere, so no object there."

[[nodes]]
id = "waters"
op = "segment.threshold"
inputs = { image = "wet.map" }
params = { thresholds = [0.065] }
note = "Eleven objects: the lake and ten small waters."

[[nodes]]
id = "buffer"
op = "objects.resize"
inputs = { objects = "waters.objects" }
params = { mode = "grow", distance = 40 }
note = "Every water grows into the free cells within 40 m, each cell to the nearest water: exact Euclidean."
why = "A shore buffer of known width."

[[nodes]]
id = "ones"
op = "raster.calc"
inputs = { n = "ndwi.map" }
params = { expression = "n * 0 + 1" }
note = "A map of ones: every cell is allowed."

[[nodes]]
id = "ring"
op = "objects.resize"
inputs = { objects = "waters.objects", mask = "ones.map" }
params = { mode = "grow", steps = 4 }
note = "The same four cells, ring by ring (8 neighbours)."
```

=== "Application"

    Add the nodes, or open the model file. Select `buffer` and press ++ctrl+enter++. In the Node pane the settings of
    [[objects.resize]] are `mode` (a choice), `distance` or `steps` (one of them), `classes`, `into`, `max_hole` and
    `neighbours`; connect a map to the input `mask` by dragging from its output port. The Viewer draws the resized objects.
    The node's `report` output is a table that says what each object did.

=== "Command line"

    ```bash
    terraflux run water.toml --out out
    ```

    The tables and the maps the model sends to an output or a `raster.write` node land in `out/`.

### Step 2: exact growth, and shrinking

Exact growth by 40 m takes the lake from **20,779 to 23,752 cells** (207.8 to 237.5 ha): a band of 29.7 ha along the
shore. The ten small waters grow far more in proportion, because a buffer is the same width around a big thing and a small
thing: the one-cell speck becomes 49 cells, the 26-cell pond 141. `shrink` is the other way: a cell of the object goes if it
is within `distance` of a cell that is not the object's. With `distance = 20` the lake loses **1,419 cells**, the band
of its edge, and the ten small waters lose nearly all of theirs (five go to nothing).

```tfgallery
columns: 3
caption: "The south shore of the lake and a small pond. Left, the 40 m buffer: exact growth, the cells added in yellow. Middle, the same objects shrunk by 20 m (the cells lost in cyan). Right, grown eight rings, but only into cells with an NDWI above -0.3 (a mask: the damp edge of the shore)."
images:
  - { src: /img/tutorials/object-based-image-analysis/shaping-objects/lake_grow.png, caption: "grow, distance = 40" }
  - { src: /img/tutorials/object-based-image-analysis/shaping-objects/lake_shrink.png, caption: "shrink, distance = 20" }
  - { src: /img/tutorials/object-based-image-analysis/shaping-objects/lake_damp.png, caption: "grow, steps = 8, mask: NDWI > -0.3" }
```

Look at the third image. The mask is the NDWI above -0.3, a condition a wet cell meets and a dry roof or street does not.
The growth runs ring by ring, eight rings, and **stops where the condition fails**: along the lake's southern edge, where
the ground is dry, it adds almost nothing, and the lake gains 710 cells (to 21,489) where the exact 40 m buffer gives it
2,973. The rule "grow while the cell still looks like water" is how you make an object follow a boundary that has a
spectral meaning.

### Step 3: the metric, on one cell

The best way to see what *exact* means is to grow something with no shape of its own: one cell. The one-cell water
object nearest to the lake's west bank (row 107, column 193 of the clip: a single bright pixel, probably the glare of a
roof) grown by four cells:

```tfgallery
columns: 4
caption: "One water cell (centre, grey) grown four cells. Exact growth makes a disc, ring by ring with 8 neighbours a square, with 4 neighbours a diamond. The cells added are yellow; the base is the image."
images:
  - { src: /img/tutorials/object-based-image-analysis/shaping-objects/speck_before.png, caption: "The cell" }
  - { src: /img/tutorials/object-based-image-analysis/shaping-objects/speck_exact.png, caption: "exact: 49 cells" }
  - { src: /img/tutorials/object-based-image-analysis/shaping-objects/speck_ring8.png, caption: "8 neighbours: 81 cells" }
  - { src: /img/tutorials/object-based-image-analysis/shaping-objects/speck_ring4.png, caption: "4 neighbours: 41 cells" }
```

A disc of radius 4 has about 50 cells (π × 4²), a square of side 9 has 81, and a diamond of the same reach has
41 (2 × 4² + 2 × 4 + 1). The numbers from the report are the three. For a lake these differences are small and matter
at the edges, but they are easy to forget: a buffer ring by ring is **not** a circle, and the 40 m band around the lake
is 4,058 cells grown with 8 neighbours and 2,973 exact, a third larger.

```tftable
data: /data/tutorials/object-based-image-analysis/shaping-objects/grow_ways.csv
labels: { way: "Way of growing", setting: "Settings", speck_cells: "The one cell, after", lake_cells: "The lake, after (from 20,779)" }
digits: 0
caption: "Four cells of growth, five ways: the cells the one-cell water and the lake hold afterwards, from the node's report. A `steps` with no mask or `into` is exact too (it is `distance` = steps × the cell)."
```

!!! tip "distance or steps?"
    `distance` is in **map units** (metres here) and `steps` is in **cells**; give one, not both (Terraflux refuses both).
    Without a `mask` or `into`, `steps = 4` is the exact growth of four cells, the same as `distance = 40` on 10 m
    cells. With a `mask` or `into`, a `distance` is rounded to whole rings. For `refine`, `steps` is the number of passes.

### Step 4: open the waters

The mode `open` is a `shrink` followed by a growth back over only the cells the object had, so a thin part that the shrink
removed does not come back. On the waters it is a speck filter.

```tftable
data: /data/tutorials/object-based-image-analysis/shaping-objects/water_report.csv
labels: { object: "Water", cells: "Cells", grow_40m: "grow 40 m", shrink_20m: "shrink 20 m", open_20m: "open 20 m", open_pieces: "Pieces after open" }
digits: 0
caption: "The report of three nodes for the eleven waters: the cells each has after growing 40 m, shrinking 20 m and opening 20 m, and the number of objects each ended in. The five waters of 20 cells or fewer go, and the lake loses 41 cells."
```

Five of the eleven waters are one to twenty cells (a pool, a roof's glare, a pond too small for a 10 m pixel): after `open`
with 20 m they are gone, the larger ones lose a few cells, and the lake, 20,779 cells, loses 41. The `report` has a row per
chosen object, so you can see this at once: `before`, `after` and `pieces`, the number of objects the cells ended in.

## Build the model: built-up patches, open, close, fill, smooth

The lake is a clean shape, because water is easy to separate. A harder outline is **urban**: houses, streets, trees in the
gardens and bare plots make a patch with fingers, necks and holes. The second set of objects is the one of parts 6 and 8:
multiresolution objects of scale 2 (3,035), their attributes, a class tree (water, forest, built-up, cerrado), and
[[objects.merge]] (by class, the default) joining neighbours of the same class into **patches**. There are 43 built-up
patches, 1,007.7 ha in all.

```tfgraph
#! caption: The patches: multiresolution objects classed by rules and merged into one object per connected piece of each class.
#! highlight: patches
#! numbered: true
#! fragment: true
[[nodes]]
id = "seg"
op = "segment.multiresolution"
inputs = { image = "refl.image" }
params = { scale = 2 }
note = "3,035 objects."

[[nodes]]
id = "feat"
op = "objects.features"
inputs = { objects = "seg.objects", image = "refl.image" }
params = { features = ["spectral"] }
note = "Means, brightness, NDVI, NDWI per object."

[[nodes]]
id = "calc"
op = "objects.calc"
inputs = { objects = "feat.objects" }
params = { expression = "out vis = (mean_red + mean_green + mean_blue) / 3" }
note = "The brightness in visible light."

[[nodes]]
id = "rules"
op = "objects.rules"
inputs = { objects = "calc.objects" }
params = { classes = [
  { name = "water", rule = "above(ndwi, -0.1, 0.2)", colour = "#2b6cb0" },
  { name = "land", rule = "below(ndwi, -0.1, 0.2)" },
  { name = "forest", rule = "above(ndvi, 0.45, 0.6)", parent = "land", colour = "#2f7d3b" },
  { name = "open", rule = "below(ndvi, 0.45, 0.6)", parent = "land" },
  { name = "built_up", rule = "above(vis, 0.06, 0.09)", parent = "open", colour = "#c0504d" },
  { name = "cerrado", rule = "below(vis, 0.06, 0.09)", parent = "open", colour = "#d9b86c" },
] }
note = "Water, forest, built-up and cerrado by fuzzy rules in a class tree."

[[nodes]]
id = "patches"
op = "objects.merge"
inputs = { objects = "rules.objects" }
note = "Neighbours of the same class become one object: 301 patches, 43 of them built-up."
```

![Where the figures of this page are](/img/tutorials/object-based-image-analysis/shaping-objects/where.png "The clip (true colour). The boxes are the two places zoomed in this tutorial: the south shore of the lake and a block of the town.")

Every later node reads `patches.objects` and is limited to the class `built_up` by `classes = ["built_up"]`.

### Step 5: the five modes on the built-up patches

The base figure is the block of the town (the lower box above), at 6 times its size, the built-up cells tinted red. The
patch has a finger that reaches the forest on the right, a neck on the left, and holes (cerrado and trees inside the
town).

![The block, before](/img/tutorials/object-based-image-analysis/shaping-objects/block_before.png "The built-up patch of the block before any change: the outline is the cell edges.")

In each figure below **yellow cells are added to the built-up class and cyan cells are removed**, compared with this one.

```tfgraph
#! caption: The four modes of the page, in a chain: each reads the objects the one before gave.
#! highlight: open, close, fill, smooth
#! numbered: true
#! fragment: true
[[nodes]]
id = "open"
op = "objects.resize"
inputs = { objects = "patches.objects" }
params = { mode = "open", classes = ["built_up"], distance = 30 }
note = "Shrink by 30 m, grow back only over the cells the object had: a part too narrow for a disc of 30 m radius (about six cells) goes."

[[nodes]]
id = "close"
op = "objects.resize"
inputs = { objects = "open.objects" }
params = { mode = "close", classes = ["built_up"], distance = 30, into = ["cerrado", "forest"] }
note = "Grow 30 m into cerrado and forest cells, then take back the added cells that are close to anything else."

[[nodes]]
id = "fill"
op = "objects.resize"
inputs = { objects = "close.objects" }
params = { mode = "fill", classes = ["built_up"], into = ["cerrado", "forest"], max_hole = 40 }
note = "A hole of cerrado or forest of 40 cells or fewer, enclosed by one built-up patch, becomes built-up."

[[nodes]]
id = "smooth"
op = "objects.resize"
inputs = { objects = "fill.objects" }
params = { mode = "smooth", classes = ["built_up"], distance = 30 }
note = "Each cell takes the most common label within 30 m."
```

Each of the four is shown on its own first (all read `patches.objects`, as the figures need), and then the chain.

```tfgallery
columns: 3
caption: "The block after each mode on its own, 20 to 30 m. Yellow: cells the built-up class gained. Cyan: cells it lost. Top: shrink 20 m, open 30 m, close 30 m. Bottom: fill (holes up to 40 cells), smooth 30 m, and the four in a chain."
images:
  - { src: /img/tutorials/object-based-image-analysis/shaping-objects/block_shrink.png, caption: "shrink, distance = 20" }
  - { src: /img/tutorials/object-based-image-analysis/shaping-objects/block_open.png, caption: "open, distance = 30" }
  - { src: /img/tutorials/object-based-image-analysis/shaping-objects/block_close.png, caption: "close, distance = 30, into cerrado and forest" }
  - { src: /img/tutorials/object-based-image-analysis/shaping-objects/block_fill.png, caption: "fill, max_hole = 40" }
  - { src: /img/tutorials/object-based-image-analysis/shaping-objects/block_smooth.png, caption: "smooth, distance = 30" }
  - { src: /img/tutorials/object-based-image-analysis/shaping-objects/block_chain.png, caption: "open, close, fill, smooth" }
```

What each did, in the numbers of the whole clip:

```tftable
data: /data/tutorials/object-based-image-analysis/shaping-objects/modes.csv
labels: { step: "Step", cells: "Built-up cells", area_ha: "Area (ha)", patches: "Patches", edge_km: "Edge (km)", change_cells: "Change (cells)" }
digits: { cells: 0, area_ha: 1, patches: 0, edge_km: 1, change_cells: 0 }
caption: "The built-up class after each mode on its own and after the chain. Patches: built-up objects. Edge: the length of the boundary of the class (a cell edge between a built-up cell and any other, times 10 m), computed by the figure's script from the map."
```

- **shrink** takes 28% of the built-up cells (28,288) and cuts the patches in pieces: the 43 become 61, because many are joined by thin necks. It is the building block of `open`, rarely what you want alone.
- **open 30 m** removes only 5.6% of the cells (5,664) and cuts the 43 patches into 56. The patch of the report below, the largest, was in one piece and is in six after: the parts narrower than six cells were what held it together.
- **close 30 m** *adds* 17,070 cells (+17%) and cuts the length of edge by 31%, from 182.8 to 125.9 km: the gaps between houses, trees and plots become built-up. It also takes cells of the cerrado and the forest next to the town, which may or may not be what you mean: see the pitfalls.
- **fill** adds only 284 cells: a hole enclosed by one patch and of 40 cells or fewer is rare at 10 m; the real gaps in the town are open to a street, which is what `close` is for.
- **smooth 30 m** changes the area by -2.3% and keeps the number of patches: it rounds, it does not cut or join.

```tftable
data: /data/tutorials/object-based-image-analysis/shaping-objects/report_open.csv
labels: { object: "Patch", before: "Cells before", after: "Cells after", pieces: "Pieces" }
digits: 0
caption: "The report of the open node for the five largest built-up patches: a row per chosen object, with the cells it had and has and the number of objects its cells ended in. An object cut in two becomes two objects of its class."
```

### Step 6: the chain, and what is lost

The chain of the page's model is open, then close, then fill, then smooth: **take out the specks and necks first, close the
gaps, fill what is left, and round at the end**. The order matters: a `close` before an `open` would join a speck to a patch
and the `open` would not find it. The chain gives 110,729 built-up cells (+9.9%), 56 patches and 125.4 km of edge, and the
block of the figure is the last of the gallery above.

Two things to know when you read the result.

- The objects the chain gives have **no attributes**: `objects.resize` makes new objects out of old cells, so the table of
  means is not the old one, and the level above, if there was one, is dropped (with a warning). The page's script ends the chain with
  [[objects.features]] on the shaped objects, to rebuild the attributes. The area-weighted *compactness* of the built-up
  patches (length times width over area, 1 for a rectangle) goes from **2.31 to 1.90**.
- A shaped class **is not the class the rules gave**. The chain added 9,958 built-up cells (99.6 ha): read it as an
  outline for shape analysis, not as the urban area. If the number you report is an area, report it from the objects before
  shaping, or from a shaping tested against a reference.

## Build the model: refine, the border optimization

The seven modes so far move a border by a distance. `refine` moves it by **similarity**: each cell on a border goes to the
adjacent object whose band means are closest to the cell's values (the bands are put in z-scores over the whole image, so
no band outweighs another). It does this in `steps` passes, and each pass reads the means of the pass before, which
makes the result the same on any number of threads. `refine` is the only mode that needs the **image**.

It acts on any objects, not only the patches. It is most useful on the fine ones of a segmentation, whose borders are
cell-wide off, and it is run here on the 3,035 multiresolution objects of scale 2.

```tfgraph
#! caption: The objects of the multiresolution segmentation, refined in three passes (node 5), and the pieces of fewer than four cells joined back into the neighbour that shares the longest border (node 6).
#! highlight: refine, clean
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "s2"
op = "data.stac"
params = { url = "https://earth-search.aws.element84.com/v1", collection = "sentinel-2-l2a", assets = ["blue", "green", "red", "nir", "swir16"], datetime = "2024-08-09", extent = [-47.89, -15.89, -47.835, -15.835], cellsize = 0.00009 }
note = "Five bands, one scene."

[[nodes]]
id = "utm"
op = "raster.reproject"
inputs = { raster = "s2.raster" }
params = { crs = "EPSG:31983", resolution = 10, extent = [190520, 8241280, 196320, 8247080] }

[[nodes]]
id = "refl"
op = "raster.calc"
inputs = { s = "utm.raster" }
params = { expression = "out image = s / 10000" }
note = "Reflectance 0 to 1."

[[nodes]]
id = "seg"
op = "segment.multiresolution"
inputs = { image = "refl.image" }
params = { scale = 2 }
note = "3,035 objects."

[[nodes]]
id = "refine"
op = "objects.resize"
inputs = { objects = "seg.objects", image = "refl.image" }
params = { mode = "refine", steps = 3 }
note = "Each border cell goes to the adjacent object of the closest band means; three passes."
why = "The segmenter's borders follow cell edges chosen by a merging order, not each cell's own values."

[[nodes]]
id = "clean"
op = "objects.remove_small"
inputs = { objects = "refine.objects" }
params = { min_cells = 4, into = "border" }
note = "A cell moved away can leave a piece of one or two cells: it joins the neighbour it shares most border with."
```

How much it improved the objects is a number you can compute: the **within-object sum of squares**, the squared distance of
every cell's bands (as z-scores) from its object's mean. A smaller sum means objects more alike inside.

```tftable
data: /data/tutorials/object-based-image-analysis/shaping-objects/refine.csv
labels: { passes: "Passes", objects: "Objects", within_ss: "Within-object sum of squares", share_of_start: "% of the start", cells_moved: "Cells moved (net, at least)" }
digits: { passes: 0, objects: 0, within_ss: 0, share_of_start: 1, cells_moved: 0 }
caption: "Border optimization of the 3,035 objects of scale 2, for 1 to 10 passes, computed from the maps of each run (the sum of squares, over the five bands in z-scores, is the script's own measure, not an output of the operator)."
```

One pass moves at least 14,803 cells (4.4% of the clip; the count is the net change of each object's cells) and lowers
the sum to 79% of the start; three passes, to 60%; ten, to 37%. The objects are more uniform inside, which is what the
operator was asked to do, and the count of objects goes **up**: 3,035, then 3,989, 5,413 and 9,251 after 1, 3 and 10 passes. A cell that leaves an object can cut it in two, and every piece of an object is an object of
its own. Ten passes triple the number of objects, and no one wants thousands of one- and two-cell fragments.

```tfgallery
columns: 3
caption: "A part of the town and the bare ground beside it (60 by 70 cells). Left, the objects of scale 2. Middle, after three passes of refine. Right, the outlines together: white on both, cyan only before, yellow only after."
images:
  - { src: /img/tutorials/object-based-image-analysis/shaping-objects/refine_before.png, caption: "scale 2: 3,035 objects" }
  - { src: /img/tutorials/object-based-image-analysis/shaping-objects/refine_after.png, caption: "refine, 3 passes: 5,413 objects" }
  - { src: /img/tutorials/object-based-image-analysis/shaping-objects/refine_diff.png, caption: "the outlines: before, after, both" }
```

In the figure the outlines of the bright bare ground in the middle and of the houses to its left change shape, and more
small objects appear. Node 6, [[objects.remove_small]], joins the pieces of
fewer than four cells to their neighbour: with it, three passes end at **4,220 objects and 60.8%** of the sum of squares
(208,472), nearly the sum without the cleaning (205,156) with 1,193 objects fewer. `refine` and `remove_small` come as a pair.

## Read the results

What to look at when you shape objects:

1. **The report.** One row per chosen object: `before`, `after` and `pieces`. A growing `pieces` is a warning: it means
   the operation cut objects in two (a thin neck gone, a field split by a hole). A `before` equal to `after` for every
   row says nothing happened: the usual cause is nothing to take (a `grow` with no free cell and no `into`).
2. **The count and the cells, class by class.** The first thing to report is how much the shaping moved: the share of
   cells, and of objects. Here: 5.6% of the built-up cells for the opening, 17% for the closing.
3. **A shape measure before and after.** The compactness fell from 2.31 to 1.90 and the length of edge from 182.8 to
   125.4 km: the outlines are simpler, which is what shaping is for.
4. **The picture.** Overlay the new outline on the image (the yellow and cyan cells of the figures). A shaping that is
   right looks like a better-drawn version of the same thing. One that is wrong adds cells where the image says no (the
   edge of the forest next to the town in the `close` figure).

Report the settings with the result (the mode, the distance in metres, the classes, `into`, the order of the chain) and
which numbers come before and which after the shaping.

## Settings that change the answer

### The distance of the opening

The slider is the opening distance on the same block, from 0 (the patch as it is) to 60 m. The numbers are of the whole clip.

```tfslider
label: Opening distance (m)
caption: "open on the built-up patches. Yellow: cells added to the class, cyan: cells lost. From 30 m a patch is cut in pieces where a part narrower than six cells joins two others, and the 43 patches become 56. The chart is the length of the boundary of the class."
frames:
  - { value: 0, src: /img/tutorials/object-based-image-analysis/shaping-objects/open_0.png, caption: "0: the patches as they are", stats: { Cells: "100,771", Patches: "43", "Edge (km)": "182.8" } }
  - { value: 10, src: /img/tutorials/object-based-image-analysis/shaping-objects/open_10.png, caption: "10 m: one cell", stats: { Cells: "99,435", Patches: "43", "Edge (km)": "181.8" } }
  - { value: 20, src: /img/tutorials/object-based-image-analysis/shaping-objects/open_20.png, caption: "20 m", stats: { Cells: "97,567", Patches: "44", "Edge (km)": "179.3" } }
  - { value: 30, src: /img/tutorials/object-based-image-analysis/shaping-objects/open_30.png, caption: "30 m (the page's)", stats: { Cells: "95,107", Patches: "56", "Edge (km)": "171.8" } }
  - { value: 40, src: /img/tutorials/object-based-image-analysis/shaping-objects/open_40.png, caption: "40 m", stats: { Cells: "90,107", Patches: "56", "Edge (km)": "159.2" } }
  - { value: 50, src: /img/tutorials/object-based-image-analysis/shaping-objects/open_50.png, caption: "50 m", stats: { Cells: "82,948", Patches: "58", "Edge (km)": "139.4" } }
  - { value: 60, src: /img/tutorials/object-based-image-analysis/shaping-objects/open_60.png, caption: "60 m: six cells", stats: { Cells: "74,307", Patches: "53", "Edge (km)": "117.6" } }
chart:
  type: line
  data: /data/tutorials/object-based-image-analysis/shaping-objects/open_sweep.csv
  x: open_m
  y: [edge_km]
  labels: { edge_km: "Edge (km)" }
  colors: { edge_km: "#3b8a5a" }
  xlabel: Opening distance (m)
  ylabel: Edge of the built-up class (km)
```

The edge falls slowly to 30 m and then fast: the cells of the first rings are single-cell specks and fingers, and from 40 m on
the opening eats into the core of real patches (the area falls 11% by 40 m and 26% by 60 m). A distance of one third to
one half of the width of the *thinnest thing you want to keep* is a safe start; here the streets and plots are 3 to 6 cells wide.

### The radius of the smoothing

```tfslider
label: Smoothing radius (m)
caption: "smooth on the built-up patches. The area and the number of patches hardly move until a radius of 40 m or more: a smoothing rounds the outline and does not cut or join. The chart is the edge."
frames:
  - { value: 0, src: /img/tutorials/object-based-image-analysis/shaping-objects/smooth_0.png, caption: "0: the patches as they are", stats: { Cells: "100,771", Patches: "43", "Edge (km)": "182.8" } }
  - { value: 10, src: /img/tutorials/object-based-image-analysis/shaping-objects/smooth_10.png, caption: "10 m: less than a cell and a half", stats: { Cells: "100,745", Patches: "43", "Edge (km)": "182.3" } }
  - { value: 20, src: /img/tutorials/object-based-image-analysis/shaping-objects/smooth_20.png, caption: "20 m", stats: { Cells: "99,506", Patches: "43", "Edge (km)": "179.9" } }
  - { value: 30, src: /img/tutorials/object-based-image-analysis/shaping-objects/smooth_30.png, caption: "30 m (the page's)", stats: { Cells: "98,416", Patches: "43", "Edge (km)": "176.3" } }
  - { value: 40, src: /img/tutorials/object-based-image-analysis/shaping-objects/smooth_40.png, caption: "40 m", stats: { Cells: "97,511", Patches: "43", "Edge (km)": "173.7" } }
  - { value: 60, src: /img/tutorials/object-based-image-analysis/shaping-objects/smooth_60.png, caption: "60 m: the smallest patches go", stats: { Cells: "95,775", Patches: "39", "Edge (km)": "168.5" } }
chart:
  type: line
  data: /data/tutorials/object-based-image-analysis/shaping-objects/smooth_sweep.csv
  x: smooth_m
  y: [edge_km]
  labels: { edge_km: "Edge (km)" }
  colors: { edge_km: "#3b8a5a" }
  xlabel: Smoothing radius (m)
  ylabel: Edge of the built-up class (km)
```

A radius of 10 m (a disc of one cell and its neighbours) changes 26 cells in 100,000: below a cell and a half the
filter does almost nothing on a 10 m image. At 60 m the 4 smallest patches vanish (43 to 39): a patch smaller than the disc
loses the vote to everything around it.

### The other settings

- **`neighbours`**: 4 or 8, for the cells an object is connected by and for the shape of a ring. Objects made with 8 stay
  whole with 8; a ring by ring growth with 4 makes diamonds.
- **`into`** for `grow`, `close` and `fill`: *which* cells may be taken. A class that is also chosen by `classes` is not
  taken. Give the **leaf** class names (here `cerrado` and `forest`, not their parent `open` or `land`).
- **`mask`**: a map, and a cell with 0 or no value is not taken. It works for `grow`, `close` and `fill` (the cells of a
  hole have to be allowed).
- **`max_hole`** for `fill`: the largest hole to fill, in cells. Without it every enclosed hole is filled, a lake of cells
  inside a built-up patch too.
- **`classes`**: the objects that change. The others are kept, and only the objects of these classes grow or shrink.

## Common pitfalls

- **Nothing to grow into.** A `grow` of objects that cover the whole image does nothing, and so does a `close`: there
  are no free cells and no `into`. The report says so (every `after` equals the `before`). Give `into` or leave cells
  without objects, as the waters do.
- **Metres and cells.** `distance` is in map units, `steps` in cells. On a geographic grid (degrees) a `distance` of 40 is
  forty degrees. Put the image in a projected system first.
- **The class tree's names.** `classes` and `into` take the names of the classes the objects have. After
  [[objects.rules]] these are the **leaves** of the tree: `forest`, `built_up`, `cerrado`, not `land` or `open`.
- **A shaped class has the wrong area.** The chain added 9.9% to the built-up area. If the area is the result, report it
  before shaping.
- **Specks left by `close`.** A closing can leave a one-cell island of the added cells where two grown halos met and the
  cells around were taken back. It happened twice in the run of this page, and the report's `pieces` shows it. Run
  [[objects.remove_small]] after.
- **The attributes are gone.** The objects out of [[objects.resize]] have a table of cells and area (with the image, the
  means), no shape, texture or context. Run [[objects.features]] after, and anything that read the level above again.
- **`refine` makes more objects.** Fragments are objects. Use few passes (one to three) and clean with
  [[objects.remove_small]].
- **An opening is not a segmentation.** After `open`, an object cut in two is two objects with the same class and
  different numbers: the first numbering is not kept. Do not compare ids before and after.

## Going further

- Where the objects come from and how to choose the scale: [What an object is](/tutorials/object-based-image-analysis/what-is-an-object/) (part 2) and [Choosing the scale](/tutorials/object-based-image-analysis/choosing-the-scale/) (part 3).
- The shape attributes that shaping changes (compactness, length, width): [Describing objects](/tutorials/object-based-image-analysis/describing-objects/) (part 4).
- Merge, grow into neighbours by a condition and remove small objects *by object* instead of by cell: [Levels](/tutorials/object-based-image-analysis/levels/) (part 8) and [[objects.grow]], [[objects.merge]], [[objects.remove_small]].
- Correct a few objects by hand: [Editing by hand](/tutorials/object-based-image-analysis/editing-by-hand/) (part 10).
- Keep a segmentation inside parcels: [Classes by parcel](/tutorials/object-based-image-analysis/classes-by-parcel/) (part 16).
- The eCognition algorithms and their operators in one table: [Coming from eCognition](/tutorials/object-based-image-analysis/from-ecognition/).
- The operator: [[objects.resize]].

## References

- Felzenszwalb, P. F., & Huttenlocher, D. P. (2012). Distance transforms of sampled functions. *Theory of Computing*, 8(1),
  415-428. https://doi.org/10.4086/toc.2012.v008a019
- Haralick, R. M., Sternberg, S. R., & Zhuang, X. (1987). Image analysis using mathematical morphology. *IEEE Transactions
  on Pattern Analysis and Machine Intelligence*, PAMI-9(4), 532-550. https://doi.org/10.1109/TPAMI.1987.4767941
- Serra, J. (1982). *Image Analysis and Mathematical Morphology*. Academic Press.
- Soille, P. (2004). *Morphological Image Analysis: Principles and Applications* (2nd ed.). Springer.
  https://doi.org/10.1007/978-3-662-05088-0
