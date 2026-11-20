---
title: "Large images: tiles, time, memory and the cluster"
short: Large images
description: "Cut a big image into tiles that are segmented in parallel and stitched, see on real runs what that costs at the tile borders and what it saves in memory and time, read the application's time estimate, and send the tiles to other computers."
order: 120
level: advanced
time: 75 min
ops: [data.stac, raster.reproject, raster.calc, segment.multiresolution, segment.region_growing, objects.to_map, raster.write]
series: Object-based image analysis
part: 12
---

Everything so far ran on a 580 × 580 cell clip, a third of a million cells, in a second or two. A whole Sentinel-2 tile is
10,980 × 10,980 cells (120 million), and an aerial orthophoto of a county can be larger. Here you will meet the three limits
that appear first, **time**, **memory** and **the number of computers you have**, and the tool Terraflux has for them: cutting the
image into **tiles** that are segmented side by side and stitched back together. You will run the same segmentation on
clips of 1, 4 and 9 million cells, whole and by tiles, on one and on two threads, and read what the tiles save and what they
cost. The cost is not small, and the pages that praise tiles usually leave it out: at the end you will know what the tile
borders do to the objects, what the repair of the borders costs, and when it is better not to use tiles at all.

All the numbers on this page come from runs made for it, on one ordinary computer that was limited to **two threads** and was
sometimes busy with other work: they show the shape of things, not the speed of your machine. Where a number is not
ours, it says whose it is.

## The idea

### What goes wrong with a big image

A segmenter such as [[segment.multiresolution]] starts from single cells and merges them, pass after pass, visiting the
regions in a pseudo-random order. That order is the method (part 2), and it has two consequences for a big image.

- **Time.** One run is **one thread**: the merges of a region depend on those of its neighbours. A bigger image is not only
  more work, it is more than proportionally more work. Along our clips the time of the whole-image run grows as the number of
  cells raised to about 1.3 (see below); the application's estimate assumes 1.17 for this method.
- **Memory.** The run holds the image as numbers of 8 bytes a cell and band, a label for every cell and a record for every
  region it has not merged yet. We measured the peak working set of the process: 3.5 GB for a clip of 9 million cells and five
  bands. A scene of 120 million cells does not fit in most computers.

### Cutting the image: tiles

The answer is the one of every large-image method: **cut the image into pieces, work on each, put them back**. For
segmentation by region merging it was proposed by Happ and colleagues (2010), who ran the multiresolution method of Baatz and
Schäpe (2000) in parallel on blocks of an image and made the merges across the blocks afterwards; the Orfeo Toolbox, with its
large-scale mean shift and its region merging by local mutual best fit, does the same to segment images "of arbitrary size"
(Michel et al., 2015; Lassalle et al., 2014). Terraflux's version has four steps.

![How tiles work](/img/tutorials/object-based-image-analysis/large-images/scheme.png "The three steps of the tiles, on a toy: tiles with a margin, each window segmented alone, only the cores kept.")

1. The image is cut into square **tiles** of `tile` cells (the last ones are clipped at the edge of the image). Around each
   tile a **margin** of `margin` cells is added: the tile with its margin is its **window**, and its own cells are its **core**.
2. Each window is segmented **by itself**, with the ordinary one-thread method and a seed of its own (the run's seed and the
   tile's number). All the windows are done in parallel, as many at a time as there are threads and as the memory for tiles
   (2 GB for all of them) allows. The margin gives a border cell the neighbours it would have in the whole image.
3. From each window only the **core** is kept, so every cell of the image belongs to exactly one piece of one tile.
4. A **second pass** repairs the borders, on the whole image, with the same criterion and the same scale. The pieces that touch
   a tile border are **dissolved back to single cells** and grown again, across the border, by the same merging; the pieces in
   the middle of a tile are left as they are, because they were already at a fixpoint. An object that crosses a border, however
   large, is therefore grown whole again. This pass is one thread and is the extra cost of the method.

The result does not depend on the order the tiles finish in, so a run gives the **same objects on one thread and on
eight** (we checked that below). It does not give the same objects as a run without tiles, but, with this repair, the tile
borders leave **no visible seam**: the second half of this page measures what the difference is.

!!! science "Key idea"
    A segmentation by tiles is **not** the whole-image segmentation made faster. It is another segmentation with the same
    criterion, and a **different but equally good partition**: every tile is merged without knowing what happens beyond its
    window, and the second pass grows the border pieces again across it. The result differs from the whole-image one as much as
    another seed's would, and the borders show no cut. What you gain is **memory** (a tile at a time) and **parallelism** (many
    tiles at once). What you pay is **extra work** (the margins are segmented twice, the second pass once more), which on
    few threads is more than the parallelism returns.

### The settings

Both segmenters that have tiles take the same two settings.

| Setting | [[segment.multiresolution]] | [[segment.region_growing]] |
|---|---|---|
| `tile` | the side of the tiles in cells. **Default 0 is automatic**: the whole image at once up to 4 million cells, tiles of 1024 above. A number forces tiles of that side; a number as large as the image's longer side means no tiles. Ignored when `from` is given. | the side of the tiles in cells, 8 or more. **Default 0: no tiles**, the whole image at once and on one thread. |
| `margin` | the margin of every tile in cells. **Default 0 is automatic**: 64 cells, at most half the tile. | the same |

The other segmenters ([[segment.slic]], [[segment.watershed]], [[segment.felzenszwalb]], the threshold and the others of
part 2 and 7) have no tiles: they read the whole image. `bounds` (the zones no object may cross, part 8) works with tiles:
each tile gets the zones of its window.

## When to use it

Use tiles when:

- **the image does not fit** in the computer's memory with room for the rest of the model (below, the memory section);
- the run is **long** (the application offers tiles from 30 seconds of estimated time) and you have **threads to spare**: tiles
  pay only when several tiles run at once;
- you can accept **another valid partition** of the image: the objects are as good as the whole-image ones but not the same ones.

Do not use them:

- on a **small image**. We measured the multiresolution method on a clip of 1 million cells, one thread: 1.8 s whole and
  5.0 s with tiles of 512, nearly three times as long, for no gain in memory worth having;
- when you need **exactly the objects the whole image gives**, for example to compare with a run made earlier without tiles
  (the borders no longer show, but the objects are not the same);
- as the first thing to try. To choose a scale or a threshold, work on a **window** of the image: [[raster.clip]] in the model,
  or `terraflux run model.toml --window xmin,ymin,xmax,ymax` for a check run on a part of the map, and `--coarsen 4` for
  one cell in every four. Run the whole image once the settings are chosen.

## What you need

- **A large image.** This page uses a 30 × 30 km window of the Sentinel-2 scene of part 2 (9 August 2024, blue, green, red, near
  infrared and `swir16`) around the Paranoá lake: 3,000 × 3,000 cells of 10 m, 9 million cells in five bands. The model of the
  next section makes it with [[data.stac]]; it is about 135 MB as it is used here, too big to offer as a download, and takes a
  couple of minutes to fetch. The windows of 500, 1,000 and 2,000 cells in the measurements are cut from its centre.
- **The small clip of the series** for the seams (580 × 580 cells, part 2). Download
  [sentinel2_brasilia.tif](/data/tutorials/object-based-image-analysis/sentinel2_brasilia.tif) (2.3 MB) if you work without a
  network; the attribution is in [ATTRIBUTION.txt](/data/tutorials/object-based-image-analysis/ATTRIBUTION.txt).
- **Memory and threads**. The whole-image run of 9 million cells needs about 4 GB free; the run by tiles of 512 about 0.4 GB. Terraflux
  never uses every core of the computer: `--threads` (or the application's setting) asks for fewer.

## Build the model

### Step 1: the image

The first three nodes are those of the earlier parts: [[data.stac]] reads the scene, [[raster.reproject]] puts it on square 10 m
cells in UTM zone 23 S, and [[raster.calc]] turns the integers into reflectance from 0 to 1. Only the extent changes, from the
5.8 km of the clip to 30 km.

### Step 2: the segmentation by tiles

[[segment.multiresolution]] reads `refl.image`, with the scale of part 3 (2.14, the first scale ESP suggests for this scene).
`tile` and `margin` are given here in full although tiles of 1024 and a margin of 64 are what this image would get without them,
because it is above 4 million cells.

```tfgraph
#! caption: The whole model: a 30 km window of the scene, its reflectance, the multiresolution segmentation by tiles of 1024 with a margin of 64, and the objects written as a map.
#! highlight: segs
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "s2"
op = "data.stac"
params = { url = "https://earth-search.aws.element84.com/v1", collection = "sentinel-2-l2a", assets = ["blue", "green", "red", "nir", "swir16"], datetime = "2024-08-09", extent = [-48.02, -16.02, -47.705, -15.705], cellsize = 0.00009 }
note = "A box in degrees a little larger than the 30 km window (about two minutes)."

[[nodes]]
id = "utm"
op = "raster.reproject"
inputs = { raster = "s2.raster" }
params = { crs = "EPSG:31983", resolution = 10, extent = [178420, 8229180, 208420, 8259180] }
note = "The 3,000 by 3,000 cell window."

[[nodes]]
id = "refl"
op = "raster.calc"
inputs = { s = "utm.raster" }
params = { expression = "out image = s / 10000" }
note = "Reflectance 0 to 1."

[[nodes]]
id = "segs"
op = "segment.multiresolution"
inputs = { image = "refl.image" }
params = { scale = 2.14, tile = 1024, margin = 64 }
note = "Nine tiles of 1024 with a margin of 64, segmented in parallel and stitched."

[[nodes]]
id = "map"
op = "objects.to_map"
inputs = { objects = "segs.objects" }
params = { what = "id" }

[[nodes]]
id = "write"
op = "raster.write"
inputs = { value = "map.map" }
params = { path = "objects.tif" }
```

For the region growing method the node is the same with its own settings. It has no tiles unless you ask for them:

```tfgraph
#! caption: Region growing by tiles: a threshold of 0.05 on the bands scaled to 0-1, specks of under 20 cells joined to a neighbour, tiles of 512.
#! highlight: rg
#! fragment: true
[[nodes]]
id = "rg"
op = "segment.region_growing"
inputs = { image = "refl.image" }
params = { threshold = 0.05, minsize = 20, tile = 512 }
note = "Tiles of 512 with the automatic margin (64)."
```

=== "Application"

    Build the nodes above as in the earlier parts (click an operator in the library, **drag from an output port to an input
    port**). Select `segs`: in the Node pane, on the row of the last run's time, the application tells **how long the node
    will take before you run it**, from the size of its image (read from the file's header or from the value its input
    gave), in the form `about 5 min on 8 threads; by tiles of 512, about 1 min` (the numbers are only an example of the form). It appears from 2 seconds of estimate on. When the
    estimate is 30 seconds or more and tiles would be faster, the row has a **Use tiles** button, which writes
    `tile = 512` in the node as one step of undo (for a segmenter that has the setting, and only at the top level of the
    model). Run to the node with ++ctrl+enter++.

=== "Command line"

    Save the graph as `large.toml`. `--profile` prints the seconds each node took and about how much memory what it gave holds:

    ```bash
    terraflux run large.toml --threads 2 --profile --out output
    ```

=== "Without a network"

    The 30 km window is not offered for download. Use an image of your own, written once with [[raster.write]] and read with
    [[raster.read]] (see the pitfalls for why a file is read differently from a map in memory), or the small clip of the
    series, in which case no tiles are needed.

## Read the results

### The borders

We segmented the 580 × 580 clip of part 2 at the scale 2.14, whole and with tiles of 200 cells (three by three tiles, the last
ones 180 cells wide), margin 64. The figure draws the objects' outlines in grey and, in magenta, every place on a tile border
where the two cells that face each other are **in different objects**. The first panel is the whole-image run with the same
lines drawn only for reference.

![Objects at the tile borders](/img/tutorials/object-based-image-analysis/large-images/seams_mr.png "Multiresolution, scale 2.14. Magenta: along the lines where the tiles meet (every 200 cells), the pairs of neighbouring cells in different objects. The whole-image run (left) has some, as every line of an image has; the tiled runs have about as many: the borders show no seam.")

There is no seam to see. In numbers, 10 % of the pairs of neighbouring cells along any line of the image are in different objects,
and that is also the share on those lines in the whole-image run. Across the tile borders it is **6.8 %** with a margin of 64 and
**9.1 %** with a margin of 1: the same as anywhere else. In earlier versions of Terraflux, which merged the border pieces instead of growing them again, 71 % and 88 % of those pairs
were cut; the repair of step 4 is what removed them.

A window around the crossing of two borders, in the town of the clip (the tile borders are faint dashed lines, magenta where objects are cut):

```tfgallery
columns: 3
caption: "The objects around the crossing of two tile borders (140 by 140 cells, 5 times enlarged). Left: no tiles. Middle: tiles of 200, margin 64. Right: tiles of 200, margin 1."
images:
  - { src: /img/tutorials/object-based-image-analysis/large-images/win_whole.png, caption: "Whole image" }
  - { src: /img/tutorials/object-based-image-analysis/large-images/win_tiles.png, caption: "Tiles, margin 64" }
  - { src: /img/tutorials/object-based-image-analysis/large-images/win_margin1.png, caption: "Tiles, margin 1" }
```

Why does the repair work where a plain merge of the pieces did not? Two windows that share a border **segment the overlap on their
own**, and they do not agree, so the piece of an object on the left of the border and the piece on the right are parts of two
different objects of two different segmentations, and the merge criterion keeps them apart. The second pass does not try to merge
those pieces: it **dissolves** the ones that touch a border into cells and grows them again, across the border, with the same
criterion and the same scale as everywhere else. An object that crosses a border is therefore grown whole, however large.

The region growing method gets the same repair:

![Region growing: the borders](/img/tutorials/object-based-image-analysis/large-images/seams_rg.png "Region growing, threshold 0.05, tiles of 200, same drawing. Along the borders 7 % of the pairs are in different objects with a margin of 64 and 9 % with a margin of 1, against 7 % along any other line.")

### Different, but how different?

The tiled segmentation is not the whole-image one: it is **another valid partition**. How different, compared with how much two honest
runs of the method differ among themselves? The merging order is pseudo-random, and another seed gives other objects. We compared the boundaries of two
segmentations by the share of each one's boundary cells that lie within one cell of the other's, and took the harmonic mean of
the two shares, an F-measure (1 is the same boundaries):

```tftable
align: { run: left }
labels: { run: Run, objects: Objects, vs_whole_f: "F against the whole run, seed 0", on_borders: "Pairs in different objects on the borders (%)", elsewhere: "On other lines (%)" }
digits: { objects: 0, vs_whole_f: 3, on_borders: 1, elsewhere: 1 }
data: /data/tutorials/object-based-image-analysis/large-images/seams_mr.csv
caption: "Multiresolution, scale 2.14, on the 580 by 580 clip. The seed-1 and seed-2 whole runs are the reference for how much two runs of the same method differ; the tiles of 200 are 3 by 3."
```

The F of the tiled run against the whole one (0.61) is the **same as that of another seed against the whole one** (0.62, 0.61): a
run by tiles is as far from the whole-image run as a second whole-image run is, and the border column (7 % against 10 %) shows no
cut. The same table for region growing says the same:

```tftable
align: { run: left }
labels: { run: Run, objects: Objects, vs_whole_f: "F against the whole run", on_borders: "On the borders (%)", elsewhere: "On other lines (%)" }
digits: { objects: 0, vs_whole_f: 3, on_borders: 1, elsewhere: 1 }
data: /data/tutorials/object-based-image-analysis/large-images/seams_rg.csv
caption: "Region growing, threshold 0.05, on the same clip. With minsize 20 the F of the rows is taken against the whole run with minsize 20."
```

The number of objects changes little with the multiresolution method: on the larger clips the tiled run has 0.5 to 1.7 % fewer objects.
With region growing and `minsize` 20 it has **5 % more on 1000 by 1000 cells and 10 % more on 2000 by 2000**, so the count is not
the same as the whole run's: if you compare counts or mean areas with a whole-image run, expect that difference.

```tftable
align: { method: left }
labels: { method: Method, size: "Side (cells)", whole: "Objects, whole", tiles: "Objects, tiles of 512", difference_pct: "Difference (%)" }
digits: { size: 0, whole: 0, tiles: 0, difference_pct: 1 }
data: /data/tutorials/object-based-image-analysis/large-images/counts.csv
caption: "Objects found on windows of the 30 km clip, whole and by tiles of 512 (multiresolution at scale 2.14; region growing at threshold 0.05, minsize 20), on two threads."
```

What remains of the difference is mostly at the coarse scales: on the small clip at the scales 4 and 8 the tiled run has 643 and 151 objects
against 739 and 187 (13 and 19 % fewer), while the share of pairs in different objects across the borders stays equal to that elsewhere (7.0 against 5.3 % at the scale 4; 2.5 against 2.6 % at the scale 8).

### The same objects on any number of threads

We ran the same 28 segmentations of the small clip (18 of them by tiles, with other tile sizes, margins, scales and seeds), once on one thread and once on two, and compared the
label maps cell by cell: **all 28 are identical**. This is what the tile numbers in the seed are for. Tiles are not a source of
irreproducibility: for a given `tile`, `margin` and seed, the objects are the same on a laptop and on a cluster.

### Time

We timed the segmenter's node (from `--profile`, the best of two runs up to 2,000 by 2,000 cells and one run for 3,000 by 3,000) on one and on two
threads, whole and with tiles of 512, on windows of 1, 4 and 9 million cells of the 30 km clip, five bands. The image was
read from a GeoTIFF on disk by `raster.read`, as a large image usually is.

```tfchart
type: line
data: /data/tutorials/object-based-image-analysis/large-images/time_multiresolution.csv
x: cells_m
y: [whole_1, whole_2, tiles_1, tiles_2]
labels: { whole_1: "Whole image, 1 thread", whole_2: "Whole image, 2 threads", tiles_1: "Tiles of 512, 1 thread", tiles_2: "Tiles of 512, 2 threads" }
colors: { whole_1: "#9aa39d", whole_2: "#3b8a5a", tiles_1: "#e8901f", tiles_2: "#2f6f9f" }
xlabel: Cells (millions)
ylabel: Seconds
height: 320
caption: "segment.multiresolution at scale 2.14 on 1, 4 and 9 million cells (five bands), with and without tiles. A whole-image run is one thread whatever you ask for: the grey and green lines are nearly one line."
```

```tftable
align: { size: left }
labels: { size: "Side (cells)", cells_m: "Cells (millions)", whole_1: "Whole, 1 thread (s)", whole_2: "Whole, 2 threads (s)", tiles_1: "Tiles of 512, 1 thread (s)", tiles_2: "Tiles of 512, 2 threads (s)" }
columns: [size, cells_m, whole_1, whole_2, tiles_1, tiles_2]
digits: { size: 0, cells_m: 0, whole_1: 1, whole_2: 1, tiles_1: 1, tiles_2: 1 }
data: /data/tutorials/object-based-image-analysis/large-images/time_multiresolution.csv
caption: "The same runs, as numbers. The 500 by 500 window (0.4 s whole) is left out: a tile of 512 holds all of it."
```

What it says:

- **The whole-image run does not use the second thread**: 12.7 s and 10.8 s on 2,000 by 2,000 cells (the difference is noise).
- **With tiles, two threads take 1.6 to 1.8 times less than one** (5.0 → 2.7 s, 29.5 → 17.0 s, 71.8 → 44.4 s). The ideal is 2: the repair
  pass over the borders is one thread, which pulls the speed-up down, and single timings on a busy machine are noisy.
- **Tiles took more total work than the whole image**, so on two threads they were not faster than the whole run: 17.0 s against 10.8 s on
  4 million cells, 44 s against 28 s on 9 million. The windows overlap by their margins (a tile of 512 with a margin of 64 is
  a window of 640, 56 % more cells), and the pass that repairs the borders goes over them once more.
- **The time grows faster than the cells**: the whole run takes 1.8 s on 1 million cells and 31.4 s on 9 million, which is 17.6
  times as long for 9 times the cells, an exponent of 1.30. The application's estimate uses 1.17, from the project's benchmark
  between 1 and 4 million cells.

So **on two threads, tiles do not make the multiresolution method faster here**. The project's own benchmark (a synthetic
image of 4 million cells and four bands, 8 threads, run by the project's developers on a machine that was doing other work, and not repeated here) says where they
pay: the whole image takes 7.2 s on one thread and 7.2 s on 8; with tiles of 512 it takes 8.8 s on one thread and **3.2 s on 8**.
The same table for region growing, whose whole-image run is also one thread: 16.2 s whole against 11.8 s by tiles on one thread
and 3.7 s on 8 (these are the project's numbers from before the borders were repaired, so the tiled ones are now somewhat higher: the project measured about 19 % more time at 4 threads for the repair). And for a whole Sentinel-2 scene (10,980 by 10,980 cells, 10 bands, 16-bit values, scale 300, tiles of 512,
8 threads), the project measured 413 s for the segmentation, 175,639 objects and a peak of 2.07 GB. We did not repeat these;
we can only say they were not ours.

Region growing, which has no automatic tiles, **does not gain from them on two threads either**, now that the borders are repaired:

```tfchart
type: line
data: /data/tutorials/object-based-image-analysis/large-images/time_region_growing.csv
x: cells_m
y: [whole_1, whole_2, tiles_1, tiles_2]
labels: { whole_1: "Whole image, 1 thread", whole_2: "Whole image, 2 threads", tiles_1: "Tiles of 512, 1 thread", tiles_2: "Tiles of 512, 2 threads" }
colors: { whole_1: "#9aa39d", whole_2: "#3b8a5a", tiles_1: "#e8901f", tiles_2: "#2f6f9f" }
xlabel: Cells (millions)
ylabel: Seconds
height: 320
caption: "segment.region_growing (threshold 0.05, minsize 20) on 1 and 4 million cells, with and without tiles. We did not run it on the 9 million cell window."
```

On 4 million cells it takes 8.5 s whole on two threads and **13.9 s by tiles**; 10.4 s whole and 18.7 s by tiles on one. (Before the repair of the borders we had measured 7.5 s by tiles on two threads: the repair is not free for this method.)

### Memory

The same runs, the peak working set of the process (read by a script every 50 ms while the model ran). It includes the process's
own start, a few tens of megabytes.

```tfchart
type: bar
data: /data/tutorials/object-based-image-analysis/large-images/time_multiresolution.csv
x: size
y: [mb_whole_2, mb_tiles_2]
labels: { mb_whole_2: "Whole image", mb_tiles_2: "Tiles of 512" }
colors: { mb_whole_2: "#9aa39d", mb_tiles_2: "#3b8a5a" }
xlabel: "Side of the window (cells)"
ylabel: "Peak memory (MB)"
height: 320
caption: "Peak memory of segment.multiresolution on windows of 1, 4 and 9 million cells (two threads). The whole-image run grows with the image; the tiled run with the number of tiles in flight, a few hundred megabytes."
```

The whole-image run holds 400 MB on 1 million cells, 1.6 GB on 4 million and **3.5 GB on 9 million**; the run by tiles holds 270, 320
and **520 MB** (two threads, two tiles at a time; the repair of the borders keeps some more in memory than the plain tiles did, 380 MB at 9 million cells before it).
For region growing the order is the other way round on the windows we ran: 350 MB and 1.4 GB by tiles against 230 and 870 MB whole, so for that method tiles are not a way to save memory here. That is the one true gain of tiles on a computer with few cores: the
**image no longer has to fit in memory**. With tiles and a raster on disk, the image is read one window at a time, and the
labels (4 bytes a cell) are kept in a temporary file mapped into memory when they would take more than about a sixteenth of the
computer's memory. Terraflux refuses to read whole, from disk, an image whose numbers (8 bytes a cell and band) would take
more than **half the computer's memory**, and says so, with what to do: read it with `raster.read` and give `tile` above 0.

### Reading from disk costs time

The images above were read from a GeoTIFF. The same 2,000 by 2,000 window, tiles of 512, from a striped GeoTIFF (as written
above), from a tiled one (256 by 256 blocks) and from memory (a [[raster.calc]] copy of it):

```tftable
align: { layout: left }
labels: { layout: Layout, threads: Threads, seconds: "Seconds", peak_mb: "Peak memory (MB)" }
digits: { threads: 0, seconds: 1, peak_mb: 0 }
data: /data/tutorials/object-based-image-analysis/large-images/layouts.csv
caption: "segment.multiresolution with tiles of 512 on a 2,000 by 2,000 window, five bands, by where the image is read from."
```

From a file the run took **2.1 to 2.5 times** as long as from memory (26 s against 12 s on one thread) and a tiled GeoTIFF was not faster than a
striped one. It also used **less than half the memory** (242 MB against 552 MB). We did not look for the reason: the
windows of the tiles overlap, and the compressed file is read and decoded again for each. If the image fits in memory with room
to spare, read it into the model (a [[raster.calc]] after the [[raster.read]], or the map of a `data.stac`) and use tiles for the threads; use the file when it does not fit.

## The cluster

When one computer is not enough, the tiles can go to **other computers**. The first pass (the windows) is the part that
splits: each window is independent of the others. The pass that repairs the borders stays on the computer that runs the model. The
computers need nothing but Terraflux: the same coordinator and workers as for batches of runs, described in
[The lab's computers](/parallel/lab-computers/) and, for a university cluster, [A university cluster with Slurm](/parallel/slurm-cluster/);
[Maps larger than memory](/parallel/maps-larger-than-memory/) tells the same story for the land change model.

=== "Command line"

    On the computer that runs the model, after starting a coordinator (`terraflux serve --code`) and workers
    (`terraflux worker --code CODE`) as in the lab page:

    ```bash
    terraflux run large.toml --coordinator host:7700 --code CODE
    ```

    Without `--tiled`, only two things go to the workers: the tiles of [[segment.multiresolution]], and the features of
    [[objects.features]] when its image was read with [[raster.read]] from disk. The workers get the file through the coordinator;
    `--shared-disk` says the workers read the same files at the same paths, so that nothing but sums travels.

=== "Application"

    Tick **Let other computers help** in the Computers pane (the same setting as for batches). When other computers are working
    for this one, the tiles of the multiresolution method in the model go to them; if they cannot (no connection, or a tile that
    fails there), the tiles run **here**, with a warning in the run, and the model finishes.

What we could check is **that the objects are the same**. We started a coordinator and one worker on this computer (both
on `127.0.0.1`, one thread each), segmented a 500 by 500 window with tiles of 128 (16 tiles) through them, and compared the
label map with that of the same run without a cluster: **identical, cell by cell**. A coordinator and a worker on one computer
cannot show a speed-up: the run through them took 2.9 s against 2.1 s here, the price of sending the file and the tiles. We
did not have computers to measure what a real cluster gives, and we will not put a number on it.

Two limits to know. The tiles of **region growing** do not go to the cluster: they use the threads of the computer that runs the
model. And the cluster takes the image only **as a file** (`raster.read`): an image made in the model, such as the
reflectance of a [[raster.calc]], is worked on here.

## Settings that change the answer

**The size of the tiles.** Move the slider: the same 580 by 580 clip, the multiresolution scale 2.14, with tiles of 400, 300,
200, 150, 100 and 64 cells (and with the whole image first, drawn with the lines of 200 for reference). The chart is the share
of neighbouring pairs in different objects, along the tile borders and along any other line.

```tfslider
label: Tile side (cells)
caption: "Tile side from 580 (the whole image: one tile) to 64. Magenta would mark the tile borders where the objects are cut: there is almost none at any size."
frames:
  - { value: 580, src: /img/tutorials/object-based-image-analysis/large-images/tile_whole.png, caption: "Whole image (lines of 200 imagined)", stats: { Objects: "2,599", "On the borders": "9.9 %", Elsewhere: "10.3 %" } }
  - { value: 400, src: /img/tutorials/object-based-image-analysis/large-images/tile_400.png, caption: "Tiles of 400", stats: { Objects: "2,608", "On the borders": "9.1 %", Elsewhere: "10.3 %" } }
  - { value: 300, src: /img/tutorials/object-based-image-analysis/large-images/tile_300.png, caption: "Tiles of 300", stats: { Objects: "2,587", "On the borders": "9.5 %", Elsewhere: "10.3 %" } }
  - { value: 200, src: /img/tutorials/object-based-image-analysis/large-images/tile_200.png, caption: "Tiles of 200", stats: { Objects: "2,549", "On the borders": "6.8 %", Elsewhere: "10.2 %" } }
  - { value: 150, src: /img/tutorials/object-based-image-analysis/large-images/tile_150.png, caption: "Tiles of 150", stats: { Objects: "2,576", "On the borders": "9.4 %", Elsewhere: "10.2 %" } }
  - { value: 100, src: /img/tutorials/object-based-image-analysis/large-images/tile_100.png, caption: "Tiles of 100", stats: { Objects: "2,397", "On the borders": "9.4 %", Elsewhere: "9.9 %" } }
  - { value: 64, src: /img/tutorials/object-based-image-analysis/large-images/tile_64.png, caption: "Tiles of 64", stats: { Objects: "2,417", "On the borders": "8.9 %", Elsewhere: "9.9 %" } }
chart:
  type: line
  data: /data/tutorials/object-based-image-analysis/large-images/tile_size.csv
  x: tile
  y: [on_borders, elsewhere]
  labels: { on_borders: "Along the tile borders", elsewhere: "Along other lines" }
  colors: { on_borders: "#c0398f", elsewhere: "#3b8a5a" }
  ylabel: "Pairs in different objects (%)"
```

The **share on the borders is the same as elsewhere at every size** (7 to 9 % against 10 %), and the F against the whole run
stays at 0.60 to 0.61, the value of another seed. What the tile size changes is the **number of objects**: it is within 2 % of the whole
run for tiles of 200 or more and **7 to 8 % lower** for tiles of 100 and 64 (2,397 and 2,417 against 2,599), where a tile is not much bigger than a few objects.
It also changes the **work**: the smaller the tile, the larger the share of the cells that are in a margin or on a border to repair
(on a scene of 10,980 cells with tiles of 1024, the border lines are 0.18 % of the cells).

**The margin.** Tiles of 200, margins of 1, 4, 16, 32 and 64:

```tfchart
type: line
data: /data/tutorials/object-based-image-analysis/large-images/margin.csv
x: margin
y: [on_borders, elsewhere]
labels: { on_borders: "Along the tile borders", elsewhere: "Along other lines" }
colors: { on_borders: "#c0398f", elsewhere: "#3b8a5a" }
xlabel: "Margin (cells)"
ylabel: "Pairs in different objects (%)"
height: 280
caption: "Tiles of 200, scale 2.14. At every margin, from 1 to 64, the border is cut about as often as any other line (7 to 11 % against 10 %)."
```

With the borders repaired, **the margin hardly matters here**: the share on the borders is 7 to 11 % at every margin and the F against the whole run is 0.61;
the objects number 2,514 with a margin of 1 and 2,549 with 64. The margin costs work (it is segmented twice), so
a smaller one is cheaper; we did not measure the time at each margin, so we cannot say what you save. The default of 64 is the safe choice.

**The scale.** The same tiles of 200, other scales (the multiresolution scale is in the image's units, part 3):

```tfchart
type: line
data: /data/tutorials/object-based-image-analysis/large-images/scale.csv
x: scale
y: [tiles_on_borders, whole_on_lines, elsewhere]
labels: { tiles_on_borders: "Tiles, along the borders", whole_on_lines: "Whole image, along those lines", elsewhere: "Tiles, along other lines" }
colors: { tiles_on_borders: "#c0398f", whole_on_lines: "#9aa39d", elsewhere: "#3b8a5a" }
xlabel: Scale
ylabel: "Pairs in different objects (%)"
height: 280
caption: "Tiles of 200, margin 64, at the scales 1, 2.14, 4 and 8 of the 580 by 580 clip."
```

At every scale the borders are about as cut as any other line (scale 1: 19 % against 20 %; scale 8: 2.5 % against 2.6 %). What changes with the scale is the **count**: the tiled run had 10,938,
2,549, 643 and 151 objects at the scales 1, 2.14, 4 and 8, against 11,145, 2,599, 739 and 187 for the whole image (2, 2, 13 and 19 % fewer). The coarser the
scale, the more the objects approach the tile and the more the two partitions differ in how many objects they make.
For a coarse scale, use tiles several times larger than the objects.

## Common pitfalls

- **Tiles on a small image.** Under about 4 million cells the default is no tiles for a reason. Forcing them cost nearly three times
  the time on 1 million cells.
- **Believing tiles give the whole-image objects.** They give another valid partition, as different from the whole-image
  one as another seed would be, with no cut along the borders. Do not compare a tiled run with one made without tiles
  object by object, and expect a different count (about 1 % fewer for the multiresolution method on the larger clips, 13 to 19 % fewer at coarse
  scales on the small clip, 5 to 10 % more for region growing with `minsize` 20); and do not change `tile` between the runs of a study you will compare.
- **Expecting a speed-up on one or two threads.** The tiles cost more total work, and with the borders repaired that holds for region
  growing too. They pay when there are many threads (the project's benchmark, before the repair: 3.2 s against 7.2 s on 8 for the multiresolution
  method) or when memory forces them, and then only for the multiresolution method on our runs.
- **Reading a file that does not fit.** The refusal to read an image above half the computer's memory is a safeguard, not a
  limit of the method. Read the image with [[raster.read]] and give `tile`; the objects are made from windows of the file.
- **Tiles for the other segmenters.** Only the multiresolution and the region growing methods have tiles. [[segment.slic]],
  [[segment.watershed]] and the others read the whole image: for a very large one, try them on a window first.
- **Trusting the time estimate to the second.** The estimate comes from the project's benchmark machine and from the size of
  the image; it ignores the content of the image and, for most settings, their value (the multiresolution `scale` changes the
  time by a few percent, but the region growing `threshold` moves it in different directions on different sizes). The
  application corrects it by the ratio between what it estimated and what it saw the last time the operator ran on your
  computer. For our 9 million cell whole-image run, the benchmark's rates extended with their 1.17 exponent would have told about
  19 s for four bands (a little more for five); it took 28 to 31 s, with five bands and on a busy machine.
- **Memory of the whole model.** The peaks above are of the segmenter. The tables of features, the polygons and the
  classification of a large image have their own, and the application estimates the memory of a run from the sizes of the maps in
  the model and asks (the *Not much memory* window, *Run anyway* or *Cancel*) when it may not fit.

## Going further

- Before this: [Levels](/tutorials/object-based-image-analysis/levels/), which builds
  fine and coarse objects with `from` (tiles are ignored there). Next in the series:
  [Deep segmentation](/tutorials/object-based-image-analysis/deep-segmentation/), the segmenters that need a model file.
- To choose the settings of a big image, run on a window first: [Choosing the scale](/tutorials/object-based-image-analysis/choosing-the-scale/).
- Many computers: [The lab's computers](/parallel/lab-computers/), [A university cluster with Slurm](/parallel/slurm-cluster/),
  [Maps larger than memory](/parallel/maps-larger-than-memory/), [Parallel and distributed computing](/parallel/).
- The operators of this page: [[segment.multiresolution]], [[segment.region_growing]], [[data.stac]], [[raster.reproject]],
  [[raster.calc]], [[objects.to_map]], [[raster.write]].

## References

- Baatz, M., & Schäpe, A. (2000). Multiresolution segmentation: an optimization approach for high quality
  multi-scale image segmentation. In *Angewandte Geographische Informationsverarbeitung XII* (pp. 12-23). Wichmann.
- Happ, P. N., Ferreira, R. S., Bentes, C., & Feitosa, R. Q. (2010). Multiresolution segmentation: a parallel approach for high
  resolution image segmentation in multicore architectures. *The International Archives of the Photogrammetry, Remote Sensing and
  Spatial Information Sciences*, XXXVIII-4/C7 (GEOBIA 2010). https://www.isprs.org/proceedings/XXXVIII/4-C7/
- Lassalle, P., Inglada, J., Michel, J., Grizonnet, M., & Malik, J. (2014). Large scale region-merging segmentation using the
  local mutual best fitting concept. In *2014 IEEE Geoscience and Remote Sensing Symposium* (pp. 4887-4890).
  https://doi.org/10.1109/igarss.2014.6947590
- Michel, J., Youssefi, D., & Grizonnet, M. (2015). Stable mean-shift algorithm and its application to the segmentation of
  arbitrarily large remote sensing images. *IEEE Transactions on Geoscience and Remote Sensing*, 53(2), 952-964.
  https://doi.org/10.1109/tgrs.2014.2330857
