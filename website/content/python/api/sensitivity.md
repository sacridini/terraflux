---
title: Sensitivity analysis
description: "Morris's elementary effects and Sobol's indices of a model's parameters (SALib's)."
order: 260
---

Which parameters of a model matter, and how much: Morris's elementary effects
(a screening of many factors with few runs) and Sobol's variance-based indices
(the share of the output's variance due to each factor alone, with its
interactions, and to pairs). The runs and the indices are those of SALib 1.6,
the same numbers; the runs of a terraflux tool can be spread over the
helpers of `tf.parallel`. The `tf.sensitivity` module.

<!-- no-run -->
```python
import numpy as np
import terraflux as tf

d = tf.sensitivity.design("sobol", {"rate": (0.01, 0.1), "threshold": (0.2, 0.8)}, n=256)
y = tf.sensitivity.evaluate(lambda p: my_model(rate=p[0], threshold=p[1]), d)
tf.sensitivity.analyze(d, y).indices                 # s1, st and their intervals

# a terraflux tool, its runs on the helpers of this computer's group
f = tf.parallel.call("surface.gravity", sources, exponent=tf.parallel.param(0))
d = tf.sensitivity.design("morris", {"exponent": (1, 3)}, n=20)
y = tf.sensitivity.evaluate(f, d, on=tf.helpers(), summary=np.mean)
```

## Designs

::: terraflux.sensitivity.design

::: terraflux.sensitivity.Design

## Running the model

::: terraflux.sensitivity.evaluate

## Indices

::: terraflux.sensitivity.analyze

::: terraflux.sensitivity.Sensitivity
