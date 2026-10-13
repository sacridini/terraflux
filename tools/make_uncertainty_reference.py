# SPDX-License-Identifier: GPL-3.0-or-later
"""What landschaft's uncertainty module gives, for Terraflux's tests, written to
tests/data/reference/uncertainty.npz:

    python tools/make_uncertainty_reference.py

Totals and products of uncertain maps (quadratic, linear, correlated), confidence intervals, the delta method
(central and forward, quadratic and linear) and Monte Carlo of expressions of a map and two parameters, and of
two parameters alone. The expressions use + - * / only (numpy's vectorised exp and power are not the C
library's).
"""

from __future__ import annotations

from pathlib import Path

import numpy as np

import landschaft as ls

ROOT = Path(__file__).resolve().parents[1]
REF = ROOT / "tests" / "data" / "reference"
U = ls.uncertainty


def main() -> None:
    rng = np.random.default_rng(42)
    shape = (23, 31)
    sd = rng.uniform(0.1, 3.0, shape)
    sd[3, 4:9] = np.nan
    w = rng.uniform(0.0, 2.0, shape)
    x = rng.normal(10.0, 3.0, shape)
    sx = rng.uniform(0.0, 1.0, shape)
    v = rng.normal(-2.0, 5.0, shape)
    sv = rng.uniform(0.0, 0.5, shape)
    carbon = rng.uniform(50.0, 150.0, shape)
    carbon[7, 7] = np.nan
    out = {"sd": sd, "w": w, "x": x, "sx": sx, "v": v, "sv": sv, "carbon": carbon}
    out["sum_quadratic"] = np.array([U.sum_sd(sd)])
    out["sum_linear_w"] = np.array([U.sum_sd(sd, w, method="linear")])
    out["sum_w2"] = np.array([U.sum_sd(sd, 2.5)])
    for rho in (-0.3, 0.0, 0.6, 1.0):
        out[f"sum_rho_{rho}"] = np.array([U.sum_sd(sd, w, correlation=rho)])
    out["product"] = U.product_sd(x, sx, v, sv)
    out["product_numbers"] = U.product_sd(x, 0.2, 3.0, sv)
    out["product_total_q"] = np.array([U.product_sd(x, sx, v, sv, total=True)])
    out["product_total_l"] = np.array([U.product_sd(x, sx, v, sv, total=True, method="linear")])
    for level in (0.5, 0.9, 0.95, 0.99, 0.999):
        lo, hi = U.interval(x, sx, level)
        out[f"interval_{level}"] = np.stack([lo, hi])

    def f(p):
        return carbon * p[0] + p[1]

    for scheme in ("central", "forward"):
        for method in ("quadratic", "linear"):
            r = U.delta_method(f, [0.47, 1.5], [0.02, 0.3], scheme=scheme, method=method)
            out[f"delta_{scheme}_{method}"] = np.stack([r.value, r.sd])
    r = U.delta_method(lambda p: carbon / (p[0] + p[1] * carbon), [2.0, 0.01], [0.1, 0.002])
    out["delta_ratio"] = np.stack([r.value, r.sd])
    r = U.delta_method(lambda p: p[0] * p[1], [2.0, 3.0], [0.1, 0.2], step=1e-4)
    out["delta_scalar"] = np.array([r.value, r.sd])
    m = U.monte_carlo(f, [0.47, 1.5], [0.02, 0.3], n=300, seed=3, quantiles=(0.025, 0.5, 0.975))
    out["mc_mean"], out["mc_sd"], out["mc_q"] = m.mean, m.sd, m.quantiles.reshape(3, *shape)
    m = U.monte_carlo(lambda p: p[0] * p[1], [2.0, 3.0], [0.1, 0.2], n=5000, seed=1)
    out["mc_scalar"] = np.array([m.mean, m.sd, *m.quantiles.ravel()])
    np.savez_compressed(REF / "uncertainty.npz", **{k: np.asarray(v, dtype=np.float64) for k, v in out.items()})
    print(REF / "uncertainty.npz", len(out))


if __name__ == "__main__":
    main()
