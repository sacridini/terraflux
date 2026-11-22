---
title: "Editing by hand: merge, cut and delete"
short: Editing by hand
description: "Correct the objects of a segmentation with your own eyes: join the pieces of one lake, cut an object that holds two things, delete a stray one. See how Terraflux keeps these edits as places in the model, what they change in the classes, and when it is better to change the rule instead."
order: 100
level: intermediate
time: 45 min
ops: [segment.multiresolution, objects.edit, objects.features, objects.samples, ml.fit, objects.classify, objects.accuracy, objects.to_map]
series: Object-based image analysis
part: 10
---

No segmenter draws every object right. On the Sentinel-2 image of Lago Paranoá, the lake comes out in a dozen pieces, one
object holds bare ground and the first roofs of a town together, and a white roof has an object of its own. You can
fix these by hand: join the pieces, cut the object in two, give the stray one to its neighbour. In this tutorial you
do each of the three, see how Terraflux keeps the correction in the model (so that the analysis can still be run
again), and learn what a correction does to the classes after it. You will also learn when not to edit: when
a change in the segmentation or in the rule is the better fix.

## The idea

A segmenter ([[segment.multiresolution]], and the others of [part 2](/tutorials/object-based-image-analysis/what-is-an-object/))
makes its objects by a rule that is the same everywhere in the image: merge neighbouring cells that look alike, up to a
limit. A rule that is the same everywhere is wrong in a few places. There are two kinds of mistake, and they are
the **over-segmentation** and the **under-segmentation** of [part 3](/tutorials/object-based-image-analysis/choosing-the-scale/):

- **One thing in many objects**: a lake in a dozen pieces, a field in thirty. The objects are small and alike, and
  classifying each of them gives the same class, so the map is not wrong, but the count of objects, their sizes and
  their shapes are (the area of the lake is the sum of twelve pieces, its perimeter is twelve perimeters).
- **Two things in one object**: bare ground and a street in the same object. This one hurts the map: the object takes
  one class and the other thing is mapped as the wrong class.
- **An object of its own that should not be**: a white roof the classifier cannot place, a sliver along an edge.

![Where the edits of this page are](/img/tutorials/object-based-image-analysis/editing-by-hand/where.png "The Sentinel-2 clip (true colour). The three boxes are the places edited in this tutorial: the lake (a merge), the object between the lake and the town (a cut) and a white roof (a delete).")

Three operations correct them, the ones that eCognition's users know as manual editing:

| Edit | What it does | Fixes |
|---|---|---|
| **Merge** | The objects at some points become one: the first point's | one thing in many objects |
| **Cut** | A line splits every object it crosses into the pieces it leaves | two things in one object |
| **Delete** | The object at a point is left with no object, or is taken by its most similar neighbour | a stray object |

!!! science "Key idea: an edit is a place, not an object number"
    The objects of a segmentation are numbered 1, 2, 3... in the order the segmenter made them. Change the scale by a
    little and every number means another object. An edit that said "merge objects 17 and 18" would break at the first
    change. So Terraflux keeps an edit as **places**: the coordinates of a point inside each object (or of the vertices
    of a line), in the objects' coordinate system. Run on another segmentation, the same list finds the objects that
    are *there*. If an edit finds nothing, or changes nothing, Terraflux says so, in a warning and in a table.

The operator is [[objects.edit]]. The Viewer writes it for you when you click; you can also write it by hand, and the
page does that, because a page cannot be clicked.

## When to use it

Edit by hand when:

- the objects will be **published or measured as objects**: the polygons of lakes or fields that will be counted, a
  map for a report, where one wrong object is seen by everyone;
- an object **is wrong for a reason the segmenter cannot know** (a boundary that a road, a fence or a property line
  makes and that has no colour);
- you have **a few** such objects. Three edits of 725 objects is a clean-up. Three hundred edits is a different
  segmentation, and you should look for it.

Do not edit when:

- the same mistake is **everywhere** (all the lakes are cut in pieces, all the roofs merge with the street): change
  the scale, the shape or compactness, or the segmenter (parts 2 and 3);
- the problem is the **class**, not the object: [[objects.edit]] changes the objects, never their classes. To change
  what an object is called, give it a sample, or change the rule (parts 5 and 6);
- the image **will change**: a new date, a new tile. The edit is a place, so it will land on something, but that
  something may be a different object (see the section on other segmentations).

Reviews of object-based classification put the errors of the segmentation among its limits (Blaschke, 2010; Ma et al.,
2017); editing by hand is a way to correct some of them, not a way to avoid choosing a good scale.

## What you need

- The Sentinel-2 clip of the series (Lago Paranoá, Brasília, 9 August 2024; 580 by 580 cells of 10 m, as in
  [part 2](/tutorials/object-based-image-analysis/what-is-an-object/)). The model below reads it with [[data.stac]]; for
  no network, the [clip to download](/data/tutorials/object-based-image-analysis/sentinel2_brasilia.tif) has the same
  five bands.
- A segmentation: [[segment.multiresolution]], here at the **scale 4**, which gives 725 objects. It is the coarser
  scale that suits the fields and the lake, and it is where a few objects reach over two things.
- Points to train a classifier and points to check it, to see what an edit does to a map. A figure cannot be clicked, so
  the 87 training and 67 reference points of this page are drawn by a script from labelled places of the image, as in
  [part 5](/tutorials/object-based-image-analysis/classifying-by-samples/)
  ([training_samples.csv](/data/tutorials/object-based-image-analysis/editing-by-hand/training_samples.csv),
  [reference_samples.csv](/data/tutorials/object-based-image-analysis/editing-by-hand/reference_samples.csv)). They
  are not field data: they are the labels of someone who looked at the image, and the accuracy numbers below are
  an example, not an accuracy of the map of Brasília.

## Edit in the application

You edit in the Viewer, on a layer of objects. The controls are in the **inspector**, the panel beside the map that
tells the attributes of the selected object, in a block called **Edit**. (These are instructions, not a screenshot:
the page does not show the application.)

<div class="steps" markdown>

1. Run the node that makes the objects, or any node that reads them (the classification, say), and show its objects
   in the Viewer.
2. **Click** an object to select it. **Ctrl+click** another to add it (or to take it out of the selection). Press ++n++ to
   add the neighbours of the selected objects.
3. In the *Edit* block of the inspector:
    - **Merge** is enabled when two objects or more are selected: they become one. The survivor is the selected object
      with the lowest number.
    - **Delete** is enabled with one or more: the selected objects are left with no object, or, with **into a
      neighbour** ticked, each is taken by its most similar neighbour.
    - **Cut** asks for a line: press the button, click the **vertices** of the line on the map, and press ++enter++. Every object
      the line crosses is split. Press the button again to cancel.
4. Each click of Merge, Delete or the end of a line writes the edit into the model as one step of undo (named *merge 3
   objects*, *delete 1 objects* or *cut objects*), and a message tells how many edits the node has.
5. **Run the model again**: the edits are in the settings of the node, and they are applied when it runs. The Viewer
   shows the edited objects.

</div>

What Terraflux does to the model when you press the button:

- If there is an [[objects.edit]] node that reads these objects, the edit is **appended** to its list.
- If there is none, it **makes one** right after the segmenter (named `edit`, with a note), connects the segmenter's
  objects and image to it, and moves every node that read the segmenter's objects onto the new node. If you edit from
  a layer further down (the classes, say), it walks up to the segmenter first.
- Each point is the **interior cell nearest the centroid** of its object (the same rule as the samples), so it is
  always inside the object it names. A line is the vertices you clicked.

## Build the model

The model of the page has the edit node by hand, with the three edits of the figure above, and everything that follows
it: the attributes, a random forest from the training points and the accuracy against the reference points.

### Step 1: the objects

```tfgraph
#! caption: The first nodes: the clip, its reflectance, and the objects at the scale 4 (725 of them, as in part 2 but with a larger scale).
#! highlight: segments
#! numbered: true
#! fragment: true
[[nodes]]
id = "refl"
op = "raster.calc"
inputs = { s = "s2.raster" }
params = { expression = "out image = s / 10000" }
note = "Reflectance, 0 to 1 (the node data.stac before it, as in part 2)."

[[nodes]]
id = "segments"
op = "segment.multiresolution"
inputs = { image = "refl.image" }
params = { scale = 4 }
note = "725 objects: the scale 4 of part 3."
```

### Step 2: the edits

[[objects.edit]] reads the objects and, if you give it, the image they were made from. It writes the **edited objects**, a
`table` (the id, the cells, the area and, with the image, the mean of each band) and a `report`. It goes **right
after the segmenter, before** the attributes and the classes: the edited objects come out with no classes and no level
above, so anything that was computed on them has to run again.

The setting that matters is `edits`, a list applied in order. Each entry has one key:

| Entry | Meaning |
|---|---|
| `{ merge = [[x, y], [x, y], ...] }` | Two points or more: the objects there become the first one's. Objects that do not touch the first one are left apart, with a warning. |
| `{ cut = [[x, y], [x, y], ...] }` | A line of two points or more: each object it crosses is split into the connected pieces it leaves without the cells of the line; each cell of the line goes to a piece next to it. |
| `{ delete = [x, y], into = "none" }` | The object at the point is left with no object (the default). |
| `{ delete = [x, y], into = "neighbour" }` | The object is taken by the neighbour whose band means are nearest (with the `image`; without it, the neighbour with the longest shared border). |

The coordinates are in the coordinate system of the objects, here SIRGAS 2000 / UTM 23S in metres. The setting
`neighbours` (4 or 8) says which cells make an object whole; leave it, and it is 4, or 8 for objects that need it.

```tfgraph
#! caption: The edit node: the objects of the segmenter and the image, and three edits.
#! highlight: edit
#! numbered: true
#! fragment: true
[[nodes]]
id = "edit"
op = "objects.edit"
inputs = { objects = "segments.objects", image = "refl.image" }
params = { edits = [{ merge = [[192955.0, 8245655.0], [193085.0, 8245835.0], [193255.0, 8245755.0], [193095.0, 8245425.0], [193345.0, 8245425.0], [192835.0, 8245385.0], [193005.0, 8245915.0], [192815.0, 8245935.0]] }, { cut = [[193065.0, 8245115.0], [193465.0, 8245115.0]] }, { delete = [192965.0, 8244085.0], into = "neighbour" }] }
note = "Eight lake objects become one; a line cuts the object at the south edge of the lake; a small stray object is taken by its neighbour."
```

The three entries are the three of the figure. The **merge** has eight points, one in each of eight lake objects (the
first is the survivor, and they are chained: each touches another). The **cut** is a line of two points, along a row
of cells, from the left edge of the object to its right edge. The **delete** is one point on the small stray object.

!!! tip "How to find the coordinates"
    In the application you do not: the click gives them. Writing them by hand, read them off the Viewer (it reads out the row, the column
    and the map coordinates of the cell under the pointer), or compute them from a row and a column: here the clip's corner is at
    x = 190,520 and y = 8,247,080, and a cell is 10 m, so the centre of the cell in row *r* and column *c* is at
    x = 190,520 + (*c* + 0.5) × 10 and y = 8,247,080 − (*r* + 0.5) × 10.

### Step 3: what comes after

```tfgraph
#! caption: The whole model: objects, the edits, the attributes of the edited objects, the training and reference points, a random forest and the accuracy.
#! highlight: edit, feat
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
id = "segments"
op = "segment.multiresolution"
inputs = { image = "refl.image" }
params = { scale = 4 }
note = "725 objects."

[[nodes]]
id = "edit"
op = "objects.edit"
inputs = { objects = "segments.objects", image = "refl.image" }
params = { edits = [{ merge = [[192955.0, 8245655.0], [193085.0, 8245835.0], [193255.0, 8245755.0], [193095.0, 8245425.0], [193345.0, 8245425.0], [192835.0, 8245385.0], [193005.0, 8245915.0], [192815.0, 8245935.0]] }, { cut = [[193065.0, 8245115.0], [193465.0, 8245115.0]] }, { delete = [192965.0, 8244085.0], into = "neighbour" }] }
note = "Three edits: 718 objects."

[[nodes]]
id = "feat"
op = "objects.features"
inputs = { objects = "edit.objects", image = "refl.image" }
params = { features = ["mean", "sd", "ndvi", "ndwi", "ndmi", "contrast", "homogeneity", "compactness"], texture_bands = ["brightness"], swir = "swir16" }
note = "The attributes of the edited objects."

[[nodes]]
id = "train_table"
op = "table.read"
params = { path = "training_samples.csv" }
note = "The 87 training points: x, y, class."

[[nodes]]
id = "train"
op = "vector.from_table"
inputs = { table = "train_table.table" }
params = { crs = "EPSG:31983" }

[[nodes]]
id = "ref_table"
op = "table.read"
params = { path = "reference_samples.csv" }
note = "The 67 reference points."

[[nodes]]
id = "ref"
op = "vector.from_table"
inputs = { table = "ref_table.table" }
params = { crs = "EPSG:31983" }

[[nodes]]
id = "samples"
op = "objects.samples"
inputs = { objects = "feat.objects", samples = "train.vector" }
note = "The objects under the training points, with their attributes."

[[nodes]]
id = "fit"
op = "ml.fit"
inputs = { table = "samples.table" }
params = { column = "class", columns = ["mean_blue", "mean_green", "mean_red", "mean_nir", "mean_swir16", "ndvi", "ndwi", "ndmi", "glcm_contrast", "glcm_homogeneity", "sd_nir", "compactness"], n_estimators = 300 }
note = "A random forest."

[[nodes]]
id = "rf"
op = "objects.classify"
inputs = { objects = "feat.objects", model = "fit.model", samples = "samples.table" }
note = "The class of every object."

[[nodes]]
id = "acc"
op = "objects.accuracy"
inputs = { objects = "rf.objects", samples = "ref.vector" }
params = { design = "objects" }
note = "The accuracy against the reference points."
```

Without the edit node, the node `feat` reads `segments.objects` and the rest is the same: this is the model of
[part 5](/tutorials/object-based-image-analysis/classifying-by-samples/) at another scale. That is how the page
compares the map with and without the edits. Run `edit` first and read its `report`:

```tftable
data: /data/tutorials/object-based-image-analysis/editing-by-hand/report.csv
labels: { edit: Edit, kind: Kind, status: Status, cells: "Cells that changed object" }
align: { kind: left, status: left }
digits: 0
badge:
  status: { done: good, partly: warning, "no object": bad, "already one": warning, "no split": warning, "not touching": warning, "no neighbour": warning }
caption: "The report of the node edit, one row per edit. All three were done. The cells are those that went to another object: the eight lake objects, the smaller piece of the cut object and the roof."
```

The statuses are `done`, `partly` (some of the objects of a merge did not touch the first), `no object` (a point or a
line on no object), `already one` (a merge of points that are in one object), `no split` (a line that separates
nothing), `not touching` (a merge of objects that are apart) and `no neighbour` (a delete into a neighbour of an object that
has none). Every status but `done` also gives a warning when the model runs.

## Read the results

### The merge: one lake

Eight objects of the lake became one. The classes do not change, since the pieces were all water, but the object does:
it is 2,866 cells larger than the survivor was (28.7 ha), and its area, its perimeter and its texture are those of a
lake and not of a piece of one.

```tfcompare
caption: Drag the handle. The eight lake objects (white dots with a magenta ring, left) are one object (right). The colours are the classes of the same random forest, trained on the objects before the edits.
before: { src: /img/tutorials/object-based-image-analysis/editing-by-hand/merge_before.png, label: Before }
after: { src: /img/tutorials/object-based-image-analysis/editing-by-hand/merge_after.png, label: After the merge }
```

### The cut: two things, two objects

The object at the south edge of the lake held the first blocks of the town, 2,751 cells
(27.5 ha). The forest of the objects before the edit called all of it **urban**. The line along one row of cells
splits it into a piece above (738 cells) and a piece below (2,013 cells), and the same forest calls the upper piece **bare soil**, the lower one staying **urban**.

```tfcompare
caption: Drag the handle. The pink line (left) is the cut; the object has a piece above, now bare soil, and a piece below, still urban (right). The classes are those of the same forest on both.
before: { src: /img/tutorials/object-based-image-analysis/editing-by-hand/cut_before.png, label: Before }
after: { src: /img/tutorials/object-based-image-analysis/editing-by-hand/cut_after.png, label: After the cut }
```

The cut gave two objects, with no sliver this time: 738 cells above and 2,013 below. On another object it can give three, with a piece of a few cells at the end of the
line that the cut left apart: a cut by a straight line along a crooked edge leaves such slivers. Look for them, and delete
them into a neighbour as in the next edit.

### The delete: a stray object the forest could not place

The small object under the cross, 20 cells, had no neighbour like it, and the forest called it cerrado in the middle of
the town. With `into = "neighbour"` it is taken by the neighbour with the nearest band means, an urban object, and it is
urban now.

```tfcompare
caption: Drag the handle. The cross (left) marks the object that is deleted; its neighbour takes its cells (right).
before: { src: /img/tutorials/object-based-image-analysis/editing-by-hand/delete_before.png, label: Before }
after: { src: /img/tutorials/object-based-image-analysis/editing-by-hand/delete_after.png, label: After the delete }
```

`into = "none"` would have left a hole: cells with no object, no attributes and no class. That is what you want for a
thing that is not a landscape object at all (a cloud shadow you mask by hand, a no-data patch); for an object that
belongs to a neighbour, use the neighbour.

### What the edits do to the map

The three edits turn 725 objects into 718. What they do to the classes is told by two numbers, and the second is not the
one you would expect.

```tftable
data: /data/tutorials/object-based-image-analysis/editing-by-hand/changes.csv
labels: { what: "The forest", cells_changed_class: "Cells that changed class", overall_objects: "Overall accuracy" }
align: { what: left }
digits: { cells_changed_class: 0, overall_objects: 3 }
format: { overall_objects: "{v}" }
caption: "The map after the edits against the map before, in two ways. Of 336,400 cells, the same forest changes 758 on the edited objects; a forest trained again on the edited objects changes 4,053. The accuracy is that of the 67 reference points."
```

The first row is the effect of the edits alone: the forest, trained on the objects as they were, is applied to the
edited objects. 758 cells (0.2 %) change class, and the accuracy is the same as before. The second row is what you get
if you **run the model again**, as you would: the training points are resolved to objects again, and one of
them has changed. The water of the training set was 13 objects of 7,140 cells, and is now 11 objects of 9,968, because two
training points were on lake objects that are one now. The forest is trained on a slightly different table, a forest
of 87 samples is sensitive to that, and 4,053 cells (1.2 %) change class, well away from the edits.

```tftable
data: /data/tutorials/object-based-image-analysis/editing-by-hand/training_counts.csv
labels: { class: Class, objects_before: "Training objects before", objects_after: "After the edits" }
align: { class: left }
digits: 0
caption: "What objects.samples picked before and after the edits. The merge put two water points on one object."
```

The accuracy against the 67 reference points goes from 76.1 % (51 objects) to 74.6 % (50). That is one object, and the
interval is of about plus or minus 12 points, so **it says nothing about whether the edits helped**; the matrix
shows where it went: after the second training the forest calls one more urban reference object forest (the bare soil class has
7 training objects). The lesson is not about these edits. It is that **anything you change upstream of the training
changes the training**, and a classifier of a few dozen samples needs to be looked at again when it does. Look at the
training counts after an edit that merges objects, and at the classes, not only at the edited objects.

```tftable
data: /data/tutorials/object-based-image-analysis/editing-by-hand/accuracy.csv
labels: { model: Model, objects: Objects, sampled: "Reference objects", overall_objects: "Overall accuracy", overall_adjusted: "Area-adjusted", kappa: Kappa }
align: { model: left }
digits: { objects: 0, sampled: 0, overall_objects: 3, overall_adjusted: 3, kappa: 3 }
caption: "The accuracy against the 67 reference points, with and without the edits (the forest trained on each set of objects)."
```

## Edits and other segmentations

The edits are places, so the same list can be run on another segmentation. Move the slider: the **same three edits**, as they
are written above, on the scales 3, 4, 6 and 8 of the multiresolution and on SLIC superpixels ([[segment.slic]], `size = 25`: about 25 cells on a side).

```tfslider
label: Segmentation
caption: "The same three edits on five segmentations of the same part of the clip (outlines in yellow). White dots with a magenta ring: the merge; the pink line: the cut; the cross: the delete. The numbers are objects before and after the edits, and the cells each edit moved."
frames:
  - { value: 1, src: /img/tutorials/object-based-image-analysis/editing-by-hand/on_scale3.png, caption: "Multiresolution, scale 3", stats: { Objects: "1,338 to 1,334", Merge: "partly (1,846 cells)", Cut: "done (443)", Delete: "done (20)" } }
  - { value: 2, src: /img/tutorials/object-based-image-analysis/editing-by-hand/on_scale4.png, caption: "Multiresolution, scale 4", stats: { Objects: "725 to 718", Merge: "done (2,866)", Cut: "done (2,013)", Delete: "done (20)" } }
  - { value: 3, src: /img/tutorials/object-based-image-analysis/editing-by-hand/on_scale6.png, caption: "Multiresolution, scale 6", stats: { Objects: "308 to 304", Merge: "done (3,154)", Cut: "done (368)", Delete: "done (147)" } }
  - { value: 4, src: /img/tutorials/object-based-image-analysis/editing-by-hand/on_scale8.png, caption: "Multiresolution, scale 8", stats: { Objects: "175 to 172", Merge: "done (1,473)", Cut: "no split (0)", Delete: "done (147)" } }
  - { value: 5, src: /img/tutorials/object-based-image-analysis/editing-by-hand/on_SLIC.png, caption: "SLIC, size 25", stats: { Objects: "491 to 485", Merge: "done (3,864)", Cut: "done (439)", Delete: "done (988)" } }
```

```tftable
data: /data/tutorials/object-based-image-analysis/editing-by-hand/other_segmentations.csv
columns: [segmentation, objects, objects_after, merge_status, merge_cells, cut_status, cut_cells, delete_status, delete_cells]
labels: { segmentation: Segmentation, objects: Objects, objects_after: "After", merge_status: Merge, merge_cells: Cells, cut_status: Cut, cut_cells: Cells, delete_status: Delete, delete_cells: Cells }
align: { segmentation: left, merge_status: left, cut_status: left, delete_status: left }
digits: 0
badge:
  merge_status: { done: good, partly: warning }
  cut_status: { done: good, partly: warning, "no split": warning }
  delete_status: { done: good, partly: warning }
caption: "The report of the node edit on each segmentation: the edits found their objects every time, but what they did is not the same."
```

All the edits found an object on every segmentation, which is what the design is for. What each did is another matter:

- **The merge** joined the lake at every scale, but at the scale 3 it is `partly`: some of the objects at the eight points
  do not touch the first one at that scale, where the objects are smaller, so Terraflux left them apart and warned. It
  moved 1,846 cells and not 2,866.
- **The cut** is a line of 41 cells along a row, drawn for one object of the scale 4. At the scale 3 it moved 443 cells, at
  the scale 6 368, and at the scale 8 nothing: the line separates no object there (`no split`). With SLIC it moved 439.
- **The delete** is the dangerous one. A point on a 20-cell object is on a 147-cell object at the scales 6 and 8, and on a
  988-cell object with SLIC: the same edit deleted **a whole object** each time, taken by its neighbour. It said `done`.

An edit that says `done` has done what it was told, not what you meant. After a change of the segmentation, **look at
every edit again** on the Viewer, or remove the ones that no longer fit: edit the setting of the node (the list is
readable) or delete the node.

## Settings that change the answer

- **`into`** (delete): `"neighbour"` gives the cells to the most similar neighbour (with the `image`: the nearest band
  means; without it, the longest shared border); `"none"` leaves cells with no object. With `"none"` the classes have a
  hole there and the map has no value on those cells.
- **`neighbours`** (4 or 8): which cells touch. With 4 (the default for objects whole with 4) an object that touches
  another only by a corner is not touching it, and a merge of the two is `not touching`.
- **`image`**: optional. With it, the node gives the mean of each band of the edited objects in its table, and a delete
  into a neighbour uses them. Connect it to the image of the segmenter (the application does).
- **The place of the node.** It goes right after the segmenter. Put it after [[objects.features]] and the classes are
  lost: the edited objects come out with no classes and no level above them.

Another way to fix the same objects, instead of editing them, is a smaller scale. Here it is, on the same reference
points:

```tftable
data: /data/tutorials/object-based-image-analysis/editing-by-hand/alternatives.csv
labels: { choice: "Segmentation", objects: Objects, sampled: "Reference objects", overall_objects: "Overall accuracy", overall_adjusted: "Area-adjusted", kappa: Kappa }
align: { choice: left }
digits: { objects: 0, sampled: 0, overall_objects: 3, overall_adjusted: 3, kappa: 3 }
caption: "The accuracy of the same random forest on the same reference points with other objects. The scale 3 and 2 have more objects (1,338 and 2,973) and the same reference points land on fewer distinct objects (62 and 60)."
```

A smaller scale makes the objects smaller everywhere: 2,973 objects at the scale 2, four times as many as at 4, and it
would also cut the object between the lake and the town. It would **also** break the lake into more pieces, and the
accuracy of 67 or 60 points cannot tell the four apart. Three edits cost three objects' worth of attention. A smaller
scale costs everything.

## Common pitfalls

- **Editing after the classes.** The edit node goes after the segmenter. If you add it after the features or the
  classification, the edited objects have none of them. In the application, the Edit block puts it in the right place and
  rewires the nodes that read the objects.
- **Trusting `done`.** See the delete above. The report says what was done, not whether it was a good idea on this
  segmentation.
- **A merge of objects that do not touch.** They stay apart (`not touching`, or `partly`). Merging two lakes in one object
  is not possible: an object is a connected patch of cells.
- **A cut that leaves slivers.** A line along a crooked edge leaves pieces of a few cells. Delete them into the
  neighbour.
- **A cut line that goes beyond the object.** The line cuts every object it crosses. Draw it from the edge of one object
  to the edge of the same object. Where it crosses others, they are split too, if the line separates them.
- **Changing the training without noticing.** A merge or a cut that moves a training point changes the training table
  (water was 11 objects and is 10). Read the counts of [[objects.samples]] after an edit.
- **Forgetting that the points are in the objects' coordinate system.** If the objects come from an image in UTM and the
  points are in degrees, the edit finds nothing (`no object`).
- **Editing instead of segmenting again.** A hundred edits are a sign that the segmentation is not the one you need.
- **Reproducibility.** The edits are in the model, so a colleague who runs it gets the same objects; but if you edit the
  *objects of a file* in a GIS afterwards, that is not in the model. Edit in the model.

## Going further

- Why the objects are the size they are, and why two things end up in one: [Choosing the scale](/tutorials/object-based-image-analysis/choosing-the-scale/) (part 3).
- Training and checking a classifier, and what the accuracy numbers mean: [Classifying by samples](/tutorials/object-based-image-analysis/classifying-by-samples/) (part 5).
- Merge and tidy objects by rule instead of by hand ([[objects.merge]], [[objects.grow]], [[objects.remove_small]]): [Levels](/tutorials/object-based-image-analysis/levels/) (part 8).
- Objects that change between two dates: [Change by objects](/tutorials/object-based-image-analysis/change-by-objects/) (part 11).
- The operator: [[objects.edit]].

## References

- Blaschke, T. (2010). Object based image analysis for remote sensing. *ISPRS Journal of Photogrammetry and Remote
  Sensing*, 65(1), 2-16. https://doi.org/10.1016/j.isprsjprs.2009.06.004
- Ma, L., Li, M., Ma, X., Cheng, L., Du, P., & Liu, Y. (2017). A review of supervised object-based land-cover image
  classification. *ISPRS Journal of Photogrammetry and Remote Sensing*, 130, 277-293.
  https://doi.org/10.1016/j.isprsjprs.2017.06.001
- Baatz, M., & Schäpe, A. (2000). Multiresolution segmentation: an optimization approach for high quality multi-scale
  image segmentation. In *Angewandte Geographische Informationsverarbeitung XII* (pp. 12-23). Wichmann.
