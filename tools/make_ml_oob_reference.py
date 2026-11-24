# SPDX-License-Identifier: GPL-3.0-or-later
"""What scikit-learn's random forests give out of bag, for Terraflux's tests, written to
tests/data/reference/ml_oob.npz and ml_oob.json:

    python tools/make_ml_oob_reference.py

RandomForestClassifier(oob_score=True) and RandomForestRegressor(oob_score=True) on the table of
tools/make_ml_reference.py (the 600 records of its fit): oob_decision_function_ and oob_score_ (the accuracy),
oob_prediction_ and oob_score_ (R2). The settings go to the json.
"""

from __future__ import annotations

import json
from pathlib import Path

import numpy as np
from sklearn.ensemble import RandomForestClassifier, RandomForestRegressor

ROOT = Path(__file__).resolve().parents[1]
REF = ROOT / "tests" / "data" / "reference"


def table_data():
    # the same as tools/make_ml_reference.py's
    g = np.random.default_rng(11)
    n = 800
    x = g.normal(size=(n, 5))
    x[:, 3] = np.round(x[:, 3] * 2)
    score = x[:, 0] + 0.5 * x[:, 1] ** 2 - x[:, 2] + 0.3 * g.normal(size=n)
    cls = np.where(score < -0.5, 1.0, np.where(score < 1.0, 2.0, 5.0))
    reg = np.sin(x[:, 0]) * 3 + x[:, 1] * x[:, 2] + x[:, 3] + 0.2 * g.normal(size=n)
    return x[:600], cls[:600], reg[:600]


def main() -> None:
    x, cls, reg = table_data()
    arrays: dict[str, np.ndarray] = {}
    meta: dict = {}
    kw = {"n_estimators": 40, "random_state": 3}
    c = RandomForestClassifier(oob_score=True, **kw).fit(x, cls)
    arrays["classifier/proba"] = c.oob_decision_function_
    arrays["classifier/score"] = np.array([c.oob_score_])
    meta["classifier"] = kw
    kw = {"n_estimators": 40, "random_state": 5, "max_features": 0.4}
    r = RandomForestRegressor(oob_score=True, **kw).fit(x, reg)
    arrays["regressor/predict"] = r.oob_prediction_
    arrays["regressor/score"] = np.array([r.oob_score_])
    meta["regressor"] = kw
    np.savez_compressed(REF / "ml_oob.npz", **arrays)
    (REF / "ml_oob.json").write_text(json.dumps(meta, indent=1) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
