# SPDX-License-Identifier: GPL-3.0-or-later
"""What landschaft's bias_surface gives, for Terraflux's tests, written to tests/data/reference/sdm_bias.npz and
.json:

    python tools/make_sdm_bias_reference.py

The records of a target group (a table, some outside the grid, a weight column) over a stack of two predictors on
a projected grid with missing cells: the Gaussian kernel density of MASS::kde2d at the cells' centres with R's
bw.nrd, isotropic and limited bandwidths, a pair of bandwidths with weights and a rescaling, every cell (no mask).
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
S = ls.sdm


def main() -> None:
    ny, nx, res = 50, 60, 1000.0
    t = (300000.0, res, 0.0, 7000000.0, 0.0, -res)
    layers = []
    for k in range(2):
        v = ls.nlm.gaussian_field(nx, ny, autocorr_range=7 + 4 * k, seed=61 + k).values * 10.0
        if k == 1:
            v[35:41, 8:17] = np.nan
        layers.append(v)
    x = t[0] + (np.arange(nx) + 0.5) * t[1]
    y = t[3] + (np.arange(ny) + 0.5) * t[5]
    env = xr.DataArray(np.stack(layers), dims=("band", "y", "x"), coords={"band": ["a", "b"], "y": y, "x": x})
    env = env.rio.write_transform(Affine(t[1], 0, t[0], 0, t[5], t[3]))
    rng = np.random.default_rng(12)
    # two clusters of effort (a town, a road) and a few records beyond the grid's edges
    town = rng.normal([318000.0, 6978000.0], [4000.0, 3000.0], size=(140, 2))
    road = np.column_stack([rng.uniform(300000.0, 360000.0, 90), 6960000.0 + rng.normal(0, 1500.0, 90)])
    out_ = np.array([[295000.0, 6990000.0], [365000.0, 6955000.0], [330000.0, 7004000.0]])
    xy = np.round(np.vstack([town, road, out_]), 2)
    pts = pd.DataFrame({"x": xy[:, 0], "y": xy[:, 1], "w": rng.integers(0, 4, len(xy)).astype(float)})
    cases = {
        "nrd": {},
        "isotropic_limits": {"isotropic": True, "limits": [None, 3000.0]},
        "pair_weights_rescale": {"bandwidth": [2500.0, 1800.0], "weights": "w", "rescale": [0.05, 0.95]},
        "no_mask": {"bandwidth": 4000.0, "mask": False},
    }
    out: dict[str, np.ndarray] = {"in/env": env.values, "in/points": pts.to_numpy(dtype=np.float64)}
    for name, kw in cases.items():
        k = dict(kw)
        if "weights" in k:
            k["weights"] = pts[k["weights"]].to_numpy()
        for key in ("bandwidth", "limits", "rescale"):
            if isinstance(k.get(key), list):
                k[key] = tuple(k[key])
        b = S.bias_surface(pts[["x", "y"]], env, x="x", y="y", **k)
        out[f"{name}/bias"] = b.values
        out[f"{name}/bandwidth"] = np.array(b.attrs["bandwidth"], dtype=np.float64)
    meta = {"transform": list(t), "columns": ["x", "y", "w"], "cases": cases}
    np.savez_compressed(REF / "sdm_bias.npz", **out)
    (REF / "sdm_bias.json").write_text(json.dumps(meta, indent=1) + "\n", encoding="utf-8")
    print(REF / "sdm_bias.npz", len(out), list(cases))


if __name__ == "__main__":
    main()
