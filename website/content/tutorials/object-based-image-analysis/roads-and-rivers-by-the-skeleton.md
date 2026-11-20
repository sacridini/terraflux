---
title: "Roads and rivers by the skeleton: long thin objects"
short: Roads and rivers
description: "Tell streets, rivers, hedgerows and canals from compact objects of the same area and the same colour by the skeleton of each object (main length, width, length over width, branches), on 0.6 m aerial imagery and on Sentinel-2, with the template Roads and rivers by the skeleton."
order: 190
level: advanced
time: 60 min
template: Roads and rivers by the skeleton
ops: [raster.read, raster.calc, segment.threshold, objects.features, objects.rules]
series: Object-based image analysis
part: 19
---

A street and a roof can be the same colour and the same size. What tells them apart is their **shape**: the street is long and
thin, the roof is not. This tutorial measures that shape with the **skeleton** of each object, the line that runs along its
middle. You will cut the bright surfaces of a Texas suburb into objects, read the length, width and branches of
their skeletons, and use two of them to separate streets from roofs. Then you do the same with the green ribbons of
the creeks around Lago Paranoá, in Brasília, where the objects are a thousand times larger. You end with the numbers, a rule that
reads them, and a clear idea of when the skeleton is a good measure and when it is not.

## The idea

Take a shape and wear it away from the outside, a layer of cells at a time, without ever breaking it in two. What is left, when
nothing more can be taken, is a thin line: the **skeleton**, or medial axis, of the shape. A bar leaves a line along its length.
A disc leaves a point. A cross leaves a cross, a ring leaves a ring. The line says how long the shape is, and the area
divided by that length says how wide it is, so a shape of the same area has a very different skeleton when it is a bar and when it is a disc.

![Nine shapes of the same area and their skeletons](/img/tutorials/object-based-image-analysis/roads-and-rivers-by-the-skeleton/toy.png "Nine shapes of about 400 m² on a grid of 1 m cells, with the length and width Terraflux measured for each. The dark lines are an illustration: a thinning of the same cells made in Python (skimage), without Terraflux's clean-up. Terraflux gives the numbers of the skeleton, not its cells.")

The skeleton group of [[objects.features]] gives, for every object:

| Column | What it is |
|---|---|
| `skel_main_length` | The length of the **main line**: the longest path along the skeleton, in map units (a diagonal step counts \( \sqrt{2} \) cells) |
| `skel_width` | The area divided by the main length: the mean width |
| `skel_length_width` | The main length over the width |
| `skel_branches` | The segments of the skeleton between ends and junctions |
| `skel_ends`, `skel_junctions` | The cells where a line stops and the cells where three or more lines meet |
| `skel_branch_length` | The mean length of a branch |
| `skel_curvature` | The turning of the main line, in radians per map unit of its length (the line simplified to one cell first, the angles summed at its vertices) |
| `skel_curvature_sd` | The spread of the turning per unit length at the vertices of the main line |

!!! science "Key idea: the ratio does not depend on the size"
    A road that is 5 m wide and 1 km long and a road that is 5 m wide and 100 m long have areas that differ ten times,
    and the **same width**. The ratio of the main length to the width is large for both, and a disc has none at all. The skeleton gives a shape descriptor
    that does not depend on the area, which is why one limit on it can separate streets from roofs of all sizes. The thinning is
    Zhang and Suen's (1984), one of the family surveyed by Lam et al. (1992); Terraflux applies it to the cells of each object, prunes the
    short branches (the `spur` setting, below) and measures the result.

This is eCognition's family of skeleton features (main line length, width, curvature, number of branches), with one difference
that matters: eCognition takes the skeleton of the object's **polygon**; Terraflux takes it of the object's **raster cells**. The
two are close and not equal (the last section).

## When to use it

- **Linear features among compact ones of the same class**: roads against buildings, rivers against lakes, hedgerows and tree lines
  against woods, canals and ditches against ponds, firebreaks and pipeline clearings, gallery forests against forest patches.
  Automatic road extraction from imagery is reviewed by Mena (2003).
- **A shape that is not a size**: if all that separates two classes is area, [[objects.rules]] on `area` is enough. If a long
  creek and a round pond have the same area, the skeleton is the tool.
- **After** a classification into the cover that has both (water, pavement, vegetation), as here, or on a mask.
- Not for objects with a **hole or a branching network** as the thing you want to measure: the main line of a network is one path
  through it, and its width (area over that length) overstates the width of any one street (the Sentinel-2 section shows it; the reference of [[objects.features]] says so too).
- Not when the objects are **a few cells wide**: an object of one or two cells has a skeleton as long as itself and no width.
  The cell must be several times smaller than the width of the thing.
- Not a substitute for a road **vector** when you have one. Use the skeleton when all you have is the image.

## What you need

- The NAIP clip of the series: 600 m of Plano, Texas, 0.6 m cells, bands red, green, blue and nir (9 November 2024;
  [naip_plano.tif](/data/tutorials/object-based-image-analysis/naip_plano.tif) to download).
- The Sentinel-2 clip of Lago Paranoá (10 m cells, five bands, 9 August 2024;
  [sentinel2_brasilia.tif](/data/tutorials/object-based-image-analysis/sentinel2_brasilia.tif)), for the second case. Their attribution is in
  [ATTRIBUTION.txt](/data/tutorials/object-based-image-analysis/ATTRIBUTION.txt).
- A mask or a cover whose objects you want to measure. Here the bright surfaces (NAIP) and the very green cells (Sentinel-2).
- The template **Roads and rivers by the skeleton** (group *Objects (OBIA)*). It runs on a synthetic land cover with a river, a road, lakes and
  buildings, and **File > Use my data...** puts yours in place of it (water as 1, vegetation as 2 and built land as 3, or move the codes in the rules).
- The toy of the figure, [shapes.tif](/data/tutorials/object-based-image-analysis/roads-and-rivers-by-the-skeleton/shapes.tif), nine shapes on a grid of 1 m cells, to try the settings.

## Build the model

### Step 1: the objects

The skeleton is of **an object**, so the first job is to make objects whose shape is the shape you want to measure. Here the thing is
a bright surface: concrete, light roofs, tennis courts. A cell is bright when the mean of red, green and blue is above 150 (of 255) and
it is not green: NDVI under 0.15. [[raster.calc]] writes 1 for those cells and 0 for the others, and
[[segment.threshold]], cutting at 0.5, makes an object of every connected piece of each: the pieces of 1 are the bright
surfaces. `neighbours = 8` joins pieces that touch by a corner.

![Bright surfaces in the NAIP clip](/img/tutorials/object-based-image-analysis/roads-and-rivers-by-the-skeleton/light.png "The cells that are bright and not green (yellow) over the NAIP clip. Streets, drives, light roofs and a few courts all have the same colour.")

Streets, drives and the lighter roofs are one yellow. They are **one class of surface**, and nothing in the colour tells them apart: the
shape will. The roofs dark grey or brown are not in this mask, which is fine, because they are not what we measure.

```tfgraph
#! caption: Read the clip, mark the bright cells that are not vegetation, and make an object of every connected piece.
#! highlight: light, pieces
#! numbered: true
#! fragment: true
[[nodes]]
id = "naip"
op = "raster.read"
params = { path = "naip_plano.tif" }
note = "The NAIP clip: red, green, blue and nir, 0.6 m."

[[nodes]]
id = "light"
op = "raster.calc"
inputs = { s = "naip.raster" }
params = { expression = "out light = 1 if ((s.red + s.green + s.blue) / 3 > 150 and (s.nir - s.red) / (s.nir + s.red) < 0.15) else 0" }
note = "1 where the cell is bright and not green."

[[nodes]]
id = "pieces"
op = "segment.threshold"
inputs = { image = "light.light" }
params = { thresholds = [0.5], neighbours = 8 }
note = "Two classes: 0 and 1. Every connected piece is an object."
```

### Step 2: the skeleton

[[objects.features]] reads the objects and an image and adds the attributes: here `area` and the group `skeleton`. The group is not in the default set
(`basic`), so you ask for it by name (`all` includes it too). The only setting of the group is **`spur`**: branches that end in a junction
after fewer cells than `spur` are cut off (default 3; 0 keeps them all). Thinning leaves small stubs where an outline is ragged, and
they would count as branches.

The columns come out for every object. The `class` column of [[segment.threshold]] (1 or 2) is still there, and is how the rules
know a bright object (class 2) from the rest.

### Step 3: the rules

[[objects.rules]] gives each object the class of the rule it fits best. The three of this model:

- **linear**: bright, a main line of at least 40 to 60 m, and a length over width of 8 to 12 or more.
- **compact**: bright, an area of 30 to 60 m² or more, and a length over width under 8 to 12.
- **small**: bright and under 30 to 60 m². A speck of pavement or a fleck of shade is not a street.

Each rule is a **fuzzy** membership: `above(skel_length_width, 8, 12)` is 0 below 8, 1 above 12 and rises in between, so an object that
is borderline in the length over the width is half linear and half compact, and the class is the one with the largest membership. The rules
are limits for this image: another resolution needs others.

!!! warning "A missing value is 0"
    A square or a disc has a skeleton of one cell, so its main length is 0 and its `skel_width` and `skel_length_width` are **missing** (the toy above; the reference of [[objects.features]] has a guard for it). A rule
    reads a missing value as 0, so `below(skel_length_width, 8, 12)` is **0 for a perfectly compact object**, and it stays unclassified. On this image only 5 of
    1,726 bright objects had no class, but write the compact side with a column that is never missing, such as `area` or `skel_main_length`, when your objects can be round.

```tfgraph
#! caption: The whole model: the bright surfaces, their skeletons and the rules that tell long and thin from compact.
#! highlight: features, rules
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "naip"
op = "raster.read"
params = { path = "naip_plano.tif" }
note = "The NAIP clip: red, green, blue and nir, 0.6 m."

[[nodes]]
id = "light"
op = "raster.calc"
inputs = { s = "naip.raster" }
params = { expression = "out light = 1 if ((s.red + s.green + s.blue) / 3 > 150 and (s.nir - s.red) / (s.nir + s.red) < 0.15) else 0" }
note = "1 where the cell is bright and not green."

[[nodes]]
id = "pieces"
op = "segment.threshold"
inputs = { image = "light.light" }
params = { thresholds = [0.5], neighbours = 8 }
note = "The objects: the connected pieces of each class."

[[nodes]]
id = "features"
op = "objects.features"
inputs = { objects = "pieces.objects", image = "light.light" }
params = { features = ["area", "skeleton"], spur = 3 }
note = "The area and the skeleton: main length, width, length over width, branches, curvature."
refs = ["Zhang and Suen 1984, A fast parallel algorithm for thinning digital patterns"]

[[nodes]]
id = "rules"
op = "objects.rules"
inputs = { objects = "features.objects" }
note = "Bright objects: long and thin is linear, otherwise compact; the small ones are specks."

[nodes.params]
classes = [
  { name = "linear", rule = "above(class, 1, 2) and above(skel_main_length, 40, 60) and above(skel_length_width, 8, 12)", colour = "#2b6cb0" },
  { name = "compact", rule = "above(class, 1, 2) and above(area, 30, 60) and below(skel_length_width, 8, 12)", colour = "#c0504d" },
  { name = "small", rule = "above(class, 1, 2) and below(area, 30, 60)", colour = "#8f8f8f" },
]
```

In the application, open the template, or paste the model into a new graph (the application reads the file's TOML), and run ++ctrl+enter++ on
`rules`; the Viewer shows the objects with the colours of the classes. From the command line, with `naip_plano.tif` beside the model:

```bash
terraflux run roads.toml --out out
```

## Read the results

![Linear and compact bright surfaces](/img/tutorials/object-based-image-analysis/roads-and-rivers-by-the-skeleton/classes.png "The bright surfaces of the clip by their shape: linear (blue), compact (red) and small (grey). The yellow square is the detail below.")

```tftable
data: /data/tutorials/object-based-image-analysis/roads-and-rivers-by-the-skeleton/classes.csv
labels: { class: Class, objects: Objects, area_ha: "Area (ha)" }
align: { class: left }
digits: { objects: 0, area_ha: 2 }
caption: "The 1,726 bright objects of the clip. The bright surfaces are 6.7 ha, 19 % of the 36 ha."
```

The 31 linear objects are the streets and the long drives (2.7 ha), the 174 compact ones the roofs, courts and wide patches (3.3 ha), and
there are 1,516 small ones. This is a map judged by eye: no street was digitised, so the page has no accuracy. To make one, take a reference
sample as in [part 5](/tutorials/object-based-image-analysis/classifying-by-samples/).

### What the skeleton sees

![A street and houses with their skeletons](/img/tutorials/object-based-image-analysis/roads-and-rivers-by-the-skeleton/detail.png "A 90 m detail of the clip: a curved street (blue) beside a park, drives and light roofs (red). The white dots are an illustrative thinning of each object's cells (skimage, made in Python), not Terraflux's skeleton.")

The street's line runs along the street. The drives and the edges of the street give it small branches: the road is wider than a line, and
the skeleton sends a stub to every bulge. The roofs have a short, thick skeleton.

The case that matters is objects of **similar area**. Between 500 and 1,500 m² the clip has 11 linear and 10 compact objects:

```tftable
data: /data/tutorials/object-based-image-analysis/roads-and-rivers-by-the-skeleton/same_area.csv
labels: { class: Class, objects: Objects, measure: Measure, median: Median, q1: "First quartile", q3: "Third quartile" }
align: { class: left, measure: left }
digits: { objects: 0, median: 1, q1: 1, q3: 1 }
caption: "The bright objects of 500 to 1,500 m², by class: the median and the quartiles of each measure. The areas are close, the main lengths are not, and the number of branches does not tell them apart."
```

Their areas are close (median 879 and 583 m²), their widths are close (7.4 and 8.8 m), and the **main length** (122 m against 67 m) and the **length over the width** (17 against 8) separate them. The
number of branches does not: 38 against 41, because ragged edges make branches on any object. So **do not use the count of branches to tell
a line from a blob**; use it for networks (below).

![Area against length over width](/img/tutorials/object-based-image-analysis/roads-and-rivers-by-the-skeleton/scatter.png "Every bright object with a defined length over width, by area (log scale) and length over width. The three classes overlap in area from about 100 to 2,000 m² and separate in length over width. The band is the ramp of the rules.")

The scatter says why area alone would not do: between 100 and 2,000 m² both classes are there. The largest linear objects are the long streets:

```tftable
data: /data/tutorials/object-based-image-analysis/roads-and-rivers-by-the-skeleton/linear_top.csv
labels: { id: Object, area_m2: "Area (m²)", main_length_m: "Main length (m)", width_m: "Width (m)", length_width: "Length / width", branches: Branches, junctions: Junctions }
digits: 0
caption: "The eight largest linear objects. The widths are 8 to 12 m (a street with its kerb and the drives that join it); the branches and junctions are mostly the edges and the drives."
```

For a **paper**: report the rule (the column, the limits and the ramps), the cell size, the area and the number of objects of each
class, and a reference sample with its accuracy. The skeleton's numbers depend on the cell size and on the mask, so give the mask too.

## Settings that change the answer

### spur: the stubs

Thinning a ragged shape leaves **stubs**: short branches that grow to every corner of the outline. `spur` cuts the branches that end in a
junction after fewer cells than its value. The toy has a bar with five notches in its edge:

```tftable
data: /data/tutorials/object-based-image-analysis/roads-and-rivers-by-the-skeleton/toy_spur.csv
labels: { spur: "spur (cells)", branches: Branches, ends: Ends, junctions: Junctions, main_length_m: "Main length (m)" }
digits: { spur: 0, branches: 0, ends: 0, junctions: 0, main_length_m: 1 }
caption: "The ragged bar of the toy with other values of spur. The main line is the same; the stubs go."
```

With `spur = 0` the bar has 5 branches and 2 junctions where it is one line; at the default 3 it still has 3, and at 5 it is a single line. The main length does not change,
because the main line is the longest path and the stubs are never on it. The **branch counts** do, so the right spur depends on how much
you read them. On the NAIP clip:

```tfchart
type: line
data: /data/tutorials/object-based-image-analysis/roads-and-rivers-by-the-skeleton/spur.csv
x: spur
y: [branches_of_the_biggest, median_branches]
labels: { branches_of_the_biggest: "Branches of the largest object (a street network)", median_branches: "Median branches of the bright objects of 60 m² or more" }
xlabel: spur (cells)
ylabel: Branches
height: 300
caption: "The branches of the largest bright object (96 at the default spur 3, 407 m of main line) and the median of the bright objects of 60 m² or more, for six values of spur (0.6 m cells)."
```

The largest object has 98 branches at `spur = 0`, 96 at the default and 38 at 20: most of its branches are stubs of its edges, and the real
side streets are the ones that survive. Spur also has a side effect on the classes: the same rules give 31 linear objects at spurs of 0 to 6, 30 at 10 and 24 at 20,
and 174, 176 and 184 compact ones, because a long spur also cuts the ends of short lines. Choose it in cells, from the stubs you see, and keep it
small (the default 3 is 1.8 m here, 30 m on a Sentinel-2 cell).

### The limits and the ramps

The classes of the rules are the three limits (40 to 60 m for the main length, 8 to 12 for the length over the width, 30 to 60 m² for the area). The
scatter shows the ramp in orange: the objects inside it are the ones that move when you move the limit. Read the scatter of your own objects before you
set them, and look at the objects inside the ramp one by one in the Viewer. Many of the 'wrong' ones are really mixed: a drive joined to a roof is an
object that is half of each.

### Curvature: what it measures

`skel_curvature` is the turning of the main line, in radians, over its length. The main line is first **simplified** (Douglas and Peucker, 1973, at a tolerance of one cell), so the staircase of cells is not counted as turning, and the angles are summed at the vertices of what is left. On the toy:

- a straight bar, along the grid or **tilted by 25°**: **0** (a tilted bar used to read 0.085 in an earlier build, from its staircase);
- an L with one 90° turn over 98 m: **0.016**, which is \( (\pi/2) / 98.4 \);
- a quarter circle of radius 64 m: **0.019**, against \( 1/64 = 0.016 \), a little over;
- a ring of 16 m radius: **0.066**, against \( 1/16 = 0.0625 \).

So for **clean shapes** curvature now tells a straight line from a bend, and its value is close to one over the radius. On **real objects** it does not separate anything yet. The 31 linear objects of the NAIP clip have a curvature between 0.04 and 0.32 rad/m, median 0.15, and the largest street (407 m of main line) 0.091. A street with a radius of 50 m would read 0.02. The reason is the object: a street 8 m wide with ragged edges and drives gives a main line that wanders from side to side by a few cells, and every wander is a real turn of the line. For the 7 gallery forests of the Sentinel-2 clip the values are between 0.0056 and 0.0096 rad/m (0.06 to 0.1 rad per 10 m cell), all alike. Use `skel_curvature` to compare objects of the same kind on the same grid, and do not read it as the bend of a road unless you have looked at the main lines. A curve you can see on the map but that is not in the number is the usual sign that the object is too ragged: clean the mask first (a majority filter, the opening of [[objects.resize]]) and compute it again.

### Networks: branches and junctions

An object that is a **network** (the streets of a quarter, the creeks of a basin) has many branches and junctions and a main line that is
only one path through it. The main length is the longest way across it and the width (area over that length) is **not a width any more**. The Sentinel-2 section shows
a network of 193 ha with a "width" of 393 m. For a network use `skel_branches`, `skel_junctions` and `skel_branch_length` (their mean length),
and the area; for the width of one reach, cut the network first (for example with [[objects.split]]).

## Another scale: the gallery forests of Lago Paranoá

The same idea at 10 m. In the Sentinel-2 clip of the series (the south edge of Lago Paranoá) the creeks run through the dry cerrado with a dark green
ribbon of **gallery forest** along each. Cells with an NDVI above 0.65 are mostly those ribbons and the thick forest, and a long ribbon of forest is a different thing, ecologically (a corridor, a riparian
buffer), from a patch of the same area.

```tfgraph
#! caption: The same model on the Sentinel-2 clip: the very green cells, their skeletons and rules on the area and the length over width.
#! highlight: green, features
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "s2"
op = "raster.read"
params = { path = "sentinel2_brasilia.tif" }
note = "Sentinel-2: blue, green, red, nir and swir16, reflectance times 10 000, 10 m cells."

[[nodes]]
id = "green"
op = "raster.calc"
inputs = { s = "s2.raster" }
params = { expression = "out green = 1 if (s.nir - s.red) / (s.nir + s.red) > 0.65 else 0" }
note = "1 where the NDVI is above 0.65."

[[nodes]]
id = "pieces"
op = "segment.threshold"
inputs = { image = "green.green" }
params = { thresholds = [0.5], neighbours = 8 }
note = "The connected pieces of each class are the objects."

[[nodes]]
id = "features"
op = "objects.features"
inputs = { objects = "pieces.objects", image = "green.green" }
params = { features = ["area", "skeleton"] }
note = "Area and skeleton of the green objects."

[[nodes]]
id = "rules"
op = "objects.rules"
inputs = { objects = "features.objects" }
note = "Large green objects: long and thin is gallery forest, otherwise a forest patch."

[nodes.params]
classes = [
  { name = "gallery forest", rule = "above(class, 1, 2) and above(area, 20000, 40000) and above(skel_length_width, 8, 14)", colour = "#2b6cb0" },
  { name = "forest patch", rule = "above(class, 1, 2) and above(area, 20000, 40000) and below(skel_length_width, 8, 14)", colour = "#c0504d" },
  { name = "small", rule = "above(class, 1, 2) and below(area, 20000, 40000)", colour = "#8f8f8f" },
]
```

![Gallery forests of the Sentinel-2 clip](/img/tutorials/object-based-image-analysis/roads-and-rivers-by-the-skeleton/s2_classes.png "Green objects by their shape: gallery forest (blue, with an illustrative thinning in white), forest patch (red) and small objects (grey). The clip is 5.8 km wide.")

```tftable
data: /data/tutorials/object-based-image-analysis/roads-and-rivers-by-the-skeleton/s2_top.csv
labels: { id: Object, class: Class, area_ha: "Area (ha)", main_length_m: "Main length (m)", width_m: "Width (m)", length_width: "Length / width", branches: Branches, junctions: Junctions }
align: { class: left }
digits: { id: 0, area_ha: 1, main_length_m: 0, width_m: 1, length_width: 1, branches: 0, junctions: 0 }
caption: "The six largest green objects. The first is the creek network of the south-east of the clip: 193 ha, a main line of 4.9 km, 201 branches."
```

There are 7 gallery forests (275 ha), 7 forest patches (101 ha) and 864 small green objects (92 ha). The largest object is a **network**: a main line of 4.9 km, 201 branches and 113 junctions, and
a "width" of 393 m that no creek has. Its length over width, 12.5, is above the ramp, so the rule calls it gallery forest, which is right, but for the wrong measure.
The second (a patch of 58 ha, length over width 8.3, 70 branches) is a thick patch of forest on the east side. The objects of the middle, such as the 20 ha ribbon of 2.3 km and a
"width" of 87 m, are the typical gallery forests. As in the NAIP case there is no reference: these are the objects the rule calls long and thin, and they look right on the image.

## How this skeleton differs from eCognition's

eCognition's skeleton features are of the object's **polygon**, a continuous outline. Terraflux's are of the **raster cells**. The differences you can expect, from how each is made:

- The raster line is a path through **cell centres**, 1 or \( \sqrt{2} \) cells a step: a diagonal line is a staircase, and its length is that of the path. The polygon's line is smooth.
- The raster line stops half a width short of the end of a bar: the toy bar is 100 m long and has a main length of 95 m. The shorter the object, the larger the share.
- An object a cell or two wide has no usable skeleton on a raster; a polygon can be as thin as you like.
- The thinning has a few choices (the clean-up of staircase steps, `spur`) that a polygon skeleton does not have.
- Curvature is measured on a simplified main line, and a wandering line on a ragged object still reads as curved (above).

For the same object, the values differ a little; I have not compared them with eCognition's own on the same polygons, so the limits of a rule set built there need to be **calibrated again** here on a sample of
objects, not copied.

## Common pitfalls

- **Measuring a network as a line.** The width of a network is area over the length of one path (the reference of [[objects.features]] says it is inflated for networks): use branches and junctions.
- **Trusting the number of branches.** Ragged outlines make stubs. Set `spur` and compare with the toy.
- **Round objects with no value.** A square or a disc has no `skel_width` or `skel_length_width`; a rule that reads them gives 0 and leaves the object unclassified.
- **Copying limits from another image.** The main length and the area are in map units; the cell size and the mask change every number. Read the scatter first.
- **A mask that glues things.** A drive joins a roof to the street: they are one object, and its skeleton is a street with a branch. Clean the mask with the opening of
  [[objects.resize]] or cut the object ([[objects.split]]) when the glue is a thin neck.
- **A cell that is too coarse.** A road 2 cells wide is a line with no width; the skeleton is for things at least a few cells wide.
- **Reading curvature as a bend** on a ragged object. See above.
- **No accuracy.** A rule that looks right on the map is a hypothesis: take a reference sample.

## Going further

- The operators: [[objects.features]] (the group `skeleton`, `spur`), [[objects.rules]], [[segment.threshold]], [[raster.calc]].
- Tell crowns apart in a canopy, the other tools of this family: [Separate tree crowns](/tutorials/object-based-image-analysis/tree-crowns/) (part 18).
- The skeleton among the other shape attributes, and the rest of the rules: [Describing objects](/tutorials/object-based-image-analysis/describing-objects/) (part 4) and
  [Rules and class trees](/tutorials/object-based-image-analysis/rules-and-class-trees/) (part 6).
- The same skeleton, used with the split on a whole map: [Urban cover of a neighbourhood from NAIP](/tutorials/object-based-image-analysis/case-urban-cover-naip/) (part 14).
- What the other tools of eCognition become in Terraflux: [Coming from eCognition](/tutorials/object-based-image-analysis/from-ecognition/).

## References

- Douglas, D. H., & Peucker, T. K. (1973). Algorithms for the reduction of the number of points required to represent a digitized line or its
  caricature. *Cartographica*, 10(2), 112-122. https://doi.org/10.3138/FM57-6770-U75U-7727
- Lam, L., Lee, S.-W., & Suen, C. Y. (1992). Thinning methodologies: a comprehensive survey. *IEEE Transactions on Pattern Analysis and
  Machine Intelligence*, 14(9), 869-885. https://doi.org/10.1109/34.161346
- Mena, J. B. (2003). State of the art on automatic road extraction for GIS update: a novel classification. *Pattern Recognition Letters*,
  24(16), 3037-3058. https://doi.org/10.1016/S0167-8655(03)00164-8
- Zhang, T. Y., & Suen, C. Y. (1984). A fast parallel algorithm for thinning digital patterns. *Communications of the ACM*, 27(3), 236-239.
  https://doi.org/10.1145/357994.358023
