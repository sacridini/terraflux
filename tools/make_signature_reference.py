# SPDX-License-Identifier: GPL-3.0-or-later
"""What landschaft's SpatialSignatureAnalysis gives, for Terraflux's tests: the signature table of square zones of
landschaft's augusta.tif, its principal components, non-negative factors, k-means and hierarchical clusters and
clustergrams, and numpy's generator (PCG64) that the k-means draws from, written to
tests/data/reference/lsm_signatures.json:

    python tools/make_signature_reference.py

Run with the landschaft the submodule points at installed. NaN written null.
"""

from __future__ import annotations

import json
import math
import warnings
from pathlib import Path

import numpy as np

import landschaft as ls

ROOT = Path(__file__).resolve().parents[1]
REF = ROOT / "tests" / "data" / "reference"
AUGUSTA = ROOT / "extern" / "landschaft" / "tests" / "data" / "lsm" / "augusta.tif"

LANDSCAPE = ["shdi", "ed", "np", "lpi", "contag"]
CLASS = ["pland"]
METHODS = ["ward", "average", "complete", "single", "centroid", "median", "weighted"]


def num(v):
    v = float(v)
    return None if math.isnan(v) else v


def matrix(df) -> list:
    return [[num(v) for v in row] for row in np.asarray(df, dtype=float)]


def main() -> None:
    a = ls.load_raster(AUGUSTA)
    sig = ls.SpatialSignatureAnalysis(a, grid=600, landscape_metrics=LANDSCAPE, class_metrics=CLASS)
    out: dict = {"landschaft": ls.__version__, "grid": 600, "landscape_metrics": LANDSCAPE, "class_metrics": CLASS}
    out["columns"] = list(sig.table.columns)
    out["zones"] = [int(z) for z in sig.table.index]
    out["table"] = matrix(sig.table)
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        pca = sig.pca()
        out["pca"] = {"scores": matrix(pca.scores), "loadings": matrix(pca.loadings),
                      "columns": list(pca.loadings.index), "explained": [num(v) for v in pca.explained_variance_ratio]}
        out["nmf"] = []
        for k in (2, 3):
            f = sig.nmf(k)
            out["nmf"].append({"k": k, "scores": matrix(f.scores), "components": matrix(f.components),
                               "error": f.reconstruction_error, "n_iter": f.n_iter})
        out["cluster"] = []
        for method, k, extra in [("kmeans", 3, {}), ("kmeans", 4, {"seed": 1}), ("kmeans", 5, {"n_init": 3}),
                                 ("kmeans", 3, {"n_components": 2})] + [(m, 4, {}) for m in METHODS]:
            labels = sig.cluster(k, method=method, **extra)
            out["cluster"].append({"method": method, "k": k, **extra, "labels": [int(v) for v in labels]})
        out["clustergram"] = []
        for method, weighted in [("kmeans", True), ("ward", True), ("average", False)]:
            cg = sig.clustergram(range(1, 7), method=method, pca_weighted=weighted)
            out["clustergram"].append({"method": method, "pca_weighted": weighted, "labels": matrix(cg.labels),
                                       "means": matrix(cg.means)})
    # numpy's generator: its raw draws, integers and doubles, and a choice by probabilities
    g = np.random.default_rng(7)
    rng = {"seed": 7, "raw": [str(v) for v in g.bit_generator.random_raw(4)]}
    g = np.random.default_rng(7)
    rng["integers"] = [int(g.integers(n)) for n in (1, 5, 100, 7, 2**31 + 5, 3)]
    rng["random"] = [float(g.random()) for _ in range(3)]
    p = np.array([0.1, 0.0, 0.25, 0.4, 0.25])
    rng["choice"] = [int(g.choice(5, p=p)) for _ in range(6)]
    out["rng"] = rng
    path = REF / "lsm_signatures.json"
    path.write_text(json.dumps(out, indent=1) + "\n", encoding="utf-8")
    print(path)


if __name__ == "__main__":
    main()
