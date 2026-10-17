# SPDX-License-Identifier: GPL-3.0-or-later
"""What scikit-learn's random forests and histogram gradient boosting and landschaft's Estimator give, for
Terraflux's tests, written to tests/data/reference/ml.npz and ml.json:

    python tools/make_ml_reference.py

- Forests on a table of 600 records and five predictors (a test of 200): RandomForestClassifier of three classes
  (labels 1, 2 and 5) with the defaults and with a depth, a leaf size, two features per split and no bootstrap;
  RandomForestRegressor with the defaults and with a third of the features per split: predict_proba / predict
  and feature_importances_.
- HistGradientBoostingClassifier (two and three classes) and Regressor without early stopping, with the defaults
  and with every setting changed, their predict_proba / predict; a regressor of 210,000 records (its bins from a
  subsample).
- landschaft's Estimator on the Plum Island maps (pie_landuse, pie_drivers): PLUS's land expansion analysis
  (target "expansion", 1985 to 2010, a sample of 4000 cells and a stratified one) and the presence of each class
  (a balanced sample), with RandomForestClassifier(n_estimators=10, random_state=0): the probability maps and
  the drivers' importances; and the expansion by HistGradientBoostingClassifier(max_iter=20).
"""

from __future__ import annotations

import json
from pathlib import Path

import numpy as np
from sklearn.ensemble import (HistGradientBoostingClassifier, HistGradientBoostingRegressor,
                              RandomForestClassifier, RandomForestRegressor)

import landschaft as ls
from landschaft.simulate._maps import read_map

ROOT = Path(__file__).resolve().parents[1]
REF = ROOT / "tests" / "data" / "reference"


def table_data():
    g = np.random.default_rng(11)
    n = 800
    x = g.normal(size=(n, 5))
    x[:, 3] = np.round(x[:, 3] * 2)  # a predictor of few values (ties)
    score = x[:, 0] + 0.5 * x[:, 1] ** 2 - x[:, 2] + 0.3 * g.normal(size=n)
    cls = np.where(score < -0.5, 1.0, np.where(score < 1.0, 2.0, 5.0))
    reg = np.sin(x[:, 0]) * 3 + x[:, 1] * x[:, 2] + x[:, 3] + 0.2 * g.normal(size=n)
    return x[:600], cls[:600], reg[:600], x[600:]


def main() -> None:
    arrays: dict[str, np.ndarray] = {}
    meta: dict = {}
    x, cls, reg, test = table_data()
    arrays["table/x"], arrays["table/class"], arrays["table/value"], arrays["table/test"] = x, cls, reg, test

    for name, kw in {
        "default": {"n_estimators": 30, "random_state": 3},
        "shaped": {"n_estimators": 15, "random_state": 8, "max_depth": 5, "min_samples_leaf": 3,
                   "max_features": 2, "bootstrap": False},
    }.items():
        m = RandomForestClassifier(**kw).fit(x, cls)
        arrays[f"classifier/{name}/proba"] = m.predict_proba(test)
        arrays[f"classifier/{name}/importance"] = m.feature_importances_
        meta[f"classifier/{name}"] = kw
    for name, kw in {
        "default": {"n_estimators": 20, "random_state": 5},
        "shaped": {"n_estimators": 12, "random_state": 9, "max_features": 0.4, "min_samples_split": 6},
    }.items():
        m = RandomForestRegressor(**kw).fit(x, reg)
        arrays[f"regressor/{name}/predict"] = m.predict(test)
        arrays[f"regressor/{name}/importance"] = m.feature_importances_
        meta[f"regressor/{name}"] = kw

    # histogram gradient boosting, without early stopping
    for name, (est, target, kw) in {
        "binary": (HistGradientBoostingClassifier, "binary", {"max_iter": 30}),
        "multiclass": (HistGradientBoostingClassifier, "table/class", {"max_iter": 25}),
        "shaped": (HistGradientBoostingClassifier, "table/class",
                   {"max_iter": 15, "learning_rate": 0.2, "max_leaf_nodes": 8, "min_samples_leaf": 10,
                    "l2_regularization": 1.0, "max_bins": 32, "max_features": 0.5, "max_depth": 4,
                    "random_state": 4}),
        "regression": (HistGradientBoostingRegressor, "table/value", {"max_iter": 30}),
    }.items():
        y = (cls == 2.0) * 7.0 if target == "binary" else arrays[target]
        m = est(early_stopping=False, **kw).fit(x, y)
        out = m.predict_proba(test) if est is HistGradientBoostingClassifier else m.predict(test)
        arrays[f"boosting/{name}/predict"] = out
        if target == "binary":
            arrays["table/binary"] = y
        meta[f"boosting/{name}"] = {"target": target, **kw}
    # more than 200,000 records: the thresholds from a subsample (integers' quotients, the same in Rust)
    i = np.arange(210_000)
    big = np.column_stack([(i * 7919 % 100003) / 100003, (i * 104729 % 99991) / 99991])
    yb = big[:, 0] * 3.0 + (big[:, 1] > 0.5) - big[:, 0] * big[:, 1]
    m = HistGradientBoostingRegressor(early_stopping=False, max_iter=3, random_state=2).fit(big, yb)
    arrays["boosting/big/predict"] = m.predict(big[:: 2100])

    lu = ls.load_raster(ls.simulate.example_data("pie_landuse"))
    ef = ls.load_raster(ls.simulate.example_data("pie_drivers"))
    arrays["pie/t0"] = read_map(lu[0]).as_float()
    arrays["pie/t1"] = read_map(lu[2]).as_float()
    names = [str(v) for v in ef[ef.dims[0]].values]
    for k, n in enumerate(names):
        arrays[f"pie/driver/{n}"] = read_map(ef[k]).as_float()
    meta["pie/drivers"] = names
    rf = RandomForestClassifier(n_estimators=10, random_state=0)
    S = ls.simulate
    for name, kw in {
        "expansion": {"target": "expansion", "sample": 4000, "seed": 1},
        "stratified": {"target": "expansion", "sample": 0.05, "stratify": True, "seed": 2},
        "presence": {"target": "presence", "sample": 3000, "balance": True, "seed": 3},
        "boosting": {"target": "expansion", "sample": 4000, "seed": 4},
    }.items():
        model = HistGradientBoostingClassifier(max_iter=20, early_stopping=False, random_state=0) \
            if name == "boosting" else rf
        est = S.Estimator(model, **kw)
        est.fit(lu[0], ef, t1=lu[2] if kw["target"] == "expansion" else None)
        p = est.predict(ef)
        for c in p[p.dims[0]].values:
            arrays[f"pie/{name}/{int(c)}"] = p.sel({p.dims[0]: c}).values
        if name != "boosting":  # scikit-learn's boosting has no importances
            arrays[f"pie/{name}/importance"] = est.importance().values
        meta[f"pie/{name}"] = {**kw, "classes": [int(c) for c in p[p.dims[0]].values],
                               "drivers": sorted(names), "train": {str(int(c)): int(len(v[0]))
                                                                      for c, v in est._train.items()}}

    np.savez_compressed(REF / "ml.npz", **arrays)
    (REF / "ml.json").write_text(json.dumps(meta, indent=1) + "\n", encoding="utf-8")
    print(REF / "ml.npz", len(arrays))


if __name__ == "__main__":
    main()
