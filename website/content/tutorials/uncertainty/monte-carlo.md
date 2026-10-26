---
title: Monte Carlo error propagation
short: Monte Carlo
description: Carry the uncertainty of a model's parameters through to its maps by drawing them many times - the mean, standard deviation, quantiles and intervals of every cell, how many draws are enough, the delta method as a shortcut, and why the uncertainty of a total is not the sum of the cells'.
order: 10
level: intermediate
time: 40 min
template: Carbon and its uncertainty
ops: [uncertainty.monte_carlo, uncertainty.interval, uncertainty.delta, uncertainty.sum_sd, raster.calc]
---

Every number in an ecological model comes with an uncertainty: an allometric equation fitted on a few hundred
trees, a carbon fraction measured in a laboratory, a dispersal distance read from the literature. When the model
turns a map of tree cover into a map of carbon, those uncertainties travel into every cell of the result. In this
tutorial you will open the template **Carbon and its uncertainty**, which propagates the uncertainty of three
parameters to a carbon map by the **Monte Carlo method**; read its maps of the mean, the standard deviation and the
quantiles; find out how many draws are enough; compare it with the faster **delta method**; and learn the one
mistake that makes the uncertainty of a total carbon stock a hundred times too small.

## The idea

Suppose the carbon of a forest cell (tonnes per hectare) is computed from its canopy cover \(c\) (0 to 1) by an
allometry and a carbon fraction:

\[ C = a\, c^{\,b}\, f \]

where \(a\) and \(b\) are the allometry's coefficients and \(f\) the share of biomass that is carbon. None of them is
known exactly: say \(a = 250 \pm 25\) t/ha, \(b = 1.5 \pm 0.15\) and \(f = 0.47 \pm 0.02\) (mean ± standard
deviation). What is the uncertainty of \(C\)?

The **Monte Carlo method** (Metropolis & Ulam, 1949) answers by brute force: draw a value of each parameter from its
distribution, compute \(C\) with them, and repeat many times. The draws of \(C\) are a sample of its distribution:
their mean, standard deviation and quantiles describe what \(C\) can be, given what is known of the parameters. It
needs no calculus and works for any model, however non-linear; the price is computing the model many times. It is
the method recommended for propagating uncertainty in measurements (JCGM, 2008), in GIS models (Heuvelink, 1998) and
in estimates of forest biomass (Chave et al., 2004).

!!! science "Key idea: Monte Carlo"
    With \(n\) draws \(\theta^{(1)}, \dots, \theta^{(n)}\) of the parameters, the model gives \(y^{(i)} = g(\theta^{(i)})\)
    for each. Then
    \[ \bar y = \frac{1}{n}\sum_i y^{(i)}, \qquad s = \sqrt{\frac{1}{n-1}\sum_i \left(y^{(i)} - \bar y\right)^2} \]
    estimate the mean and the standard deviation of \(y\), and the sorted draws give its quantiles (the 2.5% and
    97.5% quantiles bound an interval holding 95% of the draws). These estimates are themselves uncertain: the
    *Monte Carlo error* of the mean is \(s / \sqrt{n}\), so four times as many draws halve it.

In a map, the same draws of the parameters are used for every cell: draw 17 has the same \(a\), \(b\) and \(f\)
everywhere. That is right when the parameters are the same for the whole map (one allometry, one carbon fraction),
and it matters a great deal for totals (see [The total](#the-total-errors-that-are-not-independent)).

## When to use it

- **Uncertain parameters of a calculation**: an allometry, an emission factor, a conversion coefficient, the
  weights of a habitat suitability index.
- **Non-linear models**, where the delta method's approximation may fail: exponents, thresholds, ratios.
- **Reporting**: carbon stocks and their changes are reported with their uncertainty (the IPCC asks for it), and a
  map of the uncertainty shows where an estimate can be trusted.

What [[uncertainty.monte_carlo]] is **not** for: uncertainty of the *input maps* themselves (errors of
classification of a land cover map, different from cell to cell), which needs maps drawn with spatially correlated
errors (Heuvelink, 1998), nor the stochasticity of a simulation, measured by replicates with different seeds (see
[Randomness and reproducibility](../../building-models/randomness/)).

## What you need

Only Terraflux: the template **Carbon and its uncertainty** (**Templates** menu, group *Surfaces and uncertainty*)
draws its canopy with a neutral landscape model. With your own data, replace its first node with a [[raster.read]]
of a tree cover map (0 to 1; divide a percentage by 100 with [[raster.calc]]), and its parameters with those of your
allometry and their standard errors.

## Build the model

```tfgraph
#! caption: The template "Carbon and its uncertainty": a canopy cover, the carbon of every cell with its three parameters drawn 500 times, a 90% interval from the mean and standard deviation, and the relative uncertainty.
#! highlight: carbon
#! numbered: true
format = "terraflux/1"
name = "Carbon and its uncertainty"

[[nodes]]
id = "canopy"
op = "nlm.gaussian_field"
params = { rows = 100, columns = 100, range = 15, seed = 61 }
note = "The canopy's cover, 0 to 1 (in its place, a raster.read of a tree cover map)."

[[nodes]]
id = "carbon"
op = "uncertainty.monte_carlo"
inputs = { canopy = "canopy.map" }
params = { expression = "a * canopy ** b * fraction", parameters = [["a", 250, 25], ["b", 1.5, 0.15], ["fraction", 0.47, 0.02]], n = 500, seed = 1 }
note = "The carbon, t/ha: the biomass from the cover by an allometry a cover^b (a 250 ± 25, b 1.5 ± 0.15), times its carbon fraction (0.47 ± 0.02); the three drawn 500 times from normal distributions."

[[nodes]]
id = "interval"
op = "uncertainty.interval"
inputs = { value = "carbon.mean", sd = "carbon.sd" }
params = { level = 0.9 }
note = "A normal interval of 90 % from the mean and the deviation."

[[nodes]]
id = "relative"
op = "raster.calc"
inputs = { m = "carbon.mean", s = "carbon.sd" }
params = { expression = "100 * s / m if m > 0 else None" }
note = "The deviation as a share of the mean (%)."
```

**Node 1**, [[nlm.gaussian_field]], draws a canopy cover from 0 to 1, smooth over about 15 cells.

**Node 2**, [[uncertainty.monte_carlo]], is the heart of the model:

- its **inputs** take any names, and the expression uses them: here `canopy`, connected to the cover. Inputs can
  be maps or numbers;
- `expression` is the model, in the language of [map algebra](../../building-models/map-algebra/), written with
  the inputs and the parameters;
- `parameters` lists the uncertain parameters: `["name", mean, standard deviation]` each. They are drawn from
  **normal distributions**, independently of each other;
- `n` is the number of draws (1,000 by default; 500 here), `seed` the seed of the generator (numpy's: the draws are
  those of `numpy.random.default_rng(seed).standard_normal`), and `quantiles` the quantiles wanted
  (`[0.025, 0.975]` by default).

Its outputs: `mean` and `sd`, a map each, and `quantiles`, a stack with a layer per quantile (`q0.025` and
`q0.975`). With only numbers as inputs, they are numbers and a table.

**Node 3**, [[uncertainty.interval]], makes a **normal** confidence interval from a mean and a standard deviation:
\( \bar y \pm z\, s \), \(z\) the normal quantile of the level (1.645 for 90%). **Node 4** is the relative
uncertainty, the standard deviation as a percentage of the mean (the *coefficient of variation*).

## Read the results

```tfgallery
columns: 2
caption: "The canopy cover and the mean carbon over the 500 draws: from 0 where there is no canopy to 117 t/ha under a full one. The mean follows the cover, bent by the exponent b = 1.5: half a canopy gives about a third of a full canopy's carbon."
images:
  - { src: /img/tutorials/uncertainty/monte-carlo/canopy.png, caption: "canopy.map" }
  - { src: /img/tutorials/uncertainty/monte-carlo/mean.png, caption: "carbon.mean" }
```

The standard deviation is largest where the carbon is largest (up to 12.9 t/ha), but **relative** to the mean it is
largest where the canopy is sparse: 11% under a full canopy, up to 54% under a thin one. The exponent \(b\) is the
reason: \(c^b\) is insensitive to \(b\) when \(c\) is near 1, and very sensitive to it when \(c\) is small. That is a
typical result to report: the estimate is most uncertain in the degraded forests where carbon accounting often
matters most.

One cell, row 50 and column 50, has a canopy of 0.50. Its 500 draws of carbon:

![The draws of one cell](/img/tutorials/uncertainty/monte-carlo/cell_draws.png "The 500 draws of the carbon of one cell (canopy 0.50): mean 41.9 t/ha, standard deviation 6.3, 95% of the draws between 30.2 and 55.1 t/ha. The figure script drew the same parameters with numpy and found the same mean and standard deviation, to the precision of the file.")

So the cell's carbon is **41.9 t/ha, with a 95% interval of 30.2 to 55.1 t/ha**. The interval is not symmetric
around the mean (11.7 below, 13.2 above): the model is non-linear. A normal interval from the mean and standard
deviation, \(41.9 \pm 1.96 \times 6.3\) = 29.6 to 54.2, misses that asymmetry; the quantiles of the draws do not.
Report the quantiles when the model is non-linear, the normal interval ([[uncertainty.interval]]) when it is close to
linear or when only a standard deviation is wanted.

```tfgallery
columns: 2
caption: "The 2.5% and 97.5% quantiles of every cell (the layers of carbon.quantiles): the 95% interval of the carbon, cell by cell."
images:
  - { src: /img/tutorials/uncertainty/monte-carlo/q_low.png, caption: "q0.025" }
  - { src: /img/tutorials/uncertainty/monte-carlo/q_high.png, caption: "q0.975" }
```

## Settings that change the answer: the number of draws

How many draws are enough? The template's model was run with 10 to 5,000 draws, each with **ten seeds**, and the
standard deviation of the same cell compared:

```tfslider
label: Draws
caption: "The map of the standard deviation with 10 to 5,000 draws (seed 1). With few draws it is noisy, because each cell's standard deviation is estimated from few values; from about 1,000 draws it no longer changes visibly. The numbers: the standard deviation of cell (50, 50) with seed 1, and its range over ten seeds."
frames:
  - { value: 10, src: /img/tutorials/uncertainty/monte-carlo/sd_10.png, caption: "10 draws", stats: { "sd, seed 1": "3.08", "sd, ten seeds": "3.08 to 10.60" } }
  - { value: 20, src: /img/tutorials/uncertainty/monte-carlo/sd_20.png, caption: "20 draws", stats: { "sd, seed 1": "5.87", "sd, ten seeds": "4.41 to 8.68" } }
  - { value: 50, src: /img/tutorials/uncertainty/monte-carlo/sd_50.png, caption: "50 draws", stats: { "sd, seed 1": "5.39", "sd, ten seeds": "5.07 to 7.68" } }
  - { value: 100, src: /img/tutorials/uncertainty/monte-carlo/sd_100.png, caption: "100 draws", stats: { "sd, seed 1": "6.18", "sd, ten seeds": "5.57 to 7.36" } }
  - { value: 200, src: /img/tutorials/uncertainty/monte-carlo/sd_200.png, caption: "200 draws", stats: { "sd, seed 1": "5.86", "sd, ten seeds": "5.85 to 6.72" } }
  - { value: 500, src: /img/tutorials/uncertainty/monte-carlo/sd_500.png, caption: "500 draws", stats: { "sd, seed 1": "6.28", "sd, ten seeds": "5.91 to 6.75" } }
  - { value: 1000, src: /img/tutorials/uncertainty/monte-carlo/sd_1000.png, caption: "1,000 draws", stats: { "sd, seed 1": "6.16", "sd, ten seeds": "6.04 to 6.60" } }
  - { value: 2000, src: /img/tutorials/uncertainty/monte-carlo/sd_2000.png, caption: "2,000 draws", stats: { "sd, seed 1": "6.28", "sd, ten seeds": "6.21 to 6.48" } }
  - { value: 5000, src: /img/tutorials/uncertainty/monte-carlo/sd_5000.png, caption: "5,000 draws", stats: { "sd, seed 1": "6.34", "sd, ten seeds": "6.30 to 6.45" } }
chart:
  type: line
  data: /data/tutorials/uncertainty/monte-carlo/spread.csv
  x: n
  y: [low, seed1, high]
  labels: { low: "lowest of ten seeds", seed1: "seed 1", high: "highest of ten seeds" }
  colors: { low: "#9aa59d", seed1: "#e8901f", high: "#9aa59d" }
  ylabel: "sd of cell (50, 50), t/ha"
```

With 10 draws, the standard deviation of the cell could come out anywhere from 3.1 to 10.6 t/ha depending on the
seed: a factor of three. With 100 draws, from 5.6 to 7.4; with 1,000, from 6.0 to 6.6; with 5,000, from 6.30 to 6.45.
The extreme quantiles settle more slowly still: the 97.5% quantile of the cell ranged from 43.9 to 63.9 t/ha over
the seeds with 10 draws, 54.6 to 56.5 with 1,000 and 55.4 to 56.0 with 5,000.

```tfchart
type: line
data: /data/tutorials/uncertainty/monte-carlo/spread.csv
x: n
y: [q_low, q_high]
labels: { q_low: "lowest of ten seeds", q_high: "highest of ten seeds" }
colors: { q_low: "#2f6f9f", q_high: "#b4508a" }
ylabel: "97.5% quantile of cell (50, 50), t/ha"
height: 280
caption: "The range, over ten seeds, of the 97.5% quantile of the same cell, as draws grow. A quantile in the tail rests on few draws (2.5% of them), so it needs many more draws than the mean or the standard deviation."
```

Practical rules:

- **1,000 draws** (the default) are enough for means and standard deviations to two significant digits;
- **several thousand** for the 2.5% and 97.5% quantiles, and more for more extreme ones;
- **check it**: run with two seeds; if the numbers you report differ in a digit you report, add draws.

The cost is small here (5,000 draws of 10,000 cells take about a second), and grows with the number of cells
times the number of draws.

## A shortcut: the delta method

When the model is close to linear in its parameters, its uncertainty can be computed without drawing anything, from
its derivatives. The **delta method** (the first-order *law of propagation of uncertainty* of metrology's Guide to the
expression of uncertainty in measurement, the GUM) approximates

\[ s_y \approx \sqrt{\sum_j \left( \frac{\partial g}{\partial \theta_j}\, \sigma_j \right)^2 } \]

where \(\sigma_j\) is the standard deviation of parameter \(j\) and the derivatives are taken at the parameters' means.
[[uncertainty.delta]] computes it, with the derivatives by finite differences, from the same `expression` and
`parameters`:

```tfgraph
#! caption: The delta method on the same model: no draws, the standard deviation from the derivatives of the expression.
#! highlight: delta
format = "terraflux/1"

[[nodes]]
id = "canopy"
op = "nlm.gaussian_field"
params = { rows = 100, columns = 100, range = 15, seed = 61 }

[[nodes]]
id = "delta"
op = "uncertainty.delta"
inputs = { canopy = "canopy.map" }
params = { expression = "a * canopy ** b * fraction", parameters = [["a", 250, 25], ["b", 1.5, 0.15], ["fraction", 0.47, 0.02]] }
```

Against Monte Carlo with 5,000 draws, the delta method's standard deviations differ by 1% on average (at most 0.12
t/ha), and its value (the model at the parameters' means) is on average 0.19 t/ha below the Monte Carlo mean: the
mean of a non-linear function is not the function of the means. For this model the shortcut is good. Use Monte Carlo
when the model is strongly non-linear (thresholds, ratios, large uncertainties of exponents), when you need
quantiles, or simply to check the delta method once.

```tfchart
type: line
data: /data/tutorials/uncertainty/monte-carlo/delta.csv
x: canopy
y: [monte_carlo, delta]
labels: { monte_carlo: "Monte Carlo, 5,000 draws", delta: "Delta method" }
colors: { monte_carlo: "#3b8a5a", delta: "#e8901f" }
xlabel: Canopy cover
ylabel: sd of carbon (t/ha)
height: 280
caption: "The standard deviation of the carbon against the canopy cover, by the two methods, for 60 cells across the range of cover. The lines almost coincide."
```

## The total: errors that are not independent

The forest's **total** carbon is the sum of its cells. If each cell is a hectare, the total of the mean map is
**527,228 t**. What is its standard deviation?

The tempting answer treats the cells' errors as independent and adds their variances: \( s_{tot} = \sqrt{\sum_i s_i^2} \).
That is what [[uncertainty.sum_sd]] does with `method = "quadratic"`, and it gives **728 t**: 0.14% of the total.

It is wrong by a factor of a hundred. The cells' errors come from the **same** parameters: in a draw where \(a\) is
high, every cell's carbon is high. Errors that move together do not cancel in a sum; they add up. With the errors
perfectly correlated, the standard deviation of the sum is the sum of the standard deviations, which
[[uncertainty.sum_sd]] gives with `method = "linear"`: **71,184 t**. And the truth, computed by the figure script
from the same 5,000 draws of the parameters applied to the whole map, is **69,755 t** (13% of the total), with a 95%
interval of 397,938 to 673,067 t.

```tfgraph
#! caption: The standard deviation of the total carbon, under two assumptions about the cells' errors.
#! fragment: true
#! highlight: total_shared
format = "terraflux/1"

[[nodes]]
id = "total_independent"
op = "uncertainty.sum_sd"
inputs = { sd = "carbon.sd" }
params = { method = "quadratic" }
note = "Independent errors: sqrt(sum of the variances). Wrong here."

[[nodes]]
id = "total_shared"
op = "uncertainty.sum_sd"
inputs = { sd = "carbon.sd" }
params = { method = "linear" }
note = "Errors that move together (shared parameters): the sum of the standard deviations."
```

![The total carbon's distribution](/img/tutorials/uncertainty/monte-carlo/total.png "The total carbon of the 10,000 cells in each of 5,000 draws of the parameters (bars), with the mean ± 1.96 standard deviations under the two assumptions: independent errors (orange, too narrow to see) and errors that move together (green), which matches the spread of the draws.")

!!! warning "Before summing uncertainties, ask where the errors come from"
    Errors from **shared parameters** (one allometry, one emission factor) are correlated across cells: use
    `method = "linear"`, or a `correlation` close to 1. Errors **of each cell on its own** (independent measurement
    errors, plot by plot) are independent: use `quadratic`. Mixed sources: `correlation` between 0 and 1
    (\( s_{tot}^2 = (1-\rho)\sum s_i^2 + \rho\,(\sum s_i)^2 \)). Treating shared errors as independent is the most
    common way of reporting a total far more precise than it is.

## Common pitfalls

- **Standard errors or standard deviations?** The `parameters` take the standard deviation of the parameter's
  *estimate*: for a fitted coefficient, its standard error, not the residual standard deviation of the data.
- **Correlated parameters.** [[uncertainty.monte_carlo]] draws the parameters independently. The coefficients of a
  fitted allometry (\(a\) and \(b\)) are usually correlated; drawing them independently overstates (or understates)
  the uncertainty. When the correlation is strong, propagate it from the fit's covariance outside, or report the
  assumption.
- **Normal distributions only.** A parameter that cannot be negative, with a large uncertainty, may be drawn
  negative. Check that the mean is several standard deviations from any bound.
- **Too few draws** for the quantiles (above). Two seeds tell you.
- **Adding variances of cells** that share parameters (above).
- **Confusing uncertainty with variability.** The standard deviation map says how uncertain each cell's carbon is,
  not how variable carbon is across the forest.

## Going further

- [Randomness and reproducibility](../../building-models/randomness/): seeds, replicates and how many are enough.
- [Map algebra](../../building-models/map-algebra/): the language of the expressions.
- [Distance and restoration](../../distance-restoration/): restoration plans, which propagate uncertainty
  with these operators.
- The operators [[uncertainty.monte_carlo]], [[uncertainty.delta]], [[uncertainty.interval]], [[uncertainty.sum_sd]],
  [[uncertainty.product_sd]].

## References

- Chave, J., Condit, R., Aguilar, S., Hernandez, A., Lao, S., & Perez, R. (2004). Error propagation and scaling for tropical forest biomass estimates. *Philosophical Transactions of the Royal Society of London. Series B: Biological Sciences*, 359(1443), 409–420. https://doi.org/10.1098/rstb.2003.1425
- Heuvelink, G. B. M. (1998). *Error Propagation in Environmental Modelling with GIS*. Taylor & Francis. https://doi.org/10.4324/9780203016114
- JCGM (2008). *Evaluation of measurement data — Supplement 1 to the "Guide to the expression of uncertainty in measurement" — Propagation of distributions using a Monte Carlo method* (JCGM 101:2008). Joint Committee for Guides in Metrology.
- Metropolis, N., & Ulam, S. (1949). The Monte Carlo method. *Journal of the American Statistical Association*, 44(247), 335–341. https://doi.org/10.1080/01621459.1949.10483310
