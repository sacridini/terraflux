# SPDX-License-Identifier: GPL-3.0-or-later
"""landschaft's fit with scikit-learn's models (a GLM: StandardScaler and LogisticRegression; random forests:
RandomForestClassifier), for Terraflux's tests of sdm.fit's glm and random_forest, written to
tests/data/reference/sdm_models.npz and .json:

    python tools/make_sdm_models_reference.py

The records are those of tools/make_sdm_workflow_reference.py (presences and background on three continuous
predictors and a categorical one, random folds). Per case: the folds' metrics, their means and standard deviations,
the threshold and the model's suitability at every record. The GLMs are fitted to a tight tolerance (the optimum);
the forests are scikit-learn's own.
"""

from __future__ import annotations

import json
import warnings
from pathlib import Path

import numpy as np
import pandas as pd
import xarray as xr
from sklearn.ensemble import RandomForestClassifier
from sklearn.linear_model import LogisticRegression
from sklearn.pipeline import make_pipeline
from sklearn.preprocessing import StandardScaler

import landschaft as ls

ROOT = Path(__file__).resolve().parents[1]
REF = ROOT / "tests" / "data" / "reference"
S = ls.sdm


def records():
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
    return S.random_folds(data, 4, seed=2)


def glm(c):
    return make_pipeline(StandardScaler(), LogisticRegression(C=c, tol=1e-12, max_iter=100000))


# name: (the estimator, sdm.fit's settings)
CASES = {
    "glm": (glm(1.0), {"model": "glm"}),
    "glm_c01": (glm(0.1), {"model": "glm", "C": 0.1}),
    "rf": (RandomForestClassifier(random_state=1), {"model": "random_forest"}),
    "rf_leaf5": (RandomForestClassifier(50, min_samples_leaf=5, random_state=1),
                 {"model": "random_forest", "n_estimators": 50, "min_samples_leaf": 5}),
    "rf_all_depth": (RandomForestClassifier(30, max_features=None, max_depth=4, random_state=11),
                     {"model": "random_forest", "n_estimators": 30, "max_features": "all", "max_depth": 4,
                      "random_state": 11}),
    "rf_nobootstrap": (RandomForestClassifier(20, bootstrap=False, min_samples_split=6, max_features=2,
                                              random_state=7),
                       {"model": "random_forest", "n_estimators": 20, "bootstrap": False, "min_samples_split": 6,
                        "max_features": 2, "random_state": 7}),
}


def main() -> None:
    data = records()
    X, y = data.X, data.y
    out = {"records": np.column_stack([X.to_numpy(np.float64), y, np.asarray(data.folds, np.float64)])}
    meta = {"columns": [*X.columns, "presence", "fold"], "cases": {}}
    for name, (est, settings) in CASES.items():
        with warnings.catch_warnings():
            warnings.simplefilter("ignore")
            r = S.fit(data, {name: est})
        m = r.models[name]
        out[f"{name}/metrics"] = r.metrics.drop(columns="model").to_numpy(dtype=np.float64)
        out[f"{name}/summary"] = r.summary.to_numpy(dtype=np.float64).ravel()
        out[f"{name}/threshold"] = np.array([r.thresholds[name]])
        out[f"{name}/suitability"] = m.predict_proba(X)[:, 1]
        meta["cases"][name] = settings
    np.savez_compressed(REF / "sdm_models.npz", **out)
    (REF / "sdm_models.json").write_text(json.dumps(meta, indent=1) + "\n", encoding="utf-8")
    print(REF / "sdm_models.npz", list(meta["cases"]))


if __name__ == "__main__":
    main()
