---
title: "Measuring a segmentation against reference objects"
short: Reference objects
description: "Draw twelve reference objects on the map, measure every scale of the multiresolution segmentation against them (over-segmentation, under-segmentation, D, F and ED2), see where each object errs, and let segment.optimize choose the scale by the reference instead of by the variance, with the template Choose the segmentation by reference objects."
order: 200
level: advanced
time: 60 min
template: Choose the segmentation by reference objects
ops: [segment.evaluate, segment.optimize, segment.multiresolution, vector.read, objects.features]
series: Object-based image analysis
part: 20
---

In [Choosing the scale](/tutorials/object-based-image-analysis/choosing-the-scale/) the image helped you choose: ESP and USPO
looked at the objects and the pixels and said which scale was likely. Neither of them knew what *you* meant by an object.
Here you will tell it. You will draw twelve polygons over the Sentinel-2 clip of Lago Paranoá, the things you would want the
segmentation to find (the lake, a stretch of cerrado, a gallery forest, a block of roofs, an avenue), and measure every scale of
the multiresolution segmentation against them. You will read three numbers (over-segmentation, under-segmentation and their
combination), paint on the map where each object errs, and let [[segment.optimize]] choose the scale by the reference. At the
end you will know what a reference of twelve polygons can say about a segmentation, and what it cannot.

## The idea

A segmentation is good when its objects **are** the things you care about. The way to check that is to draw some of those
things, the **reference objects**, and see how well the objects cover them. Two things can go wrong, and they pull in opposite
directions.

- **Over-segmentation**: a reference is cut in pieces. No object holds the whole of it.
- **Under-segmentation**: an object spans more than the reference, so it holds a bit of something else.

For one reference \( r \) and one object \( s \) that overlap, with \( |\cdot| \) the number of cells, Clinton and colleagues (2010) measure

\[ OS = 1 - \frac{|r \cap s|}{|r|}, \qquad US = 1 - \frac{|r \cap s|}{|s|}, \qquad D = \sqrt{\frac{OS^2 + US^2}{2}}. \]

\( OS \) is the share of the reference the object leaves out, \( US \) the share of the object that lies outside the reference, and
\( D \) joins them. All three are 0 for a perfect match. The two measures are of **pairs**, so the first question is which object
goes with which reference: Clinton's rule keeps the pairs that overlap and either hold each other's centroid or share more than
half of the object or of the reference. A reference cut in four pieces gives four pairs, each with a high \( OS \) and a low \( US \).

!!! science "Key idea: OS and US are two different errors"
    Cut a reference in two and the objects are halves of it: \( OS \) is 0.5 and \( US \) is 0. Merge two references into one object and
    the object is twice the size of each: \( OS \) is 0 and \( US \) is 0.5. A single number, \( D \), cannot tell them apart; the pair can, and
    the choice of what to do (a smaller scale, or a larger one) depends on which of them is large.

[[segment.evaluate]] gives the numbers of the package `segmetric` (Costa and colleagues, 2018), and Terraflux's results equal that package's on the cases
of its tests: `os`, `us`, `d`, `qr` (the quality rate, \( 1 - |r \cap s| / |r \cup s| \)), `afi` (the area fit index of Lucieer and Stein, 2002), `precision`,
`recall` and `f_overlap` (their F-measure, after Zhang and colleagues, 2015). It adds the three measures of Liu and colleagues (2012), which `segmetric`
does not have: `pse`, the share of the corresponding objects that lies outside the references, `nsr`, how far the number of corresponding objects is from the number of references, and
`ed2 = sqrt(pse² + nsr²)`, their distance from a perfect segmentation. Terraflux follows the paper as read; there is no
reference implementation to compare it with.

## When to use it

- **Choosing a scale by what you mean** rather than by the variance: the references are your question.
- **Comparing segmenters** (multiresolution, SLIC, watershed...) on the same places.
- **Reporting** how good a segmentation is, in a paper: the numbers have names the literature knows.
- **Finding where it errs**: the objects coloured by their error, to decide where to edit or to split.

It is not the accuracy of a classification: a segmentation that matches the references can still be classified wrongly, and a
poor one can be classified well. [Classifying by samples](/tutorials/object-based-image-analysis/classifying-by-samples/) (part 5) measures the classes.

## What you need

- The Sentinel-2 clip of the series (`data.stac` in the application, the same as in the other parts), cut into objects at several scales.
- Twelve polygons, drawn by you over the places that matter: in the application, **Draw on the map** in the Viewer. Here they are in
  a GeoPackage, `reference.gpkg`, which you can replace with your own.
- About an hour. Twenty to fifty reference objects is a good number for a real study; twelve are enough to see how the measures behave.

## Build the model

### Step 1: draw the reference

Open the segmentation in the Viewer. In the panel at the right, under **Draw on the map**, choose **Polygon** and click the corners
of each thing; a double click closes it. Then press **Add as reference objects**: Terraflux writes the polygons as a GeoPackage beside the model
(`drawn/`), adds a [[vector.read]] node that reads it and a [[segment.evaluate]] node that measures the objects you are looking at against it, **all in one step**
(one *undo* takes them away again). If the objects already have a [[segment.evaluate]], the new polygons replace its reference.

![The twelve reference objects drawn over the clip](/img/tutorials/object-based-image-analysis/measuring-against-reference-objects/references.png "The twelve references drawn by eye: 1 the lake, 2 an inlet, 3 the dry cerrado, 4 the bare shore, 5 a building site, 6 a gallery forest strip, 7 the forest along a creek, 8 to 10 three blocks of roofs, 11 an avenue and 12 a white roof. Together they cover 17,509 cells, 5 % of the clip.")

Draw them over **different** looks of the image, not over one: water, bare ground, forest, roofs, a road. A reference is a sample of what you want the
segmentation to find, and the scale that suits the lake is not the scale that suits the roofs (part 3).

### Step 2: measure

```tfgraph
#! caption: The segmentation at a scale and its measure against the reference: the polygons are read from the file, the objects are compared with them.
#! highlight: ref, evaluate
#! numbered: true
#! fragment: true
[[nodes]]
id = "ref"
op = "vector.read"
params = { path = "reference.gpkg" }
note = "The twelve reference polygons."

[[nodes]]
id = "segments"
op = "segment.multiresolution"
inputs = { image = "refl.image" }
params = { scale = 6 }
note = "The segmentation to measure."

[[nodes]]
id = "evaluate"
op = "segment.evaluate"
inputs = { objects = "segments.objects", reference = "ref.vector" }
note = "OS, US, D, QR, AFI, precision, recall, the F of the overlap, and PSE, NSR and ED2, averaged over the pairs; the table has a row for each pair and the objects output the os and us of each object."
```

The `summary` output is one row; the `table` has a row for each pair of a reference and an object that some measure uses; the `objects` output is the segmentation with the columns `os`
and `us`, the error of each object against the reference it shares most with (an object that touches no reference has none).

### Step 3: choose the scale by the reference

Give the same polygons to [[segment.optimize]] as its `reference` and each scale is measured as [[segment.evaluate]] does. Its table of scores gains the columns
`os`, `us`, `d`, `ed2` and `f_overlap`, and its objective can be `d`, `ed2` or `f_overlap`: the scale with the lowest error (or the highest overlap F) wins. The `objective` column of the table is
that measure scaled from 1 (the best of the scales) to 0, as for the other objectives. Without a reference nothing changes.

```tfgraph
#! caption: The whole model: the scene, its reflectance, the reference, ten scales measured against it and the best one measured again.
#! highlight: opt, best
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
id = "ref"
op = "vector.read"
params = { path = "reference.gpkg" }
note = "The reference objects."

[[nodes]]
id = "opt"
op = "segment.optimize"
inputs = { image = "refl.image", reference = "ref.vector" }
params = { values = [1, 1.5, 2, 3, 4, 6, 8, 12, 16, 24], objective = "ed2" }
note = "Each scale measured against the reference; the winner is the scale of the lowest ED2."

[[nodes]]
id = "best"
op = "segment.evaluate"
inputs = { objects = "opt.objects", reference = "ref.vector" }
note = "The best segmentation measured again: its summary, and the os and us of its objects."
```

```bash
terraflux run model.toml
```

## Read the results

### The scales, one by one

```tftable
digits: { scale: 1, objects: 0, os: 3, us: 3, d: 3, f_overlap: 3, ed2: 2, uspo_half: 3 }
labels: { scale: Scale, objects: Objects, os: OS, us: US, d: D, f_overlap: "F (overlap)", ed2: ED2, uspo_half: "USPO objective / 2" }
columns: [scale, objects, os, us, d, f_overlap, ed2, uspo_half]
data: /data/tutorials/object-based-image-analysis/measuring-against-reference-objects/scores.csv
caption: "The scores of segment.optimize with the twelve references on the ten scales of part 3: the means of OS, US and D over the pairs, the F of the overlap, ED2, and, for comparison, the objective of the same run without a reference (the sum of two scores scaled from 0 to 1, over 2)."
```

Read the columns as a story. At scale 1 the clip has 11,145 objects of about 30 cells: each reference is cut in dozens of pieces, so OS is 0.98 and US is small. As the scale grows the objects
swallow more, OS falls and US rises, and from scale 12 on US passes 0.7: an object holds a reference and what is around it. The two cross at about 8, near 0.5.

```tfchart
type: line
data: /data/tutorials/object-based-image-analysis/measuring-against-reference-objects/scores.csv
x: scale
y: [os, us, d, f_overlap]
labels: { os: "OS (reference cut)", us: "US (object spans more)", d: "D", f_overlap: "F of the overlap" }
colors: { os: "#d1495b", us: "#2e86c1", d: "#444444", f_overlap: "#3b8a5a" }
xlabel: Scale
ylabel: Measure
height: 300
caption: "The measures against the twelve references at the ten scales: OS falls and US rises with the scale; D is flat near 0.57 from scale 6 to 12 because the two errors trade; F peaks at 0.33 at scales 4 and 6."
```

**D hides the trade**: its minimum is at scale 8 (0.565) but it is within 0.01 of its value at 6 and 12. ED2 does not forgive: it grows with the number of objects (`nsr`: 45 at scale 1, where there are
11,145 objects for twelve references) and with the share of the objects outside the references (`pse`: 6 at scale 16). Its minimum is at scale **6** (0.95), with 313 objects.

```tfchart
type: line
data: /data/tutorials/object-based-image-analysis/measuring-against-reference-objects/scores.csv
x: scale
y: [ed2_scaled, uspo_half]
labels: { ed2_scaled: "ED2 against the reference (scaled, 1 the best)", uspo_half: "USPO objective / 2 (no reference)" }
colors: { ed2_scaled: "#e8901f", uspo_half: "#8e6bbf" }
xlabel: Scale
ylabel: Objective
height: 300
caption: "What each method would choose. The reference puts the best at 6; the objective of USPO (the variance and Moran's I, no reference) is highest at 1.5, again high at 24, and lowest in the middle, at 8."
```

On this range of scales USPO's objective is highest at 1.5 and nearly as high at 24, with its lowest value in the middle, so it points at a scale none of the references likes. The reference chooses a scale in
the middle. The variance inside objects falls as they get smaller whatever they hold, so it cannot say that an object is the *right* one; a reference can. (Part 3 reads the same scores on a finer set of scales, and
USPO's answer there is its own.)

### Where it errs

The `objects` output of [[segment.evaluate]] gives each object its `os` and `us`. In the Viewer, the panel **Reference objects** shows the numbers (ED2, D, F, OS, US) and a box, **Show where it errs**, that
colours each object by `os` less `us`: red where the reference was cut (the object is a piece of it), blue where the object holds what is outside the reference. The figures below are the same colours.

```tfgallery
columns: 3
caption: "The objects of three scales coloured by os less us (red: the reference was cut; blue: the object holds more than the reference), with the twelve references drawn in black."
images:
  - { src: /img/tutorials/object-based-image-analysis/measuring-against-reference-objects/errors_fine.png, caption: "Scale 1: red, the references are in pieces" }
  - { src: /img/tutorials/object-based-image-analysis/measuring-against-reference-objects/errors_best.png, caption: "Scale 6: the best by ED2" }
  - { src: /img/tutorials/object-based-image-analysis/measuring-against-reference-objects/errors_coarse.png, caption: "Scale 16: blue, the objects span more" }
```

At scale 1 almost every object near a reference is red. At scale 16 the lake is almost one object, which is good, but the avenue (11) and the blocks of roofs (8 and 10) are
inside one object that is the whole town: dark blue. At scale 6 the lake and the building site are well matched, the cerrado is in several pieces and the thin avenue is still swallowed.
**No scale is right for all twelve**, and this is what the table of pairs says:

```tftable
sortable: true
align: { name: left }
labels: { reference: Ref., name: Reference, object: Object, overlap: "Cells in common", reference_cells: "Reference cells", object_cells: "Object cells", os: OS, us: US, d: D }
columns: [reference, name, object, overlap, reference_cells, object_cells, os, us, d]
digits: { os: 3, us: 3, d: 3 }
data: /data/tutorials/object-based-image-analysis/measuring-against-reference-objects/pairs_best.csv
caption: "The pairs of the segmentation at scale 6 that Clinton's rule keeps: 24 pairs for twelve references. The building site (5) and the inlet (2) have one object each, with OS and US between 0.1 and 0.3; the avenue (11) has two objects, each with US above 0.8."
```

## Settings that change the answer

- **The references.** Everything above is relative to them. Twelve polygons on 5 % of the image say how the segmentation does *there*. Draw over the classes you will map, not over the
  easy places, and draw more than you think you need.
- **What the reference is made of.** A polygon is laid on the grid as [[segment.from_vector]] does it: a cell belongs to the polygon holding its centre, and where polygons overlap the later one wins.
  The areas are counts of cells, so the numbers equal `segmetric`'s for polygons made of whole cells and are close for others.
- **`metrics`.** Name the measures you want and the others are not computed or shown.
- **The objective.** `ed2` penalises both errors and the number of objects; `d` is blind to the trade; `f_overlap` rewards the matching of the best object with each reference.
  Run the three and see if they agree: when they do not, that is information.

!!! warning "ED2 is not bounded by 1"
    `nsr` is the difference between the number of references and the number of corresponding objects over the number of references, and `pse` is a share of the reference area: both can be above 1.
    The ED2 of 45.5 at scale 1 is the number of objects over the number of references, not an error of 4,550 %.

## Common pitfalls

- **Judging the classification by the segmentation, or the segmentation by the classification.** A segmentation that matches the references is a good start, not a good map.
- **Tuning on the reference you will report with.** Choose the scale on some references and report on others, as with samples.
- **Reading D alone.** It is flat where the two errors trade; read OS and US.
- **A reference that is not a thing.** A polygon over half a field and half a road has no object that matches it: draw things.
- **References on the easy places.** The lake and the cerrado are easy; the avenue and the roofs are not. A reference of lakes chooses a coarse scale.
- **Trusting twelve polygons for a paper.** They show how the measures behave. Use enough to cover the variety of the image.

## Going further

- The operators: [[segment.evaluate]], [[segment.optimize]], [[segment.multiresolution]], [[vector.read]].
- Choose the scale without a reference, and why one scale is not enough: [Choosing the scale](/tutorials/object-based-image-analysis/choosing-the-scale/) (part 3).
- Edit the objects that err by hand: [Editing by hand](/tutorials/object-based-image-analysis/editing-by-hand/) (part 10).
- Measure the classes, not the objects: [Classifying by samples](/tutorials/object-based-image-analysis/classifying-by-samples/) (part 5), with the accuracy and the confusion matrix.
- The template of this tutorial, **Choose the segmentation by reference objects**, runs on a drawn image whose regions are known: open it from the Templates menu.

## References

- Clinton, N., Holt, A., Scarborough, J., Yan, L., & Gong, P. (2010). Accuracy assessment measures for object-based image segmentation goodness. *Photogrammetric Engineering & Remote Sensing*, 76(3), 289-299.
  https://doi.org/10.14358/pers.76.3.289
- Costa, H., Foody, G. M., & Boyd, D. S. (2018). Supervised methods of image segmentation accuracy assessment in land cover mapping. *Remote Sensing of Environment*, 205, 338-351.
  https://doi.org/10.1016/j.rse.2017.11.024
- Liu, Y., Bian, L., Meng, Y., Wang, H., Zhang, S., Yang, Y., Shao, X., & Wang, B. (2012). Discrepancy measures for selecting optimal combination of parameter values in object-based image analysis.
  *ISPRS Journal of Photogrammetry and Remote Sensing*, 68, 144-156. https://doi.org/10.1016/j.isprsjprs.2012.01.007
- Lucieer, A., & Stein, A. (2002). Existential uncertainty of spatial objects segmented from satellite sensor imagery. *IEEE Transactions on Geoscience and Remote Sensing*, 40(11), 2518-2521.
  https://doi.org/10.1109/TGRS.2002.805072
- Weidner, U. (2008). Contribution to the assessment of segmentation quality for remote sensing applications. *International Archives of the Photogrammetry, Remote Sensing and Spatial
  Information Sciences*, 37(B7), 479-484.
- Zhang, X., Feng, X., Xiao, P., He, G., & Zhu, L. (2015). Segmentation quality evaluation using region-based precision and recall measures for remote sensing images. *ISPRS Journal of
  Photogrammetry and Remote Sensing*, 102, 73-84. https://doi.org/10.1016/j.isprsjprs.2015.01.009
