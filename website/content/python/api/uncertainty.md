---
title: Uncertainty
description: "Totals, products, the delta method, Monte Carlo and confidence intervals."
order: 250
---

Propagating the uncertainty of inputs to results: the standard deviation of
a total of uncertain pixel values, of a product of two uncertain layers, of a
result with respect to uncertain parameters (the delta method, checked by
Monte Carlo), and confidence intervals. The `tf.uncertainty` module; the
forms are those plangea uses in its restoration plans, with their errors
corrected (each function's notes say which).

<!-- no-run -->
```python
import terraflux as tf

tf.uncertainty.sum_sd(carbon_sd, weights=area * restored)          # a total's deviation
tf.uncertainty.product_sd(x, sd_x, v, sd_v)                         # each product's
d = tf.uncertainty.delta_method(f, params, sd)                      # d.value, d.sd
low, high = tf.uncertainty.interval(d.value, d.sd, level=0.95)
```

## Totals and products

::: terraflux.uncertainty.sum_sd

::: terraflux.uncertainty.product_sd

## Parameters

::: terraflux.uncertainty.delta_method

::: terraflux.uncertainty.DeltaResult

::: terraflux.uncertainty.monte_carlo

::: terraflux.uncertainty.MonteCarloResult

## Intervals

::: terraflux.uncertainty.interval
