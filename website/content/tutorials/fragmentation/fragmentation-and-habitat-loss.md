---
title: Fragmentation and habitat loss
short: Fragmentation and loss
description: "What habitat fragmentation is and how it differs from habitat loss, and how GuidosToolbox measures it around every cell: the forest area density (FAD) in a moving window, its six classes, and the same at five observation scales (FAD-MS), on a real forest map."
order: 10
level: beginner
time: 45 min
template: Fragmentation of a forest
ops: [fragmentation.fad, fragmentation.fad_multiscale, raster.read, view.plot]
series: Measuring fragmentation
part: 1
---

"Fragmentation" is one of the most used words in conservation and one of the most ambiguous. It can mean the loss of
habitat, the breaking of what remains into pieces, or both. This series measures it the way GuidosToolbox does, around
every cell of a map rather than patch by patch, as the US Forest Service and the European Commission's Joint Research
Centre have done for national and global forest assessments. This first part separates fragmentation from habitat
loss, then measures the simplest and most used of GuidosToolbox's indices, the **forest area density** (FAD): the
share of forest in a window around every forest cell, at one scale and at five. At the end you will have a map of how
fragmented a forest is at the scale of a few hundred metres and at the scale of several kilometres, and the numbers to
report.

## The idea

### Two processes behind one word

As a process, the fragmentation of a habitat usually does four things at once: there is less habitat, more patches,
smaller patches and more isolated patches (Fahrig, 2003). The first is **habitat loss**; the other three describe the
**configuration** of what remains, and are called **fragmentation *per se***.

![Habitat amount and fragmentation](/img/tutorials/fragmentation/fragmentation-and-habitat-loss/amount-fragmentation.png "Four landscapes of 150 × 150 cells drawn by Terraflux's neutral landscape models: 60% or 20% habitat (rows), aggregated or fragmented (columns). Down a column the habitat is lost; along a row it is fragmented at a constant amount. Real landscapes mostly move along the diagonal, from the top left to the bottom right.")

The distinction matters because the two have different effects. Reviewing more than a hundred studies, Fahrig (2003)
found that habitat loss has large, consistently negative effects on biodiversity, while fragmentation *per se* has
weaker effects, as often positive as negative. A later review went further: most significant effects of
fragmentation *per se* are positive (Fahrig, 2017). Others disagree, stressing that large intact areas are
irreplaceable for many specialist species and that the positive effects often come from generalists and edge species
(Fletcher et al., 2018). And at the scale of the planet, Haddad et al. (2015) found that 70% of the world's forest is
within 1 km of an edge, and that experiments of fragmentation consistently report losses of species and of
ecosystem functions over time.

You do not need to take sides to draw the practical lesson: **always measure the amount of habitat alongside its
configuration**, and interpret a measure of configuration knowing the amount. In the figure above, the number of
patches is 73 and 145 at 60% habitat, 95 and 767 at 20%: it changes with both.

### Fragmentation around every cell

Patch metrics ([landscape metrics](/tutorials/landscape-metrics/patch-and-class-metrics/)) give a number per patch,
class or map. Riitters et al. (2000) proposed another view for continental assessments: move a window across the
map, and describe the habitat **in the window around every cell**. The result is a map, which shows *where* the
forest is fragmented, not just how much, and which does not depend on how patches are delimited. The simplest such
measure is the share of habitat in the window.

!!! science "Key idea: forest area density"
    For a forest cell, its **forest area density** is the share of forest among the cells with data in the square
    window of \(w \times w\) cells centred on it:

    \[ \text{FAD} = 100 \times \frac{\text{forest cells in the window}}{\text{cells with data in the window}} \]

    Missing cells (outside the map, or with no data) are left out of the denominator. FAD is given only for forest
    cells; background cells get GuidosToolbox's code 101, missing ones 102. GuidosToolbox reports it in six classes
    (Vogt & Riitters, 2017): **rare** (below 10%), **patchy** (10 to 40%), **transitional** (40 to 60%),
    **dominant** (60 to 90%), **interior** (90 to 100%) and **intact** (100%: no break in the window).

![FAD of a toy forest](/img/tutorials/fragmentation/fragmentation-and-habitat-loss/toy-window.png "A forest of 15 × 15 cells run through fragmentation.fad with a window of 7 cells. Left: the window of the cell marked in red holds 34 forest cells of 49, so its FAD is 69% (dominant). Right: the FAD of every forest cell. Cells deep in the large block score in the 80s and 90s; cells of the narrow strip, surrounded by fields, in the 20s and 30s.")

The window's side is the **observation scale**. With cells of 25 m, a window of 27 cells covers 675 × 675 m (46
ha); one of 243 cells, 6 km. A forest cell can be intact within 175 m and patchy within 6 km: that is not a
contradiction but two answers to two questions, and GuidosToolbox's **multiscale** analysis (FAD-MS) gives five of
them at once, with windows of 7, 13, 27, 81 and 243 cells, and their average (Vogt & Riitters, 2017).

## When to use it

- **Mapping where the habitat is fragmented**, at the scale that matters to a species or a process: where the forest
  is intact, where it is dissolving into fields.
- **Reporting fragmentation** of a region or a country: the share of the forest in each class. The FAO's *State of
  the World's Forests 2020* reported the world's forest by FAD's classes at several scales (Vogt et al., 2019), as the
  US Forest Service does for the forests of the United States (Riitters et al., 2002; Riitters & Wickham, 2012).
- **Following change** between two dates ([part 3](/tutorials/fragmentation/change-and-reports/)).
- **Not** for the structural role of each cell (core, edge, bridge): that is [MSPA](/tutorials/habitat-patterns/mspa/).
  Not for connectivity for a species with a dispersal distance: that is the
  [habitat networks](/tutorials/habitat-networks/). FAD measures the *amount* of habitat around each cell; its
  arrangement in the window needs the other indices of [part 2](/tutorials/fragmentation/fixed-observation-scale/).

## What you need

A map of the habitat, coded as GuidosToolbox expects: **0 missing, 1 background, 2 foreground** (the habitat), and
optionally 3 and 4 for *special backgrounds* (3 a background that fragments, such as roads; 4 one that does not, such
as water that a forest species simply does not use). Or any map of classes, with the classes of the habitat given
in `foreground`: no reclassification is needed.

We use GuidosToolbox's sample forest map, [`input.tif`](/data/tutorials/habitat-patterns/mspa/input.tif) (1,000 ×
1,000 cells), the map of the [habitat patterns](/tutorials/habitat-patterns/mspa/) tutorials. In the application,
**Templates > Patterns (GuidosToolbox) > Fragmentation of a forest** starts a model with FAD-MS and FOS on a forest
drawn by chance; replace its first node by a [[raster.read]] of your map.

![The forest](/img/tutorials/fragmentation/fragmentation-and-habitat-loss/forest-zoom.png "The south of the sample forest map (300 × 300 cells): forest in green, the rest in beige. Large forests to the south and west, a settled valley with small woods and tree lines in the middle.")

## Build the model

```tfgraph
#! caption: The forest map, its FAD in a window of 27 cells, and its FAD at five scales.
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "forest"
op = "raster.read"
params = { path = "input.tif" }
note = "0 missing, 1 background, 2 forest."

[[nodes]]
id = "fad"
op = "fragmentation.fad"
inputs = { map = "forest.raster" }
params = { window = 27 }
note = "The forest's share around every forest cell; its six classes."

[[nodes]]
id = "multiscale"
op = "fragmentation.fad_multiscale"
inputs = { map = "forest.raster" }
note = "Windows of 7, 13, 27, 81 and 243 cells, and their average."

[[nodes]]
id = "plot"
op = "view.plot"
inputs = { value = "fad.classes" }
params = { title = "FAD, 27 cells" }
```

<div class="steps" markdown>

1. **Read the map** (node 1): [[raster.read]], `path` the file. Its output `map` is the forest map.
2. **FAD at one scale** (node 2). In the library's **Fragmentation**, click [[fragmentation.fad]], and drag from node 1's
   `map` to its `map`. Set `window` to the side of the window in cells (odd, so that it has a centre; default 7).
   With a map of many classes, set `foreground` to the habitat's class (or list of classes).
3. **FAD at five scales** (node 3): [[fragmentation.fad_multiscale]], connected the same way. `windows` changes the
   scales (default 7, 13, 27, 81, 243).
4. **Look** (node 4): [[view.plot]] of `fad.classes` shows the six classes in GuidosToolbox's colours.

</div>

The two operators read the same map and do not depend on each other. Each gives:

- `map`: the density (0–100%) on the forest cells, 101 on the background, 102 on missing cells;
- `classes`: the classes 1 (rare) to 6 (intact), with the same codes elsewhere;
- `stats`: per window, the share of the forest in each class and the average density.

FAD-MS also gives `scales`, a stack of the density at each window, so you can look at any scale on its own.

## Read the results

### One scale

```tfcompare
caption: "The south of the map. Left of the handle, the density of forest within 27 cells of every forest cell (white: not forest); right, its six classes. The interiors of the large forests are dark green (interior and intact); their edges and the woods of the valley dominant and transitional; the tree lines across the fields patchy and rare."
before: { src: /img/tutorials/fragmentation/fragmentation-and-habitat-loss/fad27-density-zoom.png, label: Density }
after: { src: /img/tutorials/fragmentation/fragmentation-and-habitat-loss/fad27-classes-zoom.png, label: Classes }
```

![FAD classes of the whole map](/img/tutorials/fragmentation/fragmentation-and-habitat-loss/fad27-classes.png "FAD's six classes over the whole map, window of 27 cells.")

The `stats` table of node 2:

| Class | Intact | Interior | Dominant | Transitional | Patchy | Rare | Average FAD |
|---|---|---|---|---|---|---|---|
| % of the forest | 7.3 | 32.7 | 32.2 | 14.0 | 12.2 | 1.6 | 73.5% |

Read it as: within 27 cells, 40% of this forest is interior or intact (90% forest or more around it), a third
dominant, and 28% transitional to rare, in surroundings that are mostly not forest. The average density, 73.5%,
summarises it in one number: a forest cell's surroundings are, on average, three-quarters forest.

### Five scales

Move the slider from the smallest window to the largest, and then to their average:

```tfslider
label: Window (cells)
caption: "FAD's classes over the whole map at the five windows of FAD-MS and their average. As the window grows, the intact class disappears (no window of 81 cells is all forest), interior shrinks to the cores of the largest forests, and transitional and patchy spread: at the scale of 6 km, this is a forest in a mixed landscape. Numbers: % of the forest in each class."
frames:
  - { value: 7, src: /img/tutorials/fragmentation/fragmentation-and-habitat-loss/ms-7.png, caption: "7 cells", stats: { Intact: "51.7%", Interior: "13.9%", Dominant: "18.9%", Transitional: "8.4%", Patchy: "6.6%", "Average FAD": "85.2%" } }
  - { value: 13, src: /img/tutorials/fragmentation/fragmentation-and-habitat-loss/ms-13.png, caption: "13 cells", stats: { Intact: "30.5%", Interior: "24.2%", Dominant: "24.8%", Transitional: "10.4%", Patchy: "9.1%", "Average FAD": "80.1%" } }
  - { value: 27, src: /img/tutorials/fragmentation/fragmentation-and-habitat-loss/ms-27.png, caption: "27 cells", stats: { Intact: "7.3%", Interior: "32.7%", Dominant: "32.2%", Transitional: "14.0%", Patchy: "12.2%", "Average FAD": "73.5%" } }
  - { value: 81, src: /img/tutorials/fragmentation/fragmentation-and-habitat-loss/ms-81.png, caption: "81 cells", stats: { Intact: "0%", Interior: "17.1%", Dominant: "36.6%", Transitional: "26.4%", Patchy: "18.9%", "Average FAD": "61.6%" } }
  - { value: 243, src: /img/tutorials/fragmentation/fragmentation-and-habitat-loss/ms-243.png, caption: "243 cells", stats: { Intact: "0%", Interior: "5.6%", Dominant: "22.1%", Transitional: "45.2%", Patchy: "27.0%", "Average FAD": "51.4%" } }
  - { value: "average", src: /img/tutorials/fragmentation/fragmentation-and-habitat-loss/ms-avg.png, caption: "The average of the five (five classes)", stats: { Interior: "16.9%", Dominant: "56.6%", Transitional: "16.8%", Patchy: "9.6%", "Average FAD": "70.4%" } }
chart:
  type: line
  data: /data/tutorials/fragmentation/fragmentation-and-habitat-loss/ms-shares.csv
  x: window
  y: [intact, interior, dominant, transitional, patchy, rare]
  colors: { intact: "#007800", interior: "#00af00", dominant: "#8cc864", transitional: "#ffc800", patchy: "#fa8c5a", rare: "#d73228" }
  ylabel: "% of the forest"
```

The average density falls steadily with the window, from 85% at 7 cells to 51% at 243: this forest is solid at the
scale of a few hundred metres and only transitional at the scale of several kilometres. That is a result, not a
nuisance: **report the curve**, rather than choose one scale. The FAD-MS average combines the five scales into one
map of five classes (intact cannot be reached by an average, so interior includes it); a cell is "interior" there only
if its surroundings are forest at every scale.

FAD-MS's `stats` also gives **AVCON** at every scale, the average density over the whole area with data (not only
over the forest): 36.5% at 7 cells, 22.0% at 243. It falls as the forest is lost, even where the forest left is not
fragmented, so it combines amount and fragmentation in one indicator.

!!! tip "Report it"
    Report the map (source, date, cell size, what counted as forest), the window(s) in cells and in metres, the share
    of the forest in each class at each window, and the average density. For one scale, choose it from the ecology of
    the species or process (the distance of edge effects, a home range); when in doubt, report FAD-MS.

## Settings that change the answer

- **The window**: above. In metres, it depends on the cell size: 27 cells are 675 m at 25 m, 2.7 km at 100 m.
- **What is foreground**: the classes of the habitat. Forest only, or forest and woodland? A definition that leaves
  out tree lines and small woods makes the forest look less fragmented.
- **Special backgrounds** (3 and 4): GuidosToolbox's codes for backgrounds that fragment the forest (3, roads for
  example) and that do not (4, for example water that a forest species simply does not use). Coding them apart
  says what counts as a break in the forest.
- **The map's resolution**: a finer map sees more small openings and tree lines; Wickham and Riitters (2019) found
  forest fragmentation markedly different with data of 1 m than of 30 m.

## Common pitfalls

- **Reading FAD as fragmentation per se.** FAD is the *amount* of forest around a cell. A forest cell with 50% forest
  around it may be at the edge of a large block or in a scatter of small woods; the indices of
  [part 2](/tutorials/fragmentation/fixed-observation-scale/) tell them apart.
- **Comparing FAD at different windows or cell sizes.** Compare like with like.
- **Forgetting the map's border.** Near the border, windows are cut: the density is computed on the cells with data
  only, so it is not biased, but it rests on fewer cells.
- **A map coded 0/1.** GuidosToolbox reads 1 as background and 2 as foreground; a map with forest coded 1 would make
  the forest the background. Give `foreground`, or reclassify.
- **Degrees.** On a map in degrees (as this sample), cells are not square in metres and differ by latitude; for
  scales in metres, project first.

## Going further

- [Part 2](/tutorials/fragmentation/fixed-observation-scale/): the arrangement of the forest in the window
  (connectivity, contagion, entropy), % cover maps and the landscape mosaic.
- [Part 3](/tutorials/fragmentation/change-and-reports/): the change of fragmentation between two dates, and
  GuidosToolbox's reports.
- [Habitat patterns](/tutorials/habitat-patterns/): MSPA, the structural role of every forest cell.
- [Neutral landscapes](/tutorials/neutral-landscapes/): landscapes of controlled amount and fragmentation.
- The operator reference: [[fragmentation.fad]], [[fragmentation.fad_multiscale]].

## References

- Fahrig, L. (2003). Effects of habitat fragmentation on biodiversity. *Annual Review of Ecology, Evolution, and Systematics*, 34, 487–515. https://doi.org/10.1146/annurev.ecolsys.34.011802.132419
- Fahrig, L. (2017). Ecological responses to habitat fragmentation per se. *Annual Review of Ecology, Evolution, and Systematics*, 48, 1–23. https://doi.org/10.1146/annurev-ecolsys-110316-022612
- Fletcher, R. J., Jr., Didham, R. K., Banks-Leite, C., Barlow, J., Ewers, R. M., Rosindell, J., et al. (2018). Is habitat fragmentation good for biodiversity? *Biological Conservation*, 226, 9–15. https://doi.org/10.1016/j.biocon.2018.07.022
- Haddad, N. M., Brudvig, L. A., Clobert, J., Davies, K. F., Gonzalez, A., Holt, R. D., et al. (2015). Habitat fragmentation and its lasting impact on Earth's ecosystems. *Science Advances*, 1(2), e1500052. https://doi.org/10.1126/sciadv.1500052
- Riitters, K., Wickham, J., O'Neill, R., Jones, B., & Smith, E. (2000). Global-scale patterns of forest fragmentation. *Conservation Ecology*, 4(2), 3. https://doi.org/10.5751/ES-00209-040203
- Riitters, K. H., Wickham, J. D., O'Neill, R. V., Jones, K. B., Smith, E. R., Coulston, J. W., et al. (2002). Fragmentation of continental United States forests. *Ecosystems*, 5(8), 815–822. https://doi.org/10.1007/s10021-002-0209-2
- Riitters, K. H., & Wickham, J. D. (2012). Decline of forest interior conditions in the conterminous United States. *Scientific Reports*, 2, 653. https://doi.org/10.1038/srep00653
- Vogt, P., & Riitters, K. (2017). GuidosToolbox: universal digital image object analysis. *European Journal of Remote Sensing*, 50(1), 352–361. https://doi.org/10.1080/22797254.2017.1330650
- Vogt, P., Riitters, K. H., Caudullo, G., & Eckhardt, B. (2019). *FAO – State of the World's Forests: Forest fragmentation* (EUR 29972 EN). Publications Office of the European Union. https://doi.org/10.2760/145325
- Wickham, J., & Riitters, K. H. (2019). Influence of high-resolution data on the assessment of forest fragmentation. *Landscape Ecology*, 34(9), 2169–2182. https://doi.org/10.1007/s10980-019-00820-z
