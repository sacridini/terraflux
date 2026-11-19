# SPDX-License-Identifier: GPL-3.0-or-later
"""What scikit-learn's KMeans gives, for Terraflux's tests of ml.kmeans: four well-separated groups of records of
three variables in different units, and for the variables standardised (each by its standard deviation, ddof 0, as
StandardScaler) the best of 20 runs for k = 2 to 6 (inertia, labels, silhouette_score, calinski_harabasz_score); two
runs from given centres (the first the k-means++ ones scikit-learn draws, the second points picked at random, one
start) to compare Lloyd's iterations alone; and a run on the variables as they are. Written to
tests/data/reference/kmeans.json:

    python tools/make_kmeans_reference.py

Needs numpy and scikit-learn.
"""

from __future__ import annotations

import json
from pathlib import Path

import numpy as np
import sklearn
from sklearn.cluster import KMeans, kmeans_plusplus
from sklearn.metrics import calinski_harabasz_score, silhouette_score

ROOT = Path(__file__).resolve().parents[1]
REF = ROOT / "tests" / "data" / "reference"


def records(rng: np.random.Generator) -> tuple[np.ndarray, np.ndarray]:
    """Four groups (150, 100, 200 and 150 records), at unequal distances in a latent space of three variables of
    unit spread, then each variable put in other units."""
    centres = np.array([[0.0, 0.0, 0.0], [5.0, 0.0, 0.0], [0.0, 9.0, 0.0], [0.0, 0.0, 16.0]])
    sizes = [150, 100, 200, 150]
    truth = np.repeat(np.arange(4), sizes)
    f = centres[truth] + rng.normal(size=(truth.size, 3))
    x = np.column_stack([100.0 + 25.0 * f[:, 0], 0.5 * f[:, 1], 3000.0 + 400.0 * f[:, 2]])
    order = rng.permutation(truth.size)
    return x[order], truth[order]


def fit(z: np.ndarray, k: int) -> dict:
    km = KMeans(n_clusters=k, init="k-means++", n_init=20, random_state=1).fit(z)
    return {
        "k": k,
        "inertia": float(km.inertia_),
        "labels": km.labels_.tolist(),
        "centres": km.cluster_centers_.tolist(),
        "silhouette": float(silhouette_score(z, km.labels_)),
        "calinski_harabasz": float(calinski_harabasz_score(z, km.labels_)),
    }


def from_centres(z: np.ndarray, init: np.ndarray) -> dict:
    km = KMeans(n_clusters=len(init), init=init, n_init=1, max_iter=300, tol=1e-4).fit(z)
    return {
        "init": init.tolist(),
        "inertia": float(km.inertia_),
        "labels": km.labels_.tolist(),
        "centres": km.cluster_centers_.tolist(),
        "n_iter": int(km.n_iter_),
    }


def main() -> None:
    rng = np.random.default_rng(20261119)
    x, truth = records(rng)
    z = (x - x.mean(axis=0)) / x.std(axis=0, ddof=0)
    plus, _ = kmeans_plusplus(z, 4, random_state=3)
    random_points = z[rng.choice(len(z), 6, replace=False)]
    raw = KMeans(n_clusters=4, init="k-means++", n_init=20, random_state=1).fit(x)
    out = {
        "scikit-learn": sklearn.__version__,
        "columns": ["a", "b", "c"],
        "x": x.tolist(),
        "truth": truth.tolist(),
        "mean": x.mean(axis=0).tolist(),
        "scale": x.std(axis=0, ddof=0).tolist(),
        "fits": [fit(z, k) for k in range(2, 7)],
        "plus_plus": from_centres(z, plus),
        "random": from_centres(z, random_points),
        "raw": {"k": 4, "inertia": float(raw.inertia_), "labels": raw.labels_.tolist()},
    }
    (REF / "kmeans.json").write_text(json.dumps(out), encoding="utf-8")
    print("wrote", REF / "kmeans.json")


if __name__ == "__main__":
    main()
