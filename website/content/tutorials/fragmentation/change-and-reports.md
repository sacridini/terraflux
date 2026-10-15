---
title: Change of fragmentation and GuidosToolbox's reports
short: Change and reports
description: "How the fragmentation of a forest changed between two dates, as GuidosToolbox measures it: FAD at five scales at both dates and the forest moved between classes (change.fad), the change of FOS cell by cell (change.fos), the shift of the landscape mosaic (change.mosaic), and the reports to publish."
order: 30
level: advanced
time: 50 min
template: Forest gain and loss
ops: [change.fad, change.fos, change.mosaic, fragmentation.fos, fragmentation.landscape_mosaic, map.calc]
series: Measuring fragmentation
part: 3
---

Forest statistics usually report area: so many hectares lost, so many gained. But the same loss of area can leave a
forest in large blocks or riddle it with holes, and the second does more harm to its interior species. This last part
of the series measures how the **fragmentation** of a forest changed between two dates, with GuidosToolbox's three
change analyses: of the forest area density at five scales ([[change.fad]]), of the connectivity at one scale
([[change.fos]]), and of the landscape mosaic around it ([[change.mosaic]]). It ends with the reports GuidosToolbox
writes, which are the tables a monitoring report or a paper needs. At the end you will be able to say not only how
much forest was lost, but whether the forest that remains is more fragmented, where, and at which scales.

## The idea

### Change of fragmentation, not of area

Compare the forest of two dates cell by cell and you get gain and loss
([habitat patterns, part 4](/tutorials/habitat-patterns/networks-and-change/)). Compare their fragmentation and you
ask a different question: **for the forest present at both dates, did its surroundings become less forested?** A
forest cell that was not cleared can still be worse off: its neighbours were, so it went from interior to dominant, or
from dominant to transitional.

GuidosToolbox answers it in three steps:

1. compute FAD (or FOS) at both dates, with the same windows;
2. keep the **common forest**, the cells that are forest at both dates;
3. count how its cells moved between the classes (rare to intact): a move to a less connected class is an
   *increase* of fragmentation, to a more connected one a *decrease*; and give the change of the average density
   (FAD_AV) and of the average connectivity over the whole area (AVCON).

!!! science "Key idea: two numbers of change"
    For each window, \(\Delta\text{FAD}_\text{AV} = \text{FAD}_\text{AV}(B) - \text{FAD}_\text{AV}(A)\) is the change of
    the mean density around forest cells: the fragmentation of the forest that remains. \(\Delta\text{AVCON}\) is
    the change of the mean density over the whole reporting area; since AVCON is \(\text{FAD}_\text{AV}\) times the
    share of forest, it falls with the loss of forest *and* with its fragmentation. A landscape that lost forest
    from its edges only has a small \(\Delta\text{FAD}_\text{AV}\) and a large \(\Delta\text{AVCON}\); one whose
    forest was pierced everywhere, large changes of both.

This is how the US Forest Service has followed its forests: Wickham et al. (2008) and Riitters and Wickham (2012)
found that forest interior declined proportionally more than forest area in the conterminous United States, and
Riitters et al. (2016) found the same for the world's forests between 2000 and 2012 with global tree cover maps.

### The shift of the mosaic

The landscape mosaic (part 2) changes too: a forest can stay intact while the farmland around it becomes suburbs.
[[change.mosaic]] computes the mosaic's heatmap at both dates and their difference on the triangle of compositions:
where the landscape moved, from which corner towards which (Riitters et al., 2009).

## When to use it

- **National and regional reporting** of forest fragmentation over time (the FAO's and the US Forest Service's
  indicators are of this kind; Vogt et al., 2019).
- **Evaluating a policy or a disturbance**: did the forest left after logging, fires or a new road network become
  more fragmented than its area loss suggests?
- **Comparing two scenarios** of a land change model (date B is then a simulated map).
- **Not** for the area of change itself: use [[change.simple]] or [[change.morphological]]
  ([habitat patterns, part 4](/tutorials/habitat-patterns/networks-and-change/)). And for maps of many classes,
  [change detection](/tutorials/change-detection/) gives transition matrices and intensity analysis.

## What you need

Two maps of the same area and grid, made the same way, coded 0 missing, 1 background, 2 forest (or with the forest
classes given in `foreground`). Maps from different sensors or classifiers make change that is not there; check them
with morphological change first ([habitat patterns, part 4](/tutorials/habitat-patterns/networks-and-change/)).

We use GuidosToolbox's sample forest as date A, [`input.tif`](/data/tutorials/habitat-patterns/mspa/input.tif), and as
date B the same forest cleared by a rule (the model below), the second date of habitat patterns' part 4. For the
mosaic, [`clc.tif`](/data/tutorials/habitat-patterns/mspa/clc.tif) and a second date where Hannover and its villages
grow into the farmland around them. In the application, **Templates > Patterns (GuidosToolbox) > Forest gain and
loss** has simple and morphological change and [[change.fad]] on two forests drawn by chance.

## Build the model

```tfgraph
#! caption: Two dates of a forest; the change of FAD at five scales, and of FOS by FAC at 27 cells.
#! numbered: true
#! highlight: fad_change, fos_change
format = "terraflux/1"

[[nodes]]
id = "before"
op = "raster.read"
params = { path = "input.tif" }
note = "Date A."

[[nodes]]
id = "pressure"
op = "nlm.gaussian_field"
params = { rows = 1000, columns = 1000, range = 60, seed = 28 }

[[nodes]]
id = "after"
op = "map.calc"
inputs = { f = "before.map", p = "pressure.map" }
params = { expression = "if f == 2 and p > 0.8 then 1 else if f == 2 and p > 0.55 and nbsum(f == 1, 3) >= 1 then 1 else if f == 1 and p < 0.15 and nbsum(f == 2, 1) >= 4 then 2 else f" }
note = "Date B (with your maps: a raster.read)."

[[nodes]]
id = "fad_change"
op = "change.fad"
inputs = { before = "before.map", after = "after.map" }
note = "FAD-MS at both dates; the forest moved between classes."

[[nodes]]
id = "fos_a"
op = "fragmentation.fos"
inputs = { map = "before.map" }
params = { window = 27, method = "FAC" }

[[nodes]]
id = "fos_b"
op = "fragmentation.fos"
inputs = { map = "after.map" }
params = { window = 27, method = "FAC" }

[[nodes]]
id = "fos_change"
op = "change.fos"
inputs = { before = "fos_a.map", after = "fos_b.map" }
note = "Cell by cell: the change of connectivity."
```

How it connects:

- **Nodes 1 to 3** are the two dates. Node 3 clears the forest where a smooth random "pressure" is high (above
  0.8) and eats its edges where it is moderate (above 0.55): there a forest cell with at least one background cell
  within 3 cells (`nbsum(f == 1, 3) >= 1`, the background cells of its 7 × 7 window counted) is cleared. A few
  background cells with 4 forest cells around them, where the pressure is lowest, grow back. Whole blocks lost and
  edges retreating: 17% of the forest. With your maps, nodes 2 and 3 become a second [[raster.read]].
- **Node 4**, [[change.fad]], takes the two *forest maps* (`before` is date A, `after` date B). It computes FAD-MS of
  both itself, at the `windows` (default 7, 13, 27, 81 and 243 cells), with the `foreground` classes if given.
- **Nodes 5 to 7**: [[change.fos]] takes two *FOS maps* (or FAD maps), not forest maps: compute FOS at both dates
  with the same settings (nodes 5 and 6), then connect their `map` outputs to `before` and `after`.

## Read the results

### FAD at five scales

```tfcompare
caption: "FAD-MS's average classes in the south of the map at date A (left of the handle) and date B (right): interior green, dominant light green, transitional yellow, patchy orange, rare red, background grey. The forest left at B has lost its interior around the clearings; large areas went from dominant to transitional."
before: { src: /img/tutorials/fragmentation/change-and-reports/ms-a-zoom.png, label: Date A }
after: { src: /img/tutorials/fragmentation/change-and-reports/ms-b-zoom.png, label: Date B }
```

The `stats` table of [[change.fad]], a row per window (0 the FAD-MS average):

| Window (cells) | Forest at A | Forest at B | Common forest | In another class | FAD_AV at A | at B | Δ FAD_AV | Δ AVCON |
|---|---|---|---|---|---|---|---|---|
| 7 | 428,490 | 354,172 | 354,098 | 77,581 | 85.2 | 83.6 | −1.6 | −6.9 |
| 13 | | | | 101,383 | 80.1 | 78.0 | −2.2 | −6.7 |
| 27 | | | | 103,912 | 73.5 | 70.1 | −3.4 | −6.7 |
| 81 | | | | 110,405 | 61.6 | 55.9 | −5.7 | −6.6 |
| 243 | | | | 141,763 | 51.4 | 43.4 | −8.0 | −6.6 |
| average | | | | 86,522 | 70.4 | 66.2 | −4.2 | −6.7 |

How to read it:

- The forest lost 17.3% of its area (428,490 to 354,172 cells).
- Of the 354,098 cells of common forest, between 22% (7 cells) and 40% (243 cells) **moved to another class**, and of
  those, more than 99.7% to a *less* connected class (the `increased_percent` column, 99.7 to 100%). Almost none
  gained: this change was a pure fragmentation.
- **Δ FAD_AV grows with the window**: −1.6 points at 7 cells, −8.0 at 243. A small window sees the clearings only from
  the cells near the new edges; a large one sees them from far inside the forest. The clearing pressure varies over
  distances of about 60 cells, so the forest left was hit hardest at the scales of whole landscapes, not of stands.
- **Δ AVCON** is about −6.7 points at every scale: it adds the loss of forest to the fragmentation of what remains.

The `matrix` output gives, for each window, the cells of each class at A in each class at B. At 27 cells, as a
share of each class at A:

![The change matrix at 27 cells](/img/tutorials/fragmentation/change-and-reports/fad-matrix-27.png "Each row is a class at date A, each column its class at date B, as % of the row. The first column is the forest lost (background at B): 38% of the rare forest, 31 to 32% of the patchy and transitional, 19% of the dominant, 7% of the interior and under 1% of the intact, since the edges retreated. Below the diagonal, the forest that remained but became more fragmented: 35% of the interior became dominant, 38% of the intact became interior. Above the diagonal (fragmentation decreased): almost nothing.")

The most telling row is the last: of the forest that was **intact** at A (100% forest within 27 cells), under 1%
was cleared, yet only 60% is still intact at B; the rest, though still standing, now has clearings within 340 m.
For a species of the forest interior, the loss is not 17% of the forest but 40% of its best habitat.

### FOS, cell by cell

[[change.fos]] gives a map: on the common forest, the change of connectivity (in GuidosToolbox's code, the
connectivity at A minus that at B, plus 100: 100 is no change, above 100 a loss), and the forest gained (250) and
lost (251), stable background (252), missing (254).

![The change of FOS](/img/tutorials/fragmentation/change-and-reports/fos-change-zoom.png "The change of FOS by FAC at 27 cells in the south of the map, drawn as B − A: red where the remaining forest lost connectivity, the darker the more; black the forest lost. The forest beside the clearings lost 20 to 60 points of connectivity; far from them, none.")

Its `stats` table: 74 cells gained, 74,281 lost, 353,426 of common forest, of which 115,983 (33%) changed class,
99.9% of them to a more fragmented one. Its `classes` table is the matrix of the common forest's classes at A and B, as
for FAD. The map shows *where* the forest that remains was degraded: the halo around every clearing, up to half a
window (13 cells) from it.

### The landscape mosaic

```tfgraph
#! caption: CORINE, a second date where the city grows into the farmland around it, and the change of the landscape mosaic.
#! numbered: true
#! highlight: change
format = "terraflux/1"

[[nodes]]
id = "corine"
op = "raster.read"
params = { path = "clc.tif" }

[[nodes]]
id = "pressure"
op = "nlm.gaussian_field"
params = { rows = 2000, columns = 2000, range = 80, seed = 5 }

[[nodes]]
id = "corine_b"
op = "map.calc"
inputs = { c = "corine.map", p = "pressure.map" }
params = { expression = "if c >= 12 and c <= 22 and p > 0.5 and nbsum(c <= 11, 4) >= 8 then 2 else if c >= 23 and c <= 25 and p > 0.75 and nbsum(c <= 11, 4) >= 20 then 2 else c" }
note = "Farmland (and a little forest) beside towns becomes discontinuous urban fabric (2)."

[[nodes]]
id = "change"
op = "change.mosaic"
inputs = { before = "corine.map", after = "corine_b.map" }
params = { window = 27, agriculture = [12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22], natural = [23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39], developed = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11] }
```

Node 3 makes the second date: farmland cells (CORINE's 12 to 22) with at least 8 developed cells (CORINE's 1 to 11)
in the 9 × 9 cells around them (`nbsum(c <= 11, 4) >= 8`), where a smooth random field is above 0.5, become urban
fabric; so do forest cells (23 to 25) with 20 developed cells around them where the field is above 0.75. Developed
land goes from 14.8% to 17.2% of the map, all of it beside existing towns. [[change.mosaic]] takes the two land covers and the same groups as
[[fragmentation.landscape_mosaic]].

```tfcompare
caption: "The landscape mosaic within 675 m around the city of Hannover (1,000 × 1,000 cells of 25 m) at date A (left of the handle) and B (right), in GuidosToolbox's colours (pure agriculture dark blue, pure developed dark red, pure natural green; the mixes in between). The rings of agriculture-developed classes around the city and the villages widen."
before: { src: /img/tutorials/fragmentation/change-and-reports/lm-a-zoom.png, label: Date A }
after: { src: /img/tutorials/fragmentation/change-and-reports/lm-b-zoom.png, label: Date B }
```

![The change of the heatmap](/img/tutorials/fragmentation/change-and-reports/mosaic-delta.png "The change of the mosaic's heatmap, B − A, on GuidosToolbox's triangle: blue where the share of windows fell, red where it rose. The pure-agriculture corner lost 1.8 points; the windows moved along the bottom edge, towards the developed corner and the mixes of developed land and agriculture.")

With 2.3 more points of developed land, the share of the landscape in pure agriculture fell from 28.7% to 26.8%,
and the classes of developed land with agriculture grew: *Developed-Agriculture* (developed above 60%, agriculture
10% or more) from 3.8% to 4.9%, *Mosaic Agriculture-Developed* from 3.8% to 4.4%, *Developed (>60%)* from 1.6% to
2.4%. Since the towns grew at their edges, the windows that changed most already held some developed land: the
mosaic says not only how much land was built on, but how far the developed land now presses on the agricultural
and natural land around it, which the areas of the land covers do not show.
The `heatmap` table has every class of 103 with its share at both dates and the change; `heatmap_csv` is
GuidosToolbox's delta_heatmap.csv.

## GuidosToolbox's reports

Every analysis has a `report` (or `heatmap_csv`) output: the text file GuidosToolbox itself writes, line for line, so
that results made in Terraflux can be compared with GuidosToolbox's and published in the same form. In the
application, select the node and the report shows under the canvas; from the command line, make it an output and it
is written to the output folder. The report of [[change.fad]] (GuidosToolbox's FAD_change_mscale.txt) begins with the
matrix of the FAD-MS average in cells, then the scales:

```text
Foreground cover at time A [pixels]: 428490
Foreground cover at time B [pixels]: 354172
Net foreground cover change (A->B) [pixels]: -74318
Observation scale:             7          13          27          81         243      mscale
Diff: FAD_AV(A->B):      -1.6104     -2.1567     -3.4021     -5.7412     -7.9601     -4.1754
Diff: AVCON(A->B):       -6.9044     -6.7221     -6.6723     -6.6125     -6.6411     -6.7110

Change matrix constrained to the 86522 pixels in different fragmentation classes [%]:
Fragmentation decrease (=connectivity increase) - above the matrix diagonal [%]: 0.0866832
Fragmentation increase (=connectivity decrease) - below the matrix diagonal [%]: 99.9133
```

(The full report: [fad-change-report.txt](/data/tutorials/fragmentation/change-and-reports/fad-change-report.txt).)

The reports of the series:

| Operator | Output | GuidosToolbox's file |
|---|---|---|
| [[fragmentation.fos]] | `report` | fos.txt: classes, foreground, patches, ECA, coherence, AVCON |
| [[change.fad]] | `report` | FAD_change_mscale.txt: matrix, ΔFAD_AV, ΔAVCON, increase and decrease |
| [[fragmentation.landscape_mosaic]] | `heatmap_csv` | heatmap.csv |
| [[change.mosaic]] | `heatmap_csv` | delta_heatmap.csv |
| [[networks.gtn_components]], [[networks.gtn_importance]] | `report` | the network's logs |

!!! tip "Report it"
    For a change of fragmentation, report: the forest at both dates and the net change; for each window, FAD_AV at
    both dates and its change, the share of the common forest that changed class and the share of those whose
    fragmentation increased; ΔAVCON; a map of the FOS change of a typical area; and the matrix at the scale that
    matters most for the species. State the windows in metres.

## Settings that change the answer

- **The windows**: the change depends on the scale, as the table shows. Report several.
- **The comparability of the maps**: spurious change at the edges (a shift of a cell) moves forest cells between
  classes too. Check the share of spurious change with [[change.morphological]] first.
- **The method of FOS** (FAD, FED, FAC), the same at both dates.
- **The groups of the mosaic**, the same at both dates.

## Common pitfalls

- **Swapping the dates.** `before` is A. Swapped, every increase of fragmentation becomes a decrease.
- **FOS maps of different settings.** [[change.fos]] compares two FOS maps value by value; maps made with different
  windows or methods compare nothing.
- **Reading the FOS change code as B − A.** The map holds A − B + 100: values above 100 are a *loss* of
  connectivity.
- **Area change reported as fragmentation change**, or the reverse. Report both, and say which is which.
- **Different grids.** The two maps must have the same rows, columns and cells.

## Going further

- [Habitat patterns, part 4](/tutorials/habitat-patterns/networks-and-change/): simple and morphological change, and
  the network of MSPA at two dates.
- [Change detection](/tutorials/change-detection/): transitions and intensity analysis for maps of many classes.
- [Land change simulation](/tutorials/land-change/): making date B by simulation, to compare scenarios.
- The operator reference: [[change.fad]], [[change.fos]], [[change.mosaic]].

## References

- Riitters, K. H., & Wickham, J. D. (2012). Decline of forest interior conditions in the conterminous United States. *Scientific Reports*, 2, 653. https://doi.org/10.1038/srep00653
- Riitters, K., Wickham, J., Costanza, J. K., & Vogt, P. (2016). A global evaluation of forest interior area dynamics using tree cover data from 2000 to 2012. *Landscape Ecology*, 31(1), 137–148. https://doi.org/10.1007/s10980-015-0270-9
- Riitters, K. H., Wickham, J. D., & Wade, T. G. (2009). An indicator of forest dynamics using a shifting landscape mosaic. *Ecological Indicators*, 9(1), 107–117. https://doi.org/10.1016/j.ecolind.2008.02.003
- Vogt, P., & Riitters, K. (2017). GuidosToolbox: universal digital image object analysis. *European Journal of Remote Sensing*, 50(1), 352–361. https://doi.org/10.1080/22797254.2017.1330650
- Vogt, P., Riitters, K. H., Caudullo, G., & Eckhardt, B. (2019). *FAO – State of the World's Forests: Forest fragmentation* (EUR 29972 EN). Publications Office of the European Union. https://doi.org/10.2760/145325
- Wickham, J. D., Riitters, K. H., Wade, T. G., & Homer, C. (2008). Temporal change in fragmentation of continental US forests. *Landscape Ecology*. https://doi.org/10.1007/s10980-008-9258-z
