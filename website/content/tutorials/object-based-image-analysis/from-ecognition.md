---
title: Coming from eCognition
short: Coming from eCognition
description: "What is what when you know Trimble eCognition's rule sets: the process tree is a graph, the segmentations are segment.* operators, the class hierarchy is objects.rules, the levels are objects with a parent; with a worked example and what is deliberately not there."
order: 20
level: intermediate
time: 30 min
template: Classify an image by rules
ops: [segment.multiresolution, segment.scale, objects.features, objects.calc, objects.rules, objects.nearest, objects.merge, objects.grow, objects.remove_small, objects.sample, objects.accuracy, objects.to_vector]
---

If you have built rule sets in Trimble eCognition, you already know object-based image analysis: segment an image,
measure the objects, classify them with rules or samples, merge and refine, export. This page maps what you know
onto Terraflux, so that you can find each piece, and ends with the template **Classify an image by rules**, an
eCognition-style class hierarchy, as a worked example. If you have never used eCognition, start with [Your first map
by objects](/tutorials/object-based-image-analysis/first-map-by-objects/).

## The idea

eCognition's rule set is a **process tree**: a list of processes, run in order, that segment, compute features,
assign classes, merge and loop. In Terraflux the same work is a **graph**: each process is a node of an operator, the
nodes are joined by what they give and take (the objects, the image, a table of samples), and a node is run again only
when something it reads changed. There is no hidden state: the objects and classes a node reads are the ones the node
before it gives.

## What is what

| In eCognition | In Terraflux |
|---|---|
| Process tree, rule set | The model (the graph); a group (`control.group`) to fold a part; `control.repeat` for a loop |
| Multiresolution segmentation | [[segment.multiresolution]] (`scale`, `shape`, `compactness`, band `weights`) |
| Chessboard, quadtree segmentation | [[segment.chessboard]], [[segment.quadtree]] |
| Spectral difference segmentation | [[segment.spectral_difference]] (`max_difference`, band `weights`) on the objects of a first segmentation; or [[objects.merge]] with `by = "condition"` (a condition on the two neighbours' means) |
| Multi-threshold segmentation | [[segment.threshold]] (`thresholds`, or `classes` for multi-level Otsu; `band`) |
| Other segmentations | [[segment.slic]], [[segment.snic]], [[segment.felzenszwalb]], [[segment.watershed]], [[segment.meanshift]]; objects made elsewhere: [[segment.from_vector]], [[segment.from_map]] |
| ESP tool (estimation of scale parameter) | [[segment.scale]]; and [[segment.multiresolution]] with no `scale` takes its first suggestion |
| Image object level | Objects that have a **parent**: [[segment.multiresolution]] with `from` (the level below becomes the start regions) and [[objects.merge]] with `level` make a level above; `below` outputs hold the level beneath |
| Level, parent and child, super- and sub-objects | The Viewer: select an object and press ++p++ (its parent), ++c++ (its children), ++n++ (its neighbours); in [[objects.calc]] `parent.x` and, with `below`, `children.count(...)`, `children.area(...)`, `children.mean(...)` |
| Features (object, relational) | [[objects.features]] (groups `spectral`, `shape`, `texture`, `context`, or `all`); the context in [[objects.calc]]: `nb.mean(x)`, `nb.count(cond)`, `border_to("water")`, `distance_to("road")` |
| Customized features, arithmetic features | [[objects.calc]]: a script of one statement a line in Python's syntax over the attributes, `cls` (the class name), `id`, `cells` and `area` |
| Feature view | The Viewer: the **inspector** (every attribute of the selected object), the attribute table (a row per object, the one picked outlined on the map), and [[objects.to_map]] with `what` set to an attribute, to draw it as a map |
| Class hierarchy | [[objects.rules]]: `classes`, each `{ name, rule, parent, colour }`, a class inheriting its parent's rule; and the Model pane's **Classes** view |
| Membership functions | `above(x, a, b)`, `below(x, a, b)`, `between(x, a, b, c, d)` and `about(x, m, w)` in [[objects.calc]] and in a class's `rule` |
| Fuzzy `and (min)`, `or (max)`, `not` | The same: `and` is the smaller of two values, `or` the larger, `not x` is 1 - x |
| Standard Nearest Neighbour, samples, sample editor | The Viewer's **Samples** mode (a class palette, click objects) writes the points; [[objects.samples]] gets the objects' attributes under them; [[objects.nearest]] classifies as Standard Nearest Neighbour; a random forest ([[ml.fit]] with [[objects.classify]]) is the other path |
| Merge region | [[objects.merge]] (`by = "class"`, or by a `condition`) |
| Grow region | [[objects.grow]] (`class` and a `condition` on the candidate neighbour) |
| Remove objects, find small objects | [[objects.remove_small]] (`min_cells` or `min_area`, `into` the most similar neighbour or the longest border) |
| Assign class, classification | [[objects.rules]], [[objects.nearest]], [[objects.classify]] (the class and its membership are attributes of the objects); [[objects.reclassify]] renames, recolours and merges classes |
| Accuracy assessment, error matrix | [[objects.sample]] (the stratified sample to label) and [[objects.accuracy]]: confusion matrix, user's and producer's accuracy, kappa, and the area of each class with its confidence interval |
| Export vector layer, export raster | [[objects.to_vector]] and [[vector.write]]; [[objects.to_map]] and [[raster.write]] |
| Loop while something changes | `control.repeat` with `until` set to the `done` output of [[objects.merge]], [[objects.grow]] or [[objects.remove_small]] |

## A worked example

The template does with a class hierarchy what an eCognition rule set would with *assign class* processes: water by
its NDWI, vegetation by its NDVI (forest and grassland apart by the short-wave infrared), the rest by its NDVI
(cropland and urban). Open **Templates > Objects (OBIA) > Classify an image by rules**. Its graph reads:

```tfgraph
#! caption: Segment, measure what the rules read, classify by the class hierarchy, clean up and take the polygons.
#! fragment: true
#! highlight: rules, small, same
[[nodes]]
id = "segments"
op = "segment.multiresolution"
inputs = { image = "image.image" }

[[nodes]]
id = "features"
op = "objects.features"
inputs = { objects = "segments.objects", image = "image.image" }
params = { features = ["mean", "ndvi", "ndwi"] }

[[nodes]]
id = "rules"
op = "objects.rules"
inputs = { objects = "features.objects" }
params = { classes = [
  { name = "wet", rule = "above(ndwi, 0, 0.2)" },
  { name = "water", parent = "wet", colour = "#2b6cb0" },
  { name = "vegetation", rule = "above(ndvi, 0.45, 0.55)" },
  { name = "forest", rule = "below(mean_swir1, 0.18, 0.22)", parent = "vegetation", colour = "#2f7d32" },
  { name = "grassland", rule = "above(mean_swir1, 0.18, 0.22)", parent = "vegetation", colour = "#9bc53d" },
] }

[[nodes]]
id = "small"
op = "objects.remove_small"
inputs = { objects = "rules.objects" }
params = { min_cells = 20 }

[[nodes]]
id = "same"
op = "objects.merge"
inputs = { objects = "small.objects" }
params = { by = "class" }

[[nodes]]
id = "polygons"
op = "objects.to_vector"
inputs = { objects = "same.objects" }
```

(The template's hierarchy has also *other*, with *cropland* and *urban* below it; the nodes `small`, `same` and
`polygons` here are the cleaning an eCognition rule set would do next, shown on a shortened hierarchy.)

<div class="steps" markdown>

1. **Segment.** [[segment.multiresolution]] takes `scale`, `shape` and `compactness` as eCognition does; `shape = 0.1`
   and `compactness = 0.5` are the usual start. Left without `scale`, it takes the first one [[segment.scale]]
   suggests for the image.
2. **Measure.** [[objects.features]] with `features = ["mean", "ndvi", "ndwi"]`: only what the rules read. NDVI and
   NDWI are found from the bands named `red`, `nir` and `green`.
3. **Classify.** [[objects.rules]]. Each class has a `rule`, a membership from 0 to 1. `above(ndwi, 0, 0.2)` is 0 up
   to an NDWI of 0 and 1 from 0.2, a ramp between: the fuzzy version of a threshold. A class with a `parent`
   inherits it: its membership is the smaller of its own rule and its parent's, so *forest* needs both a high NDVI
   and a low short-wave infrared. Only the **leaves** (the classes with no daughter) are what an object becomes: the
   leaf of the largest membership, or no class under `min`. The parents only narrow the daughters' choice, as in
   eCognition's inheritance.
4. **Read the thresholds from the objects.** In the Viewer select `features` and look at the attribute table
   (its columns are the attributes, a row an object): the thresholds of the rules come from there, as they do from
   the feature view in eCognition. Select `rules`
   and click an object: the inspector tells the membership of each class and **what decided**, the rules and their
   values for this object. The margin (the largest membership minus the next) is low where the rules are unsure.
5. **Refine.** [[objects.remove_small]] merges the objects under `min_cells` into a neighbour, then
   [[objects.merge]] with `by = "class"` joins neighbours of one class into one object.
6. **Export.** [[objects.to_vector]] and [[vector.write]], or [[objects.to_map]] for a raster.

</div>

### Rules that read the neighbours

A rule may read the neighbours' classes: `border_to("water")` is the share of the object's perimeter shared with
water, `nb.count(cls == "water")` the number of water neighbours. Those read the classes the objects had **before**
the node, so chain two [[objects.rules]]: the first classifies, the second corrects, as two *assign class* processes
in sequence would.

### By samples

Rules are not the only path to the same kind of result. Samples: the Viewer's **Samples** mode, then
[[objects.nearest]] (eCognition's Standard Nearest Neighbour: the distance of an object to the samples over the
attributes you choose, in standard deviations, turned into a membership by `slope`) or a random forest
([[objects.classify]]). Both start from [Your first map by objects](/tutorials/object-based-image-analysis/first-map-by-objects/).

## What is not there

- **No "copy level" and no segmentation inside the objects of a class.** Levels are made by merging upwards
  ([[segment.multiresolution]] with `from`, [[objects.merge]] with `level`): the objects of a level are never made
  by printing one segmentation into another. That keeps every object inside exactly one object of the level above.
  To work inside a class, give it a branch of the class tree, which reads the region its mother class won
  ([Coming from InterIMAGE](/tutorials/object-based-image-analysis/from-interimage/)).

## Going further

- [Your first map by objects in 10 minutes](/tutorials/object-based-image-analysis/first-map-by-objects/): the
  samples path from start to end.
- [Coming from InterIMAGE](/tutorials/object-based-image-analysis/from-interimage/): a class tree with a method per
  class.

## References

- Baatz, M., & Schäpe, A. (2000). Multiresolution segmentation: an optimization approach for high quality multi-scale
  image segmentation. In *Angewandte Geographische Informationsverarbeitung XII* (pp. 12-23). Wichmann.
- Drăguţ, L., Tiede, D., & Levick, S. R. (2010). ESP: a tool to estimate scale parameter for multiresolution image
  segmentation of remotely sensed data. *International Journal of Geographical Information Science*, 24(6), 859-871.
  https://doi.org/10.1080/13658810903174803
- Olofsson, P., Foody, G. M., Herold, M., Stehman, S. V., Woodcock, C. E., & Wulder, M. A. (2014). Good practices for
  estimating area and assessing accuracy of land change. *Remote Sensing of Environment*, 148, 42-57.
  https://doi.org/10.1016/j.rse.2014.02.015
