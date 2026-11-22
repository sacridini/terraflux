---
title: "Separate tree crowns: split, fuse and find the tops"
short: Tree crowns
description: "Cut the clumps of touching tree crowns into single crowns with a watershed (objects.split), join the pieces that were cut too fine (objects.fuse) and mark the top of each neighbourhood (objects.extrema), on real 0.6 m aerial imagery, and learn what h, sigma, max_neck and the fusion's compactness do."
order: 180
level: advanced
time: 60 min
template: Separate tree crowns
ops: [raster.read, raster.calc, raster.focal, segment.threshold, objects.rules, objects.split, objects.fuse, objects.features, objects.extrema]
series: Object-based image analysis
part: 18
---

Trees that grow close together have crowns that touch, and a threshold on the vegetation cannot tell where one crown ends:
it gives a clump. In this tutorial you cut the clumps of a Texas suburb into single crowns with a **watershed**, join back
the pieces that were cut too fine, and mark the top of every neighbourhood. You end with a layer of crowns, a count, an area
for each, and a set of tops, and you know which settings decide how fine the crowns come out and which of them to check first.

The page uses a real NAIP image at 0.6 m. NAIP has no height model, so the **surface** that the watershed floods is the
vegetation index NDVI, not a canopy height. That is a stand-in, and the page says what it costs. The same operators, run on a
canopy height model from lidar, are what the template **Separate tree crowns** does.

## The idea

A crown is a bump of canopy. Seen from above, a tree has a bright, high centre and falls off to the edges, and between two
trees there is a **saddle**: a low line, a gap in the foliage or a seam of shade. If you turn the picture upside down, each crown is a
basin with its own bottom and the saddles are the ridges between basins. Fill the basins with water from their bottoms and
stop each lake where it meets another: the shores are the lines between crowns. That is the **watershed transformation**,
which Vincent and Soille (1991) made fast with a queue that floods the cells in order of height, and it is the tool that image
software, eCognition's *watershed transformation* and CellProfiler's *declumping* (Carpenter et al., 2006) among them, uses to
split touching things: crowns, buildings, cells, mounds.

Two things can go wrong and both are real. Every small bump has a bottom, so a rough crown, with a sunlit leaf here and a
shadow there, floods into many pieces: **over-splitting**. And a crown whose neighbour is as tall and joined to it
by no saddle at all floods as one: **under-splitting**. The settings of [[objects.split]] choose between them.

!!! science "Key idea: a centre must stand h above its saddle"
    A bump counts as a crown centre only if it stands at least **h** above the saddle where it meets a higher bump
    (these are the *h-maxima* of the surface; the watershed of Terraflux's [[segment.watershed]] uses the same rule for its
    markers). In the drawing below, the first two bumps are centres; the third stands less than h above the saddle that joins
    it to the second, so it is a lump on the second crown and not a crown of its own. **h is in the units of the surface**: NDVI here, metres for a height model,
    map units of distance for the distance transform.

![A profile across the canopy: two centres and a bump](/img/tutorials/object-based-image-analysis/tree-crowns/profile.png "A schematic profile of the surface along a line (an explanatory drawing, not data). Centres are the bumps that stand h above the saddle that joins them to a higher one.")

The surface can also be the **distance to the object's border** (the default of [[objects.split]]): a round crown is a basin
of its own, two round crowns joined by a narrow neck are two basins and the neck is the saddle. That needs no image at
all, only the shape of the clump, and it is the better choice when the clumps are round and the image has no usable
brightness. Both ways are tried below.

After the split there are usually too many pieces, and one crown may be in two or three. [[objects.fuse]] joins a piece to a
neighbour only when the object they make is *better*, by a measure you write (here, how compact the union is). And
[[objects.extrema]] finds, among neighbouring objects, the one whose attribute is highest: the **top**. These three
operators are eCognition's *watershed transformation*, *image object fusion* and *find local extrema*.

## When to use it

- **Counting and sizing trees** from imagery: crowns per hectare, the distribution of crown areas, the share of the
  canopy in large crowns. Methods for individual tree crowns from passive imagery are reviewed by
  Ke and Quackenbush (2011); a classic one finds the tops as **local maxima** of the image after
  smoothing (Wulder et al., 2000), which is the idea behind [[objects.extrema]] and the h of [[objects.split]], here applied to objects.
- **Any touching round things**: buildings that were segmented as a block, cells, termite mounds, ponds that touch. The distance
  version works on a binary mask.
- Not for a **closed canopy of one species** where the crowns have no saddle between them: there is nothing to flood
  and the watershed finds the texture of the leaves, not the trees. A height model helps there; an image does not.
- Not a replacement for **lidar**. A canopy height model sees the tops; a colour image sees the sunlit sides. Terraflux has
  no point-cloud type, so a canopy height model (a surface minus the ground, or a lidar raster you made elsewhere) comes in as
  a raster.
- If you want to **segment the whole image** into small regions, not to divide objects you already have, use a segmenter
  ([[segment.watershed]], [[segment.slic]] and the others of [part 2](/tutorials/object-based-image-analysis/what-is-an-object/)).
  The case study [Urban cover of a neighbourhood from NAIP](/tutorials/object-based-image-analysis/case-urban-cover-naip/) uses the split
  on the trees of the same image as one step of a larger map.

## What you need

- The NAIP clip of the series: a 600 m window of Plano, Texas (0.6 m cells, bands red, green, blue and nir, 9 November 2024, USDA
  NAIP through the Planetary Computer). The [clip to download](/data/tutorials/object-based-image-analysis/naip_plano.tif) is
  the file the model reads, so the page runs without a network: that scene of 9 November 2024 is no longer in the Planetary Computer (its NAIP items over Plano now end in 2022). With a scene that is, [[data.stac]] reads NAIP straight onto a UTM grid with `bands` and `crs`, as in [part 14](/tutorials/object-based-image-analysis/case-urban-cover-naip/). Its attribution is in the
  [ATTRIBUTION.txt](/data/tutorials/object-based-image-analysis/ATTRIBUTION.txt) beside it.
- A surface. Here the NDVI, \( (nir - red) / (nir + red) \) (Tucker, 1979), made by the model. If you have a **canopy height model**
  (a normalised digital surface model, in metres), use it instead: replace the first three nodes by a [[raster.read]] of it,
  mask the canopy at a height (the template cuts at 3 m) and give the same raster to `surface`. The `h` is then in metres (the template uses 0.3 m on its synthetic one).
- The template **Separate tree crowns** (group *Objects (OBIA)* of the application's templates). It runs on a synthetic
  height model, from the clump to the tops in nine nodes, and **File > Use my data...** puts your raster in place of it.

![The window of the figures in the NAIP clip](/img/tutorials/object-based-image-analysis/tree-crowns/where.png "The NAIP clip (600 m wide, true colour). Most figures on this page show the yellow square: 90 m of a street with trees along it, and the houses between them.")

## Build the model

### Step 1: the surface and the canopy

![NAIP, the smoothed NDVI and the canopy](/img/tutorials/object-based-image-analysis/tree-crowns/surface.png "Left: the 90 m window in true colour. Middle: the smoothed NDVI, which the watershed floods. Right: the cells above 0.3, which are the canopy.")

Lawns and trees are both green in NDVI, but in this image the trees stand out: the lawn is mostly below 0.3 and the trees above. The
NDVI of a 0.6 m image is noisy (a leaf, a shadow), so a **Gaussian smooth** comes first: [[raster.focal]] with `shape = "gaussian"`,
`radius = 3` and `sigma = 1` cell. Without it the canopy mask would be speckled and each speck would be an object.

```tfgraph
#! caption: Read the clip, make its NDVI and smooth it.
#! highlight: ndvi, smooth
#! numbered: true
#! fragment: true
[[nodes]]
id = "naip"
op = "raster.read"
params = { path = "naip_plano.tif" }
note = "The NAIP clip: red, green, blue and nir, 0.6 m."

[[nodes]]
id = "ndvi"
op = "raster.calc"
inputs = { s = "naip.raster" }
params = { expression = "out ndvi = (s.nir - s.red) / (s.nir + s.red)" }
note = "NDVI of every cell (Tucker 1979)."

[[nodes]]
id = "smooth"
op = "raster.focal"
inputs = { map = "ndvi.ndvi" }
params = { stat = "mean", shape = "gaussian", radius = 3, sigma = 1 }
note = "A Gaussian smooth: the surface the watershed floods, and what the canopy is cut from."
```

Then the canopy: [[segment.threshold]] cuts the smoothed NDVI at 0.3 (`thresholds = [0.3]`) into two classes and makes an object of every
connected piece of each, with `neighbours = 8` so crowns that touch by a corner are one. The objects of the high class are
the **clumps**. [[objects.rules]] gives the classes names, `canopy` and `open`: the name matters, because the split and the fuse
take **class names** to say which objects they work on.

!!! note "Why 0.3"
    It is read off this image, not a rule: the lawn and the roads are below it and the trees above. It is a setting like the
    others: try 0.25 and 0.35 and look at the canopy. A different image, another date or another sensor needs its own.

```tfgraph
#! caption: The canopy: the cells above 0.3 become objects, and the objects of the high class are named canopy.
#! highlight: clumps, classes
#! numbered: true
#! fragment: true
[[nodes]]
id = "clumps"
op = "segment.threshold"
inputs = { image = "smooth.map" }
params = { thresholds = [0.3], neighbours = 8 }
note = "Two classes: below and above 0.3. Each connected piece is an object."

[[nodes]]
id = "classes"
op = "objects.rules"
inputs = { objects = "clumps.objects" }
note = "Names the classes: the high one is the canopy."

[nodes.params]
classes = [
  { name = "canopy", rule = "above(class, 1, 2)" },
  { name = "open", rule = "below(class, 1, 2)" },
]
```

The clip has **726 canopy clumps**, from a single crown to a whole row of trees along a street.

### Step 2: split the clumps

[[objects.split]] reads the objects and, with `by = "surface"`, the map to flood. The settings that matter:

| Setting | What it is | Here |
|---|---|---|
| `by` | `distance` (the default) floods the distance to the object's border; `surface` floods the map on the `surface` input, its peaks being the centres (`invert = true` for dark centres) | `surface` |
| `classes` | The class names whose objects are split (default all). The others are left as they are | `["canopy"]` |
| `h` | How high a centre must stand over its saddle. In the surface's units; by `distance`, in map units (default one cell). By `surface` the default is 0.05 of the map's range | 0.03 |
| `sigma` | By `surface`: a Gaussian smooth of the surface inside the operator, in cells (default 0) | 1 |
| `min_size` | A piece of fewer cells goes back to the neighbour piece it shares the longest border with (default 0) | 10 |
| `max_neck` | eCognition's *length factor*: a cut stays only if it is shorter than `max_neck` times the width of the smaller piece (twice its greatest distance to the border). Default: no limit | not used |
| `neighbours` | 4 or 8: which cells connect a piece (default 8) | 8 |

The pieces take the class of their object and replace it in the same level. Everything else computed on the objects, such as the
attributes and the level above, is dropped (with a warning), so [[objects.features]] runs **after** the split.
The node also writes a `report`, a row for each clump that was cut and the number of pieces.

```tfgraph
#! caption: The split: the canopy clumps cut at the saddles of the smoothed NDVI.
#! highlight: split
#! numbered: true
#! fragment: true
[[nodes]]
id = "split"
op = "objects.split"
inputs = { objects = "classes.objects", surface = "smooth.map" }
params = { by = "surface", classes = ["canopy"], h = 0.03, sigma = 1, min_size = 10 }
note = "Every peak of the NDVI that stands 0.03 over its saddle makes a crown. sigma smooths the surface a little more first."
why = "The surface, not the shape, tells where a crown ends: two crowns joined by a wide neck still have a saddle in their foliage."
```

The 726 clumps give **2,549 crowns**: 119 clumps were cut, into 1,942 pieces (the biggest, a wooded patch of the park, into 813), and the other 607 stay as they were, single crowns or groups in which no peak stood out.

### Step 3: join what was cut too fine

The split is generous on purpose: a small `h` finds every crown, and also cuts some crowns in two. [[objects.fuse]]
repairs that. For each *seed* object, it computes the **measure of the object that would result** from joining it to each
neighbour (the *candidates*) and joins the pair when that object is good enough:

- **`fitting`** is the measure, an expression over the *joined* object, written as in [[objects.calc]]: `compactness`,
  `shape_index`, `border_index`, `length`, `width`, `length_width`, `area`, `cells`, `perimeter`, and with an `image` also
  `mean_<band>` and `sd_<band>`. Others, such as texture, skeleton or context, are refused with the list of those that work.
- **`goal`** says which direction is better: `min` for compactness (1 is a rectangle, higher is less compact), `max` for
  a mean of a band.
- **`threshold`** is the value the joined object must reach: at most it, for `min`.
- **`improve`** (true by default) asks the joined object to be at least as good as the seed's own value; a tie joins, so two
  rectangles make a rectangle.
- `seeds` and `candidates` are class names (`unclassified` counts as a class), the result has the seed's class, `mode`
  is `best` (each seed its best candidate) or `all`, and it repeats until nothing joins or `max_passes` (100).

```tfgraph
#! caption: The fusion: neighbouring canopy pieces are joined when the object they make has a compactness of at most 1.5.
#! highlight: fuse
#! numbered: true
#! fragment: true
[[nodes]]
id = "fuse"
op = "objects.fuse"
inputs = { objects = "split.objects" }
params = { seeds = ["canopy"], candidates = ["canopy"], fitting = "compactness", goal = "min", threshold = 1.5, improve = false }
note = "Joins two pieces of canopy when the union is compact. improve is off so the threshold alone decides."
why = "Fusion by the result, as eCognition's: the shape of the whole decides, not the pieces."
```

`improve` is off here and `threshold` does the work. The next sections show what each does, what *compactness* is in this
operator, and what happens with the other choices.

### Step 4: the tops

[[objects.features]] gives the crowns their attributes: here the mean and maximum of the smoothed NDVI, the area, and the shape
group. [[objects.extrema]] then compares an attribute among neighbouring objects. `attribute` is a column or an
expression as in [[objects.calc]], `kind` is `max` or `min`, and the neighbourhood is
**either** `radius` (map units between the centroids of the objects, here metres) **or** `steps` (how many objects
across the neighbours, 1 by default); not both. `among` limits which classes are compared and marked, and the extrema get the class
`class`. With `strict` (the default, eCognition's *exclude equal*) a tie is no extremum.

```tfgraph
#! caption: The whole model: from the image to the crowns and their tops.
#! highlight: features, tops
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "naip"
op = "raster.read"
params = { path = "naip_plano.tif" }
note = "The NAIP clip: red, green, blue and nir, 0.6 m."

[[nodes]]
id = "ndvi"
op = "raster.calc"
inputs = { s = "naip.raster" }
params = { expression = "out ndvi = (s.nir - s.red) / (s.nir + s.red)" }
note = "NDVI of every cell (Tucker 1979)."

[[nodes]]
id = "smooth"
op = "raster.focal"
inputs = { map = "ndvi.ndvi" }
params = { stat = "mean", shape = "gaussian", radius = 3, sigma = 1 }
note = "A Gaussian smooth: the surface, and what the canopy is cut from."

[[nodes]]
id = "clumps"
op = "segment.threshold"
inputs = { image = "smooth.map" }
params = { thresholds = [0.3], neighbours = 8 }
note = "Two classes by 0.3: the objects are the connected pieces."

[[nodes]]
id = "classes"
op = "objects.rules"
inputs = { objects = "clumps.objects" }
note = "The high class is the canopy."

[nodes.params]
classes = [
  { name = "canopy", rule = "above(class, 1, 2)" },
  { name = "open", rule = "below(class, 1, 2)" },
]

[[nodes]]
id = "split"
op = "objects.split"
inputs = { objects = "classes.objects", surface = "smooth.map" }
params = { by = "surface", classes = ["canopy"], h = 0.03, sigma = 1, min_size = 10 }
note = "The clumps cut at the saddles of the NDVI."
refs = ["Vincent and Soille 1991, Watersheds in digital spaces"]

[[nodes]]
id = "fuse"
op = "objects.fuse"
inputs = { objects = "split.objects" }
params = { seeds = ["canopy"], candidates = ["canopy"], fitting = "compactness", goal = "min", threshold = 1.5, improve = false }
note = "Pieces joined while the union stays compact."

[[nodes]]
id = "features"
op = "objects.features"
inputs = { objects = "fuse.objects", image = "smooth.map" }
params = { features = ["mean", "max", "area", "shape"] }
note = "Mean and maximum NDVI, area and shape of each crown."

[[nodes]]
id = "tops"
op = "objects.extrema"
inputs = { objects = "features.objects" }
params = { attribute = "max_image", kind = "max", among = ["canopy"], radius = 15, class = "top" }
note = "The crown with the highest maximum NDVI within 15 m is a top."
```

Run it in the application as for the other pages: open the template, **File > Use my data...**, or paste the model into a
new graph; ++ctrl+enter++ on a node runs the graph up to it, and the Viewer shows the objects of a node with their classes. From
the command line, with `naip_plano.tif` beside the file:

```bash
terraflux run tree-crowns.toml --out out
```

The outputs of every node are in `out/`; to see a map of the pieces add [[objects.to_map]] to the node's `objects` output and a
[[raster.write]] after it.

## Read the results

![Four steps on the same 90 m](/img/tutorials/object-based-image-analysis/tree-crowns/step_rgb.png "The same 90 m window as in the figures below: a street with trees, in true colour.")

```tfgallery
columns: 2
caption: The four stages of the model in the 90 m window. Yellow lines are the outlines of canopy objects; pink dots are the tops.
images:
  - { src: /img/tutorials/object-based-image-analysis/tree-crowns/step_clumps.png, caption: "1. The canopy clumps: a row of trees is one object" }
  - { src: /img/tutorials/object-based-image-analysis/tree-crowns/step_split.png, caption: "2. After objects.split: crowns, cut at the saddles" }
  - { src: /img/tutorials/object-based-image-analysis/tree-crowns/step_fuse.png, caption: "3. After objects.fuse: the pieces that were too fine are joined" }
  - { src: /img/tutorials/object-based-image-analysis/tree-crowns/step_tops.png, caption: "4. The tops (objects.extrema, 15 m)" }
```

```tftable
data: /data/tutorials/object-based-image-analysis/tree-crowns/stages.csv
labels: { stage: Stage, objects: "Canopy objects" }
align: { stage: left }
digits: 0
caption: "The canopy objects of the whole 600 m clip at each stage (the last row counts the objects marked as tops, which are crowns too)."
```

The clip has 726 clumps, 2,549 crowns after the split, 2,242 after the fusion (168 objects were made out of 475 pieces) and 471
tops. After the fusion the canopy objects have a mean area of 65 m² and a median of 39 m²; together they cover about 14.7 ha, 41 % of the clip. An
object of 39 m² is 7 m across, which is a plausible size for a yard tree, but it is one number from one image and not an accuracy.

![The tops over the whole clip](/img/tutorials/object-based-image-analysis/tree-crowns/whole.png "The 471 tops (pink) over the whole clip. They follow the trees along the streets and in the wooded park, and also mark the odd bright patch of shrub.")

!!! warning "There is no reference here"
    No crown was measured in the field and no lidar was used, so this page cannot give you an accuracy, a commission or an
    omission rate. Everything above was judged by eye on the image. For a paper you need a **reference**: crowns delineated by
    hand on a sample of the image (a few hundred trees), or a field inventory, and then the share of reference crowns
    matched by exactly one object (a count of how many are merged and how many are split). [[objects.sample]] and
    [[objects.accuracy]] from [part 5](/tutorials/object-based-image-analysis/classifying-by-samples/) do the sampling and the
    assessment of the classes; matching crown outlines is a job for your own script.

What each number means for the work:

- **Clumps cut**: the report of [[objects.split]] has a row per clump that was cut and its number of pieces (here 119 clumps, with 813 pieces
  at most). A clump that was **not** cut is either a single crown or an under-split group: look at the largest.
- **Area of a crown**: from the table of the objects (`area`, in m²). A very small crown is a crumb; a very large one an unsplit group.
  Report the distribution (median and quartiles), not only the mean, which a few big clumps pull up.
- **Tops**: the table of [[objects.extrema]] has a column `extremum`: 1 for a top, 0 for a canopy object that is not, empty for the
  objects that were not compared (the open ground). Tops are a count of the *greenest* crown of each 15 m neighbourhood,
  not of the *tallest* tree (see the next section).

## Settings that change the answer

### h: how fine the crowns come out

```tfslider
label: "h (NDVI)"
caption: "The same canopy split with six values of h (sigma 1, no fusion yet). A small h cuts every bump, a large one leaves clumps. The numbers are for the whole clip; the picture is the 90 m window."
frames:
  - { value: 0.01, src: /img/tutorials/object-based-image-analysis/tree-crowns/h_0_01.png, caption: "h = 0.01", stats: { Crowns: "3,903", "Clumps cut": 141, "Largest (m2)": "244" } }
  - { value: 0.02, src: /img/tutorials/object-based-image-analysis/tree-crowns/h_0_02.png, caption: "h = 0.02", stats: { Crowns: "3,095", "Clumps cut": 132, "Largest (m2)": "358" } }
  - { value: 0.03, src: /img/tutorials/object-based-image-analysis/tree-crowns/h_0_03.png, caption: "h = 0.03 (this page)", stats: { Crowns: "2,549", "Clumps cut": 119, "Largest (m2)": "631" } }
  - { value: 0.05, src: /img/tutorials/object-based-image-analysis/tree-crowns/h_0_05.png, caption: "h = 0.05", stats: { Crowns: "1,935", "Clumps cut": 95, "Largest (m2)": "1,224" } }
  - { value: 0.1, src: /img/tutorials/object-based-image-analysis/tree-crowns/h_0_1.png, caption: "h = 0.1", stats: { Crowns: "1,251", "Clumps cut": 58, "Largest (m2)": "1,835" } }
  - { value: 0.2, src: /img/tutorials/object-based-image-analysis/tree-crowns/h_0_2.png, caption: "h = 0.2", stats: { Crowns: "819", "Clumps cut": 18, "Largest (m2)": "6,190" } }
chart:
  type: line
  data: /data/tutorials/object-based-image-analysis/tree-crowns/h.csv
  x: h
  y: [crowns]
  labels: { crowns: "Crowns after the split" }
  ylabel: Crowns
```

`h` is the setting to tune first, and the **largest crown** is the number to watch. At h = 0.01 the largest crown is 244 m², about
18 m across, which is a big tree, and there are 3,903 pieces, so some are lumps of foliage. At h = 0.2 only 18 of the 726 clumps are cut and the largest object is
6,190 m², which is a group of trees. The right value is in between and depends on the image: look at the rows of trees you know, in the window of
the figure, and take the h at which one tree is one piece.

h is in the surface's units, which makes it **not portable**. An NDVI of 0.03 is a small step on a 0 to 0.7 scale. A height model in
metres needs an h in metres (0.5 to 1 m is a first guess), and the default, 0.05 of the surface's range, is a reasonable start for any surface.

### sigma: smooth first

```tfslider
label: "sigma (cells)"
caption: "The split with the surface smoothed more before it is flooded (h = 0.03). The map was already smoothed once, with sigma 1 in raster.focal."
frames:
  - { value: 0, src: /img/tutorials/object-based-image-analysis/tree-crowns/sigma_0.png, caption: "sigma = 0", stats: { Crowns: "3,976", "Clumps cut": 144, "Largest (m2)": "379" } }
  - { value: 1, src: /img/tutorials/object-based-image-analysis/tree-crowns/sigma_1.png, caption: "sigma = 1 (this page)", stats: { Crowns: "2,549", "Clumps cut": 119, "Largest (m2)": "631" } }
  - { value: 2, src: /img/tutorials/object-based-image-analysis/tree-crowns/sigma_2.png, caption: "sigma = 2", stats: { Crowns: "1,555", "Clumps cut": 78, "Largest (m2)": "877" } }
  - { value: 3, src: /img/tutorials/object-based-image-analysis/tree-crowns/sigma_3.png, caption: "sigma = 3", stats: { Crowns: "1,231", "Clumps cut": 59, "Largest (m2)": "1,212" } }
chart:
  type: line
  data: /data/tutorials/object-based-image-analysis/tree-crowns/sigma.csv
  x: sigma
  y: [crowns]
  labels: { crowns: "Crowns after the split" }
  ylabel: Crowns
```

`sigma` and `h` do almost the same work, and you can trade them: smoothing removes the small bumps by blurring them, `h` removes them by
their height. Smoothing also **moves** the centres a little and blurs the saddles, so prefer a small sigma and an h you
can read. A rough crown (big leaves, strong shade) needs more smoothing than a smooth one.
Remember the NDVI was already smoothed in [[raster.focal]]; `sigma` of the split adds to that.

### max_neck and min_size: undo the thin cuts and the crumbs

```tfslider
label: "max_neck"
caption: "The split with a limit on how long a cut may be (h = 0.03). A cut stays only if it is shorter than max_neck times the width of the smaller piece. A low value undoes the long cuts."
frames:
  - { value: 1, src: /img/tutorials/object-based-image-analysis/tree-crowns/neck_None.png, caption: "No limit (this page)", stats: { Crowns: "2,549", "Clumps cut": 119, "Largest (m2)": "631" } }
  - { value: 2, src: /img/tutorials/object-based-image-analysis/tree-crowns/neck_4.png, caption: "max_neck = 4", stats: { Crowns: "2,549", "Clumps cut": 119, "Largest (m2)": "631" } }
  - { value: 3, src: /img/tutorials/object-based-image-analysis/tree-crowns/neck_2.png, caption: "max_neck = 2", stats: { Crowns: "2,524", "Clumps cut": 117, "Largest (m2)": "631" } }
  - { value: 4, src: /img/tutorials/object-based-image-analysis/tree-crowns/neck_1.png, caption: "max_neck = 1", stats: { Crowns: "1,538", "Clumps cut": 91, "Largest (m2)": "2,214" } }
  - { value: 5, src: /img/tutorials/object-based-image-analysis/tree-crowns/neck_0_5.png, caption: "max_neck = 0.5", stats: { Crowns: "931", "Clumps cut": 58, "Largest (m2)": "8,607" } }
```

`max_neck` is eCognition's *length factor*: two parts joined by a **wide neck** are probably one object, so a cut that is long next to
the width of the smaller piece is undone. The width of a piece is taken as twice its largest distance to the border. For crowns,
a value around 1 is a strong constraint (1,538 crowns instead of 2,549); 2 does almost nothing here. It is the right tool when the clump is
a rounded shape with real necks, such as buildings or cells; for a canopy with no neck, `h` is the lever.

`min_size` sends a piece of fewer cells to the neighbour piece it shares the longest border with. On this image it changes little
until it gets large:

```tftable
data: /data/tutorials/object-based-image-analysis/tree-crowns/min.csv
columns: [min_size, crowns, clumps_cut, largest_crown_m2]
labels: { min_size: "min_size (cells)", crowns: Crowns, clumps_cut: "Clumps cut", largest_crown_m2: "Largest crown (m2)" }
digits: { min_size: 0, crowns: 0, clumps_cut: 0, largest_crown_m2: 0 }
caption: "min_size from 0 to 50 cells (50 cells are 18 m², at 0.36 m² a cell). Up to 10 cells it barely changes anything; at 50 cells it gives 185 pieces back to their neighbours."
```

The surface peaks are already far apart, so the h-maxima leave few crumbs; set `min_size` to the smallest crown you want to count
(in cells: area in m² over 0.36).

### The distance instead of the surface

```tfcompare
caption: "Drag the handle. The split by the surface (left, 2,549 crowns) and by the distance to the border (right, h = 0.6 m, 1,416 crowns), both before the fusion. The distance cuts at the necks of the clump's outline, and these clumps have few."
before: { src: /img/tutorials/object-based-image-analysis/tree-crowns/surface_split.png, label: "By the surface (NDVI)" }
after: { src: /img/tutorials/object-based-image-analysis/tree-crowns/dist_0_6.png, label: "By the distance" }
```

```tftable
data: /data/tutorials/object-based-image-analysis/tree-crowns/distance.csv
columns: [by, h, crowns, clumps_cut, largest_crown_m2]
labels: { by: Surface, h: h, crowns: Crowns, clumps_cut: "Clumps cut", largest_crown_m2: "Largest (m2)" }
align: { by: left }
digits: { h: 2, crowns: 0, clumps_cut: 0, largest_crown_m2: 0 }
caption: "Splitting by the distance, with h in metres (by distance, h is in map units, here metres; the default is one cell, 0.6 m), and by the NDVI with the settings of the page."
```

The distance needs no image band and no smoothing, and the cuts are at the narrowest parts of the clump's outline: for a clump of round crowns
that is exactly right. These clumps are not round: many are a row of trees along a street, whose outline is a long ragged strip, and the distance cuts at the
bays of that outline. The surface knows where the crowns are because the foliage tells it. **The surface is the better guide when the image has the
crowns in it; the distance, when it has only the outline.** A thresholded height model, where the clumps' outlines are good, is the case for the distance.

### The fusion: threshold, improve and compactness

The split left 2,549 crowns. The fusion's threshold decides how many pairs are joined back:

```tfslider
label: "compactness at most"
caption: "The fusion on the 2,549 crowns with a threshold on the compactness of the joined object. Pieces joined are tinted in the page's figures; here the whole window. Above 2 the joined objects stop being crowns."
frames:
  - { value: 1, src: /img/tutorials/object-based-image-analysis/tree-crowns/fuse_0.png, caption: "No fusion", stats: { Objects: "2,549", "Largest (m2)": "631" } }
  - { value: 2, src: /img/tutorials/object-based-image-analysis/tree-crowns/fuse_1_2.png, caption: "At most 1.2", stats: { Objects: "2,549", "Largest (m2)": "631" } }
  - { value: 3, src: /img/tutorials/object-based-image-analysis/tree-crowns/fuse_1_3.png, caption: "At most 1.3", stats: { Objects: "2,544", "Largest (m2)": "631" } }
  - { value: 4, src: /img/tutorials/object-based-image-analysis/tree-crowns/fuse_1_5.png, caption: "At most 1.5 (this page)", stats: { Objects: "2,242", "Largest (m2)": "751" } }
  - { value: 5, src: /img/tutorials/object-based-image-analysis/tree-crowns/fuse_2_0.png, caption: "At most 2", stats: { Objects: "1,000", "Largest (m2)": "9,616" } }
  - { value: 6, src: /img/tutorials/object-based-image-analysis/tree-crowns/fuse_3_0.png, caption: "At most 3", stats: { Objects: "759", "Largest (m2)": "24,284" } }
```

Up to 1.5 the fusion joins few pieces (475 into 168 objects) and the largest object barely grows. At 2 it undoes most of the split (the largest object is 1 ha), and
at 3 it builds objects of 2.4 ha. The threshold has a **sweet spot** and it is low. Here is what the fusion joined at 1.5 in the window, tinted:

```tfcompare
caption: "Drag the handle. The orange pieces of the split (left) are joined into the orange objects of the fusion (right), compactness at most 1.5. Six objects were made in this window."
before: { src: /img/tutorials/object-based-image-analysis/tree-crowns/fuse_before.png, label: "Before the fusion" }
after: { src: /img/tutorials/object-based-image-analysis/tree-crowns/fuse_after.png, label: "After the fusion" }
```

#### What compactness is in objects.fuse

The measure is \( \text{length} \times \text{width} / \text{area} \): 1 for a rectangle, about 1.27 for a disc, more for a ragged
outline or an L. [[objects.fuse]] measures the length and the width of the joined object as [[objects.features]] does, as the sides of the **minimum-area rotated
rectangle** around it, so a limit on `compactness` means the same in both operators: a limit you read off the `compactness` column of the features of your objects can be given
to the fusion as it is. (An earlier build of the fusion used the axis-aligned box and gave larger values for tilted pieces; it no longer does.)

A check on this page's run. The objects made by joins, measured afterwards by [[objects.features]], should all be at or under the threshold:

```tftable
data: /data/tutorials/object-based-image-analysis/tree-crowns/fuse_check.csv
labels: { objects: Objects, count: Count, median_compactness: "Median compactness", max_compactness: "Largest compactness" }
align: { objects: left }
digits: { count: 0, median_compactness: 2, max_compactness: 2 }
caption: "Compactness, from objects.features, of the 168 objects made by the fusion (threshold 1.5) and of all the canopy objects after it. Every joined object is at or under 1.5; more than half of the others are above it, because the fusion only joins when the union is compact and leaves the other pieces as they were."
```

What follows for your settings:

- Compactness does not punish **elongation**: a long strip is a rectangle and has compactness 1. The fusion joined the row of trees at
  the centre of the figure above into one long object. If that is not what you want, put the **length over the width** in the measure,
  which the fusion also knows: `fitting = "max(compactness, length_width / 2)"` joins only when the union is compact **and** at most three times longer than wide. On this image that changes little: 169 objects are made by joins instead of 168, and the largest is 733 m² instead of 751. Look at your own rows of trees before you decide that it matters.

```tftable
data: /data/tutorials/object-based-image-analysis/tree-crowns/fuse_fitting.csv
labels: { fitting: Fitting, objects: Objects, joined: "Objects made by a join", largest_m2: "Largest (m2)" }
align: { fitting: left }
digits: 0
caption: "The same fusion (threshold 1.5, improve off) with two measures: compactness alone, and compactness together with the length over the width."
```

#### improve and threshold

The two settings work on different things. `improve` compares the joined object with the **seed's own value**, `threshold` with a **fixed
value**. They can be used together:

```tftable
data: /data/tutorials/object-based-image-analysis/tree-crowns/fuse_modes.csv
labels: { setting: Setting, objects: "Canopy objects", joined: "Objects made by a join", largest_m2: "Largest (m2)" }
align: { setting: left }
digits: 0
caption: "The fusion of the 2,549 crowns with other combinations of improve and threshold (compactness, goal min). The pairs 'improve, threshold 1.5' with mode best and mode all give the same here."
```

- **`improve` alone** (no threshold) joins a pair whenever the union is at least as compact as the seed. 383 objects were made by joins and the largest is 2,106 m²: a join
  only has to be as good as the seed, so a seed can take one neighbour after another, and this ends in groups.
- **`threshold` alone** (`improve = false`) is the page's choice: a fixed shape rule that is easy to read, and it does not depend on where the seed started.
- **Both**: the pair must be at least as compact as the seed **and** reach the threshold. It is the most conservative: 154 objects made by joins against 168.
- **Neither** (`improve = false`, no `threshold`) lets every pair qualify, so the clumps come back (735 objects against 726 clumps), and the largest is 6.4 ha. It is the
  only combination of the table that is not a repair, and it is not what you want after a split. The operator now **warns** when it runs with `improve = false` and no `threshold`.

`mode = "all"` lets a seed whose best candidate was taken in the pass fall back to the next one in the same pass; with `best` it waits for the next pass. Here the
result is the same.

### radius: how the tops are found

```tfslider
label: "radius (m)"
caption: "The tops (pink) with a larger neighbourhood: a crown is a top when no crown within the radius has a higher maximum NDVI. The count is for the whole clip."
frames:
  - { value: 5, src: /img/tutorials/object-based-image-analysis/tree-crowns/radius_5.png, caption: "radius = 5 m", stats: { Tops: "2,068" } }
  - { value: 10, src: /img/tutorials/object-based-image-analysis/tree-crowns/radius_10.png, caption: "radius = 10 m", stats: { Tops: 993 } }
  - { value: 15, src: /img/tutorials/object-based-image-analysis/tree-crowns/radius_15.png, caption: "radius = 15 m (this page)", stats: { Tops: 471 } }
  - { value: 25, src: /img/tutorials/object-based-image-analysis/tree-crowns/radius_25.png, caption: "radius = 25 m", stats: { Tops: 177 } }
  - { value: 40, src: /img/tutorials/object-based-image-analysis/tree-crowns/radius_40.png, caption: "radius = 40 m", stats: { Tops: 71 } }
chart:
  type: line
  data: /data/tutorials/object-based-image-analysis/tree-crowns/radius.csv
  x: radius
  y: [tops]
  labels: { tops: "Tops" }
  ylabel: Tops
```

With a radius of 5 m almost every crown is a top; with 40 m there are 71 for 2,242 crowns. What a top is depends on the radius, so choose it for the
question: the radius of the stand you call a neighbourhood. Use `radius` when the objects are small and many (after a split) and `steps` when they differ in
size. And remember what the attribute is: here the **maximum NDVI**, so a top is the *greenest* crown of its neighbourhood. With a canopy height model,
the same node with `attribute = "max_image"` gives the **tallest**, and that is the tree that sets the height of the stand. The page has no height, so do not read the
tops as tree heights.

## Common pitfalls

- **Reading the NDVI as a height.** The watershed floods whatever you give it. On NDVI it finds the greenest bumps, not the tallest
  trees; with a shaded crown beside a sunlit one, the shaded one has a lower NDVI and may lose its centre.
- **An h that is not in your units.** 0.03 is for the NDVI here and 0.03 m on a height model would split every leaf. Start from the default (0.05 of the range) and
  change it by looking at the rows of trees you know.
- **Splitting all the objects.** Without `classes`, [[objects.split]] also divides the open ground and the roofs. Name the class.
- **Computing attributes before the split.** The split and the fusion drop the other attributes and the level above, with a warning; run
  [[objects.features]] after them.
- **Fusing without limits.** `improve = false` and no `threshold` give back the clumps (almost); the operator warns. Always give one of the two.
- **Tops with `strict` on a flat top.** A tie is not a top by default. If a plateau has none, set `strict = false`: all the tied objects are tops.
- **Counting tops as trees.** A top is the greenest crown of a neighbourhood; a neighbourhood can hold more than one tree of a height, and an
  over-split tree gives pieces that compete with each other.
- **No reference.** A count of crowns from a figure is a hypothesis. Compare it with trees you counted on a sample.

## Going further

- The operators: [[objects.split]], [[objects.fuse]], [[objects.extrema]], [[raster.focal]], [[segment.threshold]], [[objects.rules]].
- The same split on the trees of a full map (with the roofs and streets): [Urban cover of a neighbourhood from NAIP](/tutorials/object-based-image-analysis/case-urban-cover-naip/) (part 14).
- The other half of this family, long thin objects by their skeleton: [Roads and rivers by the skeleton](/tutorials/object-based-image-analysis/roads-and-rivers-by-the-skeleton/) (part 19).
- What the other tools of eCognition become in Terraflux: [Coming from eCognition](/tutorials/object-based-image-analysis/from-ecognition/).
- Join or tidy objects by rule instead of by their result: [Levels](/tutorials/object-based-image-analysis/levels/) (part 8).

## References

- Carpenter, A. E., Jones, T. R., Lamprecht, M. R., Clarke, C., Kang, I. H., Friman, O., Guertin, D. A., et al. (2006). CellProfiler: image
  analysis software for identifying and quantifying cell phenotypes. *Genome Biology*, 7(10), R100. https://doi.org/10.1186/gb-2006-7-10-r100
- Felzenszwalb, P. F., & Huttenlocher, D. P. (2012). Distance transforms of sampled functions. *Theory of Computing*, 8(1), 415-428.
  https://doi.org/10.4086/toc.2012.v008a019
- Ke, Y., & Quackenbush, L. J. (2011). A review of methods for automatic individual tree-crown detection and delineation from passive remote
  sensing. *International Journal of Remote Sensing*, 32(17), 4725-4747. https://doi.org/10.1080/01431161.2010.494184
- Tucker, C. J. (1979). Red and photographic infrared linear combinations for monitoring vegetation. *Remote Sensing of Environment*,
  8(2), 127-150. https://doi.org/10.1016/0034-4257(79)90013-0
- Vincent, L., & Soille, P. (1991). Watersheds in digital spaces: an efficient algorithm based on immersion simulations. *IEEE Transactions on
  Pattern Analysis and Machine Intelligence*, 13(6), 583-598. https://doi.org/10.1109/34.87344
- Wulder, M., Niemann, K. O., & Goodenough, D. G. (2000). Local maximum filtering for the extraction of tree locations and basal area from
  high spatial resolution imagery. *Remote Sensing of Environment*, 73(1), 103-114. https://doi.org/10.1016/S0034-4257(00)00101-2
