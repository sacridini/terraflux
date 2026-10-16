---
title: Neutral landscapes in a study
short: In a study
description: "Use neutral landscapes as an experiment (habitat amount crossed with fragmentation, with replicates) to see how landscape metrics respond, and as a null model to test whether a real forest is more aggregated than chance; with seeds, batches of replicates and a neutral copy of a real map."
order: 20
level: intermediate
time: 50 min
template: More neutral landscapes
ops: [nlm.fbm, nlm.binarize, nlm.classify, nlm.gaussian_field, nlm.random, metrics.signatures, metrics.class, metrics.landscape, raster.reclassify]
series: Neutral landscapes
part: 2
---

[Part 1](/tutorials/neutral-landscapes/neutral-landscape-models/) showed what each neutral landscape model draws.
This part puts them to work in the two ways they are used in research. First as an **experiment**: landscapes in
which the amount of habitat and its fragmentation vary independently, each combination repeated with ten seeds, to
see how eight common landscape metrics respond, which is impossible with real landscapes, where the two go together.
Then as a **null model**: is the evergreen forest of Augusta, Georgia, more aggregated than chance would make a forest
of the same amount? At the end you will be able to design a neutral landscape experiment, run its replicates as a
batch, and test a real pattern against a neutral expectation.

## The idea

### Amount and fragmentation, untangled

In real landscapes, the amount of habitat and its fragmentation are correlated: landscapes with little forest
usually have it in many small pieces. A study of real landscapes that finds fewer species where the forest is
fragmented cannot tell whether fragmentation or the loss of forest is to blame. Fahrig (2003) reviewed the
evidence and concluded that the effects of habitat loss are large and consistent, while those of fragmentation *per
se*, at a constant amount, are weaker and of either sign. To measure fragmentation at a constant amount, one needs
landscapes where the two vary independently, and neutral landscapes make them to order (With & Crist, 1995; With,
1997).

The same design answers a question about the tools themselves: does a landscape metric measure fragmentation, or
does it also change with the amount? A metric that changes with the amount alone confounds the two, and Neel et al.
(2004) and Wang et al. (2014) found that many do.

### Replicates and seeds

A neutral landscape is one draw. Its metrics vary from seed to seed, and a conclusion about a *kind* of landscape
needs several draws: replicates, each with its own seed. Their mean estimates the expected value; their spread, how
much a single landscape can differ by chance. Ten replicates are enough to see a trend; a test at the 5% level needs
at least 19 (the observed value against 19 neutral ones is one chance in 20 of being the most extreme by chance).

!!! science "Key idea: a test against a neutral model"
    Draw \(n\) neutral landscapes with the same amount of habitat as the real one, measure the same metric on each,
    and count how many are as extreme as the real value, \(b\). The *p*-value is

    \[ p = \frac{b + 1}{n + 1} \]

    With \(n = 19\) and none as extreme, \(p = 0.05\); with \(n = 999\), as small as 0.001. The test says whether the
    real pattern could come from *that* neutral model; choosing the model is choosing the hypothesis (Gardner et al.,
    1987; With, 1997).

## When to use it

- **A factorial experiment** before a field study, to know which metrics will separate amount from fragmentation in
  your range of landscapes; or as the landscapes of a simulation (dispersal, a population model) whose response to
  amount and fragmentation you want.
- **A null model** to ask whether a pattern (aggregation, connectivity, the shape of patches) is more than chance,
  and how much structure a neutral model needs to reproduce it.
- **A neutral copy of a real map** to test whether a model that uses the map (a land change model, a prioritization)
  depends on its spatial structure or only on its composition.
- **Not** to replace real landscapes in predictions about a place.

## What you need

Only Terraflux for the experiment. For the null model, a real map: we use
[`augusta.tif`](/data/tutorials/landscape-metrics/what-are-landscape-metrics/augusta.tif), the land cover of the
[landscape metrics](/tutorials/landscape-metrics/what-are-landscape-metrics/) tutorials. In the application, the
template **Templates > Surfaces and uncertainty > More neutral landscapes** has [[nlm.binarize]] and
[[nlm.classify]] on neutral surfaces.

## An experiment: amount by fragmentation

### The design

Three levels of fragmentation, made by fractal surfaces ([[nlm.fbm]]) of three smoothness values, `dimension` 0.5
(*fragmented*), 1.0 (*intermediate*) and 1.5 (*aggregated*); nine amounts of habitat, 10 to 90%, made by cutting
each surface with [[nlm.binarize]]; ten replicates (seeds 1 to 10). In all, 3 × 9 × 10 = 270 landscapes of 200 ×
200 cells of 30 m (6 × 6 km), each measured by eight class metrics of the habitat.

![The design](/img/tutorials/neutral-landscapes/neutral-landscapes-in-a-study/design.png "Nine of the 27 combinations (seed 1): rows are the three levels of fragmentation, columns 20, 50 and 80% habitat. Along a row, the amount changes and the arrangement stays alike; down a column, the arrangement changes and the amount stays exactly the same.")

### Build the model

```tfgraph
#! caption: Three fractal surfaces, each cut at nine amounts of habitat, and the class metrics of the habitat of every cut.
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "fragmented"
op = "nlm.fbm"
params = { rows = 200, columns = 200, resolution = 30, dimension = 0.5, seed = 1 }

[[nodes]]
id = "intermediate"
op = "nlm.fbm"
params = { rows = 200, columns = 200, resolution = 30, dimension = 1.0, seed = 1 }

[[nodes]]
id = "aggregated"
op = "nlm.fbm"
params = { rows = 200, columns = 200, resolution = 30, dimension = 1.5, seed = 1 }

[[nodes]]
id = "cut_f"
op = "nlm.binarize"
inputs = { map = "fragmented.map" }
params = { breaks = [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9] }
note = "Nine maps: a stack."

[[nodes]]
id = "cut_i"
op = "nlm.binarize"
inputs = { map = "intermediate.map" }
params = { breaks = [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9] }

[[nodes]]
id = "cut_a"
op = "nlm.binarize"
inputs = { map = "aggregated.map" }
params = { breaks = [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9] }

[[nodes]]
id = "metrics_f"
op = "metrics.signatures"
inputs = { maps = "cut_f.map" }
params = { class_metrics = ["pland", "np", "ed", "lpi", "ai", "cohesion", "enn_mn", "area_mn"], classes = [2] }
note = "A row per amount."

[[nodes]]
id = "metrics_i"
op = "metrics.signatures"
inputs = { maps = "cut_i.map" }
params = { class_metrics = ["pland", "np", "ed", "lpi", "ai", "cohesion", "enn_mn", "area_mn"], classes = [2] }

[[nodes]]
id = "metrics_a"
op = "metrics.signatures"
inputs = { maps = "cut_a.map" }
params = { class_metrics = ["pland", "np", "ed", "lpi", "ai", "cohesion", "enn_mn", "area_mn"], classes = [2] }
```

How the pieces connect, and why:

- **Nodes 1 to 3** draw the surfaces. Only `dimension` differs; the `seed` is the replicate. `resolution = 30` gives
  the cells a size, so that areas are in hectares and distances in metres.
- **Nodes 4 to 6**: [[nlm.binarize]] with a *list* of `breaks` gives a stack of nine maps, one per amount, all cut
  from the same surface: the arrangement is the same at every amount, only the threshold moves. Habitat is 2, the
  matrix 1.
- **Nodes 7 to 9**: [[metrics.signatures]] with a stack on `maps` gives a row per layer, and with `class_metrics`
  and `classes = [2]` a column per metric of the habitat (`np_2`, `ed_2`...). One node measures all nine amounts.

Running the model gives one replicate. For ten, run it as a **batch**, changing the seed of the three surfaces
together:

```bash
terraflux submit factorial.toml --replicates 10 --seed-input fragmented.seed,intermediate.seed,aggregated.seed
```

`--replicates 10` runs the model ten times with seeds 1 to 10 (`--first-seed` changes the start), each in a folder
of its own under `batch/`, in parallel on this machine's cores or on the machines of a cluster
([parallel computing](/parallel/)); `--seed-input` names the settings that take the seed. Make the three tables
outputs of the model so that each run writes them. In the application, the **Many runs** pane does the same.

### Read the results

The share of habitat is exactly 10, 20... 90% in every landscape: the amount is controlled. The other metrics:

```tfchart
type: line
data: /data/tutorials/neutral-landscapes/neutral-landscapes-in-a-study/factorial_np.csv
x: habitat
y: [fragmented, intermediate, aggregated]
colors: { fragmented: "#d55e00", intermediate: "#e69f00", aggregated: "#3b8a5a" }
xlabel: "Habitat (% of the landscape)"
ylabel: Number of patches
caption: "The number of habitat patches, mean of 10 replicates. It depends on fragmentation (fragmented landscapes have 2 to 5 times as many patches at low amounts) but just as much on the amount: it peaks at 20-30% and falls to almost nothing at 90%, where all the habitat has merged. A landscape with more patches is not necessarily more fragmented."
```

```tfchart
type: line
data: /data/tutorials/neutral-landscapes/neutral-landscapes-in-a-study/factorial_ed.csv
x: habitat
y: [fragmented, intermediate, aggregated]
colors: { fragmented: "#d55e00", intermediate: "#e69f00", aggregated: "#3b8a5a" }
xlabel: "Habitat (% of the landscape)"
ylabel: "Edge density (m/ha)"
caption: "Edge density, mean of 10 replicates. It separates the three levels of fragmentation clearly at every amount, but it is hump-shaped in the amount, peaking at 50%: the same edge density means a fragmented landscape at 15% habitat and an intermediate one at 45%."
```

```tfchart
type: line
data: /data/tutorials/neutral-landscapes/neutral-landscapes-in-a-study/factorial_lpi.csv
x: habitat
y: [fragmented, intermediate, aggregated]
colors: { fragmented: "#d55e00", intermediate: "#e69f00", aggregated: "#3b8a5a" }
xlabel: "Habitat (% of the landscape)"
ylabel: "Largest patch (% of the landscape)"
caption: "The largest patch index, mean of 10 replicates: a percolation curve. Below 40% habitat the aggregated landscapes have larger patches; between 50 and 60% all three cross the threshold, and above 60% the largest patch is almost all the habitat whatever the fragmentation. LPI measures fragmentation only at low amounts."
```

```tfchart
type: line
data: /data/tutorials/neutral-landscapes/neutral-landscapes-in-a-study/factorial_ai.csv
x: habitat
y: [fragmented, intermediate, aggregated]
colors: { fragmented: "#d55e00", intermediate: "#e69f00", aggregated: "#3b8a5a" }
xlabel: "Habitat (% of the landscape)"
ylabel: "Aggregation index (%)"
caption: "The aggregation index, mean of 10 replicates. It separates the three levels at every amount, in the same order; it also rises with the amount, but monotonically, so a comparison at a similar amount is easy to make."
```

How to read the four charts together:

- **No metric is a pure measure of fragmentation.** Every one changes with the amount of habitat, and some (number
  of patches, edge density) in a hump that makes the same value mean different things at different amounts.
- **The aggregation index is the most faithful** of the four here: it orders the levels of fragmentation the same way
  at every amount. Edge density does too, but with its hump. The largest patch only below the percolation threshold.
- **Report the amount and model it.** In a study of real landscapes, include `pland` as a covariate and test the
  configuration metric given the amount; better, choose landscapes in which the two vary independently (Pasher et al.,
  2013).

The spread between replicates matters too:

```tfchart
type: scatter
data: /data/tutorials/neutral-landscapes/neutral-landscapes-in-a-study/replicates_np.csv
x: habitat
y: [fragmented, intermediate, aggregated]
colors: { fragmented: "#d55e00", intermediate: "#e69f00", aggregated: "#3b8a5a" }
xlabel: "Habitat (% of the landscape; the replicates spread a little apart)"
ylabel: Number of patches
caption: "The number of patches of every one of the 270 landscapes. At 20% habitat, the ten fragmented replicates range from 1,193 to 1,368 patches, the aggregated ones from 214 to 346: a single landscape can be far from the mean. The replicates of the aggregated level vary most, relative to their mean, because they have fewer, larger patches."
```

## A null model: is the forest aggregated?

### The real pattern

The evergreen forest covers 37.5% of Augusta's map, in 139 patches with an aggregation index of 77.4% (part 2 of the
landscape metrics). Is that aggregated? Compared with what? Three neutral models give three answers, each a
different hypothesis:

- **Random** ([[nlm.random]]): the forest's cells were placed independently. The weakest null: almost any real
  pattern beats it.
- **Fractal, rough** ([[nlm.fbm]], `dimension = 1.0`): the forest follows an autocorrelated but rough environment.
- **Fractal, smooth** ([[nlm.fbm]], `dimension = 1.5`): it follows a smooth environment, large hills and valleys.

Each is drawn on the grid of the real map, cut to the forest's exact share, and measured the same way, 19 times.

### Build the model

```tfgraph
#! caption: The real forest's metrics, and those of a neutral forest of the same amount on the same grid (one replicate).
#! numbered: true
#! highlight: neutral, cut
format = "terraflux/1"

[[nodes]]
id = "cover"
op = "raster.read"
params = { path = "augusta.tif" }

[[nodes]]
id = "forest"
op = "raster.reclassify"
inputs = { map = "cover.raster" }
params = { rules = "42 -> 2\n* -> 1" }
note = "Evergreen forest (2) and the rest (1)."

[[nodes]]
id = "observed"
op = "metrics.class"
inputs = { map = "forest.map" }
params = { metrics = ["pland", "np", "ed", "ai", "lpi"], show = "ai" }

[[nodes]]
id = "neutral"
op = "nlm.fbm"
inputs = { like = "cover.raster" }
params = { dimension = 1.5, seed = 1 }
note = "On the real map's grid."

[[nodes]]
id = "cut"
op = "nlm.binarize"
inputs = { map = "neutral.map" }
params = { breaks = 0.3754 }
note = "The forest's share, 37.54%."

[[nodes]]
id = "expected"
op = "metrics.class"
inputs = { map = "cut.map" }
params = { metrics = ["pland", "np", "ed", "ai", "lpi"], show = "ai" }
```

Two connections make the comparison fair. Node 4 takes the real map on its input `like`: the neutral landscape is
drawn on the same grid (rows, columns, cell size), so areas and distances are comparable. Node 5 cuts it at the
forest's share, 0.3754, read from node 3's table: the amount is the same, and only the arrangement differs. The
replicates are again a batch, `--seed-input neutral.seed`.

### Read the results

```tfgallery
columns: 4
caption: "The real evergreen forest of Augusta (left), and one replicate of each neutral model with exactly the same amount (37.5%) on the same grid: random, fractal rough (dimension 1.0), fractal smooth (dimension 1.5)."
images:
  - { src: /img/tutorials/neutral-landscapes/neutral-landscapes-in-a-study/evergreen.png, caption: "Observed" }
  - { src: /img/tutorials/neutral-landscapes/neutral-landscapes-in-a-study/null_random.png, caption: "Random" }
  - { src: /img/tutorials/neutral-landscapes/neutral-landscapes-in-a-study/null_fbm10.png, caption: "Fractal, rough" }
  - { src: /img/tutorials/neutral-landscapes/neutral-landscapes-in-a-study/null_fbm15.png, caption: "Fractal, smooth" }
```

![The real forest against 19 neutral forests of each model](/img/tutorials/neutral-landscapes/neutral-landscapes-in-a-study/null_test.png "The aggregation index (left) and the number of patches (right, a log scale) of 19 replicates of each neutral model (dots), and of the real forest (the orange line).")

| Model | Aggregation index (19 replicates) | Patches (19 replicates) | The real forest (AI 77.4%, 139 patches) is... |
|---|---|---|---|
| Random | 37.0–38.2% | 455–526 | far more aggregated: \(p = 0.05\) |
| Fractal, rough | 70.5–74.7% | 244–325 | more aggregated than all 19: \(p = 0.05\) |
| Fractal, smooth | 82.1–86.9% | 101–208 | less aggregated than all 19, within their number of patches |

The real forest is much more aggregated than random, and more than a forest following a rough environment; it is
less aggregated than a forest following a smooth one, but has a similar number of patches. Its structure lies
between the two fractal models, nearer the smooth one. With 19 replicates the smallest possible *p*-value is 0.05; for
a paper, run 99 or 999.

What does that tell an ecologist? That the forest's aggregation is not chance, which few would doubt, and, more
usefully, *how much* structure a model of it needs. A simulation of this landscape (a land change model, a species
spreading through it) run on rough fractal forests would make the forest too scattered; on smooth ones, slightly too
compact.

!!! paper "In the literature"
    Gardner et al. (1987) proposed to compare the patterns of real landscapes with those of random maps of the same
    amounts of habitat: the first neutral landscape models. With and Crist (1995) used fractal neutral
    landscapes to show that species' responses to habitat loss have thresholds that depend on how the species
    moves. Remmel and Fortin (2013) drew many neutral landscapes with a map's
    estimated autocorrelation to tell whether two maps' patterns differ beyond chance.

## A neutral copy of a real map

[[nlm.classify]] with a real map on its input `real` cuts a neutral surface into the classes of the real map with
their exact shares. The copy has the real composition and a neutral configuration: a reference to test what a
model's result owes to the real arrangement.

```tfgraph
#! caption: A Gaussian field on Augusta's grid, cut into its 15 classes with their real shares.
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "cover"
op = "raster.read"
params = { path = "augusta.tif" }

[[nodes]]
id = "field"
op = "nlm.gaussian_field"
inputs = { like = "cover.raster" }
params = { range = 8, seed = 7 }

[[nodes]]
id = "neutral"
op = "nlm.classify"
inputs = { map = "field.map", real = "cover.raster" }
note = "The real map's classes and shares."

[[nodes]]
id = "real_metrics"
op = "metrics.landscape"
inputs = { map = "cover.raster" }
params = { metrics = ["np", "ed", "shdi", "contag"] }

[[nodes]]
id = "neutral_metrics"
op = "metrics.landscape"
inputs = { map = "neutral.classes" }
params = { metrics = ["np", "ed", "shdi", "contag"] }
```

```tfcompare
caption: "Augusta's land cover (left of the handle) and its neutral copy (right): the same 15 classes in the same shares, arranged by a Gaussian field of range 8 cells. The classes of the copy are ordered along the field's values (the classes' codes in order), so neighbouring codes meet more often than in reality."
before: { src: /img/tutorials/neutral-landscapes/neutral-landscapes-in-a-study/real_augusta.png, label: Real }
after: { src: /img/tutorials/neutral-landscapes/neutral-landscapes-in-a-study/neutral_augusta.png, label: Neutral copy }
```

| | Real | Neutral copy |
|---|---|---|
| Shannon's diversity | 1.901 | 1.901 |
| Patches | 1,144 | 3,512 |
| Edge density (m/ha) | 212 | 373 |
| Contagion (%) | 43.9 | 39.0 |

Diversity is identical (the composition is), and every configuration metric differs: the real landscape has three
times fewer patches and much less edge. A copy cut from a smoother field (a larger `range`) would come closer; trying
ranges until the copy matches one metric is a simple way to fit a neutral model to a real landscape.

## Settings that change the answer

- **The neutral model**, above: it is the hypothesis. Report which, with its settings, and prefer more than one.
- **The number of replicates**: 10 to see a trend, 19 for a test at 5%, 99 or 999 for a paper.
- **The grid**: draw on the real map's grid (`like`) so that areas, distances and the map's border are the same.
- **The amount**: cut at the real share with [[nlm.binarize]], exactly.
- **The neighbour rule** of the metrics, the same for the real and neutral maps.

## Common pitfalls

- **A null model that is too weak.** Beating the random model is easy and says little. Test against models with
  structure.
- **One replicate.** See the spread of the 270 landscapes above.
- **The same seed for every replicate** (a batch without `--seed-input` on the right settings): ten copies of one
  landscape.
- **Neutral maps on a different grid** from the real one: metrics in hectares or metres no longer compare.
- **Ignoring the map's border.** A real map is cut from a larger landscape; a neutral one is not. Large patches cut
  by the border look smaller; measure both the same way, or on an inner area.

## Going further

- [Landscape metrics](/tutorials/landscape-metrics/): every metric used here.
- [Fragmentation](/tutorials/fragmentation/fragmentation-and-habitat-loss/): habitat loss and fragmentation per se,
  measured around every cell.
- [Parallel computing](/parallel/): batches of replicates on many cores and machines.
- The operator reference: [[nlm.fbm]], [[nlm.binarize]], [[nlm.classify]], [[metrics.signatures]].

## References

- Fahrig, L. (2003). Effects of habitat fragmentation on biodiversity. *Annual Review of Ecology, Evolution, and Systematics*, 34, 487–515. https://doi.org/10.1146/annurev.ecolsys.34.011802.132419
- Gardner, R. H., Milne, B. T., Turner, M. G., & O'Neill, R. V. (1987). Neutral models for the analysis of broad-scale landscape pattern. *Landscape Ecology*, 1(1), 19–28. https://doi.org/10.1007/BF02275262
- Neel, M. C., McGarigal, K., & Cushman, S. A. (2004). Behavior of class-level landscape metrics across gradients of class aggregation and area. *Landscape Ecology*, 19(4), 435–455. https://doi.org/10.1023/B:LAND.0000030521.19856.cb
- Pasher, J., Mitchell, S. W., King, D. J., Fahrig, L., Smith, A. C., & Lindsay, K. E. (2013). Optimizing landscape selection for estimating relative effects of landscape variables on ecological responses. *Landscape Ecology*, 28(3), 371–383. https://doi.org/10.1007/s10980-013-9852-6
- Remmel, T. K., & Fortin, M.-J. (2013). Categorical, class-focused map patterns: characterization and comparison. *Landscape Ecology*, 28(8), 1587–1599. https://doi.org/10.1007/s10980-013-9905-x
- Wang, X., Blanchet, F. G., & Koper, N. (2014). Measuring habitat fragmentation: an evaluation of landscape pattern metrics. *Methods in Ecology and Evolution*, 5(7), 634–646. https://doi.org/10.1111/2041-210X.12198
- With, K. A. (1997). The application of neutral landscape models in conservation biology. *Conservation Biology*, 11(5), 1069–1080. https://doi.org/10.1046/j.1523-1739.1997.96210.x
- With, K. A., & Crist, T. O. (1995). Critical thresholds in species' responses to landscape structure. *Ecology*, 76(8), 2446–2459. https://doi.org/10.2307/2265819
