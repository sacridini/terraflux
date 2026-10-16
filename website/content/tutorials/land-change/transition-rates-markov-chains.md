---
title: Transition rates with Markov chains
short: Markov chains
description: How much land changes per time step, learned from two maps as a Markov chain, and why the rates of one year are the root of the period's matrix and not its fraction.
order: 30
level: beginner
time: 35 min
ops: [markov.fit, markov.matrix, raster.read]
series: "Land change simulation: the foundations"
part: 3
---

Every land change model needs a **quantity**: how many cells make each transition at each step. In Dinamica EGO's
model, and in many others, the quantity comes from a **Markov chain** fitted on two maps: the share of each class
that became each other class in the observed period, turned into the share that changes in one time step. In this
part you will fit the Markov chain of Sibuyan between 1997 and 2011 with [[markov.fit]], read its two matrices, see
what it projects for the decades ahead, and give a chain of your own (a scenario) with [[markov.matrix]]. You will
also see why the yearly rates are a *root* of the period's matrix, and what goes wrong when they are taken as a
fraction of it.

## The idea

### A transition matrix as probabilities

In part 1 you met the transition matrix of Sibuyan in cells. Divide each row by its total and it becomes a matrix
of **transition probabilities**: \(P_{ij}\), the probability that a cell of class \(i\) at the start of the period
is of class \(j\) at its end. Each row sums to 1; the diagonal is the probability of staying.

![The transition probabilities of Sibuyan between 1997 and 2011](/img/tutorials/land-change/transition-rates-markov-chains/matrix_period.png "The period's matrix: each row is a class in 1997, each column a class in 2011. A cell of forest in 1997 had a probability of 0.881 to be forest in 2011, 0.068 to be grassland, 0.027 coconut and 0.025 rice.")

A **Markov chain** uses such a matrix to project the areas of the classes forward. If \(a_t\) is the row of the
areas (cells) of each class at time \(t\), the areas one period later are

\[ a_{t+1} = a_t \, P \]

that is, the cells of class \(j\) at \(t+1\) are \(\sum_i a_{t,i} P_{ij}\): what each class keeps and receives.
Bell (1974) was among the first to apply this to land use observed by remote sensing. The chain assumes that the
probabilities depend only on the class a cell has now (not on its history: the *Markov property*) and that they
stay the same from one period to the next (*stationarity*). Both are simplifications; the second is the one that
matters for a projection: the chain repeats the observed period.

### From the period to the time step

The maps of Sibuyan are 14 years apart, and a simulation runs in yearly steps. What are the probabilities of one
year? The tempting answer, dividing the changes by 14, is wrong: in 14 yearly steps the changes compound, and a
cell that becomes grassland in year 3 can become coconut in year 9. The right yearly matrix \(Q\) is the one
whose 14th power gives the period's:

!!! science "Key idea: the rates per step are a matrix root"
    With \(n\) time steps in the observed period,

    \[ Q^n = P \qquad\Longrightarrow\qquad Q = P^{1/n} = \exp\!\left(\frac{1}{n}\,\log P\right) \]

    where \(P\) is the period's matrix, \(Q\) the matrix of one step, \(\log\) and \(\exp\) the matrix logarithm
    and exponential, and \(P^{1/n}\) the **principal** \(n\)-th root (the one whose eigenvalues are the real
    positive roots of \(P\)'s). [[markov.fit]] computes this, as Dinamica EGO's *DetermineTransitionMatrix* does:
    the `single` matrix of its result is \(P\), the `multi` matrix is \(Q\).

Takada et al. (2010) discuss when such a yearly matrix exists and is meaningful. A root of a matrix of
probabilities is not always a matrix of probabilities: it may have small **negative** entries (a "probability"
below zero), and when the period's matrix has a negative eigenvalue there is no real root at all (the change does
not behave like a chain of equal steps). Terraflux stops with an error in the second case; in the first, a
transition with a negative rate is simply not simulated (only transitions with a positive rate per step are).

![The yearly transition probabilities of Sibuyan](/img/tutorials/land-change/transition-rates-markov-chains/matrix_year.png "One year: the 14th root of the period's matrix. A cell of forest has a probability of 0.0054 per year of becoming grassland. Three tiny entries are negative (orange): coconut to rice and to other, rice to other.")

Look at the two matrices side by side. Forest keeps 0.881 of its cells over 14 years and 0.991 per year
(\(0.991^{14} \approx 0.88\)). Rice to coconut is 0.272 over the period, 0.023 per year. Three entries of the yearly
matrix are negative, of the order of \(10^{-5}\) or less: coconut to rice, coconut to other, rice to other. None of
these transitions happened in the period (their entries of \(P\) are 0); they appear because a cell can go from
coconut to grassland and from grassland to rice within the period, and the root has to subtract that path. They
are dropped from the simulation.

## When to use it

A Markov chain is the standard quantity of transition-based models (DINAMICA, CA-Markov models in many GIS) when
you have two maps and want the trend of the observed period to continue. Use it:

- for a **baseline** or *business-as-usual* scenario: the rates of the last period, repeated;
- to check what the observed rates imply over decades (part of the reading of any change analysis);
- as the starting point of scenarios: a matrix with the forest's losses halved, or a transition switched off,
  given with [[markov.matrix]].

It is **not** a model of why the rates are what they are. When the quantity of change depends on prices,
policies or population, take it from an economic model, a storyline or a target, and give it as a matrix (here)
or as a table of areas per step (`demand.table`, the demand of CLUE-S and PLUS). And a chain says nothing about
*where*: that is the job of the weights of evidence (next series).

## What you need

Two maps of the same classes on the same grid (part 1), and the number of time steps between them. The steps are
the time step of the simulation you plan: 14 for yearly steps between 1997 and 2011, 7 for steps of two years, 1
for a single step of 14 years. Choose the step so that the processes you care about happen within it (a forest
cleared and abandoned within one step is invisible to the chain).

## Build the model

```tfgraph
#! caption: The Markov chain of Sibuyan: the two dates of land use, and markov.fit with 14 time steps between them.
#! numbered: true
#! highlight: chain
format = "terraflux/1"

[[nodes]]
id = "land_1997"
op = "raster.read"
params = { path = "sibuyan_landuse.tif", band = "1997" }
note = "The land use at the start of the period."

[[nodes]]
id = "land_2011"
op = "raster.read"
params = { path = "sibuyan_landuse.tif", band = "2011" }
note = "And at its end."

[[nodes]]
id = "chain"
op = "markov.fit"
inputs = { t0 = "land_1997.raster", t1 = "land_2011.raster", steps = 14 }
note = "The transition probabilities of the period and of one of its 14 yearly steps (the 14th root)."

[outputs.chain]
from = "chain.markov"
```

<div class="steps" markdown>

1. **The two dates** (nodes 1 and 2): open `sibuyan_landuse.tif` twice, with the bands `1997` and `2011`
   ([part 1](/tutorials/land-change/what-land-change-models-are/) has the data).
2. **The chain** (node 3): with node 1 selected, add [[markov.fit]] from the library (subject *Markov*); connect
   node 1's `map` to `t0` and node 2's `map` to `t1`, and type `14` in its `steps` input in the Node pane. Its
   output `markov` is the chain, which the other nodes of a model read (the weights of evidence, the allocation).
3. **To read its matrices**, make the chain an output of the model (the `[outputs.chain]` table above) and run it
   from the command line: `terraflux run markov.toml` writes `output/chain.json`, with the classes, the period's
   matrix (`single`, row by row), the matrix of one step (`multi`) and the number of steps. In the application the
   node's output shows only that it is a Markov chain.

</div>

The classes of the chain are those found in either map; a class present in neither is left out. A class that has
no cells in the first map (it only appears in the second) gets a row that keeps it as it is.

### A chain given as a matrix: scenarios

[[markov.matrix]] takes the probabilities of the period in its settings, a row per class, and computes the root
in the same way. This is how a scenario changes the quantity of a model while keeping everything else. Here the
forest's losses of 1997 to 2011 are halved (each of its transitions divided by two, its persistence raised to
0.940) and the other rows are left as observed:

```tfgraph
#! caption: A scenario: the period's matrix given in the model, with the forest's losses halved; markov.matrix computes its 14th root as markov.fit does.
#! highlight: halved
format = "terraflux/1"

[[nodes]]
id = "halved"
op = "markov.matrix"
params = { classes = [1, 2, 3, 4, 5], steps = 14, single = [[0.940398, 0.013339, 0.033877, 0.01228, 0.000106], [0.0, 0.984456, 0.015544, 0.0, 0.0], [0.0, 0.091776, 0.887962, 0.01907, 0.001192], [0.0, 0.272321, 0.03125, 0.696429, 0.0], [0.0, 0.0, 0.0, 0.0, 1.0]] }
note = "Sibuyan's chain of 1997-2011 with the forest's losses halved."
```

`classes` names the class of each row and column; `single` is the period's matrix; `steps` the steps in the
period (default 1). A matrix of one step can be given directly as `multi`, and then no root is computed. In a full
Dinamica EGO model, this node takes the place of `markov.fit` and the rest of the model is unchanged.

## Read the results

### What the chain projects

Multiplying the areas of 1997 by the yearly matrix 14 times gives back, to the cell, the areas of 2011: that is
what the root guarantees. Dividing the period's changes by 14 does not. The chart compares the two, year by year,
for the three largest classes:

```tfchart
type: line
data: /data/tutorials/land-change/transition-rates-markov-chains/projection.csv
x: year
y: ["Forest (root)", "Forest (rate / 14)", "Coconut (root)", "Coconut (rate / 14)", "Grassland (root)", "Grassland (rate / 14)"]
colors: { "Forest (root)": "#3b8a5a", "Forest (rate / 14)": "#9fcdb0", "Coconut (root)": "#d9a441", "Coconut (rate / 14)": "#efd39b", "Grassland (root)": "#7d9a2e", "Grassland (rate / 14)": "#c8dc8f" }
xlabel: Year
ylabel: "% of the island"
height: 340
caption: The areas projected from 1997 by the yearly chain (the 14th root, dark lines) and by the period's changes divided by 14 (light lines). Observed in 2011, forest 57.6%, coconut 19.4%, grassland 15.1%; the root reproduces them, the division misses forest by 0.4 points and grassland by 0.4 points.
```

On Sibuyan the error of the division is small (forest 58.0% instead of 57.6% in 2011), because the changes are
small and few cells change twice. It grows with the amount of change and the number of steps, and it is not
random: dividing underestimates the losses of a class that loses a lot (forest) and the gains of a class that
passes part of what it receives on to another (grassland, which feeds coconut). For a fast-changing frontier, the difference matters.

### The long run

The same chain, run from 2011, says where the trend leads:

```tfchart
type: area
stacked: true
data: /data/tutorials/land-change/transition-rates-markov-chains/long_run.csv
x: year
y: [Forest, Coconut, Grassland, Rice, Other]
colors: { Forest: "#3b8a5a", Coconut: "#d9a441", Grassland: "#a7c957", Rice: "#4f9fc4", Other: "#b4508a" }
xlabel: Year
ylabel: "% of the island"
height: 320
caption: The areas of each class from 2011 to 2100 if the yearly rates of 1997-2011 held for ever. Forest falls from 57.6% to 40.4% in 2050 and 25.7% in 2100, coconut grows from 19.4% to 43.0%.
```

Forest only loses in this chain (its column has zeros outside the diagonal: nothing becomes forest), so a chain run
long enough removes it all. That is not a prediction about Sibuyan: it is what "the rates of 1997 to 2011 forever"
means. Reading the long run is a useful check of what a baseline assumes, and a reason to keep projections short
(a few times the observed period at most) and to give the quantity of longer ones as scenarios.

### The scenario

```tfchart
type: line
data: /data/tutorials/land-change/transition-rates-markov-chains/scenario.csv
x: year
y: [trend, halved]
labels: { trend: "Trend of 1997-2011", halved: "Forest losses halved" }
colors: { trend: "#c8553d", halved: "#3b8a5a" }
xlabel: Year
ylabel: "Forest, % of the island"
height: 280
caption: The forest of Sibuyan from 2011 to 2050 under the observed rates (40.4% in 2050) and with the forest's losses halved (48.5%).
```

Halving the forest's losses keeps about 8 more points of the island (some 37 km²) in forest by 2050. In a full
model, the same two chains give two series of maps whose *location* of change comes from the same weights of
evidence: only the quantity differs, which is what a scenario comparison needs.

### What to report

The period and its number of steps; the period's matrix (in probabilities, and the cells it comes from); the matrix
of one step or the rates of the transitions you simulate; any transition left out (negative or too rare); and, for
a scenario, the matrix you gave and why.

## Settings that change the answer

- **`steps`.** The rates per step depend on the number of steps; a model must simulate steps of the same length
  as those it was fitted with. Fitting with 14 steps and simulating 14 gives back the areas of 2011; fitting with 7
  steps (two years each) and simulating 7 steps from 2011 projects to 2025.
- **The period.** A chain repeats its period. The rates of a decade of a commodity boom, or of a decade after a
  new law, will not hold for the next; with several maps, compare the chains of successive periods
  (the intensity analysis of the [change detection](/tutorials/change-detection/) tutorials says whether the
  change sped up or slowed down) before choosing one.
- **The classes.** Merging classes changes the chain: transitions between merged classes disappear, and a class
  that is a mix of two behaviours gets rates that fit neither.

## Common pitfalls

- **Dividing the rates.** Never divide the period's probabilities by the number of steps; use the root
  (`markov.fit` with `steps`).
- **A negative eigenvalue.** When the period's matrix has one, no real yearly matrix exists and `markov.fit`
  stops with an error. It happens when a class changes mostly into another and back within the period, or with
  errors of classification that look like back-and-forth change. Use fewer steps (a longer step), check the maps,
  or give the rates per step directly as `multi` in a [[markov.matrix]].
- **Rare transitions.** A transition of one or two cells gets a rate, and the allocation will try to place it every
  step. Here forest to other (1 cell in 14 years) has a yearly rate of \(10^{-5}\): \(10^{-5} \times 4{,}723\) cells
  round to zero changes per step, so it never happens. Leave such transitions out of the weights of evidence (its
  `transitions` setting) or of a given matrix.
- **The chain is not the map.** A chain gives areas; two simulations with the same chain can have very different
  maps. The allocation makes the map.

## Going further

- [Weights of evidence](/tutorials/land-change/weights-of-evidence/): where each transition is likely, the first
  part of the series on Dinamica EGO.
- [Dinamica EGO on Sibuyan](/tutorials/land-change/dinamica-on-sibuyan/): the chain in a full model.
- The [change detection](/tutorials/change-detection/) tutorials: the transition matrix, its components and the
  intensity of change over several periods.
- The operators: [[markov.fit]], [[markov.matrix]].

## References

- Bell, E. J. (1974). Markov analysis of land use change: an application of stochastic processes to remotely sensed data. *Socio-Economic Planning Sciences*, 8(6), 311–316. https://doi.org/10.1016/0038-0121(74)90034-2
- Soares-Filho, B. S., Coutinho Cerqueira, G., & Lopes Pennachin, C. (2002). DINAMICA: a stochastic cellular automata model designed to simulate the landscape dynamics in an Amazonian colonization frontier. *Ecological Modelling*, 154(3), 217–235. https://doi.org/10.1016/S0304-3800(02)00059-5
- Takada, T., Miyamoto, A., & Hasegawa, S. F. (2010). Derivation of a yearly transition probability matrix for land-use dynamics and its applications. *Landscape Ecology*, 25(4), 561–572. https://doi.org/10.1007/s10980-009-9433-x
