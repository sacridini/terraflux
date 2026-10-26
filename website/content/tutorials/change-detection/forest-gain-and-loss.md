---
title: "Forest gain and loss: GuidosToolbox's change of a forest"
short: Forest gain and loss
description: Map where a forest was gained and lost between two dates, tell structural change from the noise along its edges with morphological change detection, and measure what the change did to the forest that remains, on Sibuyan and Plum Island.
order: 40
level: intermediate
time: 45 min
template: Forest gain and loss
ops: [change.simple, change.morphological, change.fad, change.fos, fragmentation.fos, raster.read]
series: Change detection
part: 4
---

The first three parts of this series compared maps of many classes: a transition matrix, its intensity, its
agreement with a reference. Conservation questions are often simpler and sharper: *where did the forest go, where
did it come back, and is what remains still forest interior?* In this last part you will look at one class, the
forest, with the change analyses of **GuidosToolbox**: the **simple change** (gain, loss and persistence of the
forest), the **morphological change**, which tells change that altered the forest's structure from the shifts of
its edges that two maps of the same forest always show, and the change of its **fragmentation** at several scales.
You will run them on Sibuyan's forest from 1997 to 2011 (the data of [part 1](/tutorials/change-detection/transitions/)),
on Plum Island's from 1985 to 1999, and on a map moved by one cell, to see what a misregistration does. At the end
you will be able to report how much forest was lost, how much of that loss you can trust, and how the forest left
behind was changed.

## The idea

### From many classes to one

A transition matrix treats every class alike. GuidosToolbox (Vogt & Riitters, 2017), the image analysis software
of the European Commission's Joint Research Centre, looks at one class at a time, the **foreground** (here the
forest), against everything else, the **background**. Its maps are coded 0 for missing cells, 1 for background and 2
for foreground; in Terraflux every operator of this page takes instead a `foreground` setting that names the
classes of your map that are forest (`foreground = [1]` for Sibuyan's class 1), and treats cells without a value
(the sea) as missing.

With two binary maps, every cell is one of four things:

| Code | Before | After | Meaning |
|---|---|---|---|
| 11 | background | background | stable background |
| 12 | background | forest | **gain** |
| 21 | forest | background | **loss** |
| 22 | forest | forest | stable forest |

(255 marks a cell missing at either date.) This is GuidosToolbox's **simple change**, [[change.simple]]. Its loss
is the forest row of the transition matrix of part 1 summed over every other class; its gain, the forest column.

### Not all change on a map is change on the ground

Two maps of the same forest, made from two images, never align perfectly: the images are registered to within a
fraction of a cell, the classifier hesitates on mixed cells, and the forest's edge is a fuzzy line. Every one of
these errors shows up as change **along the edges** of the forest, one or two cells wide. Townshend et al. (1992)
showed how much change a registration error of a fraction of a cell creates in satellite images, and Olofsson et
al. (2014) why map errors make change maps the least accurate of all land maps.

**Morphological change detection** (MCD; Seebach et al., 2013), [[change.morphological]], separates the change that
alters the structure of the forest from that edge noise:

!!! science "Key idea: morphological change detection"
    1. Find the **cores** of each date: the forest cells whose 5 × 5 window is all forest (two erosions of 3 × 3),
       and the same for the background.
    2. A **seed of loss** is a cell that was core forest at the first date and is core background at the second: a
       clearing too large to be an error of the edge. A seed of gain is the opposite.
    3. Every lost cell connected to a seed of loss becomes **structural loss** (a geodesic reconstruction: the seed
       grows inside the cells that changed), and the result is closed with a 3 × 3 window, which fills one-cell
       gaps inside the clearing. The same for gain.
    4. The cells that changed but are not connected to a seed are **spurious change** (code 176): change along the
       edges that a shift of a cell or two could have made.

    The map keeps the codes of simple change and adds 176. Its `stats` also give the **elasticity**: the rate at
    which the forest's interior (cells whose 8 neighbours are all forest) changed, divided by the rate at which the
    forest changed. Above 1, the interior shrank faster than the forest: the loss pierced it.

![A toy forest through simple and morphological change](/img/tutorials/change-detection/forest-gain-and-loss/toy.png "A toy forest of 24 × 30 cells, run through both operators. At the second date a block of 7 × 7 cells is cleared inside it, its eastern edge has moved back one cell, its western edge one cell out, and two single cells are cleared. Simple change counts 69 cells of loss and 18 of gain. Morphological change keeps the 49 cells of the clearing as loss, which holds cells that were core forest (inside the outline on the first panel) and are core background; the strips along the edges and the single cells, 38 cells, are spurious.")

A clearing is structural only if, somewhere inside it, a 5 × 5 block went from all forest to all background. That
makes MCD's answer depend on the size of the cells: at 250 m a seed is a block of 1.25 km a side, at 30 m one of
150 m.

### What the change did to the forest that remains

Losing 10% of a forest from its edge and losing it as a hundred holes in its middle are different events for the
species of its interior. GuidosToolbox measures the **forest area density** (FAD: the share of forest in a window
around each forest cell; Riitters et al., 2000) at both dates and asks how the forest present at both dates, the
**common forest**, moved between its classes (rare, patchy, transitional, dominant, interior, intact). The same
fragmentation measured cell by cell at one window is FOS (*fixed observation scale*). The
[fragmentation series](/tutorials/fragmentation/change-and-reports/) covers both in depth; here they answer one
question of change detection: did the forest that was not cleared lose its surroundings?

## When to use it

- **Deforestation and regrowth maps** for a report or a paper: the area lost and gained (simple change), and how
  much of it is structural (morphological change), from two forest maps, global products such as tree cover maps,
  or two classifications of your own.
- **Checking a pair of maps before trusting their change.** A large share of spurious change says the maps differ
  mostly along the edges, the signature of misregistration or of classifiers that disagree on mixed cells.
- **Forest interior and fragmentation indicators**: whether the forest that remains is less interior than before
  (Riitters & Wickham, 2012; Riitters et al., 2016).
- **Comparing a scenario with the present**: the second date can be a simulated map (see the
  [land change category](/tutorials/#land-change)).

It is *not* for many classes at once (use the transition matrix of [part 1](/tutorials/change-detection/transitions/));
nor for the rate of change against the time (the [intensity analysis](/tutorials/change-detection/intensity-analysis/)).
And morphological change does not correct a map: spurious change is change MCD cannot confirm, not change it has
shown to be false. For the area of change with confidence intervals, a sample of reference observations is the
proper tool ([part 3](/tutorials/change-detection/comparing-maps/)).

The [habitat patterns series](/tutorials/habitat-patterns/networks-and-change/) runs simple and morphological change
next to MSPA on a large forest; the [fragmentation series](/tutorials/fragmentation/change-and-reports/) runs FAD's
and FOS's change at five scales and writes GuidosToolbox's reports. This part is the change-detection view: what
these numbers add to a transition matrix, and when to believe them.

## What you need

- **Two maps of the same grid**: same extent, cell size and coordinate system, made the same way. Any classes: the
  `foreground` setting says which are forest.
- **Missing cells** as no data (the sea, clouds): they are left out (code 255).
- Here, the file of the series, [`sibuyan_landuse.tif`](/data/tutorials/land-change/what-land-change-models-are/sibuyan_landuse.tif)
  (108 × 128 cells of 250 m, bands `1997` and `2011`, forest class 1; see [part 1](/tutorials/change-detection/transitions/#what-you-need)),
  and Plum Island's maps of the CLUE-S series, [`pie_landuse.tif`](/data/tutorials/land-change/clue-s-ideas/pie_landuse.tif)
  (434 × 497 cells of about 100 m, bands `1985`, `1991` and `1999`, forest class 1).

!!! app "In the application"
    **Templates → Patterns (GuidosToolbox) → Forest gain and loss** opens the model below on a forest drawn by
    chance. Press ++f5++ to run it: the simple and morphological change open as tabs. Replace its first three
    nodes by two [[raster.read]] nodes to run it on your maps.

## Build the model

### The template

```tfgraph
#! caption: The template Forest gain and loss. Nodes 1 to 3 draw a forest at two dates; nodes 4 to 6 are GuidosToolbox's three change analyses; nodes 7 and 8 show two of the maps.
#! numbered: true
#! highlight: simple, morphological, fad
format = "terraflux/1"

[[nodes]]
id = "before"
op = "nlm.random_cluster"
params = { rows = 250, columns = 250, p = 0.55, weights = [0.4, 0.6], seed = 27 }
note = "The forest at the first date, drawn by chance: 1 the background, 2 the forest (in its place, a raster.read of a map of yours)."

[[nodes]]
id = "pressure"
op = "nlm.gaussian_field"
params = { rows = 250, columns = 250, range = 30, seed = 28 }
note = "Where clearing presses, from 0 to 1."

[[nodes]]
id = "after"
op = "raster.calc"
inputs = { f = "before.map", p = "pressure.map" }
params = { expression = "1 if f == 2 and p > 0.6 and nbsum(f == 1, 1) >= 1 else 2 if f == 1 and p < 0.2 and nbsum(f == 2, 1) >= 5 else f" }
note = "The second date: the forest cleared from its edges where the pressure is high, regrown in the gaps where it is low."

[[nodes]]
id = "simple"
op = "change.simple"
inputs = { before = "before.map", after = "after.map" }
note = "Every cell's change: stable background, gain, loss, stable forest; stats counts them."

[[nodes]]
id = "morphological"
op = "change.morphological"
inputs = { before = "before.map", after = "after.map" }
note = "The same change with the spurious one at the forest's edges (a cell's shift) told apart; stats has the net change and the elasticity."

[[nodes]]
id = "fad"
op = "change.fad"
inputs = { before = "before.map", after = "after.map" }
params = { foreground = 2, windows = [7, 13, 27, 81] }
note = "FAD at four scales at both dates: how many forest cells moved to a more fragmented class or a less fragmented one (stats), and the matrix of the classes."

[[nodes]]
id = "plot_simple"
op = "view.plot"
inputs = { value = "simple.map" }
params = { title = "Simple change" }

[[nodes]]
id = "plot_morphological"
op = "view.plot"
inputs = { value = "morphological.map" }
params = { title = "Morphological change" }
```

1. **`before`** ([[nlm.random_cluster]]) draws the first date: clusters of two classes, 1 background (40%) and 2
   forest (60%), already in GuidosToolbox's coding, so no `foreground` is needed downstream.
2. **`pressure`** ([[nlm.gaussian_field]]) draws a smooth surface from 0 to 1: where clearing presses.
3. **`after`** ([[raster.calc]]) makes the second date from the first: a forest cell with at least one background cell
   among its 8 neighbours (`nbsum(f == 1, 1) >= 1`) is cleared where the pressure passes 0.6, and a background cell
   with at least 5 forest neighbours regrows where it is below 0.2. Only the edge retreats, by one cell: keep this
   in mind when reading the morphological change below.
4. **`simple`** ([[change.simple]]) takes the first date on `before` and the second on `after`. Swap them and gain
   and loss swap too.
5. **`morphological`** ([[change.morphological]]) takes the same two inputs.
6. **`fad`** ([[change.fad]]) also takes the two *forest maps* and computes FAD at both dates itself, at the
   `windows` given (sides in cells, odd).

In the application, add each change node from the library's subject **Change detection**, drag `before.map`
onto its `before` port and `after.map` onto `after`, and select the node to see its settings in the Node pane.

```tfcompare
caption: "The template's forest at the two dates (forest green, background beige). Drag the handle: the forest's edge has moved back by one cell where the pressure is high, and a few gaps have closed."
before: { src: /img/tutorials/change-detection/forest-gain-and-loss/template_before.png, label: First date }
after: { src: /img/tutorials/change-detection/forest-gain-and-loss/template_after.png, label: Second date }
```

### On your maps: Sibuyan's forest

```tfgraph
#! caption: Sibuyan's forest (class 1) in 1997 and 2011, through simple change, morphological change and FAD's change at three windows.
#! numbered: true
#! highlight: simple, morphological, fad
format = "terraflux/1"

[[nodes]]
id = "before"
op = "raster.read"
params = { path = "sibuyan_landuse.tif", band = "1997" }
note = "Land use in 1997: 1 forest, 2 coconut, 3 grassland, 4 rice, 5 other."

[[nodes]]
id = "after"
op = "raster.read"
params = { path = "sibuyan_landuse.tif", band = "2011" }

[[nodes]]
id = "simple"
op = "change.simple"
inputs = { before = "before.raster", after = "after.raster" }
params = { foreground = [1] }

[[nodes]]
id = "morphological"
op = "change.morphological"
inputs = { before = "before.raster", after = "after.raster" }
params = { foreground = [1] }

[[nodes]]
id = "fad"
op = "change.fad"
inputs = { before = "before.raster", after = "after.raster" }
params = { foreground = [1], windows = [7, 13, 27] }
```

Nodes 1 and 2 read the two bands of the file by their names. Each change node gets `foreground = [1]`: forest is
class 1, and coconut, grassland, rice and other land are the background. To call coconut plantations forest too,
write `foreground = [1, 2]`: the answer changes, and that choice belongs in the methods of a paper. The windows of
FAD are in cells: 7, 13 and 27 cells of 250 m are 1.75, 3.25 and 6.75 km.

## Read the results

### Simple change: how much forest

![Simple change of Sibuyan's forest](/img/tutorials/change-detection/forest-gain-and-loss/simple.png "Simple change of Sibuyan's forest, 1997 to 2011, in GuidosToolbox's colours: stable forest dark grey, loss red, stable background light grey. The shares are of the island's cells.")

The `stats` table: 4,160 cells of stable forest, **563 of loss** (35.2 km²), **0 of gain**, 2,503 of stable
background, and 6,598 missing (the sea). These are exactly the numbers of the forest's row and column in the
transition matrix of [part 1](/tutorials/change-detection/transitions/): 320 cells to grassland, 126 to coconut, 116
to rice, 1 to other; nothing into forest. Simple change adds no information to the matrix; it maps it in one class
and in GuidosToolbox's codes, which the other analyses read.

The losses lie along the forest's northern and south-western edges, where the farmland and grassland of the coast
met it; [part 1's map of transitions](/tutorials/change-detection/transitions/) shows which class took each cell.

### Morphological change: how much of it is structural

![Morphological change of Sibuyan's forest](/img/tutorials/change-detection/forest-gain-and-loss/morph.png "Morphological change of the same maps: structural loss red, spurious change lilac. The large clearing of the north-west stays loss; the thin strips along the forest's edge become spurious.")

| `stats` | Cells |
|---|---:|
| Forest in 1997 | 4,723 |
| Forest in 2011 | 4,160 |
| Net change | −563 |
| Structural loss | 208 |
| Structural gain | 0 |
| Spurious change | 375 |
| Stable forest | 4,141 |
| Elasticity | 1.05 |

Two thirds of the loss (375 of 563 cells) is spurious: strips one or two cells wide along the forest's edge. Of the
208 cells of structural loss, 188 were cleared, 19 were forest at both dates and 1 background at both: the 3 × 3
closing of step 3 fills the small islands and gaps inside a clearing, so the map shows where the forest's structure
was lost rather than the exact cells cleared. Report the area lost from simple change, and the structural part from
morphological change.

![Simple and morphological change side by side](/img/tutorials/change-detection/forest-gain-and-loss/closeup.png "The north-west of the island, simple change (left) and morphological change (right). The clearing that reaches into the forest's core is structural; the retreat of the edge on the right of the close-up and the scattered cells are spurious, and the closing has filled the small forest islands inside the clearing.")

Is the spurious change real? On these maps, probably much of it is: a forest frontier advancing by a cell (250 m)
in fourteen years is plausible. MCD cannot know. It says that this change *could* have been made by the two maps
being shifted or classified differently along the edge, and that the structural part could not. The **elasticity**
of 1.05 says the forest's interior shrank about as fast as the forest: Sibuyan lost forest from its outside, not
from within.

### A misregistration: change where nothing changed

To see what MCD is for, compare the forest of 2011 with *itself*, moved one cell (250 m) to the east, as two
images registered one cell apart would be. Nothing changed on the ground.

```tfcompare
caption: "The forest of 2011 against itself moved one cell east. Left of the handle, simple change: 177 cells of 'loss' on the western edges and 177 of 'gain' on the eastern ones. Right, morphological change: all 354 cells spurious, no structural change at all."
before: { src: /img/tutorials/change-detection/forest-gain-and-loss/misreg_simple_bare.png, label: Simple change }
after: { src: /img/tutorials/change-detection/forest-gain-and-loss/misreg_bare.png, label: Morphological change }
```

![The legend of the change maps](/img/tutorials/change-detection/forest-gain-and-loss/change_legend.png)

Simple change reports 354 changed cells, 8.5% of the forest, more than half the real loss of fourteen years, from a
shift of one cell. Morphological change reports none. When two maps come from different sensors, dates of the year or
classifiers, run morphological change first: a large share of spurious change is a warning about the maps before it
is a finding about the forest.

### Resolution matters: Plum Island

Plum Island's forest (1985 to 1999, cells of about 100 m) both lost and gained: simple change gives **4,906 cells
of loss** and **1,270 of gain**, a net loss of 3,636 cells (7.4% of the forest). Morphological change calls **all
6,176 changed cells spurious**: no structural loss, no structural gain.

![Plum Island, simple and morphological change](/img/tutorials/change-detection/forest-gain-and-loss/pie_closeup.png "Part of Plum Island, 1985 to 1999. Left, simple change: many small clearings for new houses, and some regrowth. Right, morphological change: every one of them spurious.")

The change here is suburban growth in many small patches (part 1 of the [CLUE-S series](/tutorials/land-change/clue-s-ideas/)
maps it): of the 1,443 patches of loss, the median has 2 cells and the largest 77, and none contains a 5 × 5 block
(500 m a side) that went from all forest to all background. MCD was designed for large clearings seen at fine
resolution; at 100 m it cannot confirm a house lot. Yet this loss mattered: the **elasticity is 2.76**, the
interior shrank almost three times as fast as the forest, because every small clearing pierced it. Read the two
numbers together: *no structural loss* and *a high elasticity* together describe a forest perforated by small
clearings.

### What the change did to the forest left

```tfchart
type: bar
data: /data/tutorials/change-detection/forest-gain-and-loss/fad_change.csv
x: scale
y: [sibuyan_fad, pie_fad, sibuyan_avcon, pie_avcon]
labels: { sibuyan_fad: "Sibuyan, ΔFAD_AV", pie_fad: "Plum Island, ΔFAD_AV", sibuyan_avcon: "Sibuyan, ΔAVCON", pie_avcon: "Plum Island, ΔAVCON" }
colors: { sibuyan_fad: "#3b8a5a", pie_fad: "#b4508a", sibuyan_avcon: "#9fd0b0", pie_avcon: "#e3b3cf" }
xlabel: Window (cells)
ylabel: Change (percentage points)
height: 320
caption: "The change of the average forest density around forest cells (ΔFAD_AV: the fragmentation of the forest that remains) and over the whole area (ΔAVCON: loss and fragmentation together), at three windows and their average (change.fad's stats)."
```

[[change.fad]]'s `stats` table has a row per window and one for the average of all (`scale` 0). The two forests
tell different stories:

- **Sibuyan** lost forest from its edges. The forest left is barely more fragmented at a window of 7 cells (ΔFAD_AV
  −0.9 points) and only a little more at 27 cells (−3.4), but the density over the whole island fell by 8 points
  (ΔAVCON), because the forest itself shrank. Of the common forest, between 657 cells (7 cells) and 1,083 (27 cells)
  moved to another class, every one of them to a more fragmented class (`increased_percent` = 100): with no forest
  gained, no cell could become denser.
- **Plum Island** lost proportionally less forest, but its forest left lost 3.3 to 3.5 points of density at every
  window: the clearings are everywhere, inside the forest, so they are seen from every forest cell. Here 6 to 14% of
  the cells that changed class became *less* fragmented (`decreased_percent`), next to the 1,270 cells regained.

### FOS, cell by cell

[[change.fos]] maps the same change on each cell. It takes two **FOS maps**, not forest maps: compute
[[fragmentation.fos]] at both dates with the same settings, then connect their `map` outputs.

```tfgraph
#! caption: The change of fragmentation at one window, cell by cell: FOS at both dates (FAD's density, six classes), then their change.
#! numbered: true
#! highlight: fos_before, fos_after, fos_change
format = "terraflux/1"

[[nodes]]
id = "before"
op = "raster.read"
params = { path = "sibuyan_landuse.tif", band = "1997" }

[[nodes]]
id = "after"
op = "raster.read"
params = { path = "sibuyan_landuse.tif", band = "2011" }

[[nodes]]
id = "fos_before"
op = "fragmentation.fos"
inputs = { map = "before.raster" }
params = { foreground = [1], window = 13 }

[[nodes]]
id = "fos_after"
op = "fragmentation.fos"
inputs = { map = "after.raster" }
params = { foreground = [1], window = 13 }

[[nodes]]
id = "fos_change"
op = "change.fos"
inputs = { before = "fos_before.map", after = "fos_after.map" }
```

The map's codes need care. On the forest of both dates each cell holds **100 + its connectivity before − its
connectivity after** (connectivity in %): 100 is no change, **above 100 a loss** of connectivity, below 100 a gain.
Off the common forest: 250 forest gained, 251 forest lost, 252 stable background, 254 missing.

```tfslider
label: Window (cells of 250 m)
caption: "The change of FOS of Sibuyan's forest, 1997 to 2011, at four windows (each a real run of the model above with `window` changed). Black, the forest lost; yellow to dark orange, the forest left that lost connectivity; pale, no change. The larger the window, the deeper into the forest the losses are felt."
frames:
  - { value: 5, src: /img/tutorials/change-detection/forest-gain-and-loss/fos_5.png, stats: { "Common forest": "4,160", "Lost connectivity": "615", "Mean drop": "2.7 points", "In another class": "487" } }
  - { value: 7, src: /img/tutorials/change-detection/forest-gain-and-loss/fos_7.png, stats: { "Common forest": "4,160", "Lost connectivity": "924", "Mean drop": "3.3 points", "In another class": "657" } }
  - { value: 13, src: /img/tutorials/change-detection/forest-gain-and-loss/fos_13.png, stats: { "Common forest": "4,160", "Lost connectivity": "1,467", "Mean drop": "4.4 points", "In another class": "959" } }
  - { value: 27, src: /img/tutorials/change-detection/forest-gain-and-loss/fos_27.png, stats: { "Common forest": "4,160", "Lost connectivity": "2,537", "Mean drop": "5.5 points", "In another class": "1,083" } }
start: 2
```

![Legend of the FOS change](/img/tutorials/change-detection/forest-gain-and-loss/fos_legend.png)

With a window of 5 cells (1.25 km), 615 of the 4,160 cells of common forest lost connectivity, all within two cells
of a clearing; with 27 cells (6.75 km), 2,537 did, a band several kilometres deep behind each front. The halo is the
forest that a species sensitive to edges, or to forest cover within its home range, has lost without a tree being
cut in it. Choose the window from the species or the process: a home range, a dispersal distance, the reach of edge
effects.

### How to report it

For each pair of dates: the forest at both dates, the gain and the loss (simple change), the structural and the
spurious change and the elasticity (morphological change), and ΔFAD_AV and ΔAVCON at the windows that matter for
your question (FAD's change). Give the maps' source and resolution, the classes counted as forest, and the windows
in metres. Show the morphological change map with the simple one.

## Settings that change the answer

- **The classes counted as forest** (`foreground`). Plantations, mangroves, woodland: each choice changes every
  number on this page.
- **The resolution.** MCD's seeds are 5 × 5 cells, so a coarser map calls more change structural and a finer one
  more change spurious (compare Sibuyan at 250 m with Plum Island at 100 m). Compare dates only at the same
  resolution, and say which.
- **The windows** of FAD and FOS: the slider above. Small windows see the edge, large ones the landscape.
- **The order of the dates.** `before` and `after` are not symmetric: swapping them swaps gain and loss, and a FOS
  change computed the wrong way round reads every loss of connectivity as a gain.

Morphological change has no setting: its 5 × 5 cores and 3 × 3 closing are GuidosToolbox's.

## Common pitfalls

- **GuidosToolbox's legend of FOS change.** GuidosToolbox names the groups of the FOS change map "ΔFOS" with the
  difference taken as *before − after*: its "ΔFOS +21 to +100" is a **loss** of 21 to 100 points, coloured dark
  orange. The legend on this page says it in words; check the sign before writing that connectivity increased.
- **Codes that are not GuidosToolbox's.** Without `foreground`, the operators expect 1 background and 2 forest. A
  land use map with forest as class 1 run without `foreground = [1]` treats forest as background and coconut as
  forest.
- **Reading spurious as false.** Spurious change is unconfirmed, not disproved. In a well-registered pair of maps
  of the same sensor, much of it is real retreat of the edge.
- **Reading structural loss as the area cleared.** The closing adds stable cells inside clearings; take the area
  from simple change.
- **Maps of different grids.** The operators stop when the two maps differ in size. Resample one onto the other
  first, with nearest neighbour for classes, and expect a burst of spurious change.
- **Missing cells at one date only** (a cloud) are missing in the result: compare the missing counts of the
  `stats` with your maps.

## Going further

- [Part 1](/tutorials/change-detection/transitions/): the transition matrix the forest's row and column come from.
- [Part 3](/tutorials/change-detection/comparing-maps/): map error and the estimation of areas of change from a
  sample.
- [Habitat patterns, part 4](/tutorials/habitat-patterns/networks-and-change/): simple and morphological change
  next to MSPA and the forest's network.
- [Fragmentation, part 3](/tutorials/fragmentation/change-and-reports/): FAD's change at five scales, the change
  of the landscape mosaic, and GuidosToolbox's reports.
- Operators: [[change.simple]], [[change.morphological]], [[change.fad]], [[change.fos]], [[fragmentation.fos]],
  [[change.mosaic]].

## References

- Olofsson, P., Foody, G. M., Herold, M., Stehman, S. V., Woodcock, C. E., & Wulder, M. A. (2014). Good practices for estimating area and assessing accuracy of land change. *Remote Sensing of Environment*, 148, 42–57. https://doi.org/10.1016/j.rse.2014.02.015
- Riitters, K. H., Wickham, J. D., O'Neill, R. V., Jones, K. B., & Smith, E. R. (2000). Global-scale patterns of forest fragmentation. *Conservation Ecology*, 4(2), 3. https://doi.org/10.5751/ES-00209-040203
- Riitters, K. H., & Wickham, J. D. (2012). Decline of forest interior conditions in the conterminous United States. *Scientific Reports*, 2, 653. https://doi.org/10.1038/srep00653
- Riitters, K., Wickham, J., Costanza, J. K., & Vogt, P. (2016). A global evaluation of forest interior area dynamics using tree cover data from 2000 to 2012. *Landscape Ecology*, 31(1), 137–148. https://doi.org/10.1007/s10980-015-0270-9
- Seebach, L., Strobl, P., Vogt, P., Mehl, W., & San-Miguel-Ayanz, J. (2013). Enhancing post-classification change detection through morphological post-processing – a sensitivity analysis. *International Journal of Remote Sensing*, 34(20), 7145–7162. https://doi.org/10.1080/01431161.2013.815382
- Townshend, J. R. G., Justice, C. O., Gurney, C., & McManus, J. (1992). The impact of misregistration on change detection. *IEEE Transactions on Geoscience and Remote Sensing*, 30(5), 1054–1060. https://doi.org/10.1109/36.175340
- Vogt, P., & Riitters, K. (2017). GuidosToolbox: universal digital image object analysis. *European Journal of Remote Sensing*, 50(1), 352–361. https://doi.org/10.1080/22797254.2017.1330650
