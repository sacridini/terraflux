---
title: "Deep segmentation: SAM and embeddings"
short: Deep segmentation
description: "Segment an image with Segment Anything (segment.sam) and describe objects with a neural network's embeddings (objects.embed): where the weights come from, what the settings do, what they cost, and when they beat the classical segmenters and when they do not."
order: 130
level: advanced
time: 90 min
ops: [segment.sam, objects.embed, raster.clip, objects.to_map, table.select, ml.kmeans, objects.classify, segment.multiresolution, segment.slic]
series: Object-based image analysis
part: 13
---

Every segmenter of the series so far looked at the **values** of the cells and at nothing else: colours that are alike
become one object. Two operators of Terraflux use a **neural network** instead. [[segment.sam]] runs Segment Anything, a
model trained on a very large number of photographs to draw the outline of *things* (a roof, a tree, a swimming pool),
whatever they are made of. [[objects.embed]] takes objects you already have and describes each by a list of numbers that a
network computes from the image around it, to classify or group the objects by what the network sees. You will run both on the
Sentinel-2 and NAIP images of the series, see what they give next to the multiresolution and SLIC segmenters, and read
what they cost. The answer is not "use them": SAM draws very good outlines of houses and trees on a 0.6 m aerial image and
almost nothing useful on a 10 m satellite image, and the embeddings help only as far as the network knows your kind of image.
The page is meant to let you decide before you wait for ten minutes.

Everything here is **optional**. The rest of the objects operators never need a neural network, and Terraflux ships
neither the runtime nor any weights: SAM downloads its weights the first time it runs, and `objects.embed` takes a model you
provide.

## The idea

### Segment Anything

**Segment Anything** (SAM; Kirillov et al., 2023) is a model trained to answer one question: given an image and a **prompt** (a
point, a box, a rough mask), what is the outline of the thing the prompt points at? It was trained on a dataset of over a billion
masks (SA-1B) on 11 million photographs, so it works on things it has not seen. It has two parts with very different costs.

- The **image encoder**, a large vision transformer, turns the image into a grid of features. It runs **once per image** (here,
  once per tile of 1024 by 1024 cells) and is the slow part of a first run.
- The **mask decoder** is small. It turns the features and one prompt into masks, **once per prompt**.

Osco and colleagues (2023) tested it on remote sensing images, from no example to one. To get a segmentation of the whole image with no one to click, the authors of SAM proposed an **automatic mode**: put a regular grid of
points on the image, ask for the masks of each point, keep the good ones (by the model's own estimate of how well the mask fits,
and by how stable it is when the threshold is moved), remove duplicates, and paint what is left, big masks first. That is what
[[segment.sam]] does, simplified:

1. The image is cut in tiles of 1024 by 1024 cells (clipped at the edges); each tile is **resized so that its longer side is
   1024** and padded, and its three bands (you say which are red, green and blue with `bands`) are stretched between their 2nd and
   98th percentiles to 0-255.
2. A grid of `points` by `points` prompts is laid on each tile (16 by 16 by default, 256 prompts), without the cells that have no value.
3. For each prompt the **best mask** is kept if its predicted IoU is at least `iou` (0.88 by default), its stability score is at
   least `stability_thresh` (0.95, measured with the logit offset `stability_offset`, 1) and it covers at most `max_area` of the
   tile (0.9: a bigger one is taken for the background).
4. The masks are thinned by non-maximum suppression of their boxes (0.7), ranked by stability times IoU, and painted one over the
   other, **larger first**, so the smaller ones on top survive.
5. With `fill` (the default) the cells no mask holds become objects too, each connected area one; without it they are no object.
   The pieces of a mask that are not connected are different objects, and `min_size` drops the masks left with fewer cells.

Two things in the list decide how it behaves, and neither is a setting you would guess.

- **The prompts are a grid, not your clicks.** You cannot give [[segment.sam]] a point or a box. The grid decides what can be found:
  an object that no prompt falls in is not segmented.
- **The grid is laid on the resized tile.** A tile of 1024 cells has one prompt every 64 cells with 16 points. A 300 by 300
  window is enlarged 3.4 times to fit 1024 and the same 16 points fall every 19 cells of the original. **A smaller image is a finer
  grid**, and this decides the size of the objects more than any other setting. The page shows it with real runs.

### Embeddings

A **foundation model** of Earth observation (Prithvi, Clay and others) is a network trained on a huge amount of imagery, without
labels, to produce for a patch of image a vector of numbers (an **embedding**) in which patches that look alike are close. A
classifier on top of these vectors needs far fewer labels than one on raw bands: that is the promise, and it holds
only for imagery like the model's own.

[[objects.embed]] runs a network of your choice, in the ONNX format, on the image, once on each patch of `patch` by `patch` cells, and gives each
object the mean of the vectors of the patches its cells fall in, weighted by the number of its cells in each patch. The vector's
numbers join the objects' table as `emb_1`, `emb_2`... for [[ml.kmeans]] or [[ml.fit]], alone or with the attributes of part 4.
Two consequences, both easy to miss: **the embedding of an object is a property of the patches under it, not of its
outline**; and **no weights come with Terraflux**: you export the encoder of the model you trust to ONNX with its authors' tools.

## When to use it

Use SAM when:

- the image is **very high resolution** (aerial or drone photographs, 1 m or finer, three colour bands) and the things you want
  (roofs, trees, cars, ponds) are tens of cells wide and have **clear outlines**;
- you want **the outline of each thing** more than you want objects homogeneous in colour: a roof with two slopes of different tone is one
  object for SAM and two for a colour-based method;
- you can afford the time (about half a second per prompt on two cores, and about half a minute before the first) and about 3.6 GB of memory.

Do **not** use SAM when:

- the image is a **10 m satellite image** like the Sentinel-2 clip of the series, where a house is one or two cells: there is nothing for the model
  to outline (below);
- your information is in **bands SAM never sees** (near infrared, shortwave infrared): it takes three;
- you need **exactly homogeneous objects**, a scale you can tune with a slider (part 3) or results in seconds
  on a large image. The classical segmenters and the tiles of part 12 are the tools for that.

Use the embeddings when you have an encoder **trained on imagery like yours**, a modest number of labelled objects, and
information (texture, context, structure) that colour, shape and the GLCM of part 4 do not hold. Do not expect them to rescue
a bad segmentation, and do not use a model of RGB photographs on a multispectral image without thinking about what it learned.

## What you need

- **ONNX Runtime**, the program that runs the network. Terraflux does not ship it: `pip install onnxruntime` puts its shared library
  (`onnxruntime.dll`, `libonnxruntime.so` or `libonnxruntime.dylib`) in the `capi` folder of the package, and you tell Terraflux where
  it is with the environment variable `ORT_DYLIB_PATH` (or put the file beside the `terraflux` program). If it is not found,
  the operator stops with a message that says all this; every other operator works without it. We ran it with the library of the `onnxruntime` 1.29 package.
- **The weights of SAM**, if you have none of your own. See the next section: the first run downloads them, 75 MB.
- **An image of three colour bands.** The Sentinel-2 clip of the series, with the layers `blue`, `green`, `red`, `nir`
  and `swir16`, and the NAIP clip of Plano, Texas (0.6 m, `red`, `green`, `blue`, `nir`). The download links are the
  series': [sentinel2_brasilia.tif](/data/tutorials/object-based-image-analysis/sentinel2_brasilia.tif) and
  [naip_plano.tif](/data/tutorials/object-based-image-analysis/naip_plano.tif) (2.9 MB, 1,000 by 1,000 cells);
  [ATTRIBUTION.txt](/data/tutorials/object-based-image-analysis/ATTRIBUTION.txt) has their credits.
- **Memory and patience.** Every SAM run below peaked at **3.6 GB** whatever the image, and took
  from half a minute to several minutes on two cores. Try it on a small window first.

## Build the model

### Step 1: the weights

With no `encoder` and `decoder` given, [[segment.sam]] uses **SAM ViT-B** (the smallest of the three sizes the authors released),
as exported to ONNX by vietanhdev (Apache-2.0, `huggingface.co/vietanhdev/segment-anything-onnx-models`). `weights` chooses the
variant: **`quant`** (the default, quantised, a download of 75 MB) or **`full`** (full precision, 348 MB). They are downloaded
**once**, their SHA-256 is checked against the one the code knows, and the encoder and the decoder are extracted into Terraflux's
cache folder, in `sam/vit_b_quant` (the quantised ones are then 109 MB and 8.7 MB). On Windows the cache is
`%LOCALAPPDATA%\Terraflux\cache`, on macOS `~/Library/Caches/Terraflux`, on Linux `~/.cache/terraflux`; the variable `TERRAFLUX_CACHE`
moves it. Later runs, of any model, reuse them. The weights come **after** the check for the runtime: no download is started when
the run could not go on.

!!! app "In the application"
    A run that would download the weights is **held back** and a window asks first: *Download the weights of SAM?* It says
    the size of each variant (a choice of **Quantised** or **Full precision**, written to the node's `weights` as one step of
    undo), the licence (Apache-2.0), where they come from, and where they will be kept. **Download and run** goes on and
    remembers the choice on this computer; the download shows in the status bar and **Stop** cancels it. A run the application starts by
    itself only says that it needs your answer. To use your own files, give the node's `encoder` and
    `decoder` (the common SAM ONNX export: an image encoder and a mask decoder) and nothing is downloaded.

We did not time a download: the weights were already in the cache of the computer these runs were made on.

### Step 2: SAM on the image

This model reads the Sentinel-2 clip, makes the reflectance of part 2, and runs [[segment.sam]] on the red, green and blue
layers, then draws the objects as a map. `points = 8` keeps the run short: 64 prompts.

```tfgraph
#! caption: SAM on the Sentinel-2 clip: its reflectance, the objects of 8 by 8 prompts on the red, green and blue layers, and their map.
#! highlight: sam
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "s2"
op = "data.stac"
params = { url = "https://earth-search.aws.element84.com/v1", collection = "sentinel-2-l2a", assets = ["blue", "green", "red", "nir", "swir16"], datetime = "2024-08-09", extent = [-47.89, -15.89, -47.835, -15.835], cellsize = 0.00009 }
note = "The scene of 9 August 2024, five bands."

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
note = "Reflectance 0 to 1 (SAM stretches the bands itself)."

[[nodes]]
id = "sam"
op = "segment.sam"
inputs = { image = "refl.image" }
params = { bands = ["red", "green", "blue"], points = 8 }
note = "64 prompts. The weights download the first time."

[[nodes]]
id = "map"
op = "objects.to_map"
inputs = { objects = "sam.objects" }
params = { what = "id" }

[[nodes]]
id = "write"
op = "raster.write"
inputs = { value = "map.map" }
params = { path = "sam_objects.tif" }
```

For the NAIP image the model is the same without the first three nodes: a [[raster.read]] of `naip_plano.tif`, a [[raster.clip]] to the window of
300 by 300 cells that the figures below show (`extent` in its coordinates, EPSG:26914), and [[segment.sam]] with the default
`bands` (the first three layers, `red`, `green` and `blue`).

```tfgraph
#! caption: The NAIP window: a clip of 300 by 300 cells (180 m) of the image, and SAM with 24 by 24 prompts.
#! highlight: sam
#! fragment: true
[[nodes]]
id = "naip"
op = "raster.read"
params = { path = "naip_plano.tif" }

[[nodes]]
id = "crop"
op = "raster.clip"
inputs = { raster = "naip.raster" }
params = { extent = [705240.6, 3658728.0, 705420.6, 3658908.0] }
note = "300 by 300 cells at 0.6 m."

[[nodes]]
id = "sam"
op = "segment.sam"
inputs = { image = "crop.raster" }
params = { points = 24 }
note = "576 prompts: about 5 minutes on two cores."
```

=== "Application"

    Build the nodes as in the earlier parts (click an operator in the library, **drag from an output port to an input port**).
    The first run of `sam` asks about the weights, then runs; the status bar shows the progress (the download, then the work).
    Select `sam`, press ++ctrl+enter++ and open its `objects` in the Viewer: the outlines are drawn like those of any segmenter.

=== "Command line"

    Save the graph as `sam.toml`. The runtime must be found by the variable; the weights are downloaded when they are not in the cache
    (the command does not ask):

    ```bash
    export ORT_DYLIB_PATH=/path/to/libonnxruntime.so      # Windows: set ORT_DYLIB_PATH=C:\path\to\onnxruntime.dll
    terraflux run sam.toml --threads 2 --profile
    ```

The nodes after [[segment.sam]] are the ones of the earlier parts: its `objects` are objects like any other (they have a `table` of
the mean and standard deviation of the three bands), and [[objects.features]] can describe them for a classification.

## Read the results

### Sentinel-2: nothing for the model to outline

The same Sentinel-2 clip, with 4, 8 and 16 prompts per side. Move the slider.

```tfslider
label: Prompts per side
caption: "SAM on the Sentinel-2 clip (580 by 580 cells of 10 m) with a grid of 4, 8 and 16 prompts per side: the outlines over the same part of the clip as in part 2. Objects are counted over the whole clip."
frames:
  - { value: 4, src: /img/tutorials/object-based-image-analysis/deep-segmentation/s2_sam4.png, caption: "4 by 4 prompts", stats: { Objects: "3", Prompts: "16", Seconds: "33" } }
  - { value: 8, src: /img/tutorials/object-based-image-analysis/deep-segmentation/s2_sam8.png, caption: "8 by 8 prompts", stats: { Objects: "11", Prompts: "64", Seconds: "63" } }
  - { value: 16, src: /img/tutorials/object-based-image-analysis/deep-segmentation/s2_sam16.png, caption: "16 by 16 prompts", stats: { Objects: "155", Prompts: "256", Seconds: "177" } }
```

What SAM finds in this window, at every grid, is **the lake**, with an outline that follows the shore well. The town, the roads and the cerrado are
left in one huge "leftover" object (`fill`): with 16 by 16 prompts it holds 91 % of the window's cells, the lake is the second
largest (7 %), and the 62 others are small. The masks that would cover a block of houses, a ten-metre road or a
patch of grass are not things the model draws at this resolution: a house is one or two cells. With 16 by 16 prompts the
clip has 155 objects, against 2,599 for the multiresolution method at the scale of part 3, a mean of 21.7 ha per object (about 217
cells). The 4 by 4 grid gives **3 objects** for the whole clip, and no grid we tried gave the town.

At about the same number of objects, SAM is compared with the classical methods on three measures: the share of the image's variance that stays
**inside** the objects (0 would be objects of one colour each), and the share of cells on the wrong side of the lake shore
(water against land by the NDWI, as in part 3).

```tftable
align: { method: left }
labels: { method: Method, objects: Objects, mean_ha: "Mean size (ha)", inside_variance: "Variance left inside objects", shore_error_pct: "Cells on the wrong side of the shore (%)" }
digits: { objects: 0, mean_ha: 1, inside_variance: 2, shore_error_pct: 2 }
data: /data/tutorials/object-based-image-analysis/deep-segmentation/compare_s2.csv
caption: "SAM (16 by 16 prompts) against the multiresolution method and SLIC at about the same number of objects, on the whole 580 by 580 clip. The variance inside is the mean over the five bands."
```

SAM is **far less homogeneous** (0.74 of the variance left inside its objects, against 0.44 and 0.39) and **a little better at the shore**
(0.35 % against 0.52 % and 0.49 %). Both are expected: SAM does not look at the values within an object, and a sharp outline of a
lake is exactly what it is good at.

### NAIP: where it works

Now the NAIP image, 0.6 m, a window of 300 by 300 cells (a block of houses, a street, tree crowns and a pond).

![The NAIP window](/img/tutorials/object-based-image-analysis/deep-segmentation/naip_window.png "A window of 300 by 300 cells (180 by 180 m) of the NAIP image of Plano, Texas, in true colour.")

```tfslider
label: Prompts per side
caption: "SAM on the NAIP window with 8, 16 and 24 prompts per side. The window is enlarged to SAM's 1024 cells, so 24 prompts fall every 12 cells of the original."
frames:
  - { value: 8, src: /img/tutorials/object-based-image-analysis/deep-segmentation/naip_sam8.png, caption: "8 by 8 prompts", stats: { Objects: "17", Prompts: "64", Seconds: "68" } }
  - { value: 16, src: /img/tutorials/object-based-image-analysis/deep-segmentation/naip_sam16.png, caption: "16 by 16 prompts", stats: { Objects: "128", Prompts: "256", Seconds: "155" } }
  - { value: 24, src: /img/tutorials/object-based-image-analysis/deep-segmentation/naip_sam24.png, caption: "24 by 24 prompts", stats: { Objects: "171", Prompts: "576", Seconds: "334" } }
```

Here SAM does what it was built for. With 24 by 24 prompts the **roofs** are single objects with the shape of the roof (a roof of two
slopes of different tone is still one), **tree crowns** are rounded objects, the **swimming pools** and the **pond** are their own
objects, and the road and the lawn are large smooth objects. With 8 by 8 prompts it finds only the largest crowns and a few roofs:
**the density of the grid sets the smallest thing you can find**. The objects it does not find are not lost: with `fill` they join
the leftover area between the masks, which here is the dense wood in the middle of the window and the shaded ground.

Next to it, the multiresolution method at a scale that gives about the same number of objects:

```tfcompare
caption: "Drag the handle: SAM with 24 by 24 prompts (171 objects) against the multiresolution method at the scale 8 (159 objects) on the NAIP window. Same window, same size."
before: { src: /img/tutorials/object-based-image-analysis/deep-segmentation/naip_sam24.png, label: "SAM, 24 prompts" }
after: { src: /img/tutorials/object-based-image-analysis/deep-segmentation/naip_mr.png, label: "Multiresolution, scale 8" }
```

The classical objects follow the **tone**: a roof is cut along its ridge where the slopes differ, the trees of the wood become a mosaic of
pieces with no relation to a crown. SAM's follow the **things**. Over the same
window the numbers say the same:

```tftable
align: { method: left }
labels: { method: Method, objects: Objects, mean_cells: "Mean size (cells)", inside_variance: "Variance left inside objects" }
digits: { objects: 0, mean_cells: 0, inside_variance: 2 }
data: /data/tutorials/object-based-image-analysis/deep-segmentation/compare_naip.csv
caption: "SAM at 8, 16 and 24 prompts per side against the multiresolution method (scale 8) and SLIC (size 26) at about the number of objects of the last, on the 300 by 300 window. The variance is the mean over the four bands."
```

SAM at 24 prompts leaves **0.76** of the variance inside its objects, the classical methods **0.31**. Whether the better outlines are worth
that depends on your question (see below): for the area of roofs, for the number of trees or for the crowns' shape, the outline matters; for
mapping the tone of a cover, it does not.

### The grid and the size of the image

The same 16 by 16 prompts on the **whole** 1,000 by 1,000 cell NAIP clip, which is resized to 1024 and gets one prompt every 62 cells (37 m):

![SAM on the whole NAIP clip](/img/tutorials/object-based-image-analysis/deep-segmentation/naip_whole_sam16.png "SAM, 16 by 16 prompts, on the whole 1,000 by 1,000 cell NAIP clip: 38 objects. Most houses fall between prompts.")

It finds **38 objects** (against 128 in the window with the same 16 prompts), because a house is 25 to 40 cells wide and most fall
between two prompts. The answer for a large image is the same as in part 12: **work by windows**, or raise `points`, and be ready to
pay for it.

### What it costs

```tftable
sortable: true
align: { clip: left, run: left }
columns: [clip, points, prompts, objects, seconds, peak_mb]
labels: { clip: Image, points: "Points per side", prompts: Prompts, objects: Objects, seconds: "Seconds", peak_mb: "Peak memory (MB)" }
digits: { points: 0, prompts: 0, objects: 0, seconds: 1, peak_mb: 0 }
data: /data/tutorials/object-based-image-analysis/deep-segmentation/sam_runs.csv
caption: "Every SAM run of this page (quantised weights, on two logical cores). The Sentinel-2 row with 16 prompts and the cells without objects (fill false) is also in the file."
```

Across these nine runs the time follows **about 31 s + 0.54 s for each prompt** (a fit to the runs, on two cores, with the quantised weights); the 31 s are
the encoder and the start of the run. A grid of 24 by 24 took five and a half minutes; the default 16 by 16 about three; 32 by 32 would
take about ten minutes per tile, by the fit (we did not run it). An image is cut in tiles of 1024 cells and **each tile pays both**, so the cost of a large image is
the number of its tiles times these numbers. The peak memory was **3.6 GB in every run**, from the smallest to the largest grid: it is the encoder's, not
the image's.

## Embeddings: describing objects with a network

### The model

No weights come with Terraflux, so we made a small encoder for this page. It is **ResNet-18** (He et al., 2016) pretrained on Sentinel-2
imagery by self-supervised learning with MoCo (He et al., 2020), the weights distributed with the SSL4EO-S12 dataset (Wang et al.,
2023) in torchgeo (`ResNet18_Weights.SENTINEL2_ALL_MOCO`: 13 bands). Our clip has five of the bands, so we **cut the network's
first layer to those five** (blue, green, red, near infrared and shortwave infrared, B2, B3, B4, B8 and B11, assuming the weights'
bands are in the order of the sensor's, B1 to B12 with B10 between B9 and B11, which we did not verify) and exported the
encoder to ONNX with an input of 1 by 5 by P by P and an output of 512 numbers. The script of the figures,
`website/figures/object-based-image-analysis/deep-segmentation.py` (function `make_encoder`), does it; it needs `torch`, `timm` and `onnx`.

This is **not a foundation model** in the sense of Prithvi or Clay, it is a small network, and it did not see this combination of five bands. It is
a real encoder in the right format that lets us show what the operator does. Take the result as a test of the plumbing, not of
what a foundation model gives.

### The nodes

```tfgraph
#! caption: The embeddings: the objects of part 2 (nodes 1 and 2 as before), an embedding of each by patches of 16 cells, the columns of the embedding alone, k-means into five groups and the objects given their group.
#! highlight: emb, km
#! numbered: true
#! fragment: true
[[nodes]]
id = "segs"
op = "segment.multiresolution"
inputs = { image = "refl.image" }
params = { scale = 2.14 }
note = "The 2,599 objects of part 3."

[[nodes]]
id = "emb"
op = "objects.embed"
inputs = { objects = "segs.objects", image = "refl.image" }
params = { model = "ssl4eo_resnet18_5band.onnx", patch = 16, bands = ["blue", "green", "red", "nir", "swir16"], normalize = false }
note = "emb_1 to emb_512 join the table."

[[nodes]]
id = "sel"
op = "table.select"
inputs = { table = "emb.table" }
params = { drop = ["id", "cells", "area", "mean_blue", "mean_green", "mean_red", "mean_nir", "mean_swir16", "sd_blue", "sd_green", "sd_red", "sd_nir", "sd_swir16"] }
note = "Only the embedding's columns are left."

[[nodes]]
id = "km"
op = "ml.kmeans"
inputs = { table = "sel.table" }
params = { k = 5 }
note = "Five groups of the 512 numbers."

[[nodes]]
id = "cl"
op = "objects.classify"
inputs = { objects = "emb.objects", model = "km.model" }
note = "Each object takes its group."
```

The settings of [[objects.embed]]: `model`, the ONNX file of the encoder (the path); `patch`, the side P of a patch, which must be the
side the model takes; `bands`, the layers it reads, in the order of its channels; and `normalize` (default true), which standardises each band over the
image so the model sees a mean of 0 and a spread of 1 (a cell without a value is 0). We gave the reflectance as it is (`normalize = false`)
and did not try the other: the right choice is the one the model's authors trained with. The operator gives `objects` (the same ones, with `emb_1`
to `emb_D` in their table) and `table`. A model whose output is a vector per patch (1 by D) is used as it is; one whose output is a
list of tokens (1 by T by D) is averaged over the tokens.

[[ml.kmeans]] standardises each column (`standardise`, the default). A number of the 512 columns are **zero for every object**, so their standard
deviation is 0: as scikit-learn's `StandardScaler` does, [[ml.kmeans]] gives such a column a scale of 1 and **warns, naming the columns**. (An
earlier version of Terraflux stopped instead, with `"emb_1" is constant`, and the way round it was `standardise = false`.) We counted the constant columns.

### What the embeddings do

The same 2,599 objects, grouped by k-means into five groups by their **mean bands** (the five numbers of part 9) and by their
**embeddings** with patches of 8, 16, 32 and 64 cells. Each grouping is scored against the 80 reference points of part 9 (their class
is known; each group takes the majority class of the points in it, and the score is the share of points that agree): that is a
guide, not an accuracy (the points are not a probability sample, 80 is few, and one point is 1.25 percentage points).

```tfslider
label: Patch (cells)
caption: "The five groups of the objects by the embeddings, with patches of 8, 16, 32 and 64 cells, over the same part of the clip as in part 2. A group is only a number and a colour: the colours are not the same between maps."
frames:
  - { value: 8, src: /img/tutorials/object-based-image-analysis/deep-segmentation/groups_emb8.png, caption: "Patches of 8 cells (80 m)", stats: { "Agree with the points": "76 %", "Different vectors": "2,599", "Constant columns": "295" } }
  - { value: 16, src: /img/tutorials/object-based-image-analysis/deep-segmentation/groups_emb16.png, caption: "Patches of 16 cells (160 m)", stats: { "Agree with the points": "80 %", "Different vectors": "2,576", "Constant columns": "200" } }
  - { value: 32, src: /img/tutorials/object-based-image-analysis/deep-segmentation/groups_emb32.png, caption: "Patches of 32 cells (320 m)", stats: { "Agree with the points": "72.5 %", "Different vectors": "1,877", "Constant columns": "205" } }
  - { value: 64, src: /img/tutorials/object-based-image-analysis/deep-segmentation/groups_emb64.png, caption: "Patches of 64 cells (640 m)", stats: { "Agree with the points": "44 %", "Different vectors": "953", "Constant columns": "102" } }
```

```tftable
align: { grouping: left }
columns: [grouping, distinct_vectors, constant_columns, groups_with_a_point, accuracy]
labels: { grouping: Grouping, distinct_vectors: "Different embeddings (of 2,599 objects)", constant_columns: "Columns of 512 that are constant", groups_with_a_point: "Groups with a reference point", accuracy: "Agree with the 80 points" }
digits: { distinct_vectors: 0, constant_columns: 0, groups_with_a_point: 0, accuracy: 3 }
data: /data/tutorials/object-based-image-analysis/deep-segmentation/embed_accuracy.csv
caption: "k-means with five groups on the mean bands and on the embeddings of four patch sizes, scored against the 80 reference points of part 9."
```

What the table and the maps say, and what they do not:

- With patches of 8, 16 and 32 cells the groups by embeddings agree with the reference points more often than those by the mean bands: **0.64** by the
  five means, **0.73 to 0.80** by the embeddings (patch 8: 0.76, patch 16: 0.80, patch 32: 0.73). That is 7 to 13 points of 80. With patches
  of **64 cells it falls to 0.44**, below the mean bands: only 2 of its 5 groups hold a reference point and the map is three large blocks (the figure above). So the
  result depends on the patch, and the best patch is one you could only choose with reference data. It is a result with this model, five bands, one image and
  a small reference set; it is **not evidence that embeddings beat colour in general**, and we do not claim that.
- **The patch sets the resolution of the embedding.** With patches of 8 or 16 cells the 2,599 objects have almost as many different
  vectors (2,599 and 2,576). With 32 cells there are 1,877, with 64 only 953: many objects share the patches and so the vector.
  The maps get blockier as the patch grows. In the map of 16 cells, **the lake** (the orange area at the top left) and
  the **dry cerrado** (the blue area on the right) read well, and the town and the wooded strips fall mostly into one large group (green), with a
  few pink patches at their edges: the embeddings tell the lake and the cerrado from the rest, and not the town from the trees.
- **Many columns carry nothing.** 102 to 295 of the 512 numbers are the same (zero) for every object, as they are after the last activation of a
  network that does not respond to this kind of image in these channels. They cost nothing but are what [[ml.kmeans]] warns about.
- The time of the node is small next to SAM's: 1.2 s for patches of 8 cells (about 5,300 patches), 0.6 s for 16, 0.7 s for 32 and 0.6 s for 64, on two logical cores
  (each of these runs alone; earlier runs with the four embeddings side by side, on an older build, took 5 to 24 s each).

## Settings that change the answer

**`points` is the setting of SAM.** The two sliders above are its sensitivity: on the NAIP window, 8, 16 and 24 prompts per side give
17, 128 and 171 objects, and the smallest things found change from tree crowns to roofs and pools. The cost grows with the **square**
of `points`. Start low and a small window; raise it until the things you care about are found.

**The size of the image.** Because a tile is resized to 1024 cells, the same `points` gives other objects on a window and on a whole image
(128 against 38 objects above). If you compare two images, compare them at the same ground size per prompt, not at the same `points`.

**`iou`, `stability_thresh` and `max_area`** decide which masks survive: raising `iou` or `stability_thresh` gives fewer and surer masks
(the operator's advice says so), and lowering `max_area` below 0.9 drops the masks that cover most of a tile. We did not vary them: use
the defaults first and change one at a time.

**`fill`.** The objects of the clip with and without it: 155 against 137 on the Sentinel-2 clip with 16 prompts. The difference is the
leftover areas between the masks. With `fill` false the cells with no mask have **no object**, which is what you want when the output is
a set of things (trees, roofs) over a background you do not care about.

**`weights`.** `full` is 348 MB against 75 MB. We ran only `quant` and cannot say how much `full` changes the masks or the time.

## Common pitfalls

- **Running SAM on a satellite image and being disappointed.** At 10 m a house is one cell. The model was trained on photographs in which
  things are tens of pixels wide. Try it on a small window before you wait for the whole image.
- **Believing `points` counts objects.** It is the grid; many prompts fall on the same thing and the masks are merged by the
  filters, and some things get no prompt.
- **Expecting homogeneous objects.** SAM's are things, not areas of one tone: 0.76 of the variance stays inside them on the NAIP window.
  A classification of them by their mean colour (parts 4 to 6) has more to do than on the classical objects.
- **The big leftover objects.** With `fill` the cells between the masks are objects too, and they can be very large (a whole street
  and lawn). Check the biggest objects before you classify, or run with `fill = false`.
- **Tiles.** The masks of a tile are cut at its edge, and an object that crosses the border between two tiles is **two objects**.
  Tiles are 1024 cells of the image, so only large images have it.
- **Three bands only.** The infrared of the image is not seen by SAM; describe the objects with [[objects.features]] afterwards.
- **Memory.** 3.6 GB in every run. Close what you can; do not run two SAM nodes at once.
- **Taking an embedding for the object's.** It is the mean of patches. With a patch much bigger than the objects it does not describe them.
- **A model that does not match.** `patch`, `bands` and the number of channels must be what the ONNX file takes, in the order it
  was trained with; the operator checks the shape, not the meaning. Two different band orders give embeddings that run without an error.
- **Weights with other licences.** SAM's export here is Apache-2.0; for an encoder you export, check the licence of its weights
  before you publish what you made with it.

## Going further

- Before this: [Large images](/tutorials/object-based-image-analysis/large-images/), for tiles and cost. The classical segmenters and
  their scale: [What an object is](/tutorials/object-based-image-analysis/what-is-an-object/) and
  [Choosing the scale](/tutorials/object-based-image-analysis/choosing-the-scale/).
- Grouping objects without samples: [Classes without samples](/tutorials/object-based-image-analysis/classes-without-samples/);
  describing them: [Describing objects](/tutorials/object-based-image-analysis/describing-objects/).
- The operators of this page: [[segment.sam]], [[objects.embed]], [[raster.clip]], [[table.select]], [[ml.kmeans]],
  [[objects.classify]].

## References

- He, K., Fan, H., Wu, Y., Xie, S., & Girshick, R. (2020). Momentum contrast for unsupervised visual representation learning. In
  *2020 IEEE/CVF Conference on Computer Vision and Pattern Recognition* (pp. 9726-9735). https://doi.org/10.1109/cvpr42600.2020.00975
- He, K., Zhang, X., Ren, S., & Sun, J. (2016). Deep residual learning for image recognition. In *2016 IEEE Conference on Computer
  Vision and Pattern Recognition* (pp. 770-778). https://doi.org/10.1109/cvpr.2016.90
- Kirillov, A., Mintun, E., Ravi, N., Mao, H., Rolland, C., Gustafson, L., Xiao, T., Whitehead, S., Berg, A. C., Lo, W.-Y., Dollár, P.,
  & Girshick, R. (2023). Segment anything. In *2023 IEEE/CVF International Conference on Computer Vision* (pp. 3992-4003).
  https://doi.org/10.1109/iccv51070.2023.00371
- Osco, L. P., Wu, Q., de Lemos, E. L., Gonçalves, W. N., Ramos, A. P. M., Li, J., & Marcato Junior, J. (2023). The Segment Anything Model
  (SAM) for remote sensing applications: from zero to one shot. *International Journal of Applied Earth Observation and
  Geoinformation*, 124, 103540. https://doi.org/10.1016/j.jag.2023.103540
- Wang, Y., Braham, N. A. A., Xiong, Z., Liu, C., Albrecht, C. M., & Zhu, X. X. (2023). SSL4EO-S12: a large-scale multimodal,
  multitemporal dataset for self-supervised learning in Earth observation. *IEEE Geoscience and Remote Sensing Magazine*, 11(3),
  98-106. https://doi.org/10.1109/mgrs.2023.3281651
