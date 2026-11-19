# SPDX-License-Identifier: GPL-3.0-or-later
"""What scikit-learn's PCA gives, for Terraflux's tests of ml.pca: records of five variables in different units, a
second set to score with the first's means and scales, and the components, the variances and the scores of the
correlation (each variable by its standard deviation, ddof 1) and of the covariance, written to
tests/data/reference/pca.json:

    python tools/make_pca_reference.py

Needs numpy and scikit-learn. Components are given up to sign (scikit-learn's svd_flip is not Terraflux's rule: the
largest loading positive), so the test aligns each one's sign before it compares.
"""

from __future__ import annotations

import json
from pathlib import Path

import numpy as np
import sklearn
from sklearn.decomposition import PCA

ROOT = Path(__file__).resolve().parents[1]
REF = ROOT / "tests" / "data" / "reference"


def records(rng: np.random.Generator, n: int) -> np.ndarray:
    """Five variables: three driven by two hidden factors (in other units), one noise, one nearly constant scale."""
    f = rng.normal(size=(n, 2))
    a = 100.0 + 25.0 * f[:, 0] + rng.normal(scale=4.0, size=n)
    b = 0.5 * f[:, 0] - 0.8 * f[:, 1] + rng.normal(scale=0.3, size=n)
    c = 3000.0 + 400.0 * f[:, 1] + rng.normal(scale=90.0, size=n)
    d = rng.normal(loc=7.0, scale=2.0, size=n)
    e = 0.01 * f[:, 0] + 0.02 * f[:, 1] + rng.normal(scale=0.004, size=n)
    return np.column_stack([a, b, c, d, e])


def fit(x: np.ndarray, other: np.ndarray, standardise: bool) -> dict:
    mean = x.mean(axis=0)
    scale = x.std(axis=0, ddof=1) if standardise else np.ones(x.shape[1])
    pca = PCA().fit((x - mean) / scale)
    return {
        "mean": mean.tolist(),
        "scale": scale.tolist(),
        "components": pca.components_.tolist(),
        "variance": pca.explained_variance_.tolist(),
        "ratio": pca.explained_variance_ratio_.tolist(),
        "scores": pca.transform((x - mean) / scale).tolist(),
        "other_scores": pca.transform((other - mean) / scale).tolist(),
        # n_components as a share of the variance
        "k_share": {
            str(s): int(PCA(n_components=s, svd_solver="full").fit((x - mean) / scale).n_components_)
            for s in (0.5, 0.8, 0.9, 0.95, 0.99)
        },
    }


def main() -> None:
    rng = np.random.default_rng(20261119)
    x = records(rng, 400)
    other = records(rng, 60) + np.array([5.0, 0.2, -100.0, 1.0, 0.0])
    out = {
        "scikit-learn": sklearn.__version__,
        "columns": ["a", "b", "c", "d", "e"],
        "x": x.tolist(),
        "other": other.tolist(),
        "correlation": fit(x, other, True),
        "covariance": fit(x, other, False),
    }
    (REF / "pca.json").write_text(json.dumps(out), encoding="utf-8")
    print("wrote", REF / "pca.json")


if __name__ == "__main__":
    main()
