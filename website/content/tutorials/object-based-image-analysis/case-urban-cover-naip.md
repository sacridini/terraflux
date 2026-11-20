---
title: "Case study: urban cover of a neighbourhood from NAIP imagery"
short: "Case: urban cover (NAIP)"
description: "Map roofs, streets, trees, lawns and water in a Texas suburb from 0.6 m aerial imagery read straight from the Planetary Computer: fine objects, rules, patches, splitting and skeletons, a smoothed map and polygons for a report, and an honest accuracy assessment."
order: 140
level: advanced
time: 120 min
ops: [data.stac, raster.calc, segment.multiresolution, objects.features, objects.calc, objects.rules, objects.merge, objects.split, objects.resize, objects.to_map, objects.to_vector, objects.sample, objects.accuracy, objects.reclassify]
series: Object-based image analysis
part: 14
---

A city planner asks how much of a neighbourhood is roof, how much is street, and how much is shaded by trees. The answer
decides where to plant, how much rain runs off, and which blocks run hot in summer. In this tutorial you will answer it for
600 m of a suburb of Plano, Texas, from aerial photographs with cells of 0.6 m. You will read the images directly from a
public catalogue, build the map in the way the earlier parts taught (objects, rules, levels), add three operators that
only objects make possible (merging into patches, splitting touching things apart, and measuring a skeleton), and then do the
step many case studies skip: measure how wrong the map is, with a reference you draw and read yourself. At the end you will have
a map as a GeoTIFF, polygons as a GeoPackage, a table of areas that takes the map's errors into account, and a clear idea of
what you may and may not say about the result.

The map does not come out well, and that is part of the lesson. The first assessment gives an overall accuracy of 52 % by objects (72 % weighted by area), and
the page shows where and why. If you want a map that is good enough to put in a report, read the section "What you can report":
the answer is a map with fewer classes, not a better threshold.

## The idea

Cities are small-scale mosaics. A roof is 10 m wide, a street 8 m, a tree crown 6 m, a garden lawn 12 m, and a pond can be
smaller than a classroom. Sentinel-2's 10 m cells (parts 2 to 12 of this series) blur most of these into mixed cells.
**NAIP**, the National Agriculture Imagery Program of the US Department of Agriculture, photographs the continental United
States from a plane every two or three years, in the growing season, with red, green, blue and near-infrared bands and cells of
0.6 to 1 m. It is free and in the public domain. At 0.6 m a roof is hundreds of cells and a street is a line of cells fifteen
wide: each thing is an **object** in the plain sense, and the objects can be described by their shape as well as their colour.

![The NAIP clip](/img/tutorials/object-based-image-analysis/case-urban-cover-naip/clip.png "The clip of this page: 600 m by 600 m of a suburb of Plano, Texas, in true colour, NAIP of 14 October 2020. Houses with gardens and swimming pools, streets, a park with a lawn and a pond, and a great deal of tree canopy. The yellow box is the detail that most figures show.")

!!! science "Key idea"
    **Colour first, then shape, then context.** A simple rule on colour separates the vegetation from the paved things and
    from the water (the NDVI and NDWI of part 4). It does not separate a roof from a street (both are grey), or a lawn from a tree
    in shade (both are green and dark). What separates them is **geometry and context**: a house is a compact, wide patch that
    touches a lawn, a street is a long, narrow strip that joins other streets, a lawn is smooth and a canopy is textured. The
    plan of the page follows that order:

    1. **Fine objects** (multiresolution segmentation) and a **first pass of rules** on colour and texture: water, tree, lawn,
       pavement (roofs and streets, not yet apart).
    2. **Patches**: neighbouring objects of one class merged (a level above the fine objects).
    3. **Pieces**: the patches that are really several things that touch (a street network and the houses its driveways
       join) split at their necks, with [[objects.split]].
    4. **Shape**: the skeleton of each piece ([[objects.features]]) and the blue-minus-red tone decide roof or street in a
       **second pass of rules**.
    5. **Outlines** smoothed, **polygons** written, and an **accuracy assessment** against a reference.

This is the object-based way of working that Blaschke (2010) reviewed and that studies of urban cover with aerial imagery use.
Myint et al. (2011) compared per-pixel and object-based classification of urban land cover from high-resolution imagery and reported
the higher accuracy for objects; O'Neil-Dunne et al. (2014) describe a production workflow for urban tree-canopy maps that fuses
LiDAR and imagery, then refines the first objects by context, morphological smoothing and gap filling, the same moves as the
last steps of this page. Splitting uses the watershed of Vincent and Soille (1991) and the skeleton the thinning of Zhang and Suen
(1984); the accuracy assessment follows Olofsson et al. (2014).

## When to use it

- You want **urban land cover** (impervious surface, tree canopy, grass) from imagery with cells under 1 m: the percentage of
  canopy by neighbourhood, the area of roofs per parcel, the impervious area that drains to a pond.
- Your classes differ in **shape and context** as much as in colour, and you can write the difference in words.
- You need the result as **polygons** (a GIS layer to join to parcels) and not only as a raster.

It is **not** the tool when you have a **height model** (LiDAR): with a canopy height model, trees and buildings separate
directly and a threshold on height beats everything on this page. It is also not for imagery with cells of 10 m or more, where
a roof is a cell or two ([part 6](/tutorials/object-based-image-analysis/rules-and-class-trees/) and
[part 5](/tutorials/object-based-image-analysis/classifying-by-samples/) fit better), and it does not give the *use* of land
(residential, commercial): that needs parcels and more data. For a protected area seen from Sentinel-2 with more classes and a
fuller accuracy assessment, see [part 15](/tutorials/object-based-image-analysis/case-protected-area-sentinel2/).

## What you need

- **Four-band aerial imagery.** NAIP, read by [[data.stac]] from the Microsoft Planetary Computer: `collection = "naip"`,
  `assets = ["image"]` (one asset with four bands) and `sign = "planetary_computer"`, which asks the catalogue for the access
  token of the files (no key, no account). The item's bands come out as four layers named `red`, `green`, `blue` and `nir`. The
  catalogue stores NAIP in UTM metres, so you also give `crs = "EPSG:26914"` (NAD83 / UTM zone 14N) and an `extent` and a
  `cellsize` in metres: the data are warped straight onto your grid, with one resampling.
- **A date.** Planetary Computer lists this tile for 2018, 2020 and 2022 at 0.6 m (and older years at 1 m), and the three images
  are not equal:
- **A way to read an image by eye**, for the reference. Nothing replaces going to the place; when you cannot, a person who
  reads the image, honestly and blind to the map, is the next best.

![The same place in three years](/img/tutorials/object-based-image-analysis/case-urban-cover-naip/dates.png "The detail in the three NAIP flights of 0.6 m: 26 November 2018, 14 October 2020 and 4 July 2022. The 2020 image is the sharpest; 2018 is late in the autumn and the 2022 image is visibly softer.")

The 2022 flight, in July, is leaf-on but soft; the 2018 flight is late autumn with long shadows and fallen leaves; the 2020 one is
sharp, with leaves on and a few trees turning. Sharpness matters for shape, so this page uses 14 October 2020.

NAIP numbers are 8-bit (0 to 255), balanced by the provider so the images look good, and are not calibrated reflectance.
Thresholds found for one flight will not carry to another year or state without a check: this is a method to repeat, not a
recipe to copy.

The data, the model and the results are small, so you can run everything on a laptop in seconds once the image is downloaded
and cached by Terraflux. For readers without a network, [naip_plano_2020.tif](/data/tutorials/object-based-image-analysis/case-urban-cover-naip/naip_plano_2020.tif)
(3.0 MB, with its [attribution](/data/tutorials/object-based-image-analysis/case-urban-cover-naip/ATTRIBUTION.txt)) is the same clip.

## Build the model

### Step 1: read NAIP, at the grid you choose

The first node is [[data.stac]]. The catalogue's address and the collection tell it where to look; `datetime` picks the flight;
`crs`, `extent` and `cellsize` define a grid of 1,000 by 1,000 cells of 0.6 m in UTM metres. Only that window of the image is
read, and it is kept in Terraflux's cache. The second node, [[raster.calc]], divides by 255 (so the segmentation's scale works
in units from 0 to 1, as in [part 3](/tutorials/object-based-image-analysis/choosing-the-scale/)) and also gives the near
infrared **alone** as a second output, `nir`, which Step 6 will need.

```tfgraph
#! caption: NAIP read straight from the Planetary Computer onto a UTM grid of 0.6 m (node 1), and rescaled to 0 to 1 (node 2).
#! highlight: naip, refl
#! numbered: true
#! fragment: true
[[nodes]]
id = "naip"
op = "data.stac"
params = { url = "https://planetarycomputer.microsoft.com/api/stac/v1", collection = "naip", assets = ["image"], datetime = "2020-10-14", sign = "planetary_computer", crs = "EPSG:26914", extent = [705030.6, 3658458.0, 705630.6, 3659058.0], cellsize = 0.6 }
note = "NAIP of 14 October 2020: red, green, blue and nir layers, 0.6 m."

[[nodes]]
id = "refl"
op = "raster.calc"
inputs = { s = "naip.raster" }
params = { expression = "out image = s / 255\nout nir = s.nir / 255" }
note = "Reflectance 0 to 1: the four layers, and the near infrared alone."
```

An asset of several bands gives **a layer per band**, named by the item's own band names; `bands = ["red", "nir"]` would read only
those two. Every other node of the page uses those names (`mean_red`, `mean_nir`...).

![The four bands](/img/tutorials/object-based-image-analysis/case-urban-cover-naip/bands.png "The detail in each band and in near-infrared false colour. Vegetation is bright in the near infrared and red in the false colour; water and shadow are dark in all bands; roofs and streets are mid grey in the visible and dark in the near infrared.")

!!! app "In the application"
    Add [[data.stac]] from the library (*Data*), set the **url**, **collection**, **assets**, **datetime**, **sign**, **crs**,
    **extent** and **cell size** in the Node pane (the extent is four numbers: west, south, east, north, in the units of the crs),
    and press ++ctrl+enter++ to run to it. The Viewer shows the four layers and you can switch between them.

### Step 2: the fine objects

[[segment.multiresolution]] on `refl.image` with `scale = 3` gives **9,188 objects**, with a median area of 31 m² (a mean of 109
cells). The scale is in the image's units, as always: on reflectance 0 to 1, a scale of 1.5 gives 29,910 objects and 6 gives
2,850. At 3, a house roof is a handful of objects (one per roof face, because the faces differ in brightness), a tree crown is
one or two, and a stretch of street is a few. That is the point of a **fine** level: objects small enough that each is mostly one
thing, to be classified by colour and joined afterwards.

![The fine objects](/img/tutorials/object-based-image-analysis/case-urban-cover-naip/objects.png "The 9,188 fine objects (scale 3) over the detail. A roof is cut in its faces, a crown into a few pieces, the street into a few strips.")

### Step 3: describe them

[[objects.features]] with `features = ["spectral", "texture"]` and `texture_bands = ["brightness"]` gives each object the
bands' means and standard deviations, the NDVI and NDWI, and Haralick's texture of the brightness
([part 4](/tutorials/object-based-image-analysis/describing-objects/)). [[objects.calc]] adds the **visible brightness**,
`vis = (mean_red + mean_green + mean_blue) / 3`. Six attributes carry most of what the first pass needs:

![Attributes of the fine objects](/img/tutorials/object-based-image-analysis/case-urban-cover-naip/attributes.png "Six attributes of the fine objects over the detail. The NDVI separates plants from pavement and water; the NDWI singles out the pond; the visible brightness is dark under trees and bright on the street; the GLCM homogeneity is high on the smooth lawn and the pond and low in the canopy.")

```tfgraph
#! caption: The fine objects (node 1), their colour and texture (node 2) and the visible brightness (node 3).
#! highlight: seg, feat, calc
#! numbered: true
#! fragment: true
[[nodes]]
id = "seg"
op = "segment.multiresolution"
inputs = { image = "refl.image" }
params = { scale = 3 }
note = "The fine objects."

[[nodes]]
id = "feat"
op = "objects.features"
inputs = { objects = "seg.objects", image = "refl.image" }
params = { features = ["spectral", "texture"], texture_bands = ["brightness"] }
note = "Colour, indices and texture."

[[nodes]]
id = "calc"
op = "objects.calc"
inputs = { objects = "feat.objects" }
params = { expression = "out vis = (mean_red + mean_green + mean_blue) / 3" }
note = "Visible brightness."
```

### Step 4: the first pass of rules

Where do the limits of the rules come from? Not from a book: NAIP's numbers are the provider's own. They come from **looking**. A
script drew 100 random pixels of the clip; the author read each in a window of 34 m, in true colour, and wrote what is on the
ground at the cross (`tree`, `lawn`, `street`, `roof`, `water`). These **tuning points**
([tuning_points.csv](/data/tutorials/object-based-image-analysis/case-urban-cover-naip/tuning_points.csv)) are a different sample from
the reference of Step 9. The medians of their objects:

```tftable
align: { class: left }
digits: { points: 0, ndvi: 2, ndwi: 2, vis: 2, homogeneity: 2, nir: 2 }
labels: { class: Class, points: Points, ndvi: NDVI, ndwi: NDWI, vis: "Visible brightness", homogeneity: "GLCM homogeneity", nir: "Mean nir" }
caption: "Median attributes of the fine objects under the 100 tuning points, by the class read from the image. Plants have a high NDVI; the pond is smooth and has a high NDWI; trees are rough (low homogeneity) and lawns smooth; roofs and streets are close in every attribute."
data: /data/tutorials/object-based-image-analysis/case-urban-cover-naip/tuning.csv
```

The table says what is easy and what is not. Plants against pavement is easy (NDVI of 0.28 to 0.31 against -0.06 to 0.06). Trees
against lawns is a matter of **texture** (homogeneity 0.40 against 0.66), not of colour. Roofs and streets cannot be cleanly told
by colour: their medians differ (NDVI -0.06 and 0.06, brightness 0.23 and 0.38) but the ranges overlap so far that no cut
separates them, because a street in shade is as dark as a slate roof. So the first pass leaves them together as **pavement**, and the class tree below, written as in
[part 6](/tutorials/object-based-image-analysis/rules-and-class-trees/), takes the other decisions:

```tfgraph
#! caption: A class tree on colour and texture: water, and on land plants (trees or lawns) and pavement.
#! highlight: rules
#! numbered: true
#! fragment: true
[[nodes]]
id = "rules"
op = "objects.rules"
inputs = { objects = "calc.objects" }
params = { classes = [
  { name = "water", rule = "max(above(ndwi, 0.35, 0.45), min(above(ndwi, 0.15, 0.25), above(glcm_homogeneity, 0.55, 0.75)))" },
  { name = "land", rule = "1 - max(above(ndwi, 0.35, 0.45), min(above(ndwi, 0.15, 0.25), above(glcm_homogeneity, 0.55, 0.75)))" },
  { name = "veg", rule = "above(ndvi, 0.12, 0.2)", parent = "land" },
  { name = "tree", rule = "max(below(glcm_homogeneity, 0.45, 0.6), below(vis, 0.15, 0.2))", parent = "veg" },
  { name = "lawn", rule = "min(above(glcm_homogeneity, 0.45, 0.6), above(vis, 0.15, 0.2))", parent = "veg" },
  { name = "pavement", rule = "below(ndvi, 0.12, 0.2)", parent = "land" },
] }
note = "First pass: water, tree, lawn, pavement."
```

- **water** is a high NDWI **and** a smooth surface: `min(above(ndwi, 0.15, 0.25), above(glcm_homogeneity, 0.55, 0.75))`. A very
  high NDWI (above 0.35 to 0.45) is enough alone, which is how a swimming pool would be caught if it were large enough to be an
  object. `max` and `min` are the fuzzy "or" and "and" of part 6.
- **land** is everything that is not water: `1 - (the water rule)`. It is a parent: it has no cells of its own, it only narrows
  its children.
- **veg** (vegetation) is an NDVI above a ramp from 0.12 to 0.2, and **pavement** its complement.
- **tree** is a rough or a **dark** plant: `max(below(glcm_homogeneity, 0.45, 0.6), below(vis, 0.15, 0.2))`. The second term puts
  plants in shadow with the trees (a lawn in the shade of a tree has the same darkness as the tree), and it is the choice that
  costs the most (see "Read the results"). **lawn** is the plant that is smooth **and** bright.

On the 100 tuning points the finished two-pass rules (Step 7) agree with the reading in 72 % of the points. That number is optimistic:
the limits were moved until it rose (it was 63 % at the first attempt), so the real figure comes from the reference of Step 9.

![The first pass](/img/tutorials/object-based-image-analysis/case-urban-cover-naip/pass1.png "The first pass over the detail. Tree (dark green), lawn (light green), pavement (beige) and water (blue). The roofs, the street and some dark shadows are all pavement; the dark roofs next to the pond are called water here and are given back in the second pass.")

!!! app "In the application"
    In the Viewer, select the layer of `rules`; the inspector of a clicked object lists its membership in each class and its
    **margin**, the best minus the second best (part 6). A margin near 0 is a place where two classes are close and the map is
    unsure; a map of the margin is a first look at where an assessment will find errors.

### Step 5: a level above, patches

The pavement of the first pass is spread over thousands of fine objects. [[objects.merge]] with `by = "class"` joins
neighbours of the same class, as in [part 8](/tutorials/object-based-image-analysis/levels/): the 9,188 fine objects become **893
patches**. That is the level of a landscape ecologist's patch, a connected area of one class. It is also where the map
shows its hardest problem: a patch of pavement is everything that is pavement and touches.

![Pavement patches](/img/tutorials/object-based-image-analysis/case-urban-cover-naip/patches.png "The pavement patches of a 156 m window, coloured by area: red over 1 ha, orange 0.1 to 1 ha, blue under 0.1 ha. The whole street network is one red patch, and so is every house whose driveway touches it.")

Of 390,077 cells of pavement, **257,587 (66 %) are in four patches larger than 1 ha**, and the largest is 4.2 ha: the streets, and
every house a driveway joins to them. A rule on the shape of that patch (is it long and thin?) would be wrong for the houses it
swallowed. So the patches must be **taken apart** before their shape is read.

```tfgraph
#! caption: Fine objects of a class merged into patches (node 1).
#! highlight: patch
#! numbered: true
#! fragment: true
[[nodes]]
id = "patch"
op = "objects.merge"
inputs = { objects = "rules.objects" }
params = { by = "class" }
note = "Neighbours of a class become one patch."
```

!!! note
    `objects.merge` has a `level = true` option that keeps the fine objects as the patches' children, for rules that read
    `parent.x` and `children.x` (part 8). This page does not use it, because the next step, [[objects.split]], replaces the
    objects it cuts **in the same level** and drops the level above (with a warning). The patches serve here as a way station,
    not as a level that later nodes read.

### Step 6: split what touches

[[objects.split]] divides objects that touch into their pieces, as eCognition's watershed transformation does (Vincent and
Soille, 1991): it floods the **distance from each cell to the object's border**, so a piece grows from each rounded part and the cut falls at the
neck between two of them. A house is a rounded part, a driveway a neck, a street a long ridge. Three settings matter:

- `classes = ["pavement"]`: only the objects of that class are split; the rest pass through.
- `h = 1.5`: how high, in **metres** (the map's units), a centre must stand over the saddle where it meets a higher one. A lower
  bump is noise and joins its neighbour. Smaller `h` cuts more.
- `min_size = 30`: a piece of fewer than 30 cells goes to the neighbour piece it shares the longest border with.

The 163 patches of pavement become **425 pieces**; 27 patches were split, and the street network alone into 94 pieces: houses on
one side, street strips on the other.

![Pavement pieces](/img/tutorials/object-based-image-analysis/case-urban-cover-naip/pieces.png "The pavement of the same window after splitting, each piece in its own colour. Houses are separate pieces, the driveways go with a house or with the street, and the street network is cut into strips.")

**The canopy too.** The trees are 211 patches, some of them hundreds of metres across: a canopy is not a tree. A second
[[objects.split]], on the trees, can cut them into pieces that stand for crowns. Splitting by distance (the default) does not
work for a canopy that has no neck to cut at, and gives 537 pieces. The option `by = "surface"` floods a **map**, whose peaks are
the centres: here the near infrared (`refl.nir`, from Step 1), which is brightest at the top of a crown and darker in the gaps
between them, smoothed with `sigma = 3` cells and with `h = 0.03` (a centre must stand 0.03 above the saddle, in the units of the
near infrared). That gives **1,206 canopy pieces**.

```tfgallery
columns: 3
caption: "A stretch of canopy (260 by 260 cells). Left: the image. Middle: the trees split by distance, 537 pieces in the whole clip: big groves. Right: split by surface, on the smoothed near infrared, 1,206 canopy pieces of about the size of a crown."
images:
  - { src: /img/tutorials/object-based-image-analysis/case-urban-cover-naip/crowns_image.png, caption: "NAIP" }
  - { src: /img/tutorials/object-based-image-analysis/case-urban-cover-naip/crowns_distance.png, caption: "by distance" }
  - { src: /img/tutorials/object-based-image-analysis/case-urban-cover-naip/crowns_surface.png, caption: "by surface (nir)" }
```

Do not read 1,206 as a count of trees: in a dense canopy two crowns that touch are one bright mass and one tree can be two
pieces. The pieces are **canopy units**, good for the area and the patchiness of the canopy, and a rough guide to crowns. A
canopy height model, if you have one, is a much better surface.

```tfgraph
#! caption: The pavement split at its necks (node 1) and the canopy split by the near infrared (node 2).
#! highlight: pieces, crowns
#! numbered: true
#! fragment: true
[[nodes]]
id = "pieces"
op = "objects.split"
inputs = { objects = "patch.objects" }
params = { classes = ["pavement"], h = 1.5, min_size = 30 }
note = "Cut the pavement at its necks."

[[nodes]]
id = "crowns"
op = "objects.split"
inputs = { objects = "pieces.objects", surface = "refl.nir" }
params = { classes = ["tree"], by = "surface", sigma = 3, h = 0.03, min_size = 30 }
note = "Canopy pieces from the near infrared."
```

### Step 7: shape and skeleton, and the second pass

[[objects.split]] drops the attributes of the objects it cuts, so [[objects.features]] runs again, now with `features =
["spectral", "shape", "skeleton"]`. The **skeleton** group thins each object's raster to a line (Zhang and Suen, 1984) and gives its
`skel_main_length`, its `skel_width` (area divided by main length: the typical width), the number of branches and junctions and
the curvature. It is made for long thin things: roads, rivers, field edges. Here the useful number is the width. Streets are
about 10 m wide; a house with its apron, 15 m.

[[objects.calc]] adds `bmr = mean_blue - mean_red`, the **blue minus red** of the piece. Roofs of slate and composition
shingle are cool grey, a little blue; streets of concrete and asphalt are neutral to warm. The two attributes on the 31 tuning
points that fall in a pavement piece:

```tftable
align: { class: left }
digits: { pieces: 0, skel_width: 1, bmr: 3, area: 0 }
labels: { class: Class, pieces: Pieces, skel_width: "Skeleton width (m)", bmr: "Blue minus red", area: "Area (m²)" }
caption: "Median attributes of the pavement pieces under the tuning points read as street or roof."
data: /data/tutorials/object-based-image-analysis/case-urban-cover-naip/tuning_pieces.csv
```

The second pass of rules reads the class of the first (`cls`) and turns the pavement into roof or street:

```tfgraph
#! caption: The pieces' shape and skeleton (node 1), blue minus red (node 2), and the second pass (node 3).
#! highlight: pfeat, calc2, final
#! numbered: true
#! fragment: true
[[nodes]]
id = "pfeat"
op = "objects.features"
inputs = { objects = "crowns.objects", image = "refl.image" }
params = { features = ["spectral", "shape", "skeleton"] }
note = "Shape and skeleton of the pieces."

[[nodes]]
id = "calc2"
op = "objects.calc"
inputs = { objects = "pfeat.objects" }
params = { expression = "out bmr = mean_blue - mean_red" }
note = "Blue minus red: roofs are cooler than streets."

[[nodes]]
id = "final"
op = "objects.rules"
inputs = { objects = "calc2.objects" }
params = { classes = [
  { name = "water", rule = 'cls == "water" and above(area, 100, 300)', colour = "#2b6cb0" },
  { name = "tree", rule = 'cls == "tree"', colour = "#2f7d3b" },
  { name = "lawn", rule = 'cls == "lawn"', colour = "#a7d676" },
  { name = "street", rule = 'cls == "pavement" and min(below(skel_width, 11, 14), below(bmr, 0.02, 0.05))', colour = "#8c8c8c" },
  { name = "roof", rule = '(cls == "pavement" and (1 - min(below(skel_width, 11, 14), below(bmr, 0.02, 0.05)))) or (cls == "water" and below(area, 100, 300))', colour = "#c0504d" },
] }
note = "Second pass: roof or street, small water to roof."
```

- A **street** is a pavement piece that is narrow (a skeleton width under 11 to 14 m) **and** neutral to warm (a blue-minus-red
  under 0.02 to 0.05). Both must hold: `min` is "and".
- A **roof** is the rest of the pavement, **or** a "water" that is smaller than 100 to 300 m². This last term is there for the dark,
  smooth roofs that the first pass took for water, because a roof of dark slate in shade is as dark and as smooth as a pond. The pond
  is large; the dark roof is not. Keep it in mind: it will cost something (below).
- The other classes pass through.

![Skeleton and tone of the pieces](/img/tutorials/object-based-image-analysis/case-urban-cover-naip/skeleton.png "The pavement pieces of the window coloured by skeleton width (left) and by blue minus red (middle). Right: every piece of the clip with more than 36 m², roof or street as the rules gave it, in the two attributes. The grey bands are the ramps of the rules.")

The scatter plot is the honest view of the rule. The two classes are not two clouds; they are two drifts that overlap in the grey
cross, and the objects in the cross are decided by small differences. 189 pieces become streets and 229 roofs.

### Step 8: clean outlines and write the results

Objects from a multiresolution segmentation have ragged borders. [[objects.resize]] with `mode = "smooth"` gives each cell the most
common label in a disc of radius `distance`: spikes and notches go, corners round, and nothing grows outside. At 1.2 m, **12,895
cells (1.3 %)** change class; the smoothing is a pure cosmetic of the outlines.

```tftable
digits: { distance: 1, cells: 0, percent: 2 }
labels: { distance: "Radius (m)", cells: "Cells that change class", percent: "Share of the clip (%)" }
caption: "objects.resize, mode smooth, at three radii: how much of the map is rewritten. At 2.4 m the map is no longer the segmentation's."
data: /data/tutorials/object-based-image-analysis/case-urban-cover-naip/smooth.csv
```

```tfcompare
caption: Drag the handle to compare the classes before and after smoothing the outlines (radius 1.2 m).
before: { src: /img/tutorials/object-based-image-analysis/case-urban-cover-naip/smooth_before.png, label: Before }
after: { src: /img/tutorials/object-based-image-analysis/case-urban-cover-naip/smooth_after.png, label: After }
```

Smoothing can cut an object into two: 11 objects did, so the map has **2,161 objects** (2,152 with a class; nine have no class
because no rule reached the minimum membership of 0.1). [[objects.to_map]] with `what = "class"` makes the class map,
written by [[raster.write]] as a GeoTIFF, and [[objects.to_vector]] makes **one polygon per object** (2,161 polygons, 35.9 ha, the
clip's 36 ha), written by [[vector.write]] as a GeoPackage. `smooth = 1` cuts the corners of the cell edges once (Chaikin's
method), which rounds the staircase of 0.6 m cells; each time makes the polygon a little smaller. The polygons carry the object's
number (`id`), its class, its `cells` and its `area` (in m², from [[objects.resize]]'s basic table); the layer is in UTM metres.

![The polygons](/img/tutorials/object-based-image-analysis/case-urban-cover-naip/polygons.png "The polygons over the detail, outlined in yellow and tinted by class. Each object is one polygon: a roof, a street strip, a canopy piece, the lawn of the park.")

```tfgraph
#! caption: Smooth the outlines (node 1), write the map (nodes 2 and 3) and the polygons (nodes 4 and 5).
#! highlight: clean, map, polygons
#! numbered: true
#! fragment: true
[[nodes]]
id = "clean"
op = "objects.resize"
inputs = { objects = "final.objects" }
params = { mode = "smooth", distance = 1.2 }
note = "Smooth the outlines."

[[nodes]]
id = "map"
op = "objects.to_map"
inputs = { objects = "clean.objects" }
params = { what = "class" }

[[nodes]]
id = "map_file"
op = "raster.write"
inputs = { value = "map.map" }
params = { path = "urban_cover.tif" }
note = "The map, a GeoTIFF."

[[nodes]]
id = "polygons"
op = "objects.to_vector"
inputs = { objects = "clean.objects" }
params = { smooth = 1 }
note = "One polygon per object: its number, class, cells and area."

[[nodes]]
id = "polygons_file"
op = "vector.write"
inputs = { vector = "polygons.vector" }
params = { path = "urban_cover.gpkg" }
```

Both files are small downloads: [urban_cover.tif](/data/tutorials/object-based-image-analysis/case-urban-cover-naip/urban_cover.tif)
(classes 1 water, 2 tree, 3 lawn, 4 street, 5 roof) and
[urban_cover.gpkg](/data/tutorials/object-based-image-analysis/case-urban-cover-naip/urban_cover.gpkg) (5.7 MB).

### Step 9: a reference, drawn and read

This is the step that decides whether the map is worth anything. [[objects.sample]] draws a **stratified random sample of the
objects**: the strata are the map's classes and every object of a class has the same chance. Olofsson et al. (2014) advise at
least 50 per class; `per_class = 50` gives 50 objects each of tree, lawn, street and roof, and **all 6** of the water objects
(a class with fewer objects gives them all, with a warning). Each sampled object gets a point at the cell nearest its centroid
that lies in it, and a column `reference`, empty, for the true class.

```tfgraph
#! caption: Draw the reference sample (node 1) and write the points, to be read (node 2).
#! highlight: sample, points_file
#! numbered: true
#! fragment: true
[[nodes]]
id = "sample"
op = "objects.sample"
inputs = { objects = "clean.objects" }
params = { per_class = 50, seed = 1 }
note = "50 objects of each class, at random."

[[nodes]]
id = "points_file"
op = "vector.write"
inputs = { vector = "sample.points" }
params = { path = "sample_points.gpkg" }
```

**How the reference was made.** There is no field data. The 206 points were labelled **by eye, from the image itself**, by the
author of this page, and the procedure is the best honest one that was available:

1. The points were shuffled, so the number of a point does not tell its class.
2. Each point was drawn as a **chip**, a window of 34 m of the image in true colour with a cross at the point, with **no map and no
   class**: the reader did not know what the map said.
3. The label (`tree`, `lawn`, `street`, `roof`, `water`) is what is **on the ground at the cross**. A hard surface at ground
   level (street, driveway, parking, path, tennis court, the deck of a pool) is `street`; grass and low plants are `lawn`; a pool and a pond are
   `water`; in doubt between a tree and the shadow of a tree, the surface under the shadow when it shows, `tree` when not.
4. The labels went into a file, [reference_points.csv](/data/tutorials/object-based-image-analysis/case-urban-cover-naip/reference_points.csv)
   (`x`, `y`, `class`), that you can read and **relabel**: if you disagree with a label, change it and run the model again.

What this reference **is not**: it is not field data, it is not independent of the image the map was made from (the same pixels
are behind both, and their blind spots, a deep shadow or a tilted roof, are shared), and one person, who has made mistakes,
labelled it. Expect some labels to be wrong: in chips like the ones below even a careful reader hesitates. The sample is large
enough to show what a map does not do; it cannot show that a map does.

![The reference points](/img/tutorials/object-based-image-analysis/case-urban-cover-naip/reference.png "The 206 reference points over the image, coloured by the class read from the image. A white-edged dot is a point where the map and the reading agree; a black-edged cross is a point where they disagree: 100 of the 206.")

The model reads the file with [[table.read]], makes a point layer with [[vector.from_table]] (the coordinates are in the clip's CRS) and
gives it to [[objects.accuracy]] as `samples`, which finds the object under each point. `acc` makes the error matrix, the
user's and producer's accuracies, the overall accuracy and kappa, and the area of each class **estimated from the sample**
with its confidence interval, as Olofsson et al. (2014) describe
([part 5](/tutorials/object-based-image-analysis/classifying-by-samples/) explains each number).

One setting must match how the sample was drawn: **`design`**. [[objects.sample]] gives **every object of a class the same chance**,
whatever its size, so the page sets `design = "objects"`: the estimator then weights each sampled object by its **area** (the ratio
estimator of Stehman, 2014, and Radoux and Bogaert, 2017), so that a pool of 20 m² and a roof of 300 m² count for what they cover. With
points thrown at random over the map, where each point has a chance proportional to area, the setting is `design = "area"` and each
point counts once. If you leave it out, Terraflux guesses from your points (and warns that it guessed); say it.

```tfgraph
#! caption: The reference read from a file (nodes 1 and 2) and the accuracy of the map against it (node 3).
#! highlight: ref_table, ref, acc
#! numbered: true
#! fragment: true
[[nodes]]
id = "ref_table"
op = "table.read"
params = { path = "reference_points.csv" }
note = "The reference: x, y and the class read from the image."

[[nodes]]
id = "ref"
op = "vector.from_table"
inputs = { table = "ref_table.table" }
params = { crs = "EPSG:26914" }

[[nodes]]
id = "acc"
op = "objects.accuracy"
inputs = { objects = "clean.objects", samples = "ref.vector" }
params = { design = "objects" }
note = "Error matrix, accuracies and adjusted areas."
```

### The whole model

```tfgraph
#! caption: The whole model of the page: NAIP read onto a UTM grid (1, 2), fine objects and their colour and texture (3 to 5), first pass of rules (6), patches (7), pavement and canopy split (8, 9), shape and skeleton, second pass (10 to 12), smoothing (13), the map and polygons (14 to 17) and the accuracy assessment against the reference (18 to 20).
#! highlight: naip, pieces, crowns, final, acc
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "naip"
op = "data.stac"
params = { url = "https://planetarycomputer.microsoft.com/api/stac/v1", collection = "naip", assets = ["image"], datetime = "2020-10-14", sign = "planetary_computer", crs = "EPSG:26914", extent = [705030.6, 3658458.0, 705630.6, 3659058.0], cellsize = 0.6 }
note = "NAIP of 14 October 2020: red, green, blue and nir layers, 0.6 m."

[[nodes]]
id = "refl"
op = "raster.calc"
inputs = { s = "naip.raster" }
params = { expression = "out image = s / 255\nout nir = s.nir / 255" }
note = "Reflectance 0 to 1: the four layers, and the near infrared alone."

[[nodes]]
id = "seg"
op = "segment.multiresolution"
inputs = { image = "refl.image" }
params = { scale = 3 }
note = "The fine objects."

[[nodes]]
id = "feat"
op = "objects.features"
inputs = { objects = "seg.objects", image = "refl.image" }
params = { features = ["spectral", "texture"], texture_bands = ["brightness"] }
note = "Colour, indices and texture."

[[nodes]]
id = "calc"
op = "objects.calc"
inputs = { objects = "feat.objects" }
params = { expression = "out vis = (mean_red + mean_green + mean_blue) / 3" }
note = "Visible brightness."

[[nodes]]
id = "rules"
op = "objects.rules"
inputs = { objects = "calc.objects" }
params = { classes = [
  { name = "water", rule = "max(above(ndwi, 0.35, 0.45), min(above(ndwi, 0.15, 0.25), above(glcm_homogeneity, 0.55, 0.75)))" },
  { name = "land", rule = "1 - max(above(ndwi, 0.35, 0.45), min(above(ndwi, 0.15, 0.25), above(glcm_homogeneity, 0.55, 0.75)))" },
  { name = "veg", rule = "above(ndvi, 0.12, 0.2)", parent = "land" },
  { name = "tree", rule = "max(below(glcm_homogeneity, 0.45, 0.6), below(vis, 0.15, 0.2))", parent = "veg" },
  { name = "lawn", rule = "min(above(glcm_homogeneity, 0.45, 0.6), above(vis, 0.15, 0.2))", parent = "veg" },
  { name = "pavement", rule = "below(ndvi, 0.12, 0.2)", parent = "land" },
] }
note = "First pass: water, tree, lawn, pavement."

[[nodes]]
id = "patch"
op = "objects.merge"
inputs = { objects = "rules.objects" }
params = { by = "class" }
note = "Neighbours of a class become one patch."

[[nodes]]
id = "pieces"
op = "objects.split"
inputs = { objects = "patch.objects" }
params = { classes = ["pavement"], h = 1.5, min_size = 30 }
note = "Cut the pavement at its necks."

[[nodes]]
id = "crowns"
op = "objects.split"
inputs = { objects = "pieces.objects", surface = "refl.nir" }
params = { classes = ["tree"], by = "surface", sigma = 3, h = 0.03, min_size = 30 }
note = "Canopy pieces from the near infrared."

[[nodes]]
id = "pfeat"
op = "objects.features"
inputs = { objects = "crowns.objects", image = "refl.image" }
params = { features = ["spectral", "shape", "skeleton"] }
note = "Shape and skeleton of the pieces."

[[nodes]]
id = "calc2"
op = "objects.calc"
inputs = { objects = "pfeat.objects" }
params = { expression = "out bmr = mean_blue - mean_red" }
note = "Blue minus red: roofs are cooler than streets."

[[nodes]]
id = "final"
op = "objects.rules"
inputs = { objects = "calc2.objects" }
params = { classes = [
  { name = "water", rule = 'cls == "water" and above(area, 100, 300)', colour = "#2b6cb0" },
  { name = "tree", rule = 'cls == "tree"', colour = "#2f7d3b" },
  { name = "lawn", rule = 'cls == "lawn"', colour = "#a7d676" },
  { name = "street", rule = 'cls == "pavement" and min(below(skel_width, 11, 14), below(bmr, 0.02, 0.05))', colour = "#8c8c8c" },
  { name = "roof", rule = '(cls == "pavement" and (1 - min(below(skel_width, 11, 14), below(bmr, 0.02, 0.05)))) or (cls == "water" and below(area, 100, 300))', colour = "#c0504d" },
] }
note = "Second pass: roof or street, small water to roof."

[[nodes]]
id = "clean"
op = "objects.resize"
inputs = { objects = "final.objects" }
params = { mode = "smooth", distance = 1.2 }
note = "Smooth the outlines."

[[nodes]]
id = "map"
op = "objects.to_map"
inputs = { objects = "clean.objects" }
params = { what = "class" }

[[nodes]]
id = "map_file"
op = "raster.write"
inputs = { value = "map.map" }
params = { path = "urban_cover.tif" }
note = "The map, a GeoTIFF."

[[nodes]]
id = "polygons"
op = "objects.to_vector"
inputs = { objects = "clean.objects" }
params = { smooth = 1 }
note = "One polygon per object: its number, class, cells and area."

[[nodes]]
id = "polygons_file"
op = "vector.write"
inputs = { vector = "polygons.vector" }
params = { path = "urban_cover.gpkg" }

[[nodes]]
id = "ref_table"
op = "table.read"
params = { path = "reference_points.csv" }
note = "The reference: x, y and the class read from the image."

[[nodes]]
id = "ref"
op = "vector.from_table"
inputs = { table = "ref_table.table" }
params = { crs = "EPSG:26914" }

[[nodes]]
id = "acc"
op = "objects.accuracy"
inputs = { objects = "clean.objects", samples = "ref.vector" }
params = { design = "objects" }
note = "Error matrix, accuracies and adjusted areas."
```

=== "Application"

    Build the nodes in order (click an operator in the library to add it, **drag from an output port to an input port** to
    connect, set the values in the **Node pane**) and press ++ctrl+enter++ on a node to run to it: the Viewer shows its output, the
    layer of objects with their classes, a map, or a table. Select `acc` and run: its outputs are the error matrix, the table of
    accuracies and areas, the overall accuracy and the list of the reference points that the map got wrong.

=== "Command line"

    Download the model, [model.toml](/data/tutorials/object-based-image-analysis/case-urban-cover-naip/model.toml), and
    [reference_points.csv](/data/tutorials/object-based-image-analysis/case-urban-cover-naip/reference_points.csv) beside it, and run it. To
    write the accuracy tables, add `[outputs.accuracy]` with `from = "acc.table"`, and likewise for `acc.matrix` and `acc.overall`.

    ```bash
    terraflux run model.toml --threads 4 --out output
    ```

=== "Without a network"

    Download [naip_plano_2020.tif](/data/tutorials/object-based-image-analysis/case-urban-cover-naip/naip_plano_2020.tif) beside the model and
    replace the node `naip` with a [[raster.read]] of it (its bands are named `red`, `green`, `blue`, `nir`):

    ```toml
    [[nodes]]
    id = "naip"
    op = "raster.read"
    params = { path = "naip_plano_2020.tif" }
    ```

## Read the results

### The map

```tfcompare
caption: Drag the handle to compare the NAIP clip with the map (same size). Houses are red, streets grey, trees dark green, lawns light green, water blue.
before: { src: /img/tutorials/object-based-image-analysis/case-urban-cover-naip/clip_plain.png, label: NAIP }
after: { src: /img/tutorials/object-based-image-analysis/case-urban-cover-naip/map_plain.png, label: Map }
```

![The map](/img/tutorials/object-based-image-analysis/case-urban-cover-naip/map.png "The class map of the clip after smoothing: 2,161 objects. Roofs (red) are separate and mostly right; the street network (grey) is there but broken where the canopy hangs over it; trees (dark green) cover 47 % of the clip; the two ponds are blue.")

Walk the map, because an assessment of 206 points cannot see everything. The houses and the street network are recognisable. Two
things stand out:

- **The avenue on the right**, a wide street that runs from the top to the bottom of the clip near its right edge, is mapped as **roof**
  along most of its length. Its concrete is cooler than the streets of the neighbourhood, and the blue-minus-red rule takes it for a roof.
  A reader of the polygons would see it at once; the 50 street points of the reference
  are too few to catch one avenue.
- **Streets disappear under the trees.** Where the canopy overhangs a street, the street is in shadow and is mapped as tree or
  lawn. The map under-counts streets.

### The numbers

```tftable
data: /data/tutorials/object-based-image-analysis/case-urban-cover-naip/matrix.csv
columns: [map, water, tree, lawn, street, roof, total]
labels: { map: "Map \\ reference", total: Sampled }
align: { map: left }
digits: 0
heat: true
caption: "The 206 reference points by the class the map gave (rows) and the class read from the image (columns). The diagonal, 106 points, is where the map and the reading agree."
```

Overall, the map agrees with the reading at **106 of 206 points (51.5 %)**, kappa 0.38. Weighted by the area of the map's classes, with the
estimator of Olofsson et al. (2014) and the `objects` design, the overall accuracy is **72.2 % ± 8.3 %** (95 % confidence). The two differ
because the large objects, trees and roofs, are better mapped than the small ones. The table below gives the accuracy of each class
(weighted by area) and, in its last columns, the area that each class covers in the 36 ha once the errors are taken out.

```tftable
data: /data/tutorials/object-based-image-analysis/case-urban-cover-naip/accuracy.csv
columns: [class, users, users_ci, producers, producers_ci, map_ha, area_ha, area_ha_ci]
align: { class: left }
labels: { class: Class, mapped_objects: "Sampled objects", reference_objects: "Read as this class", users: "User's accuracy", users_ci: "± 95 %", producers: "Producer's accuracy", producers_ci: "± 95 %", map_ha: "Mapped area (ha)", area_ha: "Adjusted area (ha)", area_ha_ci: "± 95 % (ha)" }
digits: { mapped_objects: 0, reference_objects: 0, users: 2, users_ci: 2, producers: 2, producers_ci: 2, map_ha: 1, area_ha: 1, area_ha_ci: 1 }
caption: "User's and producer's accuracies, weighted by area, and adjusted areas by class (Olofsson et al., 2014; design = objects). The reference is the author's reading of the same image: the intervals are the sampling error only."
```

```tfchart
type: bar
data: /data/tutorials/object-based-image-analysis/case-urban-cover-naip/accuracy.csv
x: class
y: [map_ha, area_ha]
labels: { map_ha: "Counted on the map", area_ha: "Estimated from the reference" }
colors: { map_ha: "#8c8c8c", area_ha: "#3b8a5a" }
ylabel: Hectares in the 36 ha clip
caption: The area of each class counted on the map and estimated with the reference. The map over-counts trees by a fifth (17.1 against 14.2 ha), under-counts streets (4.3 against 6.4 ha) and finds half of the water (0.7 against 1.4 ha).
```

!!! note "By objects and by area"
    The matrix counts **objects**; the table weights them by **area**. They differ, and both are right. By objects, the user's accuracy of a
    class is the share of its sampled objects that are right (roof 0.68, tree 0.58, street 0.50, lawn 0.26, water 0.83); by area, it is the
    share of its *area* (roof 0.77, tree 0.75, street 0.58, lawn 0.52, water 0.97), and it is higher because the big objects are
    the better ones. Report which you mean. The areas of the last columns are the area-weighted estimates: they are the numbers to quote for
    how much of the neighbourhood each class covers.

What the numbers say, class by class:

- **Roof** is the best class: a user's accuracy of 0.77 by area (0.68 by objects) and a producer's accuracy of 0.82 (four fifths of the roof
  area was found). The error that remains is shadow: a roof next to a tall tree is half in shade. Area: 10.1 ± 1.5 ha (10.6 mapped).
- **Tree** has a user's accuracy of 0.75 and a producer's of 0.91: the class takes in nearly all the trees, and a good deal besides. 24 of the
  50 sampled **lawn** objects are trees in the reading, and the map's tree area (17.1 ha) is a fifth too large: the
  estimate is 14.2 ± 2.6 ha. The "dark plant is a tree" rule of Step 4 is why: shaded lawn goes to the trees.
- **Lawn** is the worst class (user's 0.52, producer's 0.42; by objects 0.26). A smooth, bright plant object is rare in a suburb whose lawns lie in
  the shade of trees; the map finds 3.2 ha and the estimate is 4.0 ± 1.5.
- **Street** has a user's accuracy of 0.58 and a producer's of 0.39: 12 of the 50 are roofs, and a part of the streets is under trees. The map has
  4.3 ha of streets and the estimate is 6.4 ± 2.8 ha: the map under-counts them.
- **Water** has a user's accuracy of 0.97 by area (0.83 by objects) on the 6 objects the map calls water, but a producer's accuracy of only 0.50. Of the 19 points read as
  water, only 5 are mapped as water, and **10 are mapped as roof**. They are swimming pools. A pool is smaller than 100 m², so the rule of Step 7 that sends a small
  "water" to the roofs (made for dark roofs) sends the pools there too. The estimate of the water is 1.4 ± 0.4 ha against 0.7 ha
  on the map: the ponds are 0.7 ha and the pools the rest, a believable total for a neighbourhood. With the `objects` design each small
  pool object weighs what it covers; counting every object alike (what older versions of Terraflux did) gave 3.3 ha, too much to be pools.

The chips below are a sample of the 100 reference points the map got wrong, taken at an even step through the list. They are the best
way to learn what a map does not know.

![Where the map is wrong](/img/tutorials/object-based-image-analysis/case-urban-cover-naip/errors.png "Twenty of the 100 reference points that the map got wrong, in the image: the map's class and the class read. Deep shade (a street, a lawn and a canopy are alike), pool decks and tennis courts, shrubs at the foot of a wall, and some readings that could just as well be the other class.")

Many of these are not mistakes of the rules but cases that the **image** does not settle: a dark patch between a house and a tree is
a lawn, a street or a canopy, and a reader decides by a guess. That is the ceiling of a reference read from the same image.

### What you can report

A map with half of its reference objects right (72 % by area) is not a map to publish with five classes. What a report can say depends on the question:

```tftable
digits: { users: 2, users_ci: 2, producers: 2, producers_ci: 2 }
labels: { class: Class, users: "User's accuracy", users_ci: "± 95 %", producers: "Producer's accuracy", producers_ci: "± 95 %", note: "In the five-class map" }
align: { class: left, note: left }
caption: "The map with trees and lawns as one class of vegetation (objects.reclassify), against the same reference with its labels joined: 78.6 % ± 6.5 % overall by area, kappa 0.53. Vegetation is reliable (user's 0.83, producer's 0.96); the street and water are not."
csv: |
  class,users,users_ci,producers,producers_ci,note
  vegetation,0.83,0.09,0.96,0.03,"tree 0.75 and lawn 0.52 apart"
  roof,0.77,0.12,0.80,0.07,"0.77 and 0.82 before"
  street,0.58,0.18,0.37,0.14,"0.58 and 0.39 before"
  water,0.97,0.00,0.50,0.14,"the same"
```

Join trees and lawns with [[objects.reclassify]] (`classes = { tree = "vegetation", lawn = "vegetation" }`), and join the labels of the
reference the same way: no new reference is needed, because the labels do not change, only their names. The overall accuracy
rises from 72.2 % to **78.6 % ± 6.5 %** by area (kappa from 0.38 to 0.53), and the vegetation class has a user's accuracy of 0.83 and a producer's of 0.96. The
lesson is general: **an assessment tells you which classes the data support**. This imagery
supports "plants, roofs", and with less certainty "streets"; it does not support "tree or lawn" in shade, nor pools as water,
at this scale and with these rules.

A paragraph for a report, with what is true of this map:

> Land cover of a 36 ha neighbourhood of Plano, Texas, was mapped from NAIP imagery of 14 October 2020 (0.6 m; red, green, blue, near
> infrared) by object-based classification with Terraflux: multiresolution segmentation (scale 3 on reflectance 0 to 1, 9,188 objects),
> fuzzy rules on colour and texture, objects merged into patches, impervious patches split by a watershed of their distance
> transform and classified as roof or street by the width of their skeleton and their tone, and outlines smoothed with a radius
> of 1.2 m. The map was assessed with 206 objects drawn at random in strata of the map's classes (50 per class, all 6 water
> objects) and labelled by one analyst from the image, blind to the map. Overall accuracy, weighted by area (objects design: the ratio estimator
> of Stehman, 2014), was 72.2 % (95 % CI ± 8.3 %; 51.5 % by objects, kappa 0.38); with trees and lawns joined, 78.6 % (± 6.5 %). User's accuracy
> was 0.83 for vegetation, 0.77 for roofs, 0.58 for streets and 0.97 for the six water objects (producer's accuracy 0.50: swimming pools are
> not mapped as water). Class areas estimated from the sample were 17.6 ± 1.9 ha of vegetation, 10.2 ± 1.6 ha of roofs, 6.7 ± 2.3 ha of streets
> and 1.4 ± 0.4 ha of water.

Report the matrix, the overall and the class accuracies **with** their intervals, the **design** of the sample, the adjusted areas **with**
their intervals, and how the reference was made, in a paper's methods section; say that it is not independent of the image and not field
data, as the page does.

## Settings that change the answer

**The scale of the fine objects.** Every number above is for `scale = 3`. Move the slider: it reruns the whole model at another
scale and assesses the map against the **same** 206 points (each point finds the object that lies under it at that scale).

```tfslider
label: Scale of the fine objects
caption: "The fine objects (left, outlined) and the final map (right) for five scales of the first segmentation. The chart is the overall accuracy adjusted by the area of the classes, with its 95 % interval, against the same reference points."
frames:
  - { value: 1.5, src: /img/tutorials/object-based-image-analysis/case-urban-cover-naip/scale_1_5.png, caption: "Scale 1.5: 29,910 fine objects", stats: { Fine objects: "29,910", Final objects: "3,790", Accuracy: "70.8 %", "By objects": "56.1 %", Kappa: "0.42" } }
  - { value: 2, src: /img/tutorials/object-based-image-analysis/case-urban-cover-naip/scale_2.png, caption: "Scale 2: 18,183 fine objects", stats: { Fine objects: "18,183", Final objects: "2,906", Accuracy: "65.5 %", "By objects": "56.0 %", Kappa: "0.42" } }
  - { value: 3, src: /img/tutorials/object-based-image-analysis/case-urban-cover-naip/scale_3.png, caption: "Scale 3 (the page's): 9,188 fine objects", stats: { Fine objects: "9,188", Final objects: "2,150", Accuracy: "72.2 %", "By objects": "51.5 %", Kappa: "0.38" } }
  - { value: 4, src: /img/tutorials/object-based-image-analysis/case-urban-cover-naip/scale_4.png, caption: "Scale 4: 5,649 fine objects", stats: { Fine objects: "5,649", Final objects: "1,797", Accuracy: "68.9 %", "By objects": "58.9 %", Kappa: "0.45" } }
  - { value: 6, src: /img/tutorials/object-based-image-analysis/case-urban-cover-naip/scale_6.png, caption: "Scale 6: 2,850 fine objects", stats: { Fine objects: "2,850", Final objects: "1,450", Accuracy: "66.8 %", "By objects": "58.6 %", Kappa: "0.45" } }
chart:
  type: line
  data: /data/tutorials/object-based-image-analysis/case-urban-cover-naip/scales.csv
  x: scale
  y: [overall_adjusted]
  labels: { overall_adjusted: "Overall accuracy (%)" }
  colors: { overall_adjusted: "#3b8a5a" }
  ylabel: "Adjusted overall accuracy (%)"
```

- **The accuracy hardly moves**: 65.5 to 72.2 % by area, and 51.5 to 58.9 % by objects, across a fourfold change in the scale, with
  intervals of ±8 to ±17 points (they are wide at the scales where a few large objects of a stratum carry the estimate). The
  differences are inside the sampling error. The scale changes the look of the map and the number of objects (3,790 to 1,450
  after the whole chain), not its quality. The errors are not of the segmentation.
- **What would move the accuracy is the rules and the image**, not a finer object: the pools, the shade, the avenue. That is a
  common finding in urban mapping: the limit is the information in the bands, and objects reach it sooner than pixels do but
  not past it.
- **Do not use this sweep to pick the scale.** The reference was used to measure, and a scale chosen by the best of five
  values is a scale tuned to those points. To choose, use a tuning sample (as Step 4 did) and keep the reference for a single, final
  assessment (Stehman and Foody, 2019).

**The smoothing radius** (table in Step 8) is the other setting with a visible price: a radius of 2.4 m rewrites 3.6 % of the map, and
streets and pools thinner than the disc start to disappear. **The split's `h`** works the same way: lower, and a house breaks into
pieces; higher, and a driveway keeps a house with the street (and the street's skeleton is too wide for the rule). Look at the
pieces on a detail before you move it.

## Common pitfalls

- **Judging by the overall accuracy.** 72 % hides a roof class at 0.77, a vegetation class at 0.83 and a water class that is half found.
  Read the matrix, class by class, and look at the map.
- **Tuning on the reference.** The limits of the rules came from the tuning sample, not from the 206 points. If you change a rule
  because the reference said so, draw a new reference. The table of errors is for understanding, not for fitting.
- **A reference that is not independent.** Labels read from the image that made the map share its blind spots. They are better than nothing,
  and worse than field data, LiDAR or finer imagery, which a funded study should use. Say so.
- **The wrong `design`.** The sample was drawn with the same chance for each object, so `design = "objects"`; with points thrown at random over the
  map it is `"area"`. The default guesses from the points and warns. With objects of 20 m² to thousands, the wrong choice moves the area of the
  small classes a lot (the water here: 3.3 ha with every object counted alike, 1.4 ha weighted by area).
- **A class with too few objects.** Water has six objects, all sampled, and they are the ponds; the pools
  are not in its stratum at all. A class mapped with a few objects and a rule that sends small things elsewhere will always be
  badly assessed: map pools as their own class (a finer segmentation of the water only), or leave them out and say so.
- **Merging before splitting.** [[objects.merge]] by class chained every house to the street network (one pavement patch of 4.2
  ha). Shape rules on that patch are meaningless; split first.
- **A rule for a shape that the split removed.** [[objects.split]] drops the attributes (and the level above) of what it cuts:
  run [[objects.features]] again after it, as Step 7 does.
- **Reading 1,206 canopy pieces as 1,206 trees.** The split by the near infrared gives units of about a crown, not crowns. Report them as canopy
  pieces, or use a canopy height model.
- **Thresholds from one flight on another.** NAIP is balanced by the provider; the values of a flight in 2018 are not those of 2020.
  The method transfers; the limits do not. Re-derive them from a tuning sample for each flight.
- **Shadows.** Late-season flights have long shadows, and a classifier that reads colour alone has no way to know what is under one.
  Use a flight with the sun high if you can, or a height model.
- **A CRS in degrees.** `data.stac` with an `extent` in degrees and no `crs` makes a grid in degrees: the areas of shape rules
  (`skel_width`, `area`) are then in degrees too. Give a projected `crs` with the extent and the cell size in metres.
- **Rules that depend on the units.** `above(area, 100, 300)` is in square metres here. On a grid in feet it means something
  else. Check the units of the attributes of [[objects.features]].

## Going further

- The same chain on a 10 m image, with more classes and areas by hectare for a protected area:
  [part 15](/tutorials/object-based-image-analysis/case-protected-area-sentinel2/).
- Samples instead of rules for the first pass, a random forest on these attributes:
  [Classifying by samples](/tutorials/object-based-image-analysis/classifying-by-samples/). It is the natural comparison: with the
  same 206 points as a reference, which gives the better map?
- Levels with parents and children, for context that this page took from patches only:
  [Levels](/tutorials/object-based-image-analysis/levels/).
- Change between two flights (2018 and 2022 are in the catalogue):
  [Change by objects](/tutorials/object-based-image-analysis/change-by-objects/), with the caution that NAIP's
  balance differs between flights.
- A map to edit by hand, when the avenue is not a roof: [Editing by hand](/tutorials/object-based-image-analysis/editing-by-hand/).
- The operators of this page: [[data.stac]], [[raster.calc]], [[segment.multiresolution]], [[objects.features]], [[objects.calc]],
  [[objects.rules]], [[objects.merge]], [[objects.split]], [[objects.resize]], [[objects.to_map]], [[objects.to_vector]],
  [[objects.sample]], [[objects.accuracy]], [[objects.reclassify]].

## References

- Blaschke, T. (2010). Object based image analysis for remote sensing. *ISPRS Journal of Photogrammetry and Remote Sensing*, 65(1),
  2-16. https://doi.org/10.1016/j.isprsjprs.2009.06.004
- Myint, S., Gober, P., Brazel, A., Grossman-Clarke, S., & Weng, Q. (2011). Per-pixel vs. object-based classification of urban land cover
  extraction using high spatial resolution imagery. *Remote Sensing of Environment*, 115(5), 1145-1161.
  https://doi.org/10.1016/j.rse.2010.12.017
- O'Neil-Dunne, J., MacFaden, S., & Royar, A. (2014). A versatile, production-oriented approach to high-resolution tree-canopy mapping
  in urban and suburban landscapes using GEOBIA and data fusion. *Remote Sensing*, 6(12), 12837-12865.
  https://doi.org/10.3390/rs61212837
- Olofsson, P., Foody, G., Herold, M., Stehman, S., Woodcock, C., & Wulder, M. (2014). Good practices for estimating area and
  assessing accuracy of land change. *Remote Sensing of Environment*, 148, 42-57. https://doi.org/10.1016/j.rse.2014.02.015
- Radoux, J., & Bogaert, P. (2017). Good practices for object-based accuracy assessment. *Remote Sensing*, 9(7), 646.
  https://doi.org/10.3390/rs9070646
- Stehman, S. V. (2014). Estimating area and map accuracy for stratified random sampling when the strata are different from the map classes.
  *International Journal of Remote Sensing*, 35(13), 4923-4939. https://doi.org/10.1080/01431161.2014.930207
- Stehman, S., & Foody, G. (2019). Key issues in rigorous accuracy assessment of land cover products. *Remote Sensing of
  Environment*, 231, 111199. https://doi.org/10.1016/j.rse.2019.05.018
- Vincent, L., & Soille, P. (1991). Watersheds in digital spaces: an efficient algorithm based on immersion simulations. *IEEE
  Transactions on Pattern Analysis and Machine Intelligence*, 13(6), 583-598. https://doi.org/10.1109/34.87344
- Zhang, T., & Suen, C. (1984). A fast parallel algorithm for thinning digital patterns. *Communications of the ACM*, 27(3), 236-239.
  https://doi.org/10.1145/357994.358023
