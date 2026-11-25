---
title: "Classes by parcel: objects that stay inside boundaries"
short: Classes by parcel
description: "Segment an image so that no object crosses a parcel, a block or a management unit, give each object the parcel it lies in, and class it by rules that read both the spectra and the parcel. On the Sentinel-2 clip of Lago Paranoá, with blocks made from OpenStreetMap streets."
order: 160
level: advanced
time: 60 min
template: Classes by parcel
ops: [segment.multiresolution, vector.read, objects.features, objects.rules, objects.calc, vector.rasterize, table.join, table.aggregate]
series: Object-based image analysis
part: 16
---

A segmenter sees colour. A parcel boundary is not a colour: it is a line somebody drew, between two lots, two fields, a
protected area and the land beside it. An object that crosses the line has the spectra of both sides and belongs to
neither. In this tutorial you give the segmenter a layer of boundaries so that **no object crosses one**, you tell each
object the parcel it lies in, and you write rules that use the parcel as well as the spectra: *trees inside a block are not
the same thing as forest outside it*. At the end you have the area of each class in each parcel, which is what a report by
parcel asks for.

## The idea

Land is managed in units: a cadastral lot, a farm field, a city block, a reserve, a municipality. Analysis by object (Blaschke, 2010) is a
natural fit, since objects can be summed by unit, but only if every object lies in **one** unit. A segmentation made from
the image alone does not know about the units, and some of its objects straddle two of them.

Remote sensing has done this for a long time as *per-parcel* (or *per-field*) classification: classify the parcel as a
whole, or the pixels in it, using a vector layer (Janssen & Molenaar, 1995; De Wit & Clevers, 2004). In object-based
analysis the same idea is a **thematic layer**: a map or vector that the segmentation respects, and whose attributes the
rules can read. eCognition has it under that name. Terraflux has two parts, one at each end:

- **`bounds`**, an optional input of the segmenters: the layer's zones that no object crosses.
- **`thematic`** (and `field`), an optional input of [[objects.features]]: for each object, the zone it lies in
  (`th_mode`) and how much of it does (`th_share`).

!!! science "Key idea: a zone is a number per cell"
    Every kind of layer is turned into one number for each cell, and cells with the same number are in the same zone.

    - **A map**: each value is a zone. Cells with no value are one zone together.
    - **Vectors**: each feature is a zone. Where features overlap, the later one wins. Cells in no feature are **one
      zone together** (the land between the parcels).
    - **Objects**: each object is a zone, and the cells in no object (number 0) are one zone.

    No object is made across a zone. Some segmenters keep to the zones *as they work*: they never join cells of two
    zones ([[segment.multiresolution]], [[segment.region_growing]], [[segment.felzenszwalb]],
    [[segment.spectral_difference]]). The others (SLIC, SNIC, watershed, quickshift, mean shift, threshold, chessboard,
    quadtree) segment as usual and **cut afterwards**: an object that crosses a zone becomes one object per piece in a
    zone.

The two ends are separate on purpose. You can bound a segmentation and never read the zone (the objects just do not cross
it), and you can read a layer with `thematic` that the segmentation did not use (to find out how many objects cross it).

## When to use it

- **Statistics by unit.** The area of each class in each lot, field, block or reserve; the share of tree cover by
  district.
- **A boundary the image does not show.** A fence, a property line, a municipal limit, a reserve's edge between two
  stretches of the same cerrado.
- **Rules that depend on the unit.** In a protected area a bare patch is a clearing; in a block it is a vacant plot.
- **A known road or river network as a barrier.** Objects that stop at a road never spill onto the building beside it.

Do not use it:

- when the layer is **wrong** or **coarser than the image**. The objects will follow it. A polygon drawn at a scale of
  1:50,000 is off by tens of metres, and the segmentation will make that error into its objects;
- when the unit **is** the object. If every parcel will be classed as a whole, summarise the pixels by parcel
  ([[vector.zonal]], [[raster.zonal]]) and skip the segmentation;
- when you want the boundaries to **emerge**. The scale and the shape of the segmenter ([part 3](/tutorials/object-based-image-analysis/choosing-the-scale/))
  decide the boundaries in the image; this tutorial adds boundaries from outside.

## What you need

- The Sentinel-2 clip of the series (Lago Paranoá, Brasília, 9 August 2024; 580 by 580 cells of 10 m, as in
  [part 2](/tutorials/object-based-image-analysis/what-is-an-object/)). The model reads it with [[data.stac]]; for no
  network, the [clip to download](/data/tutorials/object-based-image-analysis/sentinel2_brasilia.tif) has the same five bands.
- **A layer of parcels**, a vector in any coordinate system (it is reprojected to the image's), with an attribute if you want
  to read one.

### The blocks of this page

OpenStreetMap has no layer of lots for this part of Brasília: its `landuse` polygons here are single shops, five lots of
Lago Sul and a military site. What it has is the **streets**, drawn in detail by its volunteers (Haklay & Weber, 2008). The polygons the streets close are blocks,
so the layer of this page is made from them. A script (`website/figures/object-based-image-analysis/obia_blocks.py`) asked
the Overpass API for the streets of the clip's box, cut the plane with them, and kept the polygons of 1 to 80 hectares
that are not slivers. The result is **33 blocks**, 846 ha (25% of the clip), in a file with three attributes: `block` (1
to 33), `area_ha` and `faces`, the kind of street (primary, secondary, tertiary or residential, as OpenStreetMap calls them)
along most of the block's boundary.

![The blocks of the page](/img/tutorials/object-based-image-analysis/classes-by-parcel/blocks.png "The 33 blocks of Lago Sul, from the streets of OpenStreetMap, over the Sentinel-2 clip. The colour of a line is the kind of street the block faces. The two red boxes are the places zoomed in the figures below.")

**[blocks.geojson](/data/tutorials/object-based-image-analysis/classes-by-parcel/blocks.geojson)** is the file the model reads
(EPSG:31983, the image's coordinate system); [its attribution](/data/tutorials/object-based-image-analysis/classes-by-parcel/ATTRIBUTION.txt)
is in the same folder. The data are © OpenStreetMap contributors, under the Open Database Licence (ODbL); the blocks are
a derived database and carry the same licence. The data are from 22 November 2026.

These are **not cadastral blocks**, and the page is honest about it. Service roads are left out, so a block here is
sometimes several real ones; the blocks cover a quarter of the clip, since many streets of the lower town are not
closed polygons; and the block at the top-left corner holds 6.0 ha of the lake, since no street closes the shore. For
the method that does not matter. For a report it would: use your cadastre.

The template **Classes by parcel** (the library's *Templates*, group *Objects*) is the same model on a synthetic image,
with nine parcels drawn by [[vector.grid_zones]] in place of the file. Open it, run it, and replace the parcels with a
[[vector.read]] of yours. **File > Use my data...** does it too: the row of the parcels takes a vector file
(GeoPackage, GeoJSON, Shapefile) and says how many of its polygons overlap your image. Bring the image and the parcels
together: the drawn parcels lie on the drawn image, and the window warns when you replace only one.

## Build the model

### Step 1: the parcels

[[vector.read]] reads the file. Its outputs are `vector` (the features, with their geometry and attributes) and `table`
(the attributes that are numbers). The path is relative to the model's folder.

```tfgraph
#! caption: The blocks, read from a file in the model's folder.
#! highlight: blocks
#! numbered: true
#! fragment: true
[[nodes]]
id = "blocks"
op = "vector.read"
params = { path = "blocks.geojson" }
note = "33 polygons: block, area_ha and faces."
```

### Step 2: a segmentation that stays inside

[[segment.multiresolution]] gets the optional input `bounds`: connect to it the `vector` output of node `blocks`.
Nothing else changes: the scale is the same 2 as in part 2 (the merging of Baatz & Schäpe, 2000, with one more rule: two objects of different zones are never joined).

```tfgraph
#! caption: The same multiresolution segmentation as in part 2, held inside the blocks by its input bounds.
#! highlight: seg
#! numbered: true
#! fragment: true
[[nodes]]
id = "seg"
op = "segment.multiresolution"
inputs = { image = "refl.image", bounds = "blocks.vector" }
params = { scale = 2 }
note = "3,107 objects, none across a block's edge."
why = "A block is a unit of management: an object across its edge has the spectra of the block and the street."
```

=== "Application"

    Add [[segment.multiresolution]] and connect the reflectance to `image`. Then **drag from the `vector` output port of
    the `vector.read` node to the `bounds` input port** of the segmenter: an optional input is drawn as a port like the
    others. Select the segmenter and press ++ctrl+enter++. The Viewer's *live segmentation* and *compare* take the node's
    `bounds` too: a segmenter that is compared gets the same one.

=== "Command line"

    ```bash
    terraflux run parcels.toml --out out
    ```

    The file `blocks.geojson` sits in the model's folder.

On the clip, **without** the bounds the segmentation has 2,973 objects, and **425 of them (14.3%) have cells in more than
one zone**: they hold 13.7% of the clip's cells. Most cross the strips of street between two blocks, which belong to no
block. **With** the bounds there are 3,107 objects, 134 more (+4.5%), and none crosses.

```tfcompare
caption: "Four blocks by the lake, objects of scale 2 (yellow outlines) and the blocks (magenta). Left, without bounds: the objects that cross a block's edge are tinted cyan. Right, with bounds: none. Drag the handle."
before: { src: /img/tutorials/object-based-image-analysis/classes-by-parcel/zoom_free.png, label: "No bounds: 425 objects cross" }
after: { src: /img/tutorials/object-based-image-analysis/classes-by-parcel/zoom_bound.png, label: "bounds = blocks: none" }
```

A map of zones works as well as vectors and gives the **same objects**: the model run with `bounds` the blocks rasterized
with [[vector.rasterize]] (the attribute `block`, `fill = 0`) gives, cell by cell, the same objects as the run with the
vector. Use a vector when you have one, and a map when the zones come from a classification or a raster in your model.

### Step 3: what each object knows about its parcel

[[objects.features]] gets the optional input `thematic`: the same vector (it can be another layer: the segmentation does not
need to have used it). Two attributes are added to every object.

| Attribute | What it is |
|---|---|
| `th_mode` | The most common value of the layer among the object's cells (the smallest on a tie). For vectors: the **number of the feature**, counted from 1 in the file's order, or, with `field`, the value of that attribute. |
| `th_share` | The share of **all** the object's cells that have `th_mode`: 1 when the object is wholly in one zone. A cell in no feature counts as *none*. When *none* is the most common value, both `th_mode` and `th_share` are missing. |

```tfgraph
#! caption: The attributes of every object (the bands' means, the area) and, from the blocks, th_mode and th_share.
#! highlight: feat
#! numbered: true
#! fragment: true
[[nodes]]
id = "feat"
op = "objects.features"
inputs = { objects = "seg.objects", image = "refl.image", thematic = "blocks.vector" }
params = { features = ["spectral", "area"] }
note = "The attributes of part 4, plus th_mode and th_share."
```

```tftable
data: /data/tutorials/object-based-image-analysis/classes-by-parcel/features_head.csv
labels: { id: "Object", cells: "Cells", th_mode: "th_mode (block)", th_share: "th_share" }
digits: 0
caption: "The first objects inside a block, from the table of the features node: the block each lies in and the share of its cells in it."
```

!!! note "th_share and the land between the parcels"
    A cell in no feature counts as *none*, so an object half in a block and half in the street has `th_share` = 0.5. What
    `th_mode` says depends on which is the larger part: an object mostly in a block has that block as `th_mode` and a
    `th_share` below 1, and an object mostly outside has **no** `th_mode` (and no `th_share`), even if some of its cells
    are in a block.

    On the clip, without the bounds, 425 objects cross a zone. For **285** of them `th_share` is below 1; the other **140**
    are mostly outside a block, so their `th_mode` is missing (and they are among the 2,080 objects with no value). Together
    they are the 425, which `th_share` and the missing values find without the zone map. With the bounds, `th_share`
    is 1 for all 1,004 objects that have a value, and the 2,103 others have none. The operator's reference gives the full
    rule: [[objects.features]].

```tftable
data: /data/tutorials/object-based-image-analysis/classes-by-parcel/th_share.csv
labels: { objects: "Segmentation", total: "Objects", no_block: "th_mode missing (mostly outside)", th_share_below_1: "th_share below 1", th_share_1: "th_share = 1", cross_a_zone: "Cross any zone", cross_no_value: "of them, th_mode missing" }
digits: 0
caption: "The multiresolution objects of scale 2, without and with the bounds. The count of objects that cross any zone is the page's own (from the zone map); `th_share` below 1 and a missing `th_mode` together account for all of them."
```

With `field`, the value is read from an attribute. A **text** attribute becomes codes 1, 2, 3 in the order the texts first
appear (a number attribute is read as it is), and the table shows the names. The `faces` of the blocks:

```tfgraph
#! caption: The same objects, with the kind of street each block faces as the thematic value.
#! highlight: feat_faces
#! numbered: true
#! fragment: true
[[nodes]]
id = "feat_faces"
op = "objects.features"
inputs = { objects = "seg.objects", image = "refl.image", thematic = "blocks.vector" }
params = { features = ["area"], field = "faces" }
note = "th_mode is now the text: primary, secondary, tertiary or residential (empty outside the blocks)."
```

### Step 4: rules that read the parcel

The first pass of rules is part 6's: water, forest, built-up and cerrado, by the NDWI, the NDVI and the visible brightness.
A **second pass** reads the classes of the first and the parcel. Inside a block, a forest object is trees along the streets and
in the gardens, and a cerrado object is dry or bare ground (a vacant plot, a dry lawn, a dirt road). Outside, the same
spectra are the forest and the cerrado of the landscape. The rules say so:

```tfgraph
#! caption: Two passes of rules (nodes 2 and 3 are part 6's tree and a second pass that reads the first one's classes and th_mode), and the attributes the first reads.
#! highlight: rules2
#! numbered: true
#! fragment: true
[[nodes]]
id = "calc"
op = "objects.calc"
inputs = { objects = "feat.objects" }
params = { expression = "out vis = (mean_red + mean_green + mean_blue) / 3" }
note = "The brightness in visible light."

[[nodes]]
id = "rules1"
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
note = "Water, forest, built-up and cerrado, by the spectra."

[[nodes]]
id = "rules2"
op = "objects.rules"
inputs = { objects = "rules1.objects" }
params = { classes = [
  { name = "water", rule = 'cls == "water"', colour = "#2b6cb0" },
  { name = "built_up", rule = 'cls == "built_up"', colour = "#c0504d" },
  { name = "forest", rule = 'cls == "forest" and coalesce(th_mode, 0) == 0', colour = "#2f7d3b" },
  { name = "street_trees", rule = 'cls == "forest" and coalesce(th_mode, 0) > 0', colour = "#8fcf6f" },
  { name = "cerrado", rule = 'cls == "cerrado" and coalesce(th_mode, 0) == 0', colour = "#d9b86c" },
  { name = "vacant", rule = 'cls == "cerrado" and coalesce(th_mode, 0) > 0', colour = "#e8a0bf" },
] }
note = "The first pass's classes, with forest and cerrado split by whether the object lies in a block."
why = "The spectra tell forest from cerrado; the parcel tells what each is in the city."
```

Two things in the second pass are worth reading slowly.

- **`cls`** is the class the object had *before* this node: that is why a second [[objects.rules]] node is needed (a rule
  that reads classes reads those of the nodes before it).
- **`coalesce(th_mode, 0)`**: an object outside every block has no `th_mode`, and a comparison with a missing value is missing
  (a rule reads missing as 0), so `== 0` and `> 0` would both give 0 for it. `coalesce(x, 0)` gives 0 where `x` is missing,
  and the two comparisons become complementary. The rules of missing values are in the reference of [[objects.calc]].

![The classes](/img/tutorials/object-based-image-analysis/classes-by-parcel/classes.png "Classes of the clip after the second pass: water (blue), built-up (red), forest (dark green), trees inside a block (light green), cerrado (tan), dry or bare ground inside a block (pink). The thin black lines are the blocks.")

![The classes, zoomed](/img/tutorials/object-based-image-analysis/classes-by-parcel/classes_zoom.png "The four blocks by the lake, classed. The cerrado outside a block is tan, the dry ground inside one is pink: the same spectra, the parcel decides the name.")

### Step 5: the area of each class in each parcel

The table of the features has `th_mode` and the area of every object; the table of the rules has its class. [[table.join]] puts
them together by the object's `id`, and [[table.aggregate]] sums the area by parcel and class.

```tfgraph
#! caption: The two tables joined by object, and the area of each class summed in each block (nodes 1 and 2), and, with field = "faces", in each kind of street (3 and 4).
#! highlight: by_block, by_faces
#! numbered: true
#! fragment: true
[[nodes]]
id = "joined"
op = "table.join"
inputs = { left = "calc.table", right = "rules2.table" }
params = { on = "id" }
note = "A row per object: its area, th_mode (the block) and its class."

[[nodes]]
id = "by_block"
op = "table.aggregate"
inputs = { table = "joined.table" }
params = { by = ["th_mode", "class"], columns = ["area"], stats = ["sum"] }
note = "A row per block and class: the area in square metres."

[[nodes]]
id = "joined_faces"
op = "table.join"
inputs = { left = "feat_faces.table", right = "rules2.table" }
params = { on = "id" }

[[nodes]]
id = "by_faces"
op = "table.aggregate"
inputs = { table = "joined_faces.table" }
params = { by = ["th_mode", "class"], columns = ["area"], stats = ["sum"] }
note = "The same by the kind of street the block faces."
```

The whole model, with the clip, the blocks and everything above:

```tfgraph
#! caption: The whole model: the clip, the blocks, a segmentation held inside them, the attributes with the parcel, two passes of rules and the area of each class by block and by kind of street.
#! highlight: blocks, seg, feat, rules2
#! numbered: true
#! params: false
format = "terraflux/1"

[[nodes]]
id = "s2"
op = "data.stac"
params = { url = "https://earth-search.aws.element84.com/v1", collection = "sentinel-2-l2a", assets = ["blue", "green", "red", "nir", "swir16"], datetime = "2024-08-09", extent = [190520, 8241280, 196320, 8247080], cellsize = 10, crs = "EPSG:31983" }
note = "Five bands, one scene."

[[nodes]]
id = "refl"
op = "raster.calc"
inputs = { s = "s2.raster" }
params = { expression = "out image = s / 10000" }
note = "Reflectance 0 to 1."

[[nodes]]
id = "blocks"
op = "vector.read"
params = { path = "blocks.geojson" }
note = "The parcels: 33 blocks made from the streets of OpenStreetMap."

[[nodes]]
id = "seg"
op = "segment.multiresolution"
inputs = { image = "refl.image", bounds = "blocks.vector" }
params = { scale = 2 }
note = "3,107 objects, none across a block's edge."

[[nodes]]
id = "feat"
op = "objects.features"
inputs = { objects = "seg.objects", image = "refl.image", thematic = "blocks.vector" }
params = { features = ["spectral", "area"] }
note = "The attributes, with th_mode and th_share."

[[nodes]]
id = "calc"
op = "objects.calc"
inputs = { objects = "feat.objects" }
params = { expression = "out vis = (mean_red + mean_green + mean_blue) / 3" }

[[nodes]]
id = "rules1"
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
note = "The classes by the spectra."

[[nodes]]
id = "rules2"
op = "objects.rules"
inputs = { objects = "rules1.objects" }
params = { classes = [
  { name = "water", rule = 'cls == "water"', colour = "#2b6cb0" },
  { name = "built_up", rule = 'cls == "built_up"', colour = "#c0504d" },
  { name = "forest", rule = 'cls == "forest" and coalesce(th_mode, 0) == 0', colour = "#2f7d3b" },
  { name = "street_trees", rule = 'cls == "forest" and coalesce(th_mode, 0) > 0', colour = "#8fcf6f" },
  { name = "cerrado", rule = 'cls == "cerrado" and coalesce(th_mode, 0) == 0', colour = "#d9b86c" },
  { name = "vacant", rule = 'cls == "cerrado" and coalesce(th_mode, 0) > 0', colour = "#e8a0bf" },
] }
note = "Forest and cerrado split by the parcel."

[[nodes]]
id = "joined"
op = "table.join"
inputs = { left = "calc.table", right = "rules2.table" }
params = { on = "id" }

[[nodes]]
id = "by_block"
op = "table.aggregate"
inputs = { table = "joined.table" }
params = { by = ["th_mode", "class"], columns = ["area"], stats = ["sum"] }
note = "The area of each class in each block."

[[nodes]]
id = "feat_faces"
op = "objects.features"
inputs = { objects = "seg.objects", image = "refl.image", thematic = "blocks.vector" }
params = { features = ["area"], field = "faces" }

[[nodes]]
id = "joined_faces"
op = "table.join"
inputs = { left = "feat_faces.table", right = "rules2.table" }
params = { on = "id" }

[[nodes]]
id = "by_faces"
op = "table.aggregate"
inputs = { table = "joined_faces.table" }
params = { by = ["th_mode", "class"], columns = ["area"], stats = ["sum"] }
note = "The area of each class by the kind of street the block faces."
```

## Read the results

Inside the 33 blocks (846 ha) the second pass gives ***571.5 ha of built-up (67.6%)**, **92.7 ha of trees (11.0%)**, **175.8 ha
of dry or bare ground (20.8%)** and 6.0 ha of lake (the corner block, see above). The rules gave the *same* built-up and the
same spectra as the first pass: the parcel only renamed the forest and the cerrado.

![Trees by block](/img/tutorials/object-based-image-analysis/classes-by-parcel/green_by_block.png "The share of each block that is trees (the light green class), over the clip faded out. Most blocks hold 0 to 10%; four have more than half, and three of them are under 2 ha.")

The table of the model's `by_block` node is a row per block and class; the page's script turns it into the map above. The
same sums by the kind of street are the `by_faces` node:

```tftable
data: /data/tutorials/object-based-image-analysis/classes-by-parcel/faces_classes.csv
columns: [faces, blocks, area_ha, built_up, street_trees, vacant, water]
labels: { faces: "The block faces", blocks: "Blocks", area_ha: "Area (ha)", built_up: "Built-up (%)", street_trees: "Trees (%)", vacant: "Dry or bare (%)", water: "Water (%)" }
digits: { blocks: 0, area_ha: 1, built_up: 1, street_trees: 1, vacant: 1, water: 1 }
caption: "The area of each class in the blocks, by the kind of street the blocks face (field = faces), as a share of the blocks' area. Thirty-three blocks and a 10 m pixel: the differences are a few points, and an illustration of the method, not a finding about Brasília."
```

How to report it:

- **The unit.** Say what the parcels are, where they come from (the file, the date, the licence), how many and how much
  of the study area they cover. A reader needs to know that 75% of this clip is outside the blocks.
- **That the segmentation was bounded**, and by which layer. A result by parcel is partly a result of the parcels.
- **The classes as the rules gave them.** Report the second pass's rules (the table in the page's model) and, as in
  part 5, an accuracy against reference samples if the numbers are to be used.
- **The size of the units.** A 1-hectare block is 100 cells of 10 m; its class shares move by a point with a cell.

## Settings that change the answer

### The scale

The scale decides how big the objects are, and so how many of them cross a boundary that nobody told the segmenter about.
The slider shows the objects of a segmentation **without** the bounds, at six scales, the ones that cross a block tinted
cyan.

```tfslider
label: Scale of the multiresolution segmentation
caption: "Without the bounds, the larger the objects, the more of them cross a block. Cyan: objects with cells in more than one zone (a block, another block, or the land between). With the bounds, none ever does. The chart: the share of the objects, and of the cells, in objects that cross."
frames:
  - { value: 1, src: /img/tutorials/object-based-image-analysis/classes-by-parcel/scale_1.png, caption: "Scale 1: 11,303 objects", stats: { Objects: "11,303", "Cross a zone": "789 (7.0%)", "With bounds": "11,751" } }
  - { value: 2, src: /img/tutorials/object-based-image-analysis/classes-by-parcel/scale_2.png, caption: "Scale 2 (the page's): 2,973 objects", stats: { Objects: "2,973", "Cross a zone": "425 (14.3%)", "With bounds": "3,107" } }
  - { value: 3, src: /img/tutorials/object-based-image-analysis/classes-by-parcel/scale_3.png, caption: "Scale 3: 1,338 objects", stats: { Objects: "1,338", "Cross a zone": "277 (20.7%)", "With bounds": "1,386" } }
  - { value: 4, src: /img/tutorials/object-based-image-analysis/classes-by-parcel/scale_4.png, caption: "Scale 4: 725 objects", stats: { Objects: "725", "Cross a zone": "209 (28.8%)", "With bounds": "760" } }
  - { value: 6, src: /img/tutorials/object-based-image-analysis/classes-by-parcel/scale_6.png, caption: "Scale 6: 308 objects", stats: { Objects: "308", "Cross a zone": "99 (32.1%)", "With bounds": "331" } }
  - { value: 8, src: /img/tutorials/object-based-image-analysis/classes-by-parcel/scale_8.png, caption: "Scale 8: 175 objects", stats: { Objects: "175", "Cross a zone": "67 (38.3%)", "With bounds": "217" } }
chart:
  type: line
  data: /data/tutorials/object-based-image-analysis/classes-by-parcel/scales.csv
  x: scale
  y: [cross_free_pct, cross_cells_pct]
  labels: { cross_free_pct: "Objects that cross (%)", cross_cells_pct: "Cells in them (%)" }
  colors: { cross_free_pct: "#3b8a5a", cross_cells_pct: "#e8901f" }
  xlabel: Scale
  ylabel: "%"
```

At the finest scale 7% of the objects cross a zone; at scale 8, 38%, and they hold 43% of the clip's cells. The bounds cost objects: at
scales 6 and 8 the bounded segmentation has 7% and 24% more objects than the free one, since an object that wanted to
cross is two. (At every scale it has more: 4 to 5% more up to scale 4.)

### The segmenter

How much the bounds cost depends on whether the segmenter keeps to them **while it works** or is **cut afterwards**.

```tftable
data: /data/tutorials/object-based-image-analysis/classes-by-parcel/segmenters.csv
labels: { segmenter: "Segmenter", setting: "Setting", bounds_work: "Bounds", objects_free: "Objects, no bounds", cross_free: "Cross a zone", objects_bound: "Objects, with bounds", cross_bound: "Cross a zone (bounds)", tiny_free: "3 cells or fewer, no bounds", tiny_bound: "3 cells or fewer, with bounds" }
digits: 0
caption: "Four segmenters of the series with and without bounds = the blocks. The multiresolution and Felzenszwalb never join across a zone; SLIC and the watershed are cut where they cross, and the cut leaves small pieces."
```

[[segment.multiresolution]] adds 72 objects (27 of three cells or fewer) and [[segment.felzenszwalb]] (k = 2, which makes 1,645 objects here) adds 126, 27 of three cells or fewer. SLIC, cut afterwards, goes from 2,245 to 2,880 objects,
and **256 of the new ones are three cells or fewer**: the cut of a superpixel along a street leaves a sliver. The
watershed goes from 591 to 764 objects with 73 slivers. After a cut, run [[objects.remove_small]] ([part 8](/tutorials/object-based-image-analysis/levels/))
to give the slivers back to a neighbour.

```tfcompare
caption: "SLIC (size 12) over four blocks, cut afterwards. Left, without bounds: the superpixels that cross a block are tinted cyan. Right, with bounds: the cut leaves small pieces along the edges."
before: { src: /img/tutorials/object-based-image-analysis/classes-by-parcel/slic_free.png, label: "SLIC, no bounds: 301 cross" }
after: { src: /img/tutorials/object-based-image-analysis/classes-by-parcel/slic_bound.png, label: "SLIC, bounds: 256 slivers" }
```

### The other settings

- **The layer**, not a setting but the biggest choice: a layer that is better drawn than the image makes better objects, and a
  worse one makes worse ones.
- **`bounds` as objects.** The zones can be the objects of another segmentation: each object a zone. That is how a finer level
  is made to stay inside the coarser one, and with `from`, the objects the segmentation starts from are cut by the zones
  first ([part 8](/tutorials/object-based-image-analysis/levels/)).
- **Large images.** `bounds` works with the tiles of the multiresolution and the cluster
  ([part 12](/tutorials/object-based-image-analysis/large-images/)): the zones are cut by window like the image.

## Common pitfalls

- **An object mostly outside the parcels has no `th_mode`.** Even if part of it is in a block. A rule that compares it gets a
  missing value: use `coalesce(th_mode, 0)` first, as the second pass does, and count the objects with a `th_share` below 1
  *and* those with no `th_mode` when you look for objects that cross a boundary.
- **A wrong layer makes wrong objects.** Check the layer over the image before trusting what it bounds (the figure of the
  blocks above shows the corner block holding part of the lake). A line off by two pixels cuts a roof in two.
- **A zone smaller than a cell.** A parcel narrower than the cell has no cell centre in it, and the burning of vectors uses
  the cell's centre: it has no cells, so it makes no object. Check `th_mode` for the parcels you expect.
- **Overlapping features.** Where two polygons overlap the later one wins (the cells have one zone). Dissolve or order them first.
- **Coordinate systems.** A vector is reprojected to the image's when both are known and differ; a file with no coordinate
  system is taken as the image's. Look at it over the image.
- **`field` and text.** With a text `field` the codes are 1, 2... in the order the texts first appear, and the table shows the names;
  with `field` unset the code is the feature's number in the file. Sorting the file changes the numbers.
- **Cut-afterwards segmenters and slivers.** Plan for [[objects.remove_small]].

## Going further

- Where the segmentation scale comes from and how to read it: [Choosing the scale](/tutorials/object-based-image-analysis/choosing-the-scale/) (part 3).
- Class trees, the second pass of rules and `cls`: [Rules and class trees](/tutorials/object-based-image-analysis/rules-and-class-trees/) (part 6).
- Blocks made from objects (levels, `from`, `bounds` as objects): [Levels](/tutorials/object-based-image-analysis/levels/) (part 8).
- Smooth the outlines that the parcels leave: [Shaping objects](/tutorials/object-based-image-analysis/shaping-objects/) (part 17).
- The eCognition thematic layer, in the table of algorithms: [Coming from eCognition](/tutorials/object-based-image-analysis/from-ecognition/).
- The operators: [[segment.multiresolution]], [[objects.features]], [[vector.read]], [[table.aggregate]].

## References

- Baatz, M., & Schäpe, A. (2000). Multiresolution segmentation: an optimization approach for high quality multi-scale
  image segmentation. In *Angewandte Geographische Informationsverarbeitung XII* (pp. 12-23). Wichmann.
- Blaschke, T. (2010). Object based image analysis for remote sensing. *ISPRS Journal of Photogrammetry and Remote
  Sensing*, 65(1), 2-16. https://doi.org/10.1016/j.isprsjprs.2009.06.004
- De Wit, A. J. W., & Clevers, J. G. P. W. (2004). Efficiency and accuracy of per-field classification for operational crop
  mapping. *International Journal of Remote Sensing*, 25(20), 4091-4112. https://doi.org/10.1080/01431160310001619580
- Haklay, M., & Weber, P. (2008). OpenStreetMap: user-generated street maps. *IEEE Pervasive Computing*, 7(4), 12-18.
  https://doi.org/10.1109/MPRV.2008.80
- Janssen, L. L. F., & Molenaar, M. (1995). Terrain objects, their dynamics and their monitoring by the integration of GIS
  and remote sensing. *IEEE Transactions on Geoscience and Remote Sensing*, 33(3), 749-758.
  https://doi.org/10.1109/36.387590
