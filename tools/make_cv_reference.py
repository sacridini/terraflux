# SPDX-License-Identifier: GPL-3.0-or-later
"""What scikit-learn gives for the cross-validation of ml.fit (its `fold` and `cv` settings), for Terraflux's tests:
150 records of four predictors with a class of three and a value, five folds of different sizes, a random forest of
ten trees (random_state 0, as ml.fit's default) fitted with each fold out and tested on it (the same as
cross_val_predict with these folds), and the accuracy, Cohen's kappa, macro F1 and F1 of each class of each fold and of
all the predictions together; for the value, the RMSE, MAE and R2. Written to tests/data/reference/cv.json:

    python -I tools/make_cv_reference.py

Needs numpy and scikit-learn.
"""

from __future__ import annotations

import json
from pathlib import Path

import numpy as np
import sklearn
from sklearn.ensemble import RandomForestClassifier, RandomForestRegressor
from sklearn.metrics import (
    accuracy_score,
    cohen_kappa_score,
    confusion_matrix,
    f1_score,
    mean_absolute_error,
    mean_squared_error,
    r2_score,
)

ROOT = Path(__file__).resolve().parents[1]
REF = ROOT / "tests" / "data" / "reference"


def scores(real: np.ndarray, pred: np.ndarray) -> dict:
    labels = np.unique(np.concatenate([real, pred]))
    return {
        "n": int(len(real)),
        "accuracy": float(accuracy_score(real, pred)),
        "kappa": float(cohen_kappa_score(real, pred)),
        "macro_f1": float(f1_score(real, pred, average="macro")),
        "f1": {str(int(c)): float(v) for c, v in zip(labels, f1_score(real, pred, average=None, labels=labels))},
    }


def main() -> None:
    rng = np.random.default_rng(20261122)
    n = 150
    x = rng.normal(size=(n, 4))
    klass = np.where(x[:, 0] + 0.5 * rng.normal(size=n) > 0.6, 3, np.where(x[:, 1] + 0.5 * rng.normal(size=n) > 0, 2, 1))
    value = 3.0 * x[:, 0] - 2.0 * x[:, 2] + rng.normal(scale=0.5, size=n)
    # five folds of different sizes
    fold = np.sort(rng.integers(1, 6, size=n))[rng.permutation(n)]
    out = {"scikit-learn": sklearn.__version__, "x": x.tolist(), "class": klass.astype(float).tolist(), "value": value.tolist(), "fold": fold.astype(float).tolist()}

    predicted = np.zeros(n)
    by_fold = {}
    for f in range(1, 6):
        test = fold == f
        model = RandomForestClassifier(n_estimators=10, random_state=0).fit(x[~test], klass[~test])
        predicted[test] = model.predict(x[test])
        by_fold[str(f)] = scores(klass[test], predicted[test])
    out["classes"] = {"folds": by_fold, "all": scores(klass, predicted)}
    out["matrix"] = confusion_matrix(klass, predicted, labels=[1, 2, 3]).tolist()  # rows reference, columns predicted

    predicted = np.zeros(n)
    by_fold = {}
    for f in range(1, 6):
        test = fold == f
        model = RandomForestRegressor(n_estimators=10, random_state=0).fit(x[~test], value[~test])
        predicted[test] = model.predict(x[test])
        by_fold[str(f)] = {
            "n": int(test.sum()),
            "rmse": float(mean_squared_error(value[test], predicted[test]) ** 0.5),
            "mae": float(mean_absolute_error(value[test], predicted[test])),
            "r2": float(r2_score(value[test], predicted[test])),
        }
    out["values"] = {
        "folds": by_fold,
        "all": {
            "n": n,
            "rmse": float(mean_squared_error(value, predicted) ** 0.5),
            "mae": float(mean_absolute_error(value, predicted)),
            "r2": float(r2_score(value, predicted)),
        },
    }
    (REF / "cv.json").write_text(json.dumps(out), encoding="utf-8")
    print("wrote", REF / "cv.json")


if __name__ == "__main__":
    main()
