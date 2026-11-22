---
title: "Choosing the scale"
short: Choosing the scale
description: "Learn what too many and too few objects cost, find the scales worth trying with ESP, let segment.optimize score them by variance and Moran's I, and see why the lake, the town and the cerrado do not want the same scale."
order: 30
level: intermediate
time: 50 min
template: The scale chosen by itself
ops: [segment.scale, segment.optimize, segment.multiresolution, raster.calc, objects.to_vector, vector.write]
series: Object-based image analysis
part: 3
---

In the [previous tutorial](/tutorials/object-based-image-analysis/what-is-an-object/) you moved the scale of the
multiresolution method and watched the objects grow, but nothing told you which scale to stop at. Here you will let
the image help you decide. You will run two tools on the same Sentinel-2 clip of Lago Paranoá: **ESP**
([[segment.scale]]), which lists the scales at which the objects change from parts into wholes, and **USPO**
([[segment.optimize]]), which segments the image at many scales and scores each one. You will read their charts, apply
the winner to a segmenter in the application, and then check one more thing that no score shows: that the scale which
suits a lake is not the scale that suits a town. At the end you will have a defensible scale, a way to say why you
chose it in a paper, and a clear idea of when one scale is not enough.

## The idea

A segmenter makes a promise it cannot keep on its own: "objects of about the right size". The right size is not in the
image, it is in your question. The three pictures below are the same part of the town at three scales. Neither extreme
is wrong in itself; each is wrong for a question.

```tfgallery
columns: 3
caption: "The same town blocks at the multiresolution scales 1, 2.48 and 16 (shape 0.1, compactness 0.5). Too fine, about right for roofs and streets, too coarse."
images:
  - { src: /img/tutorials/object-based-image-analysis/choosing-the-scale/detail_fine.png, caption: "Scale 1: over-segmentation" }
  - { src: /img/tutorials/object-based-image-analysis/choosing-the-scale/detail_right.png, caption: "Scale 2.48: about right" }
  - { src: /img/tutorials/object-based-image-analysis/choosing-the-scale/detail_coarse.png, caption: "Scale 16: under-segmentation" }
```

- **Over-segmentation** (scale 1): a roof is cut into three or four pieces and the street into many more. Every piece
  is homogeneous, but a piece says almost nothing about the thing it belongs to. The noise is kept, the table of
  attributes has 11,303 rows for this clip, and each object looks very much like its neighbours.
- **Under-segmentation** (scale 16): the town is one huge object, with roofs, trees and streets inside it. This is the
  worse error of the two, because it cannot be mended later: an object that holds two classes will be given one of
  them, and half of it will be wrong. At the scale 16 the clip has 35 objects.
- **About right** (scale 2.48): an object is a roof, a patch of street or a tree, about the size of the things in the
  image and bigger than a pixel.

How do you find the middle without looking at every scale by eye? Two families of tools answer, and Terraflux has one of
each.

### Where it comes from

Both started as answers to the same complaint: in the first object-based programs the scale was chosen by trial and
error, which is slow and hard to defend. Espindola and colleagues (2006) proposed to score each candidate segmentation
by two measures that pull in opposite directions, how alike the pixels *inside* the objects are and how different the
objects are from their *neighbours*, and to take the scale where the two balance. The method is called **unsupervised
parameter optimisation**, USPO, because it needs no reference map; GRASS GIS has it as `i.segment.uspo`. Johnson and Xie
(2011) kept the two measures but combined them with an F-measure instead of a sum.

Drăguţ, Tiede and Levick (2010) took another route for the multiresolution method of Baatz and Schäpe (2000): they
segmented the image at a long series of scales, each made from the one below it, and followed how fast the average
heterogeneity inside the objects grew from one scale to the next. Where the growth jumps, the objects have just
turned from parts of things into whole things. Their tool is called **ESP**, *estimation of scale parameter*, and
Drăguţ and colleagues (2014) extended it to images with several bands (ESP2).

!!! science "Key idea"
    **ESP.** At each scale \( s \), the **local variance** \( LV(s) \) is the mean, over the objects, of the standard
    deviation of their cells (here averaged over the bands). The **rate of change** from the previous scale
    \( s^- \) is

    \[ ROC(s) = \frac{LV(s) - LV(s^-)}{LV(s^-)} \times 100. \]

    A scale where \( ROC \) is a peak, higher than at both neighbouring scales, is a scale where objects went from parts
    to wholes: ESP suggests the first three peaks.

    **USPO.** For each scale, the **variance** \( v \) is the variance of the cells inside their objects (the mean over
    the bands, each object counted by its cells): low when objects are homogeneous. **Moran's I** is

    \[ I = \frac{n}{W} \cdot \frac{\sum_i \sum_j w_{ij} (x_i - \bar{x})(x_j - \bar{x})}{\sum_i (x_i - \bar{x})^2}, \]

    where \( x_i \) is the mean of an object in one band, \( \bar{x} \) the mean of the \( x_i \), \( n \) the number of
    objects, \( w_{ij} = 1 \) when objects \( i \) and \( j \) touch (0 otherwise) and \( W = \sum_{ij} w_{ij} \). \( I \)
    is high when neighbouring objects are alike (a sign of over-segmentation) and low when they differ. Over the scales
    that were run, \( v \) and \( I \) are each **rescaled** to 1 for the lowest and 0 for the highest, and the best scale
    is the one with the highest sum of the two.

Both are guides, not answers, and the documentation of both operators says so. They tell you which scales deserve a
look; you decide by looking at the objects.

## When to use it

Use them when:

- you are about to classify or measure objects and need to **justify a scale** (in a thesis, a report, a paper) with
  something better than "it looked right";
- you start a new image, a new sensor or a new place, where the scale of another project does not carry over (a scale is
  in the units of the image, so it does not);
- you want a **starting point** and a range for the manual search.

They are **not** a replacement for looking. Neither knows what your classes are, and neither can say that two classes
sit in one object. Both are also **global**: they give one scale for the whole image. When the classes need different
scales (a lake and a town do, as the last sections show), the answer is levels, not a better score; part 8 of the series
builds them. If you have not yet read how the segmenters and their settings work, start with
[What an object is](/tutorials/object-based-image-analysis/what-is-an-object/).

## What you need

- **The image.** The same Sentinel-2 clip as in part 2: blue, green, red, near infrared and `swir16`, in UTM 23S at 10 m,
  as reflectance from 0 to 1 (the model divides by 10 000, as before; the scale of the multiresolution method means
  something different on other units). It is read by [[data.stac]], or you download
  [sentinel2_brasilia.tif](/data/tutorials/object-based-image-analysis/sentinel2_brasilia.tif) (2.3 MB) and read it with
  [[raster.read]]; its attribution is in [ATTRIBUTION.txt](/data/tutorials/object-based-image-analysis/ATTRIBUTION.txt).
- **A template to start from**, in the application: *The scale chosen by itself* (group *Objects*). It runs
  [[segment.optimize]] on a synthetic image; replace the image with yours as the template says.
- **A little patience**: ESP and USPO segment the image many times. On this clip with two threads, ESP takes less than a
  second and USPO about two seconds for ten scales; on a large image try a part first.

## Build the model

### Step 1: the image

The first two nodes are the ones of part 2: [[data.stac]] reads the scene onto square 10 m cells in UTM, and
[[raster.calc]] turns the integers into reflectance. They are shown here as a fragment; the whole model
is at the end of the section.

```tfgraph
#! caption: Steps 2 and 3 hang on the reflectance image: ESP and USPO both read the same stack (nodes 2 and 3 here).
#! highlight: esp, opt
#! numbered: true
#! fragment: true
[[nodes]]
id = "refl"
op = "raster.calc"
inputs = { s = "s2.raster" }
params = { expression = "out image = s / 10000" }
note = "Reflectance from 0 to 1 (the scene and its UTM grid come before, as in part 2)."

[[nodes]]
id = "esp"
op = "segment.scale"
inputs = { image = "refl.image" }
note = "The scales ESP suggests."

[[nodes]]
id = "opt"
op = "segment.optimize"
inputs = { image = "refl.image" }
params = { values = [1, 1.5, 2, 3, 4, 6, 8, 12, 16, 24] }
note = "Ten scales, each scored."
```

### Step 2: ESP, the scales worth trying

Add [[segment.scale]] and connect `refl.image` to its `image` input. With no setting it works like this: it takes the
**first scale** from the image itself (the square root of the summed median local standard deviation of the bands in
3 × 3 cells, here 0.207), and then segments at that scale, at twice it, at three times it... for **30 levels**
(`levels`, a minimum of 3), up to 6.20. Every level is grown from the one below, so the whole run costs about one
segmentation. To stay fast it works on the **central window** of at most 512 × 512 cells (`window`; 0 takes the whole
image): this clip has 580 × 580 cells, so ESP sees all but a 34-cell frame, and the scales it finds hold for the rest. The
other settings are those of the multiresolution method (`shape`, `compactness`, `weights`) and the `seed` of its
order of merges. `start` and `step` give a range of your own, which is the way to look closer after a first run.

The node has three outputs: `table` (a row per level: `scale`, `objects`, the local variance `lv_<band>` and the rate of
change `roc_<band>` for each band, their mean `roc_mean`, and `suggested`, 1 at a peak), `scales` (the suggested scales
alone) and `scale` (the first of them).

### Step 3: USPO, each scale scored

Add [[segment.optimize]], connect the same `refl.image` to `image`, and say which scales to try with `values` (a list) or
with `from`, `to` and `step`. Everything else has a default that fits this case: `segmenter` is the multiresolution
method, `parameter` is its `scale`, and `objective` is the sum of the two scaled scores. Settings of the segmenter that
should stay as they are go in `params`, for example `params = { shape = 0.3 }`. The runs go in parallel.

The outputs are `objects` and `table` (the objects of the best run, as the segmenter gives them), `scores` (a row per
value: `value`, `objects`, `variance`, `moran` and `objective`, NaN where a run has no score, such as one made of a
single object) and `value`, the best value.

### Step 4: the segmentation you will use

[[segment.optimize]] hands on the objects of its best run, so you can connect them straight to
[[objects.to_vector]]. More often you read the scores, choose, and put the scale in a [[segment.multiresolution]] node of
your own, which is what the later tutorials do. The model below does the second, with the first of ESP's peaks.

```tfgraph
#! caption: The whole model: scene, UTM grid, reflectance, ESP, USPO over ten scales, the multiresolution at the scale chosen (2.48) and its polygons.
#! highlight: esp, opt, segments
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "s2"
op = "data.stac"
params = { url = "https://earth-search.aws.element84.com/v1", collection = "sentinel-2-l2a", assets = ["blue", "green", "red", "nir", "swir16"], datetime = "2024-08-09", extent = [190520, 8241280, 196320, 8247080], cellsize = 10, crs = "EPSG:31983" }
note = "The scene of 9 August 2024, five bands."

[[nodes]]
id = "refl"
op = "raster.calc"
inputs = { s = "s2.raster" }
params = { expression = "out image = s / 10000" }
note = "Reflectance 0 to 1."

[[nodes]]
id = "esp"
op = "segment.scale"
inputs = { image = "refl.image" }
note = "Peaks at 2.48, 2.89 and 3.51."

[[nodes]]
id = "opt"
op = "segment.optimize"
inputs = { image = "refl.image" }
params = { values = [1, 1.5, 2, 3, 4, 6, 8, 12, 16, 24] }
note = "Best by the sum: 1.5; by the F-measure: 6."

[[nodes]]
id = "segments"
op = "segment.multiresolution"
inputs = { image = "refl.image" }
params = { scale = 2.48 }
note = "The scale you decide on."

[[nodes]]
id = "polygons"
op = "objects.to_vector"
inputs = { objects = "segments.objects" }

[[nodes]]
id = "write"
op = "vector.write"
inputs = { vector = "polygons.vector" }
params = { path = "objects_scale.gpkg" }
```

=== "Application"

    Start from the template *The scale chosen by itself* (the library's *Templates*, group *Objects*), or build the
    nodes above: click an operator in the library to add it, then **drag from an output port to an input port**. Select
    `esp` and press ++ctrl+enter++ (*Run to here*), then open its `table` output in the Viewer and choose the **ESP** chart: the
    local variance and the rate of change side by side, with a line at each suggested scale. Select `opt` and run it: its
    `scores` output has a chart of its own (see below), and under it the best value with an **Apply** button.

=== "Command line"

    Save the graph above as `scale.toml` and run it. To get the tables of `esp` and `opt` as files, make them outputs of
    the model with a block such as `[outputs.esp]` and `from = "esp.table"` (a table is written as `esp.csv` in the
    output folder); the run also notes the suggested scales in its warnings.

    ```bash
    terraflux run scale.toml --threads 4 --out output
    ```

    A [[segment.multiresolution]] with no `scale` would take the first of them.

=== "Without a network"

    As in part 2: download [sentinel2_brasilia.tif](/data/tutorials/object-based-image-analysis/sentinel2_brasilia.tif)
    beside the model and replace the node `s2` with a `raster.read` of it. The download is rounded to whole
    numbers, so the counts of this page can differ by a few objects.

    ```toml
    [[nodes]]
    id = "s2"
    op = "raster.read"
    params = { path = "sentinel2_brasilia.tif" }
    ```

## Read the results

### ESP: where the objects become wholes

Open the table of `esp`. The local variance rises with the scale, as it must (bigger objects hold more variation), and
its **rate of change** falls fast: from 66.8 % at the scale 0.62 to 3.9 % at 2.48 and to about 1 or 2 % above 3. A peak
is a scale where the fall stops for a moment, because objects stopped being parts and started to be wholes.

```tfchart
type: line
data: /data/tutorials/object-based-image-analysis/choosing-the-scale/esp.csv
x: scale
y: [roc_mean]
labels: { roc_mean: "Rate of change of the local variance (%)" }
colors: { roc_mean: "#3b8a5a" }
xlabel: Scale
ylabel: ROC (%)
height: 300
caption: "ESP on the clip: the rate of change of the local variance (mean of the five bands) at the scales 1.24 to 6.20. The first levels, where it is in the hundreds, are left out. Peaks, higher than both neighbours, at 2.48, 2.89 and 3.51."
```

ESP suggests the first three peaks: **2.48, 2.89 and 3.51**. At 2.48 the window's objects number 1,602 (1,987 over the whole
clip), at 2.89 they number 1,173 and at 3.51 they number 786. ESP is meant to
suggest a fine, a medium and a coarse scale; here the three lie close together, because the curve is dominated by the
town, where most of the variation of this scene is, and has little else to say above scale 3. If you leave `scale` empty,
[[segment.multiresolution]] takes the first of these.

!!! note
    ESP's ROC is noisy. Look at the curve as well as at the three marked scales: a gentle bump (the one near 6.0, 2.2 %)
    that ESP did not mark can be as good a scale as one it did. Run ESP again with `start` and `step` over a range you
    care about to see it in finer steps.

### USPO: two scores that pull apart

Move the slider. It shows the same part of the clip at the ten scales USPO scored, with their numbers, and the chart
beside it the two scaled scores and their sum.

```tfslider
label: Scale
caption: "The ten scales USPO scored, over the same part of the clip as in part 2 (shape 0.1, compactness 0.5). The chart is the scores of the whole clip: the variance and Moran's I rescaled from 1 (lowest) to 0 (highest) and their sum, the objective."
frames:
  - { value: 1, src: /img/tutorials/object-based-image-analysis/choosing-the-scale/scale_1.png, caption: "Scale 1", stats: { Objects: "11,303", "Mean size": "0.30 ha", Objective: "1.000" } }
  - { value: 1.5, src: /img/tutorials/object-based-image-analysis/choosing-the-scale/scale_1_5.png, caption: "Scale 1.5", stats: { Objects: "5,210", "Mean size": "0.65 ha", Objective: "1.006" } }
  - { value: 2, src: /img/tutorials/object-based-image-analysis/choosing-the-scale/scale_2.png, caption: "Scale 2", stats: { Objects: "2,973", "Mean size": "1.13 ha", Objective: "0.958" } }
  - { value: 3, src: /img/tutorials/object-based-image-analysis/choosing-the-scale/scale_3.png, caption: "Scale 3", stats: { Objects: "1,338", "Mean size": "2.51 ha", Objective: "0.892" } }
  - { value: 4, src: /img/tutorials/object-based-image-analysis/choosing-the-scale/scale_4.png, caption: "Scale 4", stats: { Objects: "725", "Mean size": "4.64 ha", Objective: "0.842" } }
  - { value: 6, src: /img/tutorials/object-based-image-analysis/choosing-the-scale/scale_6.png, caption: "Scale 6", stats: { Objects: "308", "Mean size": "10.92 ha", Objective: "0.855" } }
  - { value: 8, src: /img/tutorials/object-based-image-analysis/choosing-the-scale/scale_8.png, caption: "Scale 8", stats: { Objects: "175", "Mean size": "19.22 ha", Objective: "0.626" } }
  - { value: 12, src: /img/tutorials/object-based-image-analysis/choosing-the-scale/scale_12.png, caption: "Scale 12", stats: { Objects: "67", "Mean size": "50.21 ha", Objective: "0.874" } }
  - { value: 16, src: /img/tutorials/object-based-image-analysis/choosing-the-scale/scale_16.png, caption: "Scale 16", stats: { Objects: "41", "Mean size": "82.05 ha", Objective: "0.854" } }
  - { value: 24, src: /img/tutorials/object-based-image-analysis/choosing-the-scale/scale_24.png, caption: "Scale 24", stats: { Objects: "15", "Mean size": "224.27 ha", Objective: "1.000" } }
chart:
  type: line
  data: /data/tutorials/object-based-image-analysis/choosing-the-scale/scores.csv
  x: scale
  y: [variance_scaled, moran_scaled, objective]
  labels: { variance_scaled: "Variance (scaled)", moran_scaled: "Moran's I (scaled)", objective: "Objective (sum)" }
  colors: { variance_scaled: "#3b8a5a", moran_scaled: "#2f6f9f", objective: "#e8901f" }
  ylabel: Score
```

The two scores are made to disagree. The variance inside the objects only grows with the scale, from 0.00029 at scale 1
to 0.0015 at 24 (reflectance squared), so its scaled score falls from 1 to 0. Moran's I, the similarity of neighbours,
falls too, from 0.61 to −0.05, so its scaled score rises from 0 to 1. Their sum is **high at both ends** and low in the
middle, and this is the first thing to understand about USPO: it can only choose between "very homogeneous objects that
look like their neighbours" and "very different objects that are not homogeneous", and an optimum appears only if the
two scores trade off unevenly in between.

```tftable
digits: { scale: 1, objects: 0, mean_ha: 2, variance: 5, moran: 2, objective: 3, objective_f: 3, wrong_pct: 2 }
labels: { scale: Scale, objects: Objects, mean_ha: "Mean size (ha)", variance: Variance, moran: "Moran's I", objective: "Objective (sum)", objective_f: "Objective (F)", wrong_pct: "Cells on the wrong side of the shore (%)" }
columns: [scale, objects, mean_ha, variance, moran, objective, objective_f, wrong_pct]
data: /data/tutorials/object-based-image-analysis/choosing-the-scale/scores.csv
caption: "The scores table of segment.optimize on the whole clip, with the F-measure objective (alpha 1) from a second run, and, for each scale, the share of cells that fall on the other side of the lake shore than most of their object (water against land, cut by Otsu on the NDWI as part 7 does)."
```

Read the table, not only the winner:

- **By the sum, the best scale is 1.5** (5,210 objects), with an objective of 1.006. That is hardly above 1.000 at the scale 1
  and at the scale 24: three very different segmentations are within 0.01 of each other. A winner so close to the ends of
  the range says that the range, not the image, picked it.
- **By the F-measure** (`objective = "f"`, Johnson and Xie's), the best scale is **6** (308 objects, 10.9 ha each), and
  the objective is a smooth hump, 0 at both ends of the range and about 0.31 to 0.43 from 4 to 12. The F-measure does not
  let a run with one score at the top and the other at the bottom win.
- **The cost of a coarse scale** shows in the last column. Water against land changes at the lake shore, a sharp edge;
  even at scale 12, 0.50 % of the cells are on the wrong side of their object's majority, and 0.24 % at scale 1. At the
  scale 24 it jumps to 0.95 %, and the slider's last frame shows why: the lake has merged with the bare ground and the
  blocks south of it.

!!! tip
    Give USPO a range **around what ESP suggested** and wide enough on both sides to show a hump, then compare the sum
    and the F-measure. When both point to the same region, you have a scale; when they do not, as here, you have a range,
    and your eyes pick in it.

## The scale of each class

One scale for the whole image is a compromise. To see how big, run ESP and USPO again on three 100 × 100 cell windows (1 km
square), each over one kind of ground: the lake, the town and the dry cerrado.

![The three windows over the clip](/img/tutorials/object-based-image-analysis/choosing-the-scale/windows.png "The three windows, each 100 × 100 cells (1 km): the lake (blue), the town (yellow) and the dry cerrado (orange).")

```tftable
sortable: true
align: { window: left }
labels: { window: Window, peak1: "ESP 1st peak", peak2: "ESP 2nd", peak3: "ESP 3rd", objects1: "Objects at 1st peak", uspo: "USPO best (sum)", uspo_objects: "Objects at USPO best" }
columns: [window, peak1, peak2, peak3, objects1, uspo, uspo_objects]
digits: { peak1: 2, peak2: 2, peak3: 2, objects1: 0, uspo: 1, uspo_objects: 0 }
data: /data/tutorials/object-based-image-analysis/choosing-the-scale/windows.csv
caption: "ESP's three peaks and USPO's best scale (the sum, over the scales 0.5 to 12) in each window, run alone on that window."
```

Their numbers disagree, and that is the point.

- **The lake** has ESP peaks at 0.47, 0.60 and 0.77, far below the whole clip's, and USPO's best is 8 (3 objects). The water is
  nearly uniform, so the cost of merging two cells of water is tiny and "wholes" appear at a very small scale. But look at what a **larger** scale does in the first row below: the lake becomes a few objects
  and the shore does not move, because merging water into land costs far more than any scale shown here pays.
- **The town** has its peaks at 2.47, 3.70 and 4.32, the highest of the three, and USPO's best is 1. Here the scale
  matters at every step: by scale 12 the window holds two objects.
- **The cerrado** has peaks at 0.92, 1.27 and 1.84, and USPO picks 6 (6 objects), with 8 and 12 giving a single object that
  has no score. The grass, the dirt tracks and a patch of trees are low in contrast: at scale 4 some objects still
  follow a track, and at 12 they do not.

```tfgallery
columns: 3
caption: "Each window at the scales 1, 4 and 12. Rows: the lake, the town, the cerrado. Objects touching the window, in the segmentation of the whole clip: lake 269, 38 and 5; town 517, 36 and 2; cerrado 265, 30 and 8."
images:
  - { src: /img/tutorials/object-based-image-analysis/choosing-the-scale/win_water_1.png, caption: "Lake, scale 1" }
  - { src: /img/tutorials/object-based-image-analysis/choosing-the-scale/win_water_4.png, caption: "Lake, scale 4" }
  - { src: /img/tutorials/object-based-image-analysis/choosing-the-scale/win_water_12.png, caption: "Lake, scale 12" }
  - { src: /img/tutorials/object-based-image-analysis/choosing-the-scale/win_town_1.png, caption: "Town, scale 1" }
  - { src: /img/tutorials/object-based-image-analysis/choosing-the-scale/win_town_4.png, caption: "Town, scale 4" }
  - { src: /img/tutorials/object-based-image-analysis/choosing-the-scale/win_town_12.png, caption: "Town, scale 12" }
  - { src: /img/tutorials/object-based-image-analysis/choosing-the-scale/win_cerrado_1.png, caption: "Cerrado, scale 1" }
  - { src: /img/tutorials/object-based-image-analysis/choosing-the-scale/win_cerrado_4.png, caption: "Cerrado, scale 4" }
  - { src: /img/tutorials/object-based-image-analysis/choosing-the-scale/win_cerrado_12.png, caption: "Cerrado, scale 12" }
```

What this teaches is how to think about the scale, not which number to use:

1. **The scale is a price in the image's own units.** A merge is made when its cost is below the scale squared. A flat
   class (water, bare cerrado) reaches "whole objects" at a **low number** because its cells differ very little; a
   contrasted class (a town) needs a **high number** before roofs and streets join. So a scale is never "big for
   homogeneous things".
2. **A sharp edge protects itself.** The lake is bounded by a shore with a large contrast, so a wide range of scales gives
   the same shore and changes only how many pieces the water has. For the shore, the scale hardly matters; for the town it
   decides everything.
3. **Your classes decide.** If the lake is one of your classes and the town another, a scale tuned on the town
   (about 3 to 5) keeps the shore and the blocks; one tuned on the lake (0.5) cuts the water into hundreds of pieces; one
   tuned on the cerrado alone (6) loses the blocks. The compromise of one scale is a scale for the class that is hardest to
   cut, here the town.

When no single scale serves, build **levels**: a fine level for the town and a coarse one for the water and the cerrado,
which part 8 of the series does with `from`. A cheaper step first is to merge the pieces of a flat class afterwards, with
[[segment.spectral_difference]] (part 7).

## Settings that change the answer

**The range of values.** USPO can only choose among the scales you give it. The narrow run below, from 1 to 8 in steps of
0.5, finds the same winner (1.5, objective 1.066) but shows what the first run could not: a second hump at 7
(1.054) and a drop at 7.5 (0.523) although the scale only went up by half a unit. The
curve is bumpy; do not read a decimal into it.

```tfchart
type: line
data: /data/tutorials/object-based-image-analysis/choosing-the-scale/narrow.csv
x: scale
y: [objective]
labels: { objective: "Objective (sum)" }
colors: { objective: "#e8901f" }
xlabel: Scale
ylabel: Objective
height: 280
caption: "segment.optimize from 1 to 8 in steps of 0.5 (shape 0.1, compactness 0.5): the sum of the scaled variance and Moran's I."
```

**The objective.** `objective = "sum"` is Espindola's. `objective = "f"` is Johnson and Xie's F-measure,
\( (1 + \alpha^2)\, v\, i / (\alpha^2 i + v) \) of the scaled variance \( v \) and the scaled Moran's I \( i \); `alpha`
(default 1) weighs the variance against Moran's I, above 1 favouring homogeneous objects (smaller scales). On this clip
the sum chose 1.5 and the F-measure 6: ask both.

**What stays fixed.** `params` holds the settings USPO does not vary. If you will use `shape = 0.3` for your
segmentation, score it with `params = { shape = 0.3 }`; the best scale of one shape is not that of another. The same goes
for the bands: a change of `weights` changes the scores.

**Other segmenters.** `segmenter` and `parameter` score any segmenter that takes an image, by its main setting: the `size`
of SLIC, the `k` of Felzenszwalb, the `h` of the watershed. The score is the same two measures, so the same cautions
apply.

**The window of ESP.** With a `window` smaller than the image, ESP sees only its centre. Choose it to hold your classes
(or give 0), because the scales it suggests are those of the window.

## In the application

You can choose a scale without running the two nodes. Click a [[segment.multiresolution]] node, run it once, and
move its **scale** slider in the *Live segmentation* section: the part of the image in view is segmented as you move it
(part 2 describes it). If the model has a [[segment.scale]] node on the same image, the section draws its **ROC curve**
(the model's own table); if not, it computes one for the window in view. Under the curve, a row of buttons, **suggested**,
has one for each peak: a click puts that scale on the slider. Nothing in the model changes until you press **Apply**, which
writes the value into the node as one step of undo.

For [[segment.optimize]], select the node and look at its `scores` in the Viewer. The chart shows the **objective**, the
scaled **variance** and the scaled **Moran's I** against the value, with a vertical line at the best, and the number of
objects of each run in a smaller panel under it. Below it is the best value, and **Apply to** a list: a segmenter of the model
that reads the same image (or *a new node*, a segmenter connected to that image). **Apply** writes the best value in the
setting that was varied and, with it, the fixed `params`, as one step of undo.

!!! tip
    Before applying, look at the objects of the scale at the **Viewer**, over a part with all your classes. Scores
    decide between neighbouring scales; your eyes decide between a lake and a town.

## Common pitfalls

- **Taking the winner of a short range.** A best value at the end of the range (as the sum's 1.5 here, next to the
  smallest scale tried) is a sign to widen the range, not a result.
- **Believing the sum.** On this clip its winner is hardly above the two ends of the range. Compare it with the F-measure
  and with ESP, and look.
- **Mixing units.** The scales of this page are for reflectance from 0 to 1. Run the same image from 0 to 10 000 and
  every scale means something else (ESP's first scale would be near 2070, not 0.207).
- **Using a scale from another image.** A scale is a cost in the image's units: compare objects (their count and size),
  not numbers, between scenes.
- **One scale for every class.** If the lake and the town are both in the map, expect to compromise or to use levels.
- **Reading ESP's central window as the whole image.** On a big image the 512 × 512 default window may hold only one kind
  of ground; set `window` to hold them all, or 0.
- **Scores of runs with one object.** At a very large scale a window can end up as one object: its score is NaN, and it
  drops out. A table with NaNs at the high end says the range is too wide for that part.

## Going further

- Before this: [What an object is](/tutorials/object-based-image-analysis/what-is-an-object/). Next in the series:
  describing the objects (part 4), and classifying them from samples (part 5) and from rules (part 6).
- Water by thresholds, a segmentation that needs no scale: [Water by thresholds](/tutorials/object-based-image-analysis/water-by-thresholds/).
- The operators of this page: [[segment.scale]], [[segment.optimize]], [[segment.multiresolution]], [[objects.to_vector]]
  and [[vector.write]].
- The Viewer and its controls: [the application's Viewer](/app/viewer/).

## References

- Baatz, M., & Schäpe, A. (2000). Multiresolution segmentation: an optimization approach for high quality
  multi-scale image segmentation. In *Angewandte Geographische Informationsverarbeitung XII* (pp. 12-23). Wichmann.
- Drăguţ, L., Csillik, O., Eisank, C., & Tiede, D. (2014). Automated parameterisation for multi-scale image segmentation on
  multiple layers. *ISPRS Journal of Photogrammetry and Remote Sensing*, 88, 119-127.
  https://doi.org/10.1016/j.isprsjprs.2013.11.018
- Drăguţ, L., Tiede, D., & Levick, S. R. (2010). ESP: a tool to estimate scale parameter for multiresolution image
  segmentation of remotely sensed data. *International Journal of Geographical Information Science*, 24(6), 859-871.
  https://doi.org/10.1080/13658810903174803
- Espindola, G. M., Câmara, G., Reis, I. A., Bins, L. S., & Monteiro, A. M. (2006). Parameter selection for
  region-growing image segmentation algorithms using spatial autocorrelation. *International Journal of Remote Sensing*,
  27(14), 3035-3040. https://doi.org/10.1080/01431160600617194
- Johnson, B., & Xie, Z. (2011). Unsupervised image segmentation evaluation and refinement using a multi-scale approach.
  *ISPRS Journal of Photogrammetry and Remote Sensing*, 66(4), 473-483. https://doi.org/10.1016/j.isprsjprs.2011.02.006
- Moran, P. A. P. (1950). Notes on continuous stochastic phenomena. *Biometrika*, 37(1-2), 17-23.
  https://doi.org/10.2307/2332142
