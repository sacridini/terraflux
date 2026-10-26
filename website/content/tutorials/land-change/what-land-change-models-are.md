---
title: What land change models are
short: What land change models are
description: The ideas behind every land change simulation (how much changes, where, and in what shape), seen on two real maps of Sibuyan Island.
order: 10
level: beginner
time: 30 min
ops: [change.crosstab, change.components, change.suggest, stack.of, raster.calc, view.plot]
series: "Land change simulation: the foundations"
part: 1
---

A forest becomes a coconut plantation, a pasture is abandoned and grows back, a town spreads over rice fields.
A **land change model** learns from maps of the past how a landscape changed, and uses what it learned to draw
maps of the future: under the trend of the last decades, or under a road, a reserve or a policy that does not
exist yet. In this first part of the series you will look at a real landscape that changed, take a land change
model apart into its few pieces, and learn the words the rest of the series uses: *quantity*, *allocation*,
*transition*, *driver*, *cellular automaton*, *calibration* and *validation*. At the end you will have a small
Terraflux model that tells you, for two maps of the same place, what changed into what.

## Why simulate land change?

Changes in land use and land cover are, with climate change, the main driver of the loss of biodiversity on land.
For an ecologist, the maps a land change model draws are rarely the end of the story: they are the input of the
questions you already ask about a landscape. How much habitat will a species have in 2040? Will the corridor
between two reserves still exist? How fragmented will the forest be, and how much carbon will it hold? A
simulation lets you ask these questions about landscapes that do not exist yet, and compare them: the same
region with and without a new road, with the forest code enforced or ignored.

A famous example is the simulation of the Amazon basin by Soares-Filho et al. (2006), made with DINAMICA, the
model whose method Terraflux reproduces in the second series. Under a business-as-usual scenario, the expansion of
agriculture would remove about 40% of the Amazon forests by 2050; under a scenario of better governance, with
protected areas and enforced laws, much less. The paper did not predict the future: it showed what was at stake
in a decision, in maps that people could look at.

Why land changes is a question of its own. Lambin et al. (2001) reviewed the case studies of their time and
dismantled the simple explanations ("population growth causes deforestation", "shifting cultivators destroy the
forest"): land changes because people respond to economic opportunities, mediated by institutions, markets and
policies, in ways that differ from place to place. A land change model does not need to explain all of that.
Most models, including the ones in this series, are **inductive**: they learn *where* change happened from the
map and from a set of *drivers* (distance to roads, slope, soils...) and assume that the same associations hold
for some time to come. Keep that assumption in mind: it is the main limit of every map such a model draws.

## A landscape that changed: Sibuyan Island

Sibuyan is an island in the Philippines, about 30 km across, with one of the best preserved forests of the
country on its mountains. Its land use was mapped in 1997 and in 2011 with five classes: forest, coconut
plantations, grassland, rice fields and other uses. The maps come with 13 explanatory variables (the distances to
the sea, roads, towns and streams, elevation, slope, aspect, population density, and the soils). This is the
classic example of the CLUE-S model (Verburg & Veldkamp, 2004) and of the R package lulcc (Moulds et al., 2015),
and it is the example of this whole series.

The maps are *rasters*: grids of square *cells*, here of 250 m (6.25 ha), each holding one value. In the land use
map the value is a class code (1 forest, 2 coconut, 3 grassland, 4 rice, 5 other; 255 outside the island, the
*no data* value). The two dates are two *bands* of one file, named after their years. Download them to follow
along: [sibuyan_landuse.tif](/data/tutorials/land-change/what-land-change-models-are/sibuyan_landuse.tif) (the land
use, 1997 and 2011) and [sibuyan_drivers.tif](/data/tutorials/land-change/what-land-change-models-are/sibuyan_drivers.tif)
(the drivers, a band each). They are also in Terraflux's source, in `tests/data/inputs`.

```tfgallery
columns: 2
caption: Sibuyan's land use in 1997 and 2011, with the share of the island in each class. Forest covered 65% of the island in 1997 and 58% in 2011.
images:
  - { src: /img/tutorials/land-change/what-land-change-models-are/sibuyan_1997.png, caption: "1997" }
  - { src: /img/tutorials/land-change/what-land-change-models-are/sibuyan_2011.png, caption: "2011" }
```

At a glance the two maps look alike, and that is the first lesson of land change: **most of a landscape does not
change**, even in a decade of fast change. Here 6,483 of the 7,226 cells of the island (89.7%) kept their class.
What changed shows best on a map of its own, where every cell that changed takes the colour of its new class:

![The cells of Sibuyan that changed between 1997 and 2011](/img/tutorials/land-change/what-land-change-models-are/change_1997_2011.png "The 743 cells that changed between 1997 and 2011, coloured by their new class. Change gathers at the edges of the forest near the coast, in patches, not in scattered cells.")

Two more lessons are in this map. Change is **not random in space**: it gathers at the edge of the forest, near
the coast and the roads, and not on the mountains. And change comes in **patches**: blocks of several cells of
coconut or grassland, often next to an existing plantation. A model of Sibuyan has to reproduce both: the
*where* and the *shape*.

### The transition matrix

The table that summarises a change between two dates is the **transition matrix** (or cross-tabulation): the
cells of each class at the first date (rows) that hold each class at the second (columns). The diagonal is what
persisted; everything off the diagonal is a **transition**, a change from one class to another.

| 1997 \ 2011 | Forest | Coconut | Grassland | Rice | Other | Total 1997 |
|---|---:|---:|---:|---:|---:|---:|
| **Forest** | ==4,160== | 126 | 320 | 116 | 1 | 4,723 |
| **Coconut** | 0 | ==1,140== | 18 | 0 | 0 | 1,158 |
| **Grassland** | 0 | 77 | ==745== | 16 | 1 | 839 |
| **Rice** | 0 | 61 | 7 | ==156== | 0 | 224 |
| **Other** | 0 | 0 | 0 | 0 | ==282== | 282 |
| **Total 2011** | 4,160 | 1,404 | 1,090 | 288 | 284 | 7,226 |

Read it row by row. Of the 4,723 cells of forest in 1997, 4,160 were still forest in 2011, 320 had become
grassland, 126 coconut and 116 rice: forest lost 563 cells (35 km², 12% of its area) and gained none. Coconut
gained from forest, grassland and rice. Rice lost a quarter of its cells to coconut (61 of 224) while it gained
116 from forest: its net change is small, but a lot moved. A model of Sibuyan is, at its core, a model of these
transitions: how many cells of each, and which cells.

```tfchart
type: bar
data: /data/tutorials/land-change/what-land-change-models-are/gains_losses.csv
x: class
y: [gain, loss, net]
labels: { gain: "Gain", loss: "Loss", net: "Net change" }
colors: { gain: "#3b8a5a", loss: "#c8553d", net: "#48534d" }
ylabel: "% of the island"
height: 280
caption: The gains and losses of each class between 1997 and 2011, in % of the island. A small net change can hide a large exchange (rice).
```

## The anatomy of a land change model

There are dozens of land change models, written by different communities for different questions, but almost
all of them answer the same few questions, and the clearest way to read one is to ask how it answers each
(Verburg et al., 2004):

![The pieces of a land change model and how they connect](/img/tutorials/land-change/what-land-change-models-are/anatomy.png "The anatomy of a land change model: from the map at one time step, how much changes (quantity) and where it is likely (probability), placed on the map by the allocation; the new map starts the next step.")

!!! science "Key idea: quantity and allocation"
    A land change model separates **how much** changes from **where** it changes.

    - The **quantity** (or *demand*): how many cells make each transition at each step. It usually comes from
      outside the map: the rates of the observed period (a *Markov chain*, part 3 of this series), an economic
      model, a policy target, a storyline of a scenario.
    - The **allocation**: which cells change. It has two parts: the **probability** (or *suitability*,
      *transition potential*) of each transition in each cell, learned from where change happened and the
      drivers there; and the **rules that place** the quantity on the map: one cell at a time, by competition
      between classes, or in patches that grow or start anew.

    Around them sit **rules**: transitions that cannot happen (a town does not become primary forest in ten
    years), areas where nothing may change (a reserve), the resistance of each class to change.

Separating the two is what makes models useful for scenarios. The quantity is the most uncertain part and the
one policy changes most (how much forest will be cleared depends on prices and enforcement); the allocation
depends on the geography (roads, slopes, soils) and changes slowly. So a study typically keeps the allocation
fitted on the past and varies the quantity between scenarios, and then the scenarios differ only in what they
are meant to differ in. The separation is also what makes models checkable: a simulation can have the right
amount of change in the wrong places, or the wrong amount in the right places, and the two errors call for
different fixes (the third series measures them separately).

The families of models differ mostly in how they make the allocation:

| Family | Quantity | Probability | Placing the change | In this site |
|---|---|---|---|---|
| Transition-based cellular automata (DINAMICA; Soares-Filho et al., 2002) | transition rates per step (Markov chain) | weights of evidence per transition | new patches and the expansion of existing ones | the second series |
| Competition between classes (CLUE-S; Verburg et al., 2002) | area of each class per year | a logistic regression per class | each cell to the class of highest suitability, adjusted until the areas match | the CLUE-S tutorials |
| Patch-generating (PLUS's CARS), patch growing (FUTURES) | area per class, or population | random forests, mixed-effects regressions | patches from random seeds, of observed sizes | their own tutorials |
| Agent-based | from the decisions of agents | the agents' preferences | the agents' choices | the agents tutorials |

The [National Research Council (2014)](https://doi.org/10.17226/18385) report on land change modelling groups the
approaches in a similar way (machine learning and statistical, cellular, sector-based and spatially
disaggregated economic, agent-based, and hybrids) and stresses that the choice depends on the question: a
cellular model is good at *where* and *how much*, poor at *why*.

## Cellular automata

Most of the models above are **cellular automata** (CA). A cellular automaton has four ingredients:

- a grid of **cells**, each in one **state** (here: a land use class);
- a **neighbourhood**: the cells around each cell that influence it, usually the 8 cells of a 3 × 3 window;
- **transition rules**: how a cell's state at the next time step depends on its state, its neighbours' and the
  conditions at the cell (a driver);
- discrete **time steps**: every step applies the rules to the map of the step before.

Cellular automata came from mathematics and physics, where very simple rules produce surprisingly complex
patterns. White and Engelen (1993) showed that a CA with a few rules about the neighbourhood of each cell could
grow cities with the fractal shapes of real ones; Clarke et al. (1997) calibrated one (later called SLEUTH) on a
century of urban growth in the San Francisco Bay Area. DINAMICA (Soares-Filho et al., 2002) is a
**stochastic** CA designed for an Amazonian colonisation frontier: its rules are probabilities, not certainties,
and its transitions are drawn at random in proportion to them, so two runs with different random seeds give two
different (but equally plausible) maps.

The neighbourhood is what gives a CA its realism. Deforestation spreads from the edges of clearings; cities grow
next to cities; abandoned fields regrow next to the forest that supplies the seeds. A rule like "a cell of
forest is more likely to be cleared when some of its neighbours are already cleared" produces patches that grow,
as in the map of Sibuyan above. In the [next part](/tutorials/land-change/a-cellular-automaton-by-hand/) you will
write such a rule yourself, in a loop of map algebra.

## Calibration, validation and projection

A land change model is fitted on the past and run into the future. Three periods should be kept apart:

![Calibration, validation and projection periods along a time line](/img/tutorials/land-change/what-land-change-models-are/periods.png "With three maps, the first period calibrates the model, the second tests it, and the projection starts from the last map.")

- **Calibration**: the period whose change the model learns: its rates, the weights of its drivers, the sizes
  of its patches. Everything a fit measures comes from these two maps.
- **Validation**: a later period, whose change the model has not seen. The model, calibrated on the first
  period, simulates the second from its first map, and the simulated map is compared with the observed one.
- **Projection**: the model runs from the latest map into the future, under the trend or under scenarios.

With only two maps, as for Sibuyan, the validation can only replay the calibration period: the model simulates
1997 to 2011 from 1997 and is compared with the map of 2011 it was fitted on. That measures how well the model
*fits*, not how well it *predicts*, and should be reported as such (Pontius et al., 2004). The third series
does both, on Sibuyan and on a landscape with three dates.

!!! warning "The null model is persistence"
    Because most of a landscape does not change, a "model" that changes nothing at all would agree with Sibuyan's
    map of 2011 in 89.7% of the cells. A real model that places the right amount of change in slightly wrong
    places can agree *less* than that. This is why land change models are judged on the change itself (the
    figure of merit, the fuzzy similarity), never on the share of cells right (Pontius et al., 2004, 2008).

## When to use a land change model

Use one when your question is about a landscape that does not exist yet, or about the spatial consequences of a
decision:

- the habitat, fragmentation or connectivity of a species under scenarios of land use (the simulated maps are the
  input of landscape metrics, MSPA or connectivity analyses);
- the carbon emissions of deforestation scenarios, or the effect of a protected area or a road (Soares-Filho
  et al., 2006);
- where restoration or conservation would be most threatened, to prioritise it.

A land change model is **not** a forecast. It tells you where and how change would happen *given* a quantity and
the associations of the past; the quantity is a scenario, not a prediction. If your question is about the past
(what changed, how fast, which classes gained), you do not need a simulation: the tutorials on
[change detection](/tutorials/change-detection/) answer it with the transition matrix, the components of change
and the intensity analysis. And if the processes you care about are decisions of people, a cellular model can
only mimic their outcome; agent-based models represent the decisions themselves.

## What you need

- **Two maps of land use (or more)** of the same place, with the **same classes** and on the **same grid**
  (the same size, cells and coordinate system). The classes should be few (5 to 10) and meaningful for the
  question; more classes mean more transitions, each with fewer cells to learn from.
- **Drivers**: maps of the conditions that explain where change happens (distances to roads, rivers, towns and
  to the classes themselves; elevation, slope, soils; protected areas; population). On the same grid.
- **The time between the maps**, in the time steps you want to simulate (years, for Sibuyan: 14 steps from 1997
  to 2011).
- Ideally, a **third map** to validate the model on a period it has not seen.

## Look at the change in Terraflux

Before building a model, look at the change. The model below reads the two dates of Sibuyan, computes the
transition matrix, the components of the change and the transitions worth modelling, and makes the map of the
cells that changed.

```tfgraph
#! caption: What changed on Sibuyan between 1997 and 2011: the transition matrix, the components of change, the transitions worth modelling and a map of the cells that changed.
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "land_1997"
op = "raster.read"
params = { path = "sibuyan_landuse.tif", band = "1997" }
note = "The land use in 1997: 1 forest, 2 coconut, 3 grassland, 4 rice, 5 other."

[[nodes]]
id = "land_2011"
op = "raster.read"
params = { path = "sibuyan_landuse.tif", band = "2011" }
note = "The same file, its band of 2011."

[[nodes]]
id = "crosstab"
op = "change.crosstab"
inputs = { from = "land_1997.raster", to = "land_2011.raster" }
note = "The transition matrix: the cells of each class in 1997 (rows) in each class in 2011 (columns)."

[[nodes]]
id = "components"
op = "change.components"
inputs = { from = "land_1997.raster", to = "land_2011.raster" }
note = "Gain, persistence and loss of every class (Pontius)."

[[nodes]]
id = "series"
op = "stack.of"
inputs = { y1997 = "land_1997.raster", y2011 = "land_2011.raster" }
note = "The two dates as one stack, for change.suggest."

[[nodes]]
id = "suggest"
op = "change.suggest"
inputs = { maps = "series.stack" }
note = "The transitions worth modelling: those that make at least 1% of the change."

[[nodes]]
id = "changed"
op = "raster.calc"
inputs = { a = "land_1997.raster", b = "land_2011.raster" }
params = { expression = "b if a != b else 0" }
note = "Every cell that changed in its new class, 0 where nothing changed."

[[nodes]]
id = "plot"
op = "view.plot"
inputs = { value = "changed.map" }
params = { title = "What changed, 1997 to 2011" }
```

<div class="steps" markdown>

1. **Read the two dates.** In the application, open `sibuyan_landuse.tif` (++ctrl+o++): it becomes a
   `raster.read` node (node 1). In the Node pane, set its **band** to `1997`. Open the file again for node 2 and
   choose the band `2011`. Each node's `map` output is one band, as a map.
2. **The transition matrix.** With node 1 selected, find [[change.crosstab]] in the library (subject *Change*,
   or press ++slash++ and type `crosstab`); it comes connected to node 1's map as its `from`. Drag from node 2's
   `map` output to its `to` input. Press ++ctrl+enter++: the matrix appears under the canvas, as the table
   above.
3. **Gains and losses.** Add [[change.components]] the same way (node 4): its table gives each class's gain,
   persistence and loss, and splits the overall change into *quantity* (the net change of the classes),
   *exchange* and *shift* (classes trading places).
4. **What is worth modelling.** Put the two maps in one stack with [[stack.of]] (node 5: connect both maps; each
   input's name becomes a layer's name) and add [[change.suggest]] (node 6). It lists the transitions from
   largest to smallest, with the share of all change and of the class each makes.
5. **A map of the change.** [[raster.calc]] (node 7) computes a map cell by cell from an expression; give it two
   inputs named `a` and `b` and the expression `b if a != b else 0`. [[view.plot]] (node 8) shows it in a
   tab of its own.

</div>

## Read the results

The transition matrix is the table above. [[change.suggest]] ranks the transitions; on Sibuyan it gives:

| Transition | Cells | km² | % of all change | % of the class |
|---|---:|---:|---:|---:|
| Forest → grassland | 320 | 20.0 | 43.1 | 6.8 |
| Forest → coconut | 126 | 7.9 | 17.0 | 2.7 |
| Forest → rice | 116 | 7.3 | 15.6 | 2.5 |
| Grassland → coconut | 77 | 4.8 | 10.4 | 9.2 |
| Rice → coconut | 61 | 3.8 | 8.2 | 27.2 |
| Coconut → grassland | 18 | 1.1 | 2.4 | 1.6 |
| Grassland → rice | 16 | 1.0 | 2.2 | 1.9 |
| Rice → grassland | 7 | 0.4 | 0.9 | 3.1 |
| Forest → other | 1 | 0.06 | 0.1 | 0.02 |
| Grassland → other | 1 | 0.06 | 0.1 | 0.1 |

Seven transitions make more than 1% of the change each and 99% of it together; the last three are a handful of
cells. A model cannot learn where a transition of one cell happens: such transitions are better left out (the
weights of evidence in part 1 of the second series fit one set of weights per transition, and a transition of
one cell has nothing to fit). [[change.components]] adds that of the 743 cells that changed, 563 are
**quantity** (forest's net loss, taken by the other classes), and the rest is exchange and shift between
coconut, grassland and rice.

What to report from this step, in a paper: the maps of both dates and their classes, the transition matrix (in
cells or km² and in % of the class), the share of the landscape that changed, and which transitions you model and
why.

## Common pitfalls

- **Classes that differ between dates.** If 1997 calls rice 4 and 2011 calls it 3, the matrix shows a fake
  transition. Check that both maps share a legend before anything else; [[change.crosstab]] lists the classes of
  both.
- **No data mistaken for a class.** Sibuyan uses 255 outside the island. Terraflux reads the no-data value from
  the file; a file that does not declare it shows a class 255 in the matrix (set `nodata` on `raster.read`).
- **Maps on different grids.** Two maps from different sources rarely share their cells exactly. Put them on one
  grid first (the Prepare pane, or `raster.match`), choosing the *nearest* or *mode* resampling for classes.
- **Change that is classification error.** Two independent classifications of the same place disagree in a few
  percent of the cells even when nothing changed. Isolated single cells that change back and forth are often
  noise; a land change model will learn them as if they were change.
- **Units.** The maps of Sibuyan have no coordinate system, only cells of 250 m. Areas in km² come from the cell
  size; when a map is in degrees, cell areas differ with latitude.

## Going further

The rest of this series and the next two build a complete model of Sibuyan:

- [A cellular automaton by hand](/tutorials/land-change/a-cellular-automaton-by-hand/): a loop of map algebra
  where a city grows, to see how a simulation runs step by step.
- [Transition rates with Markov chains](/tutorials/land-change/transition-rates-markov-chains/): how much
  changes per step.
- The second series, **Dinamica EGO in Terraflux**: [weights of evidence](/tutorials/land-change/weights-of-evidence/),
  [Expander and Patcher](/tutorials/land-change/expander-and-patcher/) and
  [the full model on Sibuyan](/tutorials/land-change/dinamica-on-sibuyan/).
- The third series, **Validating and calibrating**: [validation](/tutorials/land-change/validating-a-simulation/),
  [calibration and many runs](/tutorials/land-change/calibration-and-many-runs/) and
  [maps larger than memory](/tutorials/land-change/maps-larger-than-memory/).

## References

- Brown, D. G., Verburg, P. H., Pontius, R. G., Jr., & Lange, M. D. (2013). Opportunities to improve impact, integration, and evaluation of land change models. *Current Opinion in Environmental Sustainability*, 5(5), 452–457. https://doi.org/10.1016/j.cosust.2013.07.012
- Clarke, K. C., Hoppen, S., & Gaydos, L. (1997). A self-modifying cellular automaton model of historical urbanization in the San Francisco Bay area. *Environment and Planning B: Planning and Design*, 24(2), 247–261. https://doi.org/10.1068/b240247
- Lambin, E. F., Turner, B. L., Geist, H. J., Agbola, S. B., Angelsen, A., Bruce, J. W., et al. (2001). The causes of land-use and land-cover change: moving beyond the myths. *Global Environmental Change*, 11(4), 261–269. https://doi.org/10.1016/S0959-3780(01)00007-3
- Moulds, S., Buytaert, W., & Mijic, A. (2015). An open and extensible framework for spatially explicit land use change modelling: the lulcc R package. *Geoscientific Model Development*, 8(10), 3215–3229. https://doi.org/10.5194/gmd-8-3215-2015
- National Research Council (2014). *Advancing Land Change Modeling: Opportunities and Research Requirements*. The National Academies Press. https://doi.org/10.17226/18385
- Pontius, R. G., Jr., Huffaker, D., & Denman, K. (2004). Useful techniques of validation for spatially explicit land-change models. *Ecological Modelling*, 179(4), 445–461. https://doi.org/10.1016/j.ecolmodel.2004.05.010
- Pontius, R. G., Jr., Boersma, W., Castella, J.-C., Clarke, K., de Nijs, T., Dietzel, C., et al. (2008). Comparing the input, output, and validation maps for several models of land change. *The Annals of Regional Science*, 42(1), 11–37. https://doi.org/10.1007/s00168-007-0138-2
- Soares-Filho, B. S., Coutinho Cerqueira, G., & Lopes Pennachin, C. (2002). DINAMICA: a stochastic cellular automata model designed to simulate the landscape dynamics in an Amazonian colonization frontier. *Ecological Modelling*, 154(3), 217–235. https://doi.org/10.1016/S0304-3800(02)00059-5
- Soares-Filho, B. S., Nepstad, D. C., Curran, L. M., Cerqueira, G. C., Garcia, R. A., Ramos, C. A., et al. (2006). Modelling conservation in the Amazon basin. *Nature*, 440(7083), 520–523. https://doi.org/10.1038/nature04389
- Verburg, P. H., Soepboer, W., Veldkamp, A., Limpiada, R., Espaldon, V., & Mastura, S. S. A. (2002). Modeling the spatial dynamics of regional land use: the CLUE-S model. *Environmental Management*, 30(3), 391–405. https://doi.org/10.1007/s00267-002-2630-x
- Verburg, P. H., Schot, P. P., Dijst, M. J., & Veldkamp, A. (2004). Land use change modelling: current practice and research priorities. *GeoJournal*, 61(4), 309–324. https://doi.org/10.1007/s10708-004-4946-y
- Verburg, P. H., & Veldkamp, A. (2004). Projecting land use transitions at forest fringes in the Philippines at two spatial scales. *Landscape Ecology*, 19(1), 77–98. https://doi.org/10.1023/B:LAND.0000018370.57457.58
- White, R., & Engelen, G. (1993). Cellular automata and fractal urban form: a cellular modelling approach to the evolution of urban land-use patterns. *Environment and Planning A*, 25(8), 1175–1199. https://doi.org/10.1068/a251175
