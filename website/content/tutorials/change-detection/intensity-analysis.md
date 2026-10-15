---
title: "Intensity analysis: how fast, and which classes"
short: Intensity analysis
description: Aldwaik and Pontius's intensity analysis at the interval, category and transition levels — whether change was fast or slow, which classes gained and lost actively, and which transitions targeted a class.
order: 20
level: intermediate
time: 50 min
ops: [change.intensity, change.transitions, raster.read]
series: Change detection
part: 2
---

A transition matrix says *how much* changed. It does not say whether that is a lot. Forest losing 35 km² is
alarming on a small island and negligible on a continent; built land gaining 30 km² in six years is fast if the
next eight years add the same. In this tutorial you will run the **intensity analysis** of Aldwaik and Pontius
(2012) on two landscapes: Plum Island, in Massachusetts, mapped at three dates (two periods), and Sibuyan Island,
from [part 1](/tutorials/change-detection/transitions/). You will learn to read its three levels — the
interval, the category and the transition — and to say, with numbers, which period changed fast, which classes
were active, and which classes a transition targeted or avoided.

## The idea

Intensity analysis compares every observed rate of change with the rate it would have if change were spread
**uniformly**. It grew out of an earlier idea, comparing each observed transition with the transition expected
from the sizes of the classes (Pontius et al., 2004). "Uniform" is a deliberately simple null model: if change were blind to time, every period would
change at the same annual rate; if it were blind to classes, every class would gain and lose in proportion to its
size; if a gaining class took land at random, it would take from every other class in proportion to that class's
size. Where the observed rate departs from the uniform one, something non-random is at work, and that is where an
ecologist or a geographer should look for causes.

Safaa Aldwaik and Robert Gilmore Pontius Jr. (2012) organised the comparison in three levels, each answering one
question and each nested in the previous one:

1. **Interval level.** Was the change in each period fast or slow, compared with the whole time span?
2. **Category level.** In each period, which classes gained or lost *actively* (faster than the period's rate)
   and which were *dormant*?
3. **Transition level.** For a class that gained, did it *target* some classes (take from them faster than their
   size explains) and *avoid* others? For a class that lost, which classes took its losses?

The same idea runs through all three levels: a rate is an area of change divided by the time and by the area that
could have changed, so that small classes and long periods are compared fairly with large classes and short
periods. All rates are in **% per year**.

!!! science "Key idea: the interval level"
    For a period \(t\) from year \(Y_t\) to \(Y_{t+1}\), with \(C_{tij}\) the area in class \(i\) at its start and
    class \(j\) at its end, and \(A\) the area of the map:

    \[ S_t = \frac{\text{area changed in } t}{A \,(Y_{t+1} - Y_t)} \times 100\% \qquad
       U = \frac{\text{area changed in all periods}}{A \,(Y_T - Y_1)} \times 100\% \]

    \(S_t\) is the annual rate of change of period \(t\); \(U\) is the **uniform intensity**, the rate every
    period would have if the same total change were spread evenly over the whole time span. A period with
    \(S_t > U\) was **fast**, one with \(S_t < U\) **slow**.

!!! science "Key idea: the category level"
    In period \(t\), class \(j\)'s **gain intensity** is its gross gain over its area at the *end* of the period,
    and class \(i\)'s **loss intensity** its gross loss over its area at the *start*:

    \[ G_{tj} = \frac{\text{gain of } j}{(Y_{t+1} - Y_t)\,\text{area of } j \text{ at } Y_{t+1}} \times 100\%
       \qquad
       L_{ti} = \frac{\text{loss of } i}{(Y_{t+1} - Y_t)\,\text{area of } i \text{ at } Y_t} \times 100\% \]

    Each is compared with the period's rate \(S_t\): above it the class was **active**, below it **dormant**.

!!! science "Key idea: the transition level"
    For the gains of a class \(n\): the intensity with which \(n\) took class \(i\) is
    \(R_{tin} = C_{tin} / [(Y_{t+1} - Y_t)\,\text{area of } i \text{ at } Y_t] \times 100\%\), and the uniform
    intensity \(W_{tn}\) is \(n\)'s gross gain over the area of all the other classes at \(Y_t\). Class \(n\)
    **targeted** \(i\) if \(R_{tin} > W_{tn}\) and **avoided** it otherwise.

    For the losses of a class \(m\): \(Q_{tmj}\) is the intensity with which class \(j\) took \(m\) (the
    transition over \(j\)'s area at \(Y_{t+1}\)), and \(V_{tm}\) is its uniform intensity; \(m\)'s losses
    **targeted** \(j\) if \(Q_{tmj} > V_{tm}\).

With more than two periods, the analysis also tells whether a pattern is **stationary**: the same in every period
(a class active in all of them, a transition targeted in all of them). Terraflux's [[change.intensity]] computes
all of this as OpenLand's `intensityAnalysis` does, the R package of Exavier and Zeilhofer (2020).

## When to use it

- **To compare periods of unequal length.** Censuses and satellite archives rarely give evenly spaced dates. The
  interval level puts 6 years and 8 years on the same scale.
- **To compare classes of very different sizes.** A large forest loses many hectares even when it is barely
  touched; a small class of wetlands can lose few hectares and still be disappearing fast. The category level
  divides by each class's own size.
- **To find the processes behind a transition matrix.** Urban growth that targets agriculture rather than forest,
  deforestation that feeds pasture rather than crops: the transition level says which, and whether the pattern
  held in every period. Teixeira et al. (2016), for example, used it on CORINE land cover maps of a Portuguese
  watershed to show which transitions deviated from uniform in each period.

It is not a test of statistical significance: a rate above uniform is a fact about the maps, not proof of a
process. Map errors alone can create deviations from uniform (Aldwaik & Pontius, 2013), so small deviations
deserve caution. For *how much* changed, see [part 1](/tutorials/change-detection/transitions/); for
whether a simulated map got the change right, [part 3](/tutorials/change-detection/comparing-maps/).

## What you need

A series of two or more maps of classes on the same grid, with the date of each. Two files from Terraflux's
example data are used here:

- `pie_landuse.tif`: **Plum Island Ecosystems**, Massachusetts, a long-term ecological research site north of
  Boston. Three bands, named `1985`, `1991` and `1999`; 434 × 497 cells of about 100 m (EPSG:26986, the
  Massachusetts state plane in metres); classes 1 forest, 2 built, 3 other (mostly open land and wetlands);
  255 no data. 113,563 cells with data: 1,134 km².
- `sibuyan_landuse.tif`: Sibuyan Island in 1997 and 2011, as in part 1 (1 forest, 2 coconut, 3 grassland, 4 rice,
  5 other; cells of 250 m; modified for teaching).

![Plum Island's land use in 1985, 1991 and 1999](/img/tutorials/change-detection/intensity-analysis/pie_dates.png "Plum Island Ecosystems in 1985, 1991 and 1999: forest in green, built land in purple, other land in sand. Built land grew from 33% to 38% of the area with data.")

The time between the dates matters: it is read from the layers' names when they are numbers (as here), or given
with the `times` setting (for example `times = [1985, 1991, 1999]`). Rates are "per unit of time": per year when
the times are years.

## Build the model

The model is short. A [[raster.read]] node (library: **IO**) with `path = "pie_landuse.tif"` and no band gives,
on its `stack` output, the three dates as layers named by their years. Connect that output to the `maps` input of
[[change.intensity]] (library: **Change detection**). Add [[change.transitions]] on the same stack to have the
areas behind the rates, and a [[view.plot]] of the stack to see the maps.

[[change.intensity]] needs two settings: `gain`, the class whose gains the transition level examines, and `loss`,
the class whose losses it examines. Here the question is urban growth: `gain = 2` (built) and `loss = 1` (forest).
Its other settings: `times` and `cell_area` (in m², by default from the grid's cell sides) as for
[[change.transitions]], and `area` (true by default) to give the areas in km² rather than in cells. The rates do
not depend on `area`.

```tfgraph
#! caption: Plum Island's three dates, their transitions and their intensity analysis, with built land's gains and forest's losses at the transition level.
#! highlight: intensity
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "dates"
op = "raster.read"
params = { path = "pie_landuse.tif" }
note = "Three layers: 1985, 1991, 1999 (1 forest, 2 built, 3 other)."

[[nodes]]
id = "transitions"
op = "change.transitions"
inputs = { maps = "dates.stack" }
note = "Every transition of each period, in km²."

[[nodes]]
id = "intensity"
op = "change.intensity"
inputs = { maps = "dates.stack" }
params = { gain = 2, loss = 1 }
note = "Interval, category and transition levels; built's gains, forest's losses."

[[nodes]]
id = "plot"
op = "view.plot"
inputs = { value = "dates.stack" }
params = { title = "Plum Island, 1985-1999" }
```

Select the intensity node and press ++ctrl+enter++. It has nine outputs, all tables, which the Viewer shows when
you pick them under the canvas:

| Output | What it holds (a row per...) | Key columns |
|---|---|---|
| `interval` | period | `PercentChange` (of the map in the period), `St`, `U` |
| `category_gain` | period and gaining class (`To`) | `GG_km2` (gross gain), `Gtj`, `St` |
| `category_loss` | period and losing class (`From`) | `GL_km2` (gross loss), `Lti`, `St` |
| `transition_gain` | period and class taken by `gain` (`From`) | `T_i2n_km2`, `Rtin`, `Wtn` |
| `transition_loss` | period and class taking `loss` (`To`) | `T_m2j_km2`, `Qtmj`, `Vtm` |
| `category_gain_stationarity` | class, active then dormant | periods counted, `N`, `above`, `test` |
| `category_loss_stationarity` | class, active then dormant | as above |
| `transition_gain_stationarity` | class, targeted then avoided | as above |
| `transition_loss_stationarity` | class, targeted then avoided | as above |

The column names are those of OpenLand, so that results can be compared with R. Classes without any gain (or
loss) in a period have no row in that period's category table: read a missing row as an intensity of 0.

=== "Application"

    Save the model next to `pie_landuse.tif` and press ++f5++. In the Viewer, a table can be drawn as a chart:
    the category tables as bars of `Gtj` (or `Lti`) per class, with `St` beside them.

=== "Command line"

    Add the tables as outputs at the end of the model, then `terraflux run`:

    ```toml
    [outputs.interval]
    from = "intensity.interval"

    [outputs.category_gain]
    from = "intensity.category_gain"

    [outputs.category_loss]
    from = "intensity.category_loss"

    [outputs.transition_gain]
    from = "intensity.transition_gain"
    ```

    ```bash
    terraflux run plum_island_intensity.toml --out output
    ```

    Each table is written as `output/<name>.csv`.

## Read the results

The charts below follow Aldwaik and Pontius's convention: a bar per observed intensity, a dashed line for the
uniform intensity it is compared with.

### Interval level: was a period fast or slow?

![Interval level of Plum Island](/img/tutorials/change-detection/intensity-analysis/pie_interval.png "Interval level: Plum Island changed at 0.598% of its area a year from 1985 to 1991 and 0.523% from 1991 to 1999; the uniform intensity is 0.556% a year.")

From 1985 to 1991, 3.59% of the area changed class in 6 years: 0.598% a year. From 1991 to 1999, 4.19% changed
in 8 years: 0.523% a year. More land changed in the second period, but it was longer; per year, the first
period was **fast** and the second **slow**, against a uniform intensity of 0.556% a year (the 7.78% that changed
in the two periods, over 14 years). The difference is small: both periods are within 8% of the uniform rate.

```tfchart
type: bar
data: /data/tutorials/change-detection/intensity-analysis/pie_interval.csv
x: period
y: [observed, uniform]
labels: { observed: "Observed (St)", uniform: "Uniform (U)" }
colors: { observed: "#e8901f", uniform: "#48534d" }
ylabel: "% of the area a year"
height: 240
caption: "The interval level as numbers (hover for the values): the annual rate of each period and the uniform rate."
```

### Category level: which classes were active?

![Category level of Plum Island](/img/tutorials/change-detection/intensity-analysis/pie_category.png "Category level: in both periods, built land gained actively and forest and other land lost actively; the dashed line is each period's rate St.")

In each period the dashed line is that period's rate \(S_t\). **Built land gained actively in both periods**:
1.35% of its (final) area a year from 1985 to 1991, more than twice the period's rate, and 0.93% from 1991 to
1999. Forest and other land gained dormantly. On the loss side, **forest and other land lost actively** in both
periods, and built land almost never lost (0.02 and 0.04% a year): once land is built, it stays built.

Other land is a lesson in why the category level divides by each class's size. Forest lost more km² than other
land in both periods (23.4 against 17.0 km² from 1985 to 1991), yet other land lost at a higher *intensity*
(1.03% against 0.80% a year), because it was the smaller class.

```tfchart
type: bar
data: /data/tutorials/change-detection/intensity-analysis/pie_category.csv
x: class
y: [gain, loss, uniform]
labels: { gain: "Gain intensity (Gtj)", loss: "Loss intensity (Lti)", uniform: "Period's rate (St)" }
colors: { gain: "#3b8a5a", loss: "#c0392b", uniform: "#48534d" }
ylabel: "% of the class a year"
height: 300
caption: "The category level of Plum Island, a group of bars per class and period: gain and loss intensities beside the period's rate."
```

The stationarity tables summarise the two periods. From `category_gain_stationarity` and
`category_loss_stationarity`:

| Class | Gains | Losses |
|---|---|---|
| Forest | dormant in 2 of 2 periods | active in 2 of 2 periods |
| Built | **active in 2 of 2 periods** | dormant in 2 of 2 periods |
| Other | dormant in 2 of 2 periods | active in 2 of 2 periods |

Every class behaved the same way in both periods: the category level is **stationary**. In the tables, a row
gives the class, the number of periods it was active (`above = 1`) or dormant (`above = 0`), the number of
periods `N`, and `test = 1` when that held in every period.

### Transition level: what did built land take?

![Transition level of Plum Island](/img/tutorials/change-detection/intensity-analysis/pie_transition.png "Transition level: built land's gains (left) against their uniform intensity, and forest's losses (right) against theirs, in each period.")

The left column answers "which classes did built land take?". From 1985 to 1991, built land took other land at
0.81% of it a year and forest at 0.65%, against a uniform intensity of 0.71%: it **targeted other land and
avoided forest**. From 1991 to 1999 the pattern reversed: forest at 0.58% a year, above the uniform 0.55%, and
other land at 0.51%, below it. The transition level is **not stationary**: `transition_gain_stationarity` lists
forest and other land each as targeted in 1 of 2 periods and avoided in the other. A plausible reading is that
development first took the open land that was easiest to build on, then moved into the forest when that ran
short; testing it would need data on land prices, zoning or soils.

The right column answers "which classes took the forest?". Built land took forest's losses at 0.80% and 0.63% of
built land's area a year, above the uniform intensities (0.59% and 0.48%): **forest's losses targeted built land
in both periods** and avoided other land. Forest was lost to development, not to farming or wetlands, and that was
stationary.

The two columns look at the same transition, forest → built, from its two sides, and need not agree. Built land
avoided forest in the first period (seen from its gains), while forest's losses targeted built land (seen from
forest's losses). Both are true: built land took more from other land than from forest relative to their sizes,
and of the forest that was lost, more went to built land than built land's size alone would predict.

### Sibuyan: one interval, many classes

With only two dates, Sibuyan has one interval, and the interval level is trivial: the period's rate equals the
uniform rate (10.28% of the island changed in 14 years: 0.734% a year). The category and transition levels are
not trivial.

![Category level of Sibuyan](/img/tutorials/change-detection/intensity-analysis/sib_category.png "Sibuyan's category level, 1997-2011: coconut, grassland and rice gained actively; forest, grassland and rice lost actively; the dashed line is the period's rate, 0.734% a year.")

```tfchart
type: bar
data: /data/tutorials/change-detection/intensity-analysis/sib_category.csv
x: class
y: [gain, loss, uniform]
labels: { gain: "Gain intensity (Gtj)", loss: "Loss intensity (Lti)", uniform: "Period's rate (St)" }
colors: { gain: "#3b8a5a", loss: "#c0392b", uniform: "#48534d" }
ylabel: "% of the class a year"
height: 280
caption: "Sibuyan's category level as numbers: forest lost at 0.85% of its area a year, rice gained at 3.27% and lost at 2.17%."
```

Rice is the most dynamic class: it gained at 3.27% and lost at 2.17% of its area a year, both far above the
period's 0.73%. It is a small class (14 km² in 1997) moving around the coastal plain, which part 1 found as a
large swap. Forest lost actively (0.85% a year) and never gained. Grassland both gained (2.26%) and lost (0.80%)
actively.

At the transition level, the slider shows the gains of each class of farmland in turn (the `gain` setting): which
classes did each one take?

```tfslider
label: Gaining class (gain)
caption: "The transition level of Sibuyan, 1997-2011, for the gains of coconut (gain = 2), grassland (gain = 3) and rice (gain = 4): each class's intensity of being taken, against the uniform intensity W (dashed)."
frames:
  - { value: "2 coconut", src: /img/tutorials/change-detection/intensity-analysis/sib_gain_2.png, caption: "Coconut targeted rice (1.95% of it a year) and grassland (0.66%), and avoided forest.", stats: { Targeted: "rice, grassland", Avoided: "forest, other", "Uniform W": "0.311" } }
  - { value: "3 grassland", src: /img/tutorials/change-detection/intensity-analysis/sib_gain_3.png, caption: "Grassland targeted forest (0.48% of it a year) and avoided the farmland.", stats: { Targeted: "forest", Avoided: "coconut, rice, other", "Uniform W": "0.386" } }
  - { value: "4 rice", src: /img/tutorials/change-detection/intensity-analysis/sib_gain_4.png, caption: "Rice targeted forest (0.175%) and, barely, grassland (0.136% against 0.135%).", stats: { Targeted: "forest, grassland", Avoided: "coconut, other", "Uniform W": "0.135" } }
```

The picture is of a frontier: grassland and rice advance into the forest, coconut follows by taking the grassland
and rice behind them. Coconut **avoided** forest even though it took 7.9 km² of it, because forest is so large
that 7.9 km² is a small share of it. Rice's targeting of grassland is a deviation of 0.001% a year from uniform: a
real fact of these maps, but one that a single misclassified cell could reverse (see the pitfalls).

And forest's losses (`loss = 1`): rice took them at 2.88% and grassland at 2.10% of their own area a year,
against a uniform 1.31%; coconut (0.64%) and other land (0.03%) were avoided.

![Transition level of Sibuyan](/img/tutorials/change-detection/intensity-analysis/sib_transition.png "Sibuyan's transition level: where grassland's gains came from (left), and where forest's losses went (right).")

!!! paper "How to report it"
    Report the three levels in order, each as a chart of bars against a dashed uniform line, with the areas of the
    transitions behind them (from [[change.transitions]]). Say how many dates and how long each period was, and
    the map's area. A typical paragraph: "Change was faster from 1985 to 1991 (0.60% of the area a year) than
    from 1991 to 1999 (0.52%), against a uniform 0.56%. Built land gained actively and forest lost actively in
    both periods. Forest's losses targeted built land in both periods, but built land's gains targeted other land
    in the first period and forest in the second." Cite Aldwaik and Pontius (2012) for the method and OpenLand
    (Exavier & Zeilhofer, 2020) for the software it reproduces.

## Settings that change the answer

- **`gain` and `loss`.** They do not change the interval and category levels; they choose which class the
  transition level examines. Run the node once per class of interest (as the slider above), or add one
  [[change.intensity]] node per class.
- **`times`.** The rates are per unit of time. Give the dates when the layers' names are not numbers: without
  them, the layers are numbered 0, 1, 2 and the rates become "per period", which hides periods of unequal length.
- **Which dates.** The intervals are those between consecutive layers. Adding a date splits a period and can
  reverse a conclusion: Plum Island's 14 years, analysed as one interval, would have hidden that built land's
  target switched from other land to forest.
- **The legend.** Merging classes changes every level. A class "other" that mixes open land and wetlands averages
  two different behaviours; at the transition level, built land may target one and avoid the other.
- **`cell_area` and `area`.** They change the areas reported (`GG_km2`, `T_i2n_km2`...), never the intensities.

## Common pitfalls

- **Deviations within the margin of map error.** A class barely above uniform (rice taking grassland at 0.136%
  against 0.135%) is not evidence of anything. Aldwaik and Pontius (2013) show how to compute the map error that
  would explain a deviation; at the least, flag deviations that a few cells could reverse.
- **Reading active as large.** An active class changed fast *relative to its size*. A small class can be very
  active and still contribute little area; report the areas beside the intensities.
- **Gain denominators at the end, loss denominators at the start.** \(G_{tj}\) divides by the class's area at
  the end of the period, \(L_{ti}\) by its area at the start. A class that grows quickly has a large end area,
  which lowers its gain intensity.
- **Missing rows.** A class with no gain in a period has no row in `category_gain` for it; the charts must show it
  as 0, not leave it out (forest in Sibuyan).
- **Unequal maps.** The analysis assumes the maps are comparable: same legend, same method, same grid. Changes in
  the method between dates show up as change, often as swap at class boundaries.
- **Dates read from the wrong place.** Check the `interval` table's `year_from` and `year_to`: if they read 0
  and 1, the layers' names were not numbers and `times` is needed.

## Going further

- [Part 1](/tutorials/change-detection/transitions/): the transition matrix, gains, losses, quantity, exchange
  and shift.
- [Part 3](/tutorials/change-detection/comparing-maps/): comparing a map with a reference — quantity and
  allocation disagreement, several resolutions, kappa and the estimation of areas.
- [Part 4](/tutorials/change-detection/forest-gain-and-loss/): the change of a forest seen by GuidosToolbox.
- The template **Land change analysis** (menu **Templates → Land change**) runs the transitions, components and
  intensity analysis together on three dates of a landscape drawn by chance.
- Operators: [[change.intensity]], [[change.transitions]], [[change.count]].

## References

- Aldwaik, S. Z., & Pontius, R. G., Jr. (2012). Intensity analysis to unify measurements of size and stationarity of land changes by interval, category, and transition. *Landscape and Urban Planning*, 106(1), 103–114. https://doi.org/10.1016/j.landurbplan.2012.02.010
- Aldwaik, S. Z., & Pontius, R. G., Jr. (2013). Map errors that could account for deviations from a uniform intensity of land change. *International Journal of Geographical Information Science*, 27(9), 1717–1739. https://doi.org/10.1080/13658816.2013.787618
- Exavier, R., & Zeilhofer, P. (2020). OpenLand: software for quantitative analysis and visualization of land use and cover change. *The R Journal*, 12(2), 359. https://doi.org/10.32614/RJ-2021-021
- Pontius, R. G., Jr., Shusas, E., & McEachern, M. (2004). Detecting important categorical land changes while accounting for persistence. *Agriculture, Ecosystems & Environment*, 101(2–3), 251–268. https://doi.org/10.1016/j.agee.2003.09.008
- Teixeira, Z., Marques, J. C., & Pontius, R. G., Jr. (2016). Evidence for deviations from uniform changes in a Portuguese watershed illustrated by CORINE maps: an intensity analysis approach. *Ecological Indicators*, 66, 382–390. https://doi.org/10.1016/j.ecolind.2016.01.018
