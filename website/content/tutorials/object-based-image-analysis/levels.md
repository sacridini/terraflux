---
title: "Levels: from fine objects to blocks and landscape units"
short: Levels
description: "Build several levels of objects on one image: merge fine objects into patches, segment blocks from them, classify the blocks by what they hold, hand what the blocks know back to the fine objects, and tidy the result with merge, grow and remove_small."
order: 80
level: advanced
time: 75 min
ops: [segment.multiresolution, objects.merge, objects.calc, objects.rules, objects.remove_small, objects.grow, objects.reclassify, objects.features]
series: Object-based image analysis
part: 8
---

So far the series gave every cell of the image one object, at one scale. An ecologist rarely looks at a landscape at one
scale. A roof, a house plot, a block of the town and the whole urban area are four **levels** of the same place, and what
is true at one level (this plot has a garden) changes what you say at another (this block is leafy). In this tutorial you
will build levels on the Sentinel-2 clip of Lago Paranoá: fine objects, patches made by merging them, blocks made by
segmenting from them, and landscape units made by classifying the blocks by what is inside. You will then send what a block
knows **down** to its fine objects, tidy the result, and read the areas and the patches you can report. At the end you will
have a map of fine objects, a map of landscape units, the sizes of the patches and a clear idea of how much each
choice of level changes the numbers.

## The idea

A **level** is a complete set of objects over the image: every cell is in one object of each level. Levels are related.
An object of a level has **children**, the objects of the level below that lie inside it, and a **parent**, the object of the
level above it lies in. The levels form a **hierarchy**. The idea, in object-based image analysis, came with eCognition: its
image object levels, with *sub-objects* and *super-objects*, let a rule at one level ask what lies below or above it
(Baatz and Schäpe, 2000; Benz et al., 2004). It matches an old idea in ecology, that nature is arranged in nested patches,
where each level has its own processes (Wu and Loucks, 1995), and the use of a segmentation at several scales to link a
fine object (a tree crown) to the stand and the landscape it belongs to (Burnett and Blaschke, 2003).

![The four levels](/img/tutorials/object-based-image-analysis/levels/levels.png "The four levels of this tutorial over the same detail (the yellow box of the clip, in the figure below it): the fine objects, the patches made by merging them by class, the blocks segmented from the fine objects, and the blocks classed as landscape units (blue: water, green: forest, red: urban, tan: cerrado, magenta: not classified). The number of objects of each level is in its title.")

![Where the detail is](/img/tutorials/object-based-image-analysis/levels/detail_box.png "The detail of the figure above, and of the figure of Step 6, is the yellow box on the clip.")

Terraflux makes a level above from the level below in two ways:

- **By merging** ([[objects.merge]]): neighbouring objects that join, by class or by a condition on the pair, make one object.
  The level above is then exactly the union of whole objects of the level below, and nothing is re-cut. With
  `level = true` the node also gives the level below with its **parent** set: the `below` output.
- **By segmenting from** a level: [[segment.multiresolution]] takes the objects of a level in its `from` input and merges them
  by colour and shape, as it merges cells. The result is a coarser level of the same image, with the `from` objects as its
  children, again in a `below` output.

A level is never made by drawing a second, independent segmentation over the first: two such segmentations cross each
other's borders, and an object would have two parents. A level made from the one below cannot.

!!! science "Key idea"
    **What a level can ask.** With a level above and a level below, a rule can read three kinds of things. About the
    **parent**: `parent.mean_red`, `parent.cells`, any attribute the parent has *when the level was made*. About the
    **children**: `children.count(cond)`, `children.area(cond)` (the share of the object's cells in children where `cond`
    holds) and `children.mean(x)`. And about the **neighbours** at the same level: `nb.mean(x)`, `border_to("water")`. The
    objects of a level above only join objects of the **same parent**, so a merge or a clean-up at the fine level never
    crosses a block's border.

## When to use it

Levels answer questions of **context and composition**:

- What is this block made of? (a share of built-up, of forest, of open ground: **urban fabric**, **forest stand**, **farmland
  with trees**, a *landscape unit*);
- Is this small object part of something larger? (a garden in a block of houses, a farm building in grassland, a pond in a
  wood);
- What size is the patch an object belongs to? (landscape ecology's patch area, edge and connectivity metrics need patches,
  not fine objects);
- A classification that is right at the fine scale but noisy: join the objects of a class into patches, drop the small
  ones, and report patches.

They are **not** needed when one scale suits all your classes (a lake and a field are objects at one scale), when you
only want a map of pixels, or when a finer object can say everything the coarser one would. A level above is a *choice*
of zoning, and the results depend on it (see "Settings that change the answer"). Say which one you used.

## What you need

- **The image and the fine objects.** The Sentinel-2 clip of the series (blue, green, red, near infrared, `swir16`; UTM 23S, 10 m;
  [[data.stac]] as in [part 2](/tutorials/object-based-image-analysis/what-is-an-object/), or the download
  [sentinel2_brasilia.tif](/data/tutorials/object-based-image-analysis/sentinel2_brasilia.tif), 2.3 MB, read with
  [[raster.read]]; attribution in [ATTRIBUTION.txt](/data/tutorials/object-based-image-analysis/ATTRIBUTION.txt)), and its
  2,973 objects of scale 2, classified by the class tree of
  [part 6](/tutorials/object-based-image-analysis/rules-and-class-trees/). The first seven nodes of the model below are those;
  read part 6 for what they do.
- **Attributes** the rules read ([part 4](/tutorials/object-based-image-analysis/describing-objects/)): the means, the
  NDVI and NDWI from [[objects.features]], and the visible brightness `vis` from [[objects.calc]].
- **A way to check.** Levels give areas and patch sizes that **depend on the scale**; a reference map or a set of samples
  ([part 5](/tutorials/object-based-image-analysis/classifying-by-samples/)) tells whether a level is right for your
  question.

## Build the model

### Step 1: the fine level

The first seven nodes (`s2` to `rules`) are part 6's: the scene, reflectance, multiresolution objects of `scale = 2` (2,973),
their spectral attributes, the visible brightness and the class tree (water, forest, built-up, cerrado). They are in the model
at the end of this section. Every later node reads `rules.objects`, the fine objects with their classes.

### Step 2: patches, by merging neighbours

A **patch** in landscape ecology is a connected area of one class. The fine objects of a class that touch are one patch cut
into pieces. [[objects.merge]] puts them together. Add it, connect `rules.objects` to `objects`, and choose `by`:

- `by = "class"` (the default): neighbours of the **same class** become one object. `classes = ["forest", "built_up"]`
  limits it to those classes. Objects with no class never merge.
- `by = "condition"`: neighbours for which a condition on the **pair** is true merge, and so do the chains of such pairs. The
  condition is an [[objects.calc]] expression where `a.x` and `b.x` are an attribute of the two objects and `border` is the
  length of the border they share: `abs(a.vis - b.vis) < 0.01 and abs(a.ndvi - b.ndvi) < 0.05` merges the pairs that look
  alike in the visible brightness and the NDVI, whatever their class.

By class, the 2,973 fine objects become **323 patches**. By that condition they become 1,502: merging by look keeps more
objects, because two neighbours of the same class can still differ by more than the limit.

A merged object has its `cells` and `area` added up and every other numeric attribute as the **mean of its parts weighted by
their cells**; its class is that of its largest part. The band means come out exact that way, but a standard deviation, a
median, an NDVI made from the means, a shape or a texture does not: run [[objects.features]] again on the merged objects when
you need them.

The node's `count` output is the number of objects and `done` is true when nothing merged. To merge until nothing changes (a
condition merge in a chain) put it in a `control.repeat` with `until` set to its `done`.

```tfgraph
#! caption: Patches, by merging the fine objects of a class (node 1) and, with level = true, the same patches as a level above with the fine objects as their children (node 2).
#! highlight: patches, lvl
#! numbered: true
#! fragment: true
[[nodes]]
id = "patches"
op = "objects.merge"
inputs = { objects = "rules.objects" }
params = { by = "class" }
note = "2,973 objects become 323 patches."

[[nodes]]
id = "lvl"
op = "objects.merge"
inputs = { objects = "rules.objects" }
params = { by = "class", level = true }
note = "The same patches, and `below`: the fine objects inside them."
```

### Step 3: the parent, from the children

With `level = true` the patches become a **level above** and the node's second output, `below`, is the fine objects with
their parent set. An [[objects.calc]] on `below` can now read `parent.x`. The patch's size, for each fine object:

```text
out patch_cells = parent.cells
out patch_ha = parent.area / 10000
```

Of the 2,973 fine objects, 2,405 are in a patch larger than 10 ha, 154 in a patch of less than 1 ha, and 179 are the only
object of their patch: isolated in their class. A fine object in a small patch of a class is a candidate for a clean-up (Step
7), one in a big patch is part of something solid.

### Step 4: blocks, by segmenting from the fine objects

A patch is made of one class. A **block** is made of whatever is together, and is a segmentation, not a merge: add
[[segment.multiresolution]] with the image `refl.image` and the classified fine objects (`rules.objects`) in the **`from`**
input, and a `scale`. The scale is in the units of the image, as in [part 3](/tutorials/object-based-image-analysis/choosing-the-scale/),
but what it merges are now the fine objects, not the cells, so every block is a union of whole fine objects: at scale 6 the
2,973 fine objects become **312 blocks**. The outputs are `objects` (the blocks), `below` (the fine objects with the block as
parent) and the usual table of `mean_<band>` and `sd_<band>`. `shape` and `compactness` work as before.

!!! note
    `from` takes a level of the **same grid**. Cells that are in none of its objects are in none of the new ones. A
    `bounds` input (a map, polygons or objects) additionally forbids a block to cross the zones: blocks that do not cross
    a road or a property line, for instance.

### Step 5: classify the level above by what is inside

A block has no colour of its own that says "urban fabric": it has a **composition**. A block of town is a block in which
many of the children are buildings. [[objects.rules]] on the blocks, with the `below` input connected to the block's
`below`, can say that with `children.area`: the share of the block's cells that are in children where a condition holds.

```tfgraph
#! caption: The blocks of scale 6 (node 1), classed by what they hold (node 2): urban where many children are built-up, forest, water or cerrado where most are.
#! highlight: blocks, units
#! numbered: true
#! fragment: true
[[nodes]]
id = "blocks"
op = "segment.multiresolution"
inputs = { image = "refl.image", from = "rules.objects" }
params = { scale = 6 }
note = "312 blocks from the 2,973 fine objects."

[[nodes]]
id = "units"
op = "objects.rules"
inputs = { objects = "blocks.objects", below = "blocks.below" }
params = { classes = [
  { name = "urban", rule = 'above(children.area(cls == "built_up"), 0.25, 0.5)', colour = "#c0504d" },
  { name = "forest", rule = 'above(children.area(cls == "forest"), 0.4, 0.7)', colour = "#2f7d3b" },
  { name = "water", rule = 'above(children.area(cls == "water"), 0.4, 0.7)', colour = "#2b6cb0" },
  { name = "cerrado", rule = 'above(children.area(cls == "cerrado"), 0.4, 0.7)', colour = "#d9b86c" },
] }
note = "Landscape units by composition."
```

`cls == "built_up"` is the fine objects' class (1 if it holds, 0 if not), and `children.area(...)` is its mean over the block's
area. A block with a quarter of its area in building objects starts to be urban (membership 0) and one with a half is
(membership 1): lower limits than for the others, because a block of houses is never all roofs, with its streets, gardens and
trees. It is the same fuzzy machinery as in part 6: the memberships, the margin and `min` apply.
The `below` input is needed only for `children.*`; a rule that reads the objects and their parent does not need it.

![The landscape units](/img/tutorials/object-based-image-analysis/levels/units.png "The 312 blocks of scale 6 classed by what they hold. Urban (red) covers the town with the gardens and streets in it, forest (green) the gallery along the streams and the woodland to the south-east, water (blue) the lake, cerrado (tan) the grassland. One block at the shore is not classed (magenta): its children split between classes so that no rule reaches a membership of 0.1.")

The landscape units answer a **different question** from the fine map. The fine map says where buildings are: **1,010 ha** of
built-up, with the streets and gardens between them as cerrado. The landscape units say where the town is: **1,327 ha** of
urban, 36 % more, because a block of houses is urban as a whole. Neither is the right one: one is the area of roofs and
asphalt, the other of the urban fabric. They have 761 ha of forest (the fine map has 896), 1,041 ha of cerrado (1,241) and 226.2 ha of
water (216.8); one block, 9.3 ha, is not classed.

### Step 6: send the blocks' knowledge down to the fine objects

The cerrado-coloured fine objects inside the town (a tree-lined street, a yard) are what part 6's second pass called
`urban_open`. With levels, the **block** can decide. The fine objects (`blocks.below`) have the block as parent, so a rule on
them reads `parent.x`:

```text
cls == "cerrado" and (parent.mean_red + parent.mean_green + parent.mean_blue) / 3 >= 0.07
```

An object that is cerrado by its own colour, in a block whose visible brightness is 0.07 or more, becomes `urban_open`. The
0.07 is between the two kinds of block on this clip: the cerrado blocks have a visible brightness with a 95th percentile of
0.070, the urban blocks a 5th percentile of 0.074. The result is **33,052 cells (331 ha) in 369 objects**, against 27,993
cells (280 ha) in 304 objects for part 6's rule on the neighbours' border. Two rules with similar answers, neither a
measurement: only samples or a reference map say which one is right.

![The fine objects refined by their parent](/img/tutorials/object-based-image-analysis/levels/refine.png "A detail of the town, 1.4 km wide: the image (left), the fine objects by the class tree (middle) and refined by the block they lie in (right). The pale green objects are cerrado-coloured objects in a bright block: the streets, yards and lots inside the town.")

!!! warning
    **A parent is a snapshot.** `parent.x` reads the parent as it was **when the level was made**. The blocks of Step 4 were made
    before they were classed, so `parent.cls` is `"unclassified"` for every fine object, and the landscape unit is not there. What a
    block *had* then, its table (`mean_red`, `sd_nir`, `cells`...), is. To give the fine objects the block's **class**, write the
    class to a map with [[objects.to_map]] (`what = "class"`) and give the map to [[objects.features]] as `thematic`: each fine
    object gets `th_mode`, the class number of the map under most of its cells, and `th_share`. A rule `cls == "cerrado" and th_mode
    == 1` (urban is the first unit) gives **30,908 cells** of urban open ground, 2,144 fewer than with the brightness, and the two
    maps differ in 4,796 cells: the block's brightness and its class are not the same thing.

```tfgraph
#! caption: The fine objects of a block, refined by an attribute of the block (node 1), then with the class of the block as a thematic layer (nodes 2 to 4).
#! highlight: refine, units_map
#! numbered: true
#! fragment: true
[[nodes]]
id = "refine"
op = "objects.rules"
inputs = { objects = "blocks.below" }
params = { classes = [
  { name = "water", rule = 'cls == "water"', colour = "#2b6cb0" },
  { name = "forest", rule = 'cls == "forest"', colour = "#2f7d3b" },
  { name = "built_up", rule = 'cls == "built_up"', colour = "#c0504d" },
  { name = "cerrado", rule = 'cls == "cerrado" and (parent.mean_red + parent.mean_green + parent.mean_blue) / 3 < 0.07', colour = "#d9b86c" },
  { name = "urban_open", rule = 'cls == "cerrado" and (parent.mean_red + parent.mean_green + parent.mean_blue) / 3 >= 0.07', colour = "#a7d676" },
] }
note = "The block's mean brightness decides."

[[nodes]]
id = "units_map"
op = "objects.to_map"
inputs = { objects = "units.objects" }
params = { what = "class" }
note = "The landscape units as a map."

[[nodes]]
id = "feat2"
op = "objects.features"
inputs = { objects = "blocks.below", image = "refl.image", thematic = "units_map.map" }
params = { features = ["spectral"] }
note = "th_mode: the unit under each fine object."

[[nodes]]
id = "refine2"
op = "objects.rules"
inputs = { objects = "feat2.objects" }
params = { classes = [
  { name = "water", rule = 'cls == "water"' },
  { name = "forest", rule = 'cls == "forest"' },
  { name = "built_up", rule = 'cls == "built_up"' },
  { name = "cerrado", rule = 'cls == "cerrado" and th_mode != 1' },
  { name = "urban_open", rule = 'cls == "cerrado" and th_mode == 1' },
] }
```

### Step 7: tidy up

Three operations clean a classification, and all three merge neighbours and keep each object whole, so they stay inside a
parent when there is one.

**[[objects.remove_small]]** merges every object under `min_cells` (or `min_area`, in map units squared) into a neighbour. With
`into = "similar"` (the default) the neighbour is the one whose band means are nearest; with `into = "border"` it is the one with
the longest shared border. `classes` limits it to the small objects of some classes. It repeats until no small object is left.
The size you choose is a choice of **minimum mapping unit**, and the table shows its price.

```tftable
sortable: true
digits: { min_ha: 2, objects: 0, cells_changed: 0, percent_changed: 2 }
labels: { min_cells: "min_cells", min_ha: "Least area (ha)", objects: "Objects left", cells_changed: "Cells that changed class", percent_changed: "Share of the clip (%)" }
caption: "objects.remove_small on the 2,973 classified fine objects (into = similar), at five sizes: the objects left and how many cells end in another class."
data: /data/tutorials/object-based-image-analysis/levels/remove_small.csv
```

Removing the objects under 10 cells changes nothing on the map (3 objects join neighbours of their own class). At 50 cells
(0.5 ha) 501 objects are gone, 2,472 are left and **1.3 %** of the cells change class. At 200 cells (2 ha) 759 objects remain and
9.7 % of the clip has another class: the classes of narrow things go to their neighbours. `into = "border"` instead of
`similar` at 50 cells leaves 2,462 objects and changes 9,923 cells (2.9 %) where `similar` changes 4,280: the neighbour
chosen is not the same one, and the two maps differ in 8,510 cells (2.5 %).

**[[objects.grow]]** takes one class, `class`, and a `condition` that a *neighbour* must meet to be merged into it, an
[[objects.calc]] expression computed on the candidate; it repeats until nothing changes or `max_steps`. It is for the objects
the rules left unclassified. With `min = 0.6` in part 6's tree, 29,923 cells (278 objects) are left without a class. Growing
the forest into the small ones whose NDVI is above 0.4:

```toml
[[nodes]]
id = "grow"
op = "objects.grow"
inputs = { objects = "strict.objects" }
params = { class = "forest", condition = 'cls == "unclassified" and ndvi > 0.4 and cells < 200' }
```

takes 9,279 cells into the forest (81,056 to 90,335) and leaves 20,644 cells unclassified. In the detail below it shows at the foot
of the lake. A merged object keeps the class of the object that grew, so the class only gains cells. The `cells < 200` limit is a
guard, not a need: without it (`ndvi > 0.4` alone) the forest takes 16,232 cells (97,288 in all) and 13,691 stay unclassified,
but a large mixed object, up to thousands of cells, is then swallowed whole by the first forest object it touches. Keep the
candidates small, or check what the big ones are, before you let a class grow into them.

![Grow](/img/tutorials/object-based-image-analysis/levels/grow.png "A detail of the town with the rules at min = 0.6 (left: magenta objects have no class) and after growing the forest into the small unclassified objects with an NDVI above 0.4 (right).")

**[[objects.reclassify]]** renames and joins classes (`classes` maps old names to new ones); with `merge = true` the neighbours
that now share a class merge as in [[objects.merge]]. Making forest and cerrado one **vegetation** class and merging gives
**714 objects** from the refined fine objects: patches of vegetation, of built-up land, of urban open ground and of water.

```tfgraph
#! caption: The model of this tutorial: the fine level (nodes 1 to 7), the patches as a level with their children (8, 9), blocks made from the fine objects (10), the landscape units (11), the fine objects refined by their block (12), the small ones removed (13) and the map of the units (14).
#! highlight: lvl, kids, blocks, units, refine, small
#! numbered: true
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
id = "seg"
op = "segment.multiresolution"
inputs = { image = "refl.image" }
params = { scale = 2 }
note = "The fine level: 2,973 objects."

[[nodes]]
id = "feat"
op = "objects.features"
inputs = { objects = "seg.objects", image = "refl.image" }
params = { features = ["spectral"] }

[[nodes]]
id = "calc"
op = "objects.calc"
inputs = { objects = "feat.objects" }
params = { expression = "out vis = (mean_red + mean_green + mean_blue) / 3" }

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
note = "The class tree of part 6."

[[nodes]]
id = "lvl"
op = "objects.merge"
inputs = { objects = "rules.objects" }
params = { by = "class", level = true }
note = "Patches (301) as a level above; below: the fine objects."

[[nodes]]
id = "kids"
op = "objects.calc"
inputs = { objects = "lvl.below" }
params = { expression = "out patch_ha = parent.area / 10000" }
note = "The size of the patch each fine object is in."

[[nodes]]
id = "blocks"
op = "segment.multiresolution"
inputs = { image = "refl.image", from = "rules.objects" }
params = { scale = 6 }
note = "312 blocks segmented from the fine objects."

[[nodes]]
id = "units"
op = "objects.rules"
inputs = { objects = "blocks.objects", below = "blocks.below" }
params = { classes = [
  { name = "urban", rule = 'above(children.area(cls == "built_up"), 0.25, 0.5)', colour = "#c0504d" },
  { name = "forest", rule = 'above(children.area(cls == "forest"), 0.4, 0.7)', colour = "#2f7d3b" },
  { name = "water", rule = 'above(children.area(cls == "water"), 0.4, 0.7)', colour = "#2b6cb0" },
  { name = "cerrado", rule = 'above(children.area(cls == "cerrado"), 0.4, 0.7)', colour = "#d9b86c" },
] }
note = "Landscape units: blocks classed by what they hold."

[[nodes]]
id = "refine"
op = "objects.rules"
inputs = { objects = "blocks.below" }
params = { classes = [
  { name = "water", rule = 'cls == "water"', colour = "#2b6cb0" },
  { name = "forest", rule = 'cls == "forest"', colour = "#2f7d3b" },
  { name = "built_up", rule = 'cls == "built_up"', colour = "#c0504d" },
  { name = "cerrado", rule = 'cls == "cerrado" and (parent.mean_red + parent.mean_green + parent.mean_blue) / 3 < 0.07', colour = "#d9b86c" },
  { name = "urban_open", rule = 'cls == "cerrado" and (parent.mean_red + parent.mean_green + parent.mean_blue) / 3 >= 0.07', colour = "#a7d676" },
] }
note = "A cerrado-coloured object in a bright block is urban open ground."

[[nodes]]
id = "small"
op = "objects.remove_small"
inputs = { objects = "refine.objects" }
params = { min_cells = 50 }
note = "Objects under 0.5 ha join a neighbour."

[[nodes]]
id = "plot"
op = "view.plot"
inputs = { value = "units.map" }
params = { title = "Landscape units" }
```

=== "Application"

    Open [part 6's template](/tutorials/object-based-image-analysis/rules-and-class-trees/) (*Classify an image by rules*) or build
    its first seven nodes, then add the others (click an operator in the library to add it, **drag from an output port to an input
    port** to connect, set the values in the **Node pane**). Select `units` and press ++ctrl+enter++: the Viewer shows the
    landscape units. Click an object to select it, then press ++p++ to go to its **parent** (the level above is drawn) and ++c++ to
    its **children** (when the level shown has a level beneath it); ++n++ adds the neighbours. The inspector tells the object's
    attributes, its class, its membership in each class, and its parent and children.

=== "Command line"

    Save the graph above as `levels.toml` and run it. To write the table of the landscape units, add `[outputs.units]` with
    `from = "units.table"`.

    ```bash
    terraflux run levels.toml --threads 4 --out output
    ```

=== "Without a network"

    Download [sentinel2_brasilia.tif](/data/tutorials/object-based-image-analysis/sentinel2_brasilia.tif) beside the model
    and replace the node `s2` with a [[raster.read]] of it. The download is rounded to whole numbers, so the
    numbers of this page can differ in the last decimals.

    ```toml
    [[nodes]]
    id = "s2"
    op = "raster.read"
    params = { path = "sentinel2_brasilia.tif" }
    ```

## Read the results

What you report from levels is not a single map but a **set of areas and patches at a stated level**.

**The patches.** The `cells` and `area` of a merged object are exact sums, so each patch has its area; the table counts them by
class.

```tftable
align: { class: left }
sortable: true
digits: { patches: 0, hectares: 1, mean_ha: 2, median_ha: 2, largest_ha: 1, fine_objects: 0 }
labels: { class: Class, patches: Patches, hectares: Hectares, mean_ha: "Mean patch (ha)", median_ha: "Median patch (ha)", largest_ha: "Largest (ha)", fine_objects: "Fine objects" }
caption: "The 301 patches made by merging the fine objects by class: their number, area and size, and the number of fine objects they were made of."
data: /data/tutorials/object-based-image-analysis/levels/patches.csv
```

The numbers say what a landscape ecologist wants to know. The 4 water patches are the lake (205.9 ha) and three small ones; the 44
built-up patches have a median of 1.3 ha but a mean of 23 ha, because the largest is 330 ha: **most patches are small and most
of the area is in a few large ones**, the usual shape of a patch-size distribution. Report the median and the largest beside
the mean. Note that the patches are the fine objects' classes put together: 712 fine objects of forest are 120 patches.

**The landscape units** are the answer to "what is the block made of": 1,327 ha urban, 761 ha forest, 226 ha water and
1,041 ha cerrado, with 9.3 ha unclassified, at the scale of 6. A report says the units' definition (the rules, the
thresholds, the scale), the level they are at, and that the area depends on them.

**To report it:** the image (sensor, scene, date), the scale of each level (2 for the fine objects, 6 for the blocks), the
rules at each level with their thresholds, the minimum mapping unit (the `min_cells` of the clean-up and what it changes), the
areas **and the patches** of the level you used, and a map of the level.

## Settings that change the answer

The level above is a zoning, and a zoning changes the numbers. Move the slider: it moves the **scale of the blocks** made from
the same fine objects, with the same rules for the units.

```tfslider
label: Scale of the blocks
caption: "The blocks (outlined) made from the fine objects at five scales, each classed by what it holds (red urban, green forest, blue water, tan cerrado, magenta not classified). The chart is the area of the units in the whole clip."
frames:
  - { value: 3, src: /img/tutorials/object-based-image-analysis/levels/units_3.png, caption: "Scale 3: 1,359 blocks", stats: { Blocks: "1,359", Urban: "1,133 ha", Forest: "846 ha", Cerrado: "1,166 ha" } }
  - { value: 4, src: /img/tutorials/object-based-image-analysis/levels/units_4.png, caption: "Scale 4: 723 blocks", stats: { Blocks: "723", Urban: "1,221 ha", Forest: "807 ha", Cerrado: "1,114 ha" } }
  - { value: 6, src: /img/tutorials/object-based-image-analysis/levels/units_6.png, caption: "Scale 6: 312 blocks (the page's)", stats: { Blocks: "312", Urban: "1,327 ha", Forest: "761 ha", Cerrado: "1,041 ha" } }
  - { value: 10, src: /img/tutorials/object-based-image-analysis/levels/units_10.png, caption: "Scale 10: 109 blocks", stats: { Blocks: "109", Urban: "1,511 ha", Forest: "689 ha", Cerrado: "904 ha" } }
  - { value: 16, src: /img/tutorials/object-based-image-analysis/levels/units_16.png, caption: "Scale 16: 37 blocks", stats: { Blocks: "37", Urban: "1,505 ha", Forest: "542 ha", Cerrado: "1,098 ha" } }
chart:
  type: line
  data: /data/tutorials/object-based-image-analysis/levels/scales.csv
  x: scale
  y: [urban_ha, forest_ha, cerrado_ha]
  labels: { urban_ha: "Urban", forest_ha: "Forest", cerrado_ha: "Cerrado" }
  colors: { urban_ha: "#c0504d", forest_ha: "#2f7d3b", cerrado_ha: "#d9b86c" }
  ylabel: "Hectares in the clip"
```

- **Water hardly moves**: 218 to 226 ha at every scale. The lake is a single contrast and any block over it is water.
- **The urban unit grows with the scale**, from 1,133 ha to 1,511 ha at the scale 10 (33 %), and the cerrado shrinks (1,166 to 904 ha); the scale 16 gives back some of it (1,505 and 1,098 ha). As blocks
  grow they take in the gardens, the lots and the grassland at the edge of the town, and a block of mixed content is classed by the
  larger share. The unit is where the **town** is, and how far the town goes depends on how big the cell you look with is.
- **Forest shrinks** (846 to 542 ha), most likely for the same reason: thin strips of gallery forest are absorbed by their
  neighbours and their share in a block falls under its limit.
- At 16 the clip is 34 blocks; the unit map is a coarse picture of the landscape, good for a region, not for a plot.

This is the **modifiable areal unit problem** (Jelinski and Wu, 1996): the area of a class you measure in zones depends on how
the zones are drawn and how large they are. There is no scale at which the answer is "right"; there is one that fits the question and
that you state. Choose the scale of the blocks from the question (a neighbourhood, a municipality), say it, and report the fine
map's numbers beside the units'.

## Common pitfalls

- **Merging unclassified objects.** [[objects.merge]] by class never joins objects without a class. If a map has
  many unclassified objects, patches of a class are cut by them. Grow or classify them first.
- **A parent is a snapshot.** `parent.x` is the parent as it was when the level was made (Step 6). A class given to the level
  above afterwards is not there: pass it as a map and `thematic`. If you need the class of the parent, make the level
  **after** you classify (merge by class gives a parent that has its class; segmenting from a level makes one that has none).
- **`children.*` without `below`.** `children.area(...)` needs the input `below`, the objects whose parent is the objects you
  classify, as the node that made the level gives them. Without it the rule is refused.
- **`below` of another level.** The `below` you give a node must be the level beneath **those** objects: the check
  compares them cell by cell and refuses a `below` from another segmentation.
- **Level made twice.** `level = true` on objects that already have a level above is refused: merge in the upper level,
  or take the objects without their parent.
- **Mean of means.** A merged object's attributes are means weighted by cells of its parts: good for a rule, not exact for a report.
  Run [[objects.features]] again on the merged objects before you publish numbers.
- **Chaining.** A condition merge joins a pair and then the pair's neighbours: a slow gradient (a shore) can chain objects that are far apart
  in tone. Keep the limit small against the gap between your classes, or merge by class.
- **Grow swallowing big objects.** `objects.grow` merges a whole candidate, however large. Limit the candidates (`cells < 200`) or look at the big ones first (Step 7).
- **The minimum mapping unit is a result.** `min_cells` of 200 changed 9.7 % of the clip. Say the number and what it removes.
- **Scale and units.** The scale of a segmentation from a level is in the image's units (here reflectance 0 to 1); the image in
  integers needs a scale thousands of times larger ([part 3](/tutorials/object-based-image-analysis/choosing-the-scale/)).

## Going further

- Before this: [Rules and class trees](/tutorials/object-based-image-analysis/rules-and-class-trees/), where the fine classes
  of this page come from, and [Choosing the scale](/tutorials/object-based-image-analysis/choosing-the-scale/), on the scale of a
  level.
- Samples instead of rules for any level: [Classifying by samples](/tutorials/object-based-image-analysis/classifying-by-samples/).
- Patch metrics from the polygons: [[objects.to_vector]] and [[vector.write]] give the patches as a file you can read in a
  landscape-metrics tool.
- The operators of this page: [[objects.merge]], [[objects.remove_small]], [[objects.grow]], [[objects.reclassify]],
  [[objects.calc]], [[objects.rules]], [[objects.features]], [[segment.multiresolution]].
- Coming from eCognition: [Coming from eCognition](/tutorials/object-based-image-analysis/from-ecognition/) puts its levels and its
  *merge region*, *grow region* and *remove objects* next to these operators.

## References

- Baatz, M., & Schäpe, A. (2000). Multiresolution segmentation: an optimization approach for high quality
  multi-scale image segmentation. In *Angewandte Geographische Informationsverarbeitung XII* (pp. 12-23). Wichmann.
- Benz, U. C., Hofmann, P., Willhauck, G., Lingenfelder, I., & Heynen, M. (2004). Multi-resolution, object-oriented fuzzy
  analysis of remote sensing data for GIS-ready information. *ISPRS Journal of Photogrammetry and Remote Sensing*, 58(3-4),
  239-258. https://doi.org/10.1016/j.isprsjprs.2003.10.002
- Burnett, C., & Blaschke, T. (2003). A multi-scale segmentation/object relationship modelling methodology for landscape
  analysis. *Ecological Modelling*, 168(3), 233-249. https://doi.org/10.1016/S0304-3800(03)00139-X
- Jelinski, D. E., & Wu, J. (1996). The modifiable areal unit problem and implications for landscape ecology. *Landscape
  Ecology*, 11(3), 129-140. https://doi.org/10.1007/BF02447512
- Wu, J., & Loucks, O. L. (1995). From balance of nature to hierarchical patch dynamics: a paradigm shift in ecology. *The
  Quarterly Review of Biology*, 70(4), 439-466. https://doi.org/10.1086/419172
