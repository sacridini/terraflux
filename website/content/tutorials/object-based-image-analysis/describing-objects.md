---
title: "Describing objects: colour, shape, texture and context"
short: Describing objects
description: "Measure every object of a Sentinel-2 image of Brasilia: its colours, the NDVI and NDWI, its shape, its texture and its neighbours; then look at the feature space to see which attributes tell water, forest, cerrado and town apart."
order: 40
level: intermediate
time: 60 min
ops: [data.stac, raster.reproject, raster.calc, segment.multiresolution, objects.features, objects.to_map, raster.write, table.write, objects.classify]
series: Object-based image analysis
part: 4
---

A segmentation gives you objects, but an object is only a set of cells until you measure it. In this tutorial you
take the objects of the [previous parts](/tutorials/object-based-image-analysis/what-is-an-object/) and give each one
a row of numbers: its mean colour in every band, the vegetation and water indices, its size and shape, how rough or
smooth its surface is, and what lies around it. Then you look at where those numbers put the lake, the gallery
forest, the cerrado and the town, and learn which of them are worth keeping for a classification. At the end you
will have a table of 90 columns for 3,035 objects, a map of any of them, and a short list of the ones that
separate your classes.

## The idea

A pixel has one number per band, and that is all it has. An object has everything its cells have, summed up, and
more. Terraflux's [[objects.features]] computes these **attributes** in four families, with the names and the
definitions of eCognition's *features*, so that the large literature written with that program applies:

- **Spectral**: for each band the mean, the standard deviation, the minimum, the maximum and the median over the
  object's cells; the **brightness** (the mean of the band means); `max_diff` (the largest difference between two
  band means, relative to the brightness); the `ratio` of each band; and three indices made from the band means,
  NDVI, NDWI and NDMI. These are about *colour*.
- **Shape**: the area and the perimeter, how close the outline is to a rectangle (`compactness`) or to a square
  (`shape_index`), the length and width of the smallest rectangle that holds the object, its asymmetry and
  direction, and a few more. These are about *form*.
- **Texture**: how the colour changes *inside* the object, measured with the grey-level co-occurrence matrix
  (GLCM) of Haralick and colleagues (1973): homogeneity, contrast, entropy, correlation and angular second moment.
  A flat lake is smooth, a neighbourhood of roofs, trees and shadows is rough.
- **Context**: how the object differs from its neighbours (`nb_diff_<band>`, the number of neighbours), and, once the
  objects have classes, which classes they touch and how far the nearest object of a class is.

!!! science "Key idea"
    The attributes are the **coordinates of the object in a feature space**: if you pick two attributes, every object
    is a point on a plane; pick *p* attributes and it is a point in *p* dimensions. A classifier does nothing more than
    cut that space into regions, one for each class. It works when objects of one class sit close together and
    away from the others. So describing objects is choosing the axes of the space: a good attribute puts the classes
    in different places, a bad one puts them on top of each other.

Two indices are worth writing down, because they are in nearly every land-cover study. The **normalised difference
vegetation index** (NDVI; Tucker, 1979) and the **normalised difference water index** (NDWI; McFeeters, 1996) are

\[ \text{NDVI} = \frac{\rho_{\text{nir}} - \rho_{\text{red}}}{\rho_{\text{nir}} + \rho_{\text{red}}}
\qquad\text{and}\qquad
\text{NDWI} = \frac{\rho_{\text{green}} - \rho_{\text{nir}}}{\rho_{\text{green}} + \rho_{\text{nir}}}, \]

where \( \rho_{\text{nir}} \), \( \rho_{\text{red}} \) and \( \rho_{\text{green}} \) are the reflectances of the object (the
means of its cells) in the near-infrared, red and green bands. Green plants reflect the near infrared strongly and
absorb the red, so NDVI is high over them; water absorbs the near infrared and reflects some green, so NDWI is high
over water. A third index, **NDMI**, \( (\rho_{\text{nir}} - \rho_{\text{swir}}) / (\rho_{\text{nir}} + \rho_{\text{swir}}) \),
uses a short-wave infrared band and follows the water held in the leaves and the soil. Gao (1996) proposed a
normalised difference of the near infrared and a short-wave infrared band under the name NDWI; the name NDMI avoids the
clash with McFeeters'.

There is a difference with the pixel versions that is easy to miss: Terraflux makes the indices **from the mean
bands of the object**, not as the mean of the index over the cells. For objects that are fairly uniform the two are
close; for a mixed object they are not the same, and the object-level one is the one an eCognition user would get.

### Where it comes from

Texture came first. Haralick, Shanmugam and Dinstein (1973) proposed fourteen measures of the spatial pattern of grey
tones, all computed from a matrix that counts how often a grey level \( i \) sits next to a grey level \( j \). When
the objects of OBIA arrived, the matrix could be computed over the cells of one object instead of a square window,
and the measures became attributes of the object. Laliberte and Rango (2009) used them on drone images to
classify rangeland. The idea that an object has *context*, and that a roof beside a road is not a roof beside a field,
is the other pillar of the field; Blaschke (2010) reviews both.

## When to use it

Describe your objects whenever the next step is to classify them, by samples (the [next tutorial](/tutorials/object-based-image-analysis/classifying-by-samples/))
or by rules (part 6). The attributes are also useful on their own:

- to **look** at the image: a map of NDVI by objects, or of the objects' texture, shows patterns that the colours hide;
- to **choose** which attributes a classifier should read, by their separability (below);
- to **filter** objects: the small ones, the long thin ones (roads), the very smooth ones (water, a pasture).

It is *not* worth computing everything when you have a large image and a small question. The texture is the costly
group, and 90 attributes of which 60 say the same thing slow every later step. Start with the default (`basic`) and
add what a figure or a test tells you to add.

## What you need

- **The image.** The Sentinel-2 clip of the series, the same as in part 2: five bands (blue, green, red, near
  infrared and `swir16`) at 10 m, in UTM zone 23S, over the edge of Lago Paranoá. The model below fetches it with
  [[data.stac]]; without a network, download [sentinel2_brasilia.tif](/data/tutorials/object-based-image-analysis/sentinel2_brasilia.tif)
  (see [ATTRIBUTION.txt](/data/tutorials/object-based-image-analysis/ATTRIBUTION.txt): *Contains modified Copernicus
  Sentinel data 2024*) and read it with [[raster.read]] instead of the first two nodes, as in part 2.
- **Objects.** The page cuts them with [[segment.multiresolution]] at the scale 2, as in part 2. Part 3 explains how to
  choose the scale.
- **Names for the bands.** [[objects.features]] finds the red, near-infrared and green bands from their names (`red`,
  `nir`, `green`); the short-wave infrared band needs to be told (`swir`), as you will see.

## Build the model

### Steps 1 to 4: the image and the objects

The first four nodes are the ones of the previous tutorial: [[data.stac]] reads the scene of 9 August 2024,
[[raster.reproject]] puts it in UTM at 10 m, [[raster.calc]] makes the reflectance from 0 to 1 and
[[segment.multiresolution]] cuts it at the scale 2, giving 3,035 objects.

### Step 5: the attributes

Add [[objects.features]]. It has two inputs, the `objects` and the `image` they are measured on (the stack of bands:
`refl.image`), and gives back the same `objects` with the attributes in their table, and the `table` itself. The
settings that matter:

- **`features`** says what to compute. `basic` (the default) gives, for each band, the mean and the standard deviation,
  NDVI, the area, the compactness and the homogeneity of the brightness. `all` gives every group. A group name
  (`spectral`, `shape`, `texture`, `context`), a feature name (`ndvi`, `contrast`, `shape_index`...) or a list of them
  asks for just those: `features = ["spectral", "contrast", "compactness"]`.
- **`swir`** names the short-wave infrared band (`swir = "swir16"`). Without it the model has no NDMI: the bands'
  names do not say which one is the short-wave infrared, so Terraflux leaves the index out. `red`, `nir` and `green`
  work the same way, for images whose bands are called `b4`, `b8`, `b3`.
- **`levels`** and **`texture_bands`** shape the texture (see [Settings that change the answer](#settings-that-change-the-answer)).

The model asks for `all`, to see everything once.

```tfgraph
#! caption: Steps 1 to 5: the scene, its UTM grid and reflectance, the objects (scale 2) and all their attributes (nodes 4 and 5 are new).
#! highlight: segments, feat
#! numbered: true
#! fragment: true
[[nodes]]
id = "refl"
op = "raster.calc"
inputs = { s = "utm.raster" }
params = { expression = "out image = s / 10000" }
note = "Reflectance 0 to 1 (nodes 1 to 3 as in part 2)."

[[nodes]]
id = "segments"
op = "segment.multiresolution"
inputs = { image = "refl.image" }
params = { scale = 2 }
note = "3,035 objects."

[[nodes]]
id = "feat"
op = "objects.features"
inputs = { objects = "segments.objects", image = "refl.image" }
params = { features = "all", swir = "swir16" }
note = "90 columns."
```

### Step 6: look at the attributes, and keep them

Two more nodes make the attributes useful outside the model. [[objects.to_map]] paints each object's cells with the
value of one attribute (`what = "ndvi"`; any column of the table works), which [[raster.write]] saves as a GeoTIFF.
[[table.write]] writes the whole table to a CSV file, a row per object, which a spreadsheet or R can read.

```tfgraph
#! caption: The whole model: objects, all their attributes, a map of the NDVI by objects and the table as a CSV file.
#! highlight: ndvi_map, write_map, write_table
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
id = "segments"
op = "segment.multiresolution"
inputs = { image = "refl.image" }
params = { scale = 2 }
note = "3,035 objects."

[[nodes]]
id = "feat"
op = "objects.features"
inputs = { objects = "segments.objects", image = "refl.image" }
params = { features = "all", swir = "swir16" }
note = "Colour, shape, texture and context."

[[nodes]]
id = "ndvi_map"
op = "objects.to_map"
inputs = { objects = "feat.objects" }
params = { what = "ndvi" }
note = "Each cell takes its object's NDVI."

[[nodes]]
id = "write_map"
op = "raster.write"
inputs = { value = "ndvi_map.map" }
params = { path = "ndvi_objects.tif" }

[[nodes]]
id = "write_table"
op = "table.write"
inputs = { table = "feat.table" }
params = { path = "objects_features.csv" }
note = "A row per object."
```

=== "Application"

    Build the nodes as in part 2 and add [[objects.features]] after the segmenter: drag `refl.image` to its `image`
    input and the segmenter's `objects` to its `objects`. In the **Node pane**, choose `all` for `features` and type
    `swir16` in `swir`. Select the node and press ++ctrl+enter++ (*Run to here*). In the Viewer, the layer *objects*
    of the node is drawn with the objects' outlines; click an object and the inspector lists all its attributes,
    and the attribute table has a row for each object.

=== "Command line"

    Save the graph as `describe.toml` and run it. The table is written to `objects_features.csv`, the map to
    `ndvi_objects.tif`.

    ```bash
    terraflux run describe.toml --threads 4 --out output
    ```

=== "Without a network"

    Replace the nodes `s2` and `utm` with a read of the downloaded clip, which is already in UTM:

    ```toml
    [[nodes]]
    id = "s2"
    op = "raster.read"
    params = { path = "sentinel2_brasilia.tif" }
    ```

    and make the reflectance node read `s2.raster`. The counts of the page can differ by a few objects (2,992 objects
    from the file at the scale 2, 3,035 from the server).

The run is quick: the attributes of 3,035 objects, with all the texture, take well under a second here. That changes on a large image
with a fine segmentation, as the pitfalls below say.

## Read the results

Open the table. It has `id`, `cells`, `area` (in square metres, because the clip is in UTM), and then the attributes,
one column each, in a fixed order within each family. For the five bands, which Terraflux keeps in alphabetical order,
a column is named after the band: `mean_nir`, `sd_red`, `glcm_contrast_green`. Each family is a different way of seeing
the same objects. The figures below show the part of the clip drawn in part 2 (the lake edge, the houses, the gallery
forest and the cerrado), with each object painted by an attribute.

### Colour: the spectral attributes

```tfgallery
columns: 1
caption: "Three spectral attributes of each object, over the part of the clip shown in part 2 (scale 2). The lake is dark in the NDVI and the brightness and the only bright patch of the NDWI; the gallery forest is the yellow line of the NDVI; the neighbourhoods are brightest."
images:
  - { src: /img/tutorials/object-based-image-analysis/describing-objects/spectral.png, caption: "NDVI, NDWI and brightness, object by object" }
```

Read the three panels one at a time.

- **NDVI** separates water (below zero) from everything else at once and lights up the gallery forest along the
  streams, but it is a weak guide among the land classes: the dry cerrado (a mean of about 0.47) is not far from the
  forest (0.51), and the neighbourhoods (0.32) have a lot of green in their gardens. The class means are in the table
  below.
- **NDWI** is the cleanest of all: one object type has a high value, and it is the lake. Nothing else comes close.
  That is why part 7 of the series maps water with this index alone.
- **Brightness** is the mean of the band means. The town is the brightest class: roofs, concrete and bare ground reflect
  more than vegetation in every band, and the lake is almost black (0.012).

```tftable
data: /data/tutorials/object-based-image-analysis/describing-objects/class_means.csv
labels: { class: Class, objects: "Objects", area_ha: "Area (ha)", ndvi: NDVI, ndwi: NDWI, ndmi: NDMI, brightness: Brightness, mean_nir: "Mean NIR", sd_nir: "SD of NIR", glcm_contrast: "GLCM contrast", compactness: Compactness, shape_index: "Shape index" }
align: { class: left }
digits: { objects: 0, area_ha: 2, default: 3 }
caption: "The mean of some attributes over the objects of each class that the tutorial's samples pick (93 objects: see 'The feature space' below for how they were chosen). The class names are ours: they are the labels of a person who looked at the image."
```

The table shows what the maps hinted at. The five classes are in different places in colour, but not by the same
attributes: water is apart from the rest in every one; forest has the highest NDVI; cerrado is darker and
more uniform (its `sd_nir`, 0.009, is a third of the town's); the town is brightest, with the highest spread
between its cells. Bare soil falls in the middle of everything, which tells you it will be the hardest class.

### Form: the shape attributes

```tfgallery
columns: 1
caption: "Three shape attributes of each object: the area (logarithmic scale), the compactness and the shape index."
images:
  - { src: /img/tutorials/object-based-image-analysis/describing-objects/shape.png, caption: "Area, compactness and shape index, object by object" }
```

- **`area`** is in map units (m² here); divide by 10 000 for hectares. The objects here range from 0.09 to 6.9 ha, with
  a median of 0.89 ha. They are larger where the image is flat (the cerrado, the lake) and smaller in the town.
- **`compactness`** is `length x width / area` of the smallest rectangle around the object: 1 for a rectangle,
  more for an object with arms or bays (eCognition's definition).
- **`shape_index`** is `perimeter / (4 sqrt(area))`: 1 for a square, larger for a ragged outline. The perimeter here
  counts the cell edges that touch another object, no object or the image's edge.

Look at the colours in the figure: there is no pattern in them that a person could name. The objects come from a
segmentation that cuts along colour, and their outline is as ragged in the town as in the cerrado. In the table of class means above, the
compactness of every class is between 1.41 and 1.46. **Shape attributes tell you nothing about these classes**, and
the separability table below confirms it with numbers. They would tell you something about other things: roads
(long, thin, with a large `length_width`), buildings in a very fine image (rectangular), fields (rectangular,
too), forest edges. Here, at 10 m and with objects found by colour, they do not help. Report that, rather than leaving the
columns in.

### Texture: how rough is the surface

```tfgallery
columns: 1
caption: "Three GLCM attributes of the brightness of each object. The lake and the dry cerrado are smooth (contrast near 0, homogeneity near 1); the neighbourhoods are rough."
images:
  - { src: /img/tutorials/object-based-image-analysis/describing-objects/texture.png, caption: "GLCM homogeneity, contrast and entropy of the brightness" }
```

The texture attributes come from a **grey-level co-occurrence matrix**. Terraflux takes every pair of neighbouring cells
that are *both in the object* (at distance 1, in the four directions), cuts the brightness (or a band) into 32
grey levels between its 1st and 99th percentile, and counts how often each pair of levels occurs. From that matrix:

- **`glcm_homogeneity`** is high when neighbouring cells have the same level (a smooth surface);
- **`glcm_contrast`** is high when they differ a lot (a rough one);
- **`glcm_entropy`** is high when the pairs are spread over many combinations (a disordered one);
- **`glcm_correlation`** and **`glcm_asm`** (angular second moment, the sum of squares of the matrix, high when a few
  pairs dominate) complete Haralick's set.

Terraflux computes each of them for the brightness (the plain name) and for each band (`glcm_contrast_nir`...). The
maps tell the story of the town again: the lake and the cerrado are dark in contrast and yellow in homogeneity, the
neighbourhoods, with their mix of roofs, shadows and gardens, are the opposite. Texture is **the attribute that
separates the town from the cerrado best** here, better than any spectral one (see the separability table below).
It is also **a property of the object's size**: a small object has few pairs, and its texture is noisy. An object of
a single cell has none, and its texture is a missing value.

### Context: the neighbours

```tfgallery
columns: 1
caption: "Three context attributes. Left: the NIR of the object minus that of its neighbours (red: brighter than around it). Middle: the distance from the object's centre to the nearest object of the class water. Right: the share of the object's perimeter that touches an object of the class urban."
images:
  - { src: /img/tutorials/object-based-image-analysis/describing-objects/context.png, caption: "Difference to the neighbours, distance to water and border with urban objects" }
```

Context has two kinds of attributes. The first needs only the objects:

- **`nb_diff_<band>`** is the mean difference between the object's mean and its neighbours' means, weighted by the
  length of the border they share; it is **signed**: positive when the object is brighter than around it. In the
  figure the red objects are bright patches in a darker setting and the blue ones are dark patches in a brighter
  setting, as a stand of trees among roofs would be.
- **`nb_count`** is the number of neighbours, **`border_image`** the share of the perimeter on the image's edge or
  against a cell with no object.

The second kind needs the objects to **have classes already**: with classes, `features` adds, for every class `c`,
**`rel_border_<c>`** (the share of the perimeter that touches class `c`), **`dist_<c>`** (the distance, in map units,
from the centroid to the nearest centroid of an object of class `c`) and **`nb_<c>`** (the number of neighbours of class
`c`). It is the context you use in a second pass: classify once with colour and texture, then describe the objects
again with the context of that first map and classify them again. The two panels on the right of the figure
come from such a pass. Here the first map is the random forest of the next tutorial, run only to give the objects
classes, and the node of the context reads its result:

```tfgraph
#! caption: The second pass: the objects already classified (by the forest of part 5) described again, now with the context of their classes.
#! highlight: ctx
#! fragment: true
[[nodes]]
id = "rf"
op = "objects.classify"
inputs = { objects = "feat.objects", model = "fit.model", samples = "samples.table" }
note = "Objects with a class (part 5)."

[[nodes]]
id = "ctx"
op = "objects.features"
inputs = { objects = "rf.objects", image = "refl.image" }
params = { features = ["context"], swir = "swir16" }
note = "rel_border_<class>, dist_<class>, nb_<class>"
```

The class names become part of the column names with the spaces turned into underscores (`rel_border_bare_soil`).

## The feature space

Now ask the question that describing objects is for: **which of these attributes tells my classes apart?** You need
a few objects of each class for that, **samples**: objects that you know are water, forest, cerrado, urban, bare soil.
The next tutorial shows how to give them in the Viewer by clicking; here they are only needed to look.

!!! note "About the samples of this page"
    A figure cannot be clicked. The 93 objects of this page and the next were drawn, by a script that you can read
    (`website/figures/object-based-image-analysis/obia_samples.py`), from places of the clip that were looked at in
    true and false colour: for each class a few boxes, and a fixed number of points drawn in them, one object per
    point. They stand for what you do with the *Samples* tool of the Viewer. They are **not field data**: the labels
    are those of someone who looked at the image, and some of them are wrong (an object in a "urban" box can be a
    garden), as the labels of a real project can be.

In the Viewer, select the objects layer and open **Feature space...** in its side panel. It opens a window with:

- a **scatter** of any two attributes (x and y choose them from the lists), where the objects with a sample are drawn
  in the colour of their class and the others in grey; clicking a point selects its object on the map;
- a **histogram** of one attribute per class, where dragging a range lights up the objects inside it (and
  **Make a rule** writes it as a rule of that class, which part 6 uses);
- the **separability** of the classes, attribute by attribute, and a list of the attributes that separate best.

```tfgallery
columns: 1
caption: "Left: the objects in the plane of the mean red and the mean near infrared; right: NDVI against the GLCM contrast of the brightness. Grey: the 3,035 objects; coloured: the 93 sampled."
images:
  - { src: /img/tutorials/object-based-image-analysis/describing-objects/space.png, caption: "Two views of the feature space, with the sampled objects coloured by class" }
```

On the left, the classes are on top of each other: only the lake is apart, and the other four classes (forest,
cerrado, urban, bare soil) form one cloud, with cerrado (dark yellow) at its bottom edge, where the near infrared is
lowest. **Two spectral bands are not enough.** On the right, the vertical axis (texture) pulls the classes apart:
cerrado is flat on the floor (low contrast), urban objects rise to 4, 6, 9, and forest sits between. The lake is the
cluster of dots on the left, with an NDVI below zero. Forest and urban still overlap: the garden and the tree-lined street of a
town are a forest-like texture at 10 m.

```tfgallery
columns: 1
caption: "Histograms of three attributes by class (the lake is left out, far from the rest). Where the bars of two classes lie on top of each other, the attribute does not separate them."
images:
  - { src: /img/tutorials/object-based-image-analysis/describing-objects/hist.png, caption: "NDVI, brightness and GLCM contrast by class" }
```

### Separability

The eye gives out when there are more than a few attributes. The Viewer computes the **Jeffries-Matusita (JM)
distance** between each pair of classes, for each attribute. It treats the samples of each class as a normal
distribution (a mean \( m \) and a variance \( v \)) and measures how far apart two of them are:

\[ B = \frac{(m_1 - m_2)^2}{4\,(v_1 + v_2)} + \frac{1}{2}\ln\frac{v_1 + v_2}{2\sqrt{v_1 v_2}}, \qquad
\text{JM} = 2\,\bigl(1 - e^{-B}\bigr). \]

\( B \) is the Bhattacharyya distance of the two normals: the first term grows when the means are far apart relative to the
spread, the second when the spreads differ. JM squeezes it into a scale from **0 (the same) to 2 (completely
apart)**, which is easier to read (Richards, 2013). Rule of thumb: above 1.9 the pair is separated; below 1 it
is not. The Viewer prints the pair table, with the values over 1.9 in green and under 1 in a warning colour, and a list
of the five attributes with the largest *smallest* distance over all pairs, each with a button to use it as the axes.

The table below is the same computation, made by the figure script on the same samples with the same formula, for twelve attributes and
the pairs that matter. Read it by rows (an attribute) and by columns (a pair).

```tftable
data: /data/tutorials/object-based-image-analysis/describing-objects/separability.csv
labels: { attribute: Attribute }
align: { attribute: left }
digits: 2
heat: true
caption: "The Jeffries-Matusita distance (0 the same, 2 apart) of five pairs of classes, and the least over all ten pairs, for twelve attributes of the 93 sampled objects."
```

- **Water against the rest** (`forest / water`) is 2.0 in the spectral attributes: every one of them separates it, and
  the NDWI and the NIR most of all. It is the easy class.
- **Urban against cerrado** is best told by the **texture**: homogeneity 1.81 and contrast 1.42, then the near-infrared mean
  (1.27) and brightness (0.97). The NDVI (0.38) is worse than all of them.
- **Forest against cerrado** is told by the homogeneity (1.06), the near-infrared (0.92) and the contrast (0.86), and by the
  difference to the neighbours (0.57).
- **Urban against forest** is **hard for every attribute**: the best is NDMI with 0.48, and NDVI and NDWI get 0.42 and
  0.43.
- **Shape does not help**: compactness is at 0.00 to 0.07 in the pairs shown and the area at 0.05 to 0.31.
- **No attribute separates all ten pairs**: the highest *smallest* distance is 0.12 (NDWI), because
  of the pairs of classes that are alike in every way (bare soil and cerrado, in the NDMI: 0.00). One attribute is not enough; a
  classifier that reads many is.

!!! warning "A distance from a few objects"
    The distance rests on the mean and the variance of 9 to 26 objects per class and on the assumption that the
    values are normal. Read it as a guide to which attributes to look at, not as a test. A distance of 2.0 from a
    handful of objects is a good sign; a difference between 1.4 and 1.8 is not worth a decision.

## Settings that change the answer

### The grey levels of the texture

`levels` (2 to 256, 32 by default) is the number of grey levels the brightness is cut into before the matrix is made.
Fewer levels give a smaller matrix and a coarser measure; more levels give a finer one that needs more cells in an
object to be reliable. The slider shows the GLCM contrast of the same objects with 4 to 128 levels. Each frame has its own colour
range (the contrast grows with the square of the number of levels: the median is 0.14 with 4 and 39.7 with 128, so
only the *pattern* can be compared), and the numbers beside it are the JM distances of the contrast between the pairs of classes.

```tfslider
label: Grey levels
caption: "The GLCM contrast of the brightness with from 4 to 128 grey levels. Each frame is stretched to its own range. The chart shows how well the contrast separates urban objects from cerrado and from forest."
frames:
  - { value: 4, src: /img/tutorials/object-based-image-analysis/describing-objects/levels_4.png, caption: "4 levels", stats: { "Median contrast": "0.14", "Urban / cerrado": "0.63", "Urban / forest": "0.20" } }
  - { value: 8, src: /img/tutorials/object-based-image-analysis/describing-objects/levels_8.png, caption: "8 levels", stats: { "Median contrast": "0.31", "Urban / cerrado": "1.26", "Urban / forest": "0.26" } }
  - { value: 16, src: /img/tutorials/object-based-image-analysis/describing-objects/levels_16.png, caption: "16 levels", stats: { "Median contrast": "0.79", "Urban / cerrado": "1.44", "Urban / forest": "0.27" } }
  - { value: 32, src: /img/tutorials/object-based-image-analysis/describing-objects/levels_32.png, caption: "32 levels (the default)", stats: { "Median contrast": "2.65", "Urban / cerrado": "1.42", "Urban / forest": "0.28" } }
  - { value: 64, src: /img/tutorials/object-based-image-analysis/describing-objects/levels_64.png, caption: "64 levels", stats: { "Median contrast": "10.12", "Urban / cerrado": "1.44", "Urban / forest": "0.28" } }
  - { value: 128, src: /img/tutorials/object-based-image-analysis/describing-objects/levels_128.png, caption: "128 levels", stats: { "Median contrast": "39.74", "Urban / cerrado": "1.43", "Urban / forest": "0.29" } }
chart:
  type: line
  data: /data/tutorials/object-based-image-analysis/describing-objects/levels.csv
  x: levels
  y: [urban_cerrado, urban_forest]
  labels: { urban_cerrado: "Urban / cerrado", urban_forest: "Urban / forest" }
  xlog: true
  ylabel: "Jeffries-Matusita distance"
```

Two things come out. **The values of the contrast depend on `levels`**: do not compare a contrast made with 32
levels with one made with 64, or put a threshold on it in a rule and later change the levels. And **the separation does
not improve past 16 levels** (and drops with 4 or 8): the default of 32 is a safe choice for objects of this size, and a
small number of levels loses the detail that told urban from cerrado.

### Which bands the texture reads

`texture_bands` says which layers the texture is computed for: a list of band names, or `brightness`. With `basic` the
default is the brightness only; with the other groups it is the brightness and every band, so the table gets
`glcm_contrast_blue`, `glcm_contrast_nir`... Texture of five bands is more work and, since most bands are
correlated, adds less than it costs. Naming the band you want (`texture_bands = ["nir"]`) is the cheap way to get the
one that matters.

### The objects themselves

Every attribute above is computed over the objects you gave. A coarser scale makes bigger objects, whose means are
averages over more mixed cells, whose texture has more pairs and whose shape is more ragged. The attributes are
not the property of the *image*: they belong to image **and** segmentation. If you change the scale of part 3, compute
and look at the attributes again.

## Common pitfalls

- **No NDMI without `swir`.** [[objects.features]] finds the red, near-infrared and green bands from their names but
  not the short-wave infrared one, which Sentinel-2 calls `swir16` or `swir22`: the model runs and the NDMI column is
  simply not there. Give `swir = "swir16"`.
- **Bands that do not say what they are.** For an image with bands called `b1`, `b2`... give `red`, `nir`, `green` and
  `swir`, or the indices are left out. Check the columns of the table after the first run.
- **Position and size as predictors.** The `shape` group has the `centroid_x` and `centroid_y` of every object, and
  everyone has `cells` and `area`. A classifier that reads the position learns where the samples *are*, not what they
  look like, and does well on neighbours and badly elsewhere (see the next tutorial). Leave the position out of the
  predictors, and think twice before using the size.
- **`all` on a big image.** The texture needs a pass over the cells of every object for each layer, and the median needs
  the values of an object in memory. On a large image with millions of objects, name what you will use.
- **Many correlated attributes.** `mean_blue` and `mean_green` correlate at 0.99 here; Terraflux warns of such pairs
  when you classify. More attributes do not make a classifier better, and they make the nearest-neighbour method
  worse, since each one adds to the distance.
- **Texture of tiny objects.** A missing `glcm_...` (shown as an empty cell) means the object has no pair of cells.
  Small objects have noisy texture: judge the attribute on objects of the size you will classify.
- **Reading the indices as pixel indices.** The NDVI of an object is made from the object's mean bands, not averaged
  over the cells. For a heterogeneous object the two differ.
- **Choosing by one scatter.** Two attributes are a plane, and classes that overlap in it may be apart in a third. Use
  the separability to find candidates, then check them.
- **Trusting separability from few samples.** See the warning above: nine objects are not a distribution.

## Going further

- Next in the series: [classifying by samples](/tutorials/object-based-image-analysis/classifying-by-samples/), which
  uses these attributes with the nearest-neighbour method and a random forest, and checks the result with
  [[objects.accuracy]]. Part 6 does the same with rules, where the ranges of this page become fuzzy memberships.
- The right scale for these objects: [the previous part](/tutorials/object-based-image-analysis/what-is-an-object/).
- The operators of this page: [[objects.features]], [[objects.to_map]], [[segment.multiresolution]], [[raster.write]]
  and [[table.write]]. The Viewer's windows: [the application's Viewer](/app/viewer/).

## References

- Blaschke, T. (2010). Object based image analysis for remote sensing. *ISPRS Journal of Photogrammetry and Remote
  Sensing*, 65(1), 2-16. https://doi.org/10.1016/j.isprsjprs.2009.06.004
- Gao, B. (1996). NDWI: a normalized difference water index for remote sensing of vegetation liquid water from space.
  *Remote Sensing of Environment*, 58(3), 257-266. https://doi.org/10.1016/S0034-4257(96)00067-3
- Haralick, R. M., Shanmugam, K., & Dinstein, I. (1973). Textural features for image classification. *IEEE
  Transactions on Systems, Man, and Cybernetics*, SMC-3(6), 610-621. https://doi.org/10.1109/TSMC.1973.4309314
- Laliberte, A. S., & Rango, A. (2009). Texture and scale in object-based analysis of subdecimeter resolution unmanned
  aerial vehicle (UAV) imagery. *IEEE Transactions on Geoscience and Remote Sensing*, 47(3), 761-770.
  https://doi.org/10.1109/TGRS.2008.2009355
- McFeeters, S. K. (1996). The use of the normalized difference water index (NDWI) in the delineation of open water
  features. *International Journal of Remote Sensing*, 17(7), 1425-1432. https://doi.org/10.1080/01431169608948714
- Richards, J. A. (2013). *Remote sensing digital image analysis: an introduction* (5th ed.). Springer.
  https://doi.org/10.1007/978-3-642-30062-2
- Tucker, C. J. (1979). Red and photographic infrared linear combinations for monitoring vegetation. *Remote Sensing of
  Environment*, 8(2), 127-150. https://doi.org/10.1016/0034-4257(79)90013-0
