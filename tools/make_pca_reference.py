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


def z(x: np.ndarray) -> np.ndarray:
    return (x - x.mean(axis=0)) / x.std(axis=0, ddof=1)


def dates() -> dict:
    """Three bands at two dates on 300 cells: the second date is the first with a change on a third of the cells.
    The stacked PCA of the six columns (band_1 at the first date, band_2 at the later), its `change` of each component
    (|a - b|^2 / (|a - b|^2 + |a + b|^2) of the loadings a of the first date and b of the later), and the selective PCA of
    each band's pair, all of the correlation."""
    rng = np.random.default_rng(20261122)
    n = 300
    f = rng.normal(size=(n, 2))
    first = np.column_stack([
        50.0 + 10.0 * f[:, 0] + rng.normal(scale=2.0, size=n),
        0.4 * f[:, 0] + 0.3 * f[:, 1] + rng.normal(scale=0.1, size=n),
        900.0 + 120.0 * f[:, 1] + rng.normal(scale=30.0, size=n),
    ])
    moved = rng.random(n) < 0.33
    later = first + rng.normal(scale=[1.0, 0.05, 15.0], size=(n, 3))
    later[moved] += np.array([25.0, -0.6, -300.0])
    both = np.hstack([first, later])
    pca = PCA().fit(z(both))
    change = []
    for v in pca.components_:
        a, b = v[:3], v[3:]
        d, s = np.sum((a - b) ** 2), np.sum((a + b) ** 2)
        change.append(float(d / (d + s)))
    selective = []
    for j in range(3):
        pair = z(np.column_stack([first[:, j], later[:, j]]))
        p = PCA().fit(pair)
        selective.append({
            "components": p.components_.tolist(),
            "variance": p.explained_variance_.tolist(),
            "scores": p.transform(pair).tolist(),
        })
    return {
        "bands": ["a", "b", "c"],
        "first": first.tolist(),
        "later": later.tolist(),
        "stacked": {
            "components": pca.components_.tolist(),
            "variance": pca.explained_variance_.tolist(),
            "scores": pca.transform(z(both)).tolist(),
            "change": change,
        },
        "selective": selective,
    }


def main() -> None:
    (REF / "pca_dates.json").write_text(json.dumps(dates()), encoding="utf-8")
    print("wrote", REF / "pca_dates.json")
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
