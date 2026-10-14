# SPDX-License-Identifier: GPL-3.0-or-later
"""What landschaft's diffeR port gives at coarser resolutions, for Terraflux's tests, written to
tests/data/reference/differ.npz and .json:

    python tools/make_differ_reference.py

Two maps of four classes (37 x 45 cells of 30 m, a few missing) and two continuous maps with strata:
memberships, the composite cross-tabulation (factors 2, 3 and 5), differenceMR (factor 2 and 3, in % and
proportions, scaled to a population), MAD (one stratum and several, at every resolution or the first) and
sample2pop.
"""

from __future__ import annotations

import json
from pathlib import Path

import numpy as np
import pandas as pd
import xarray as xr
from affine import Affine

import landschaft as ls

ROOT = Path(__file__).resolve().parents[1]
REF = ROOT / "tests" / "data" / "reference"
S = ls.simulate


def grid(values: np.ndarray) -> xr.DataArray:
    ny, nx = values.shape
    da = xr.DataArray(values, dims=("y", "x"), coords={"y": 5000 - (np.arange(ny) + 0.5) * 30.0,
                                                     "x": 1000 + (np.arange(nx) + 0.5) * 30.0})
    return da.rio.write_transform(Affine(30.0, 0, 1000.0, 0, -30.0, 5000.0))


def main() -> None:
    ny, nx = 37, 45
    a = ls.nlm.random_cluster(nx, ny, p=0.45, ai=[0.4, 0.3, 0.2, 0.1], seed=3, rescale=False).values.astype(float)
    b = a.copy()
    rng = np.random.default_rng(5)
    flip = rng.random((ny, nx)) < 0.25
    b[flip] = rng.integers(1, 5, int(flip.sum()))
    a[3:5, 7:9] = np.nan
    b[20, 30] = np.nan
    c1 = np.round(ls.nlm.gaussian_field(nx, ny, autocorr_range=8, seed=7).values * 50, 4)
    c2 = np.round(c1 + rng.normal(0, 4, (ny, nx)), 4)
    c1[10, 10] = np.nan
    strata = np.floor(ls.nlm.gaussian_field(nx, ny, autocorr_range=15, seed=8).values * 3)
    strata[0, :5] = np.nan
    A, B, C1, C2, ST = grid(a), grid(b), grid(c1), grid(c2), grid(strata)
    out: dict[str, np.ndarray] = {"in/a": a, "in/b": b, "in/c1": c1, "in/c2": c2, "in/strata": strata}
    meta: dict = {"memberships": {}, "composite": {}, "resolutions": {}, "mad": {}}
    for f in (2, 3):
        m = S.memberships(A, f)
        out[f"memberships/{f}"] = np.stack([m[k] for k in sorted(m)])
        meta["memberships"][str(f)] = [float(k) for k in sorted(m)]
    for f in (2, 3, 5):
        t = S.composite_matrix(A, B, f)
        out[f"composite/{f}"] = t.to_numpy()
        meta["composite"][str(f)] = [float(k) for k in t.index]
    pop = pd.DataFrame({"category": [4, 1, 2, 3], "size": [5000.0, 120.0, 900.5, 33.0]})
    out["in/population"] = pop.to_numpy(dtype=float)
    for name, kw in {"f2": {}, "f3": {"factor": 3}, "proportions": {"percent": False},
                     "population": {"population": pop}}.items():
        out[f"resolutions/{name}"] = S.difference_resolutions(A, B, **kw).to_numpy(dtype=float)
        meta["resolutions"][name] = {k: v for k, v in kw.items() if k != "population"} | (
            {"population": True} if "population" in kw else {})
    for name, kw in {"one": {}, "strata": {"strata": ST}, "first": {"multiple": False}}.items():
        out[f"mad/{name}"] = S.mad(C1, C2, **kw).to_numpy(dtype=float)
        meta["mad"][name] = {"strata": "strata" in kw, "multiple": kw.get("multiple", True)}
    ct = S.crosstab(A, B)
    out["in/crosstab"] = np.column_stack([np.asarray(ct.index, dtype=float), ct.to_numpy(dtype=float)])
    out["sample_to_population"] = S.sample_to_population(ct.astype(float), pop).to_numpy()
    np.savez_compressed(REF / "differ.npz", **out)
    (REF / "differ.json").write_text(json.dumps(meta, indent=1) + "\n", encoding="utf-8")
    print(REF / "differ.npz", len(out))


if __name__ == "__main__":
    main()
