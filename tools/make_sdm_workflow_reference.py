# SPDX-License-Identifier: GPL-3.0-or-later
"""What landschaft's modelling workflow gives with MaxNet, for Terraflux's tests, written to
tests/data/reference/sdm_workflow.npz and .json:

    python tools/make_sdm_workflow_reference.py

Presences and background points on a stack of three continuous predictors and a categorical one (a projected
grid): the dataset (the predictors at the records), the model fitted with cross-validation over random folds and
over spatial blocks (every fold's metrics, their means and standard deviations, the threshold), the response
curves (mean and median) and the permutation importance.
"""

from __future__ import annotations

import json
from pathlib import Path

import numpy as np
import pandas as pd
import xarray as xr

import landschaft as ls

ROOT = Path(__file__).resolve().parents[1]
REF = ROOT / "tests" / "data" / "reference"
S = ls.sdm


def main() -> None:
    layers = []
    for k in range(3):
        f = ls.nlm.gaussian_field(40, 30, autocorr_range=8 + 3 * k, resolution=1000.0, seed=31 + k)
        layers.append(f.copy(data=np.round(f.values * (10 + 5 * k) + k, 3)))
    cover = ls.nlm.random_cluster(40, 30, p=0.5, ai=[0.4, 0.3, 0.2, 0.1], resolution=1000.0, seed=40,
                                  rescale=False).astype(np.float64)
    layers.append(cover)
    names = ["temp", "prec", "soil", "cover"]
    env = xr.concat(layers, dim="band").assign_coords(band=names)
    env = env.rio.write_transform(layers[0].rio.transform())
    v = {n: env.sel(band=n).values.ravel() for n in names}
    suit = np.exp(-((v["temp"] - 6) ** 2) / 8 - ((v["prec"] - 9) ** 2) / 30) * np.where(v["cover"] == 2, 1.5, 1.0)
    rng = np.random.default_rng(8)
    cells = rng.choice(v["temp"].size, 90, replace=False, p=suit / suit.sum())
    bgc = rng.choice(v["temp"].size, 300, replace=False)
    cx = lambda c: 500.0 + (c % 40) * 1000.0  # noqa: E731
    cy = lambda c: 30000.0 - 500.0 - (c // 40) * 1000.0  # noqa: E731
    pres = pd.DataFrame({"x": cx(cells), "y_coord": cy(cells)})
    bg = pd.DataFrame({"x": cx(bgc), "y_coord": cy(bgc)})
    data = S.dataset(pres, bg, env, x="x", y="y_coord", categorical=["cover"])
    out: dict[str, np.ndarray] = {"in/env": env.values, "in/pres": pres.to_numpy(), "in/bg": bg.to_numpy()}
    t = data.table()
    out["dataset"] = t[["x", "y_coord", "y", "temp", "prec", "soil", "cover"]].to_numpy(dtype=np.float64)
    cases = []
    for name, folds in {"random": S.random_folds(data, 4, seed=2),
                        "blocks": S.spatial_blocks(data, size=6000.0, k=3, hexagon=False, seed=4)}.items():
        r = S.fit(folds, S.MaxNet(categorical=["cover"]))
        m = r.metrics.drop(columns="model").to_numpy(dtype=np.float64)
        out[f"{name}/folds"] = np.asarray(folds.folds, dtype=np.float64)
        out[f"{name}/metrics"] = m
        out[f"{name}/summary"] = r.summary.to_numpy(dtype=np.float64).ravel()
        out[f"{name}/threshold"] = np.array([r.thresholds["MaxNet"]])
        out[f"{name}/betas"] = np.array(list(r.models["MaxNet"].betas_.values()))
        cases.append(name)
    r = S.fit(folds, S.MaxNet(categorical=["cover"]), cv=False)
    for fixed in ("mean", "median"):
        rc = S.response_curves(r, n=25, fixed=fixed)
        out[f"curves_{fixed}"] = rc[["value", "suitability"]].to_numpy(dtype=np.float64)
    imp = S.importance(r, n_repeats=3, seed=7)
    out["importance"] = imp[["importance", "sd"]].to_numpy(dtype=np.float64)
    meta = {"cases": cases, "importance": imp["predictor"].tolist()}
    np.savez_compressed(REF / "sdm_workflow.npz", **out)
    (REF / "sdm_workflow.json").write_text(json.dumps(meta, indent=1) + "\n", encoding="utf-8")
    print(REF / "sdm_workflow.npz", len(out), meta)


if __name__ == "__main__":
    main()
