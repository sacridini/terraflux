# SPDX-License-Identifier: GPL-3.0-or-later
"""What landschaft's neutral landscape models give, for Terraflux's tests, written to
tests/data/reference/nlm.npz and nlm.json:

    python tools/make_nlm_reference.py

Every model of landschaft.nlm on small grids (with and without NLMR's quirks where they differ; the Voronoi
mosaics without them, each germ with its own value) and the utilities (classify, binarize, merge, rescale). The
json holds each case's model, settings and Terraflux's operator and parameters; the npz its values.
"""

from __future__ import annotations

import json
from pathlib import Path

import numpy as np

import landschaft as ls

ROOT = Path(__file__).resolve().parents[1]
REF = ROOT / "tests" / "data" / "reference"
N = ls.nlm


def main() -> None:
    out: dict[str, np.ndarray] = {}
    cases: list[dict] = []

    def case(name: str, op: str, params: dict, da, inputs: dict | None = None) -> None:
        values = np.asarray(getattr(da, "values", da), dtype=np.float64)
        out[name] = values
        cases.append({"name": name, "op": op, "params": params, "inputs": inputs or {}, "shape": list(values.shape)})

    size = {"rows": 13, "columns": 17}
    case("random", "nlm.random", {**size, "seed": 7}, N.random(17, 13, seed=7))
    case("random_raw", "nlm.random", {**size, "seed": -3, "rescale": False}, N.random(17, 13, seed=-3, rescale=False))
    case("percolation", "nlm.percolation", {**size, "seed": 5, "prob": 0.4}, N.percolation(17, 13, prob=0.4, seed=5))
    g = N.planar_gradient(17, 13, seed=11)
    case("planar", "nlm.planar_gradient", {**size, "seed": 11}, g)
    out["planar_direction"] = np.array([g.attrs["direction"]])
    case("planar_raw", "nlm.planar_gradient", {**size, "direction": 37.5, "rescale": False},
         N.planar_gradient(17, 13, direction=37.5, rescale=False))
    case("edge", "nlm.edge_gradient", {**size, "direction": 120.0}, N.edge_gradient(17, 13, direction=120.0))
    g = N.edge_gradient(17, 13, seed=9)
    case("edge_drawn", "nlm.edge_gradient", {**size, "seed": 9}, g)
    out["edge_drawn_direction"] = np.array([g.attrs["direction"]])
    case("distance_haversine", "nlm.distance_gradient", {"rows": 20, "columns": 30, "origin": [5, 10, 3, 8]},
         N.distance_gradient(30, 20, origin=[5, 10, 3, 8]))
    case("distance_gdal", "nlm.distance_gradient", {"rows": 20, "columns": 400, "origin": [1, 3, 2, 5]},
         N.distance_gradient(400, 20, origin=[1, 3, 2, 5]))
    case("distance_exact", "nlm.distance_gradient",
         {"rows": 20, "columns": 30, "resolution": 2.5, "origin": [3, 8, 5, 10], "nlmr_compat": False,
          "rescale": False},
         N.distance_gradient(30, 20, origin=[3, 8, 5, 10], resolution=2.5, nlmr_compat=False, rescale=False))
    case("curds", "nlm.curds", {"curds": [0.5, 0.3, 0.6], "steps": [4, 3, 2], "seed": 3},
         N.curds([0.5, 0.3, 0.6], [4, 3, 2], seed=3))
    case("curds_wheyed", "nlm.curds",
         {"curds": [0.5, 0.3, 0.6], "steps": [4, 3, 2], "wheyes": [0.1, 0.2, 0.3], "seed": 4},
         N.curds([0.5, 0.3, 0.6], [4, 3, 2], wheyes=[0.1, 0.2, 0.3], seed=4))
    case("mpd", "nlm.mpd", {"rows": 20, "columns": 25, "seed": 6, "roughness": 0.7},
         N.mpd(25, 20, roughness=0.7, seed=6))
    case("mpd_torus", "nlm.mpd", {"rows": 20, "columns": 25, "seed": 6, "torus": True, "nlmr_compat": False},
         N.mpd(25, 20, seed=6, torus=True, nlmr_compat=False))
    case("neigh", "nlm.neigh", {"rows": 15, "columns": 20, "seed": 4, "p_neigh": 0.7, "p_empty": 0.1},
         N.neigh(20, 15, p_neigh=0.7, p_empty=0.1, seed=4))
    case("neigh_props", "nlm.neigh",
         {"rows": 15, "columns": 20, "seed": -8, "p_neigh": 0.5, "p_empty": 0.3, "neighbourhood": 8,
          "proportions": [0.2, 0.3, 0.5], "rescale": False},
         N.neigh(20, 15, p_neigh=0.5, p_empty=0.3, neighbourhood=8, proportions=[0.2, 0.3, 0.5], seed=-8,
                 rescale=False))
    case("rectangles", "nlm.random_rectangular_cluster", {"rows": 15, "columns": 20, "seed": 2, "minl": 2, "maxl": 5},
         N.random_rectangular_cluster(20, 15, minl=2, maxl=5, seed=2))
    mf = N.mosaic_field(18, 16, n=5, collect=True, infinit=True, seed=12)
    case("mosaic", "nlm.mosaic_field", {"rows": 16, "columns": 18, "n": 5, "seed": 12, "collect": True,
                                        "infinit": True}, mf["mosaic_field"])
    out["mosaic/steps"] = np.asarray(mf["steps"].values, dtype=np.float64)
    out["mosaic/infinit"] = np.asarray(mf["infinit"].values, dtype=np.float64)
    mf = N.mosaic_field(18, 16, n=4, collect=True, nlmr_compat=False, rescale=True, seed=13)
    case("mosaic_plain", "nlm.mosaic_field", {"rows": 16, "columns": 18, "n": 4, "seed": 13, "collect": True,
                                              "nlmr_compat": False}, mf["mosaic_field"])
    out["mosaic_plain/steps"] = np.asarray(mf["steps"].values, dtype=np.float64)
    case("mosaic_infinit", "nlm.mosaic_field", {"rows": 16, "columns": 18, "n": 0, "infinit": True, "seed": 14,
                                                "rescale": False},
         N.mosaic_field(18, 16, n=None, infinit=True, seed=14, rescale=False))
    case("tess", "nlm.mosaic_tess", {"rows": 20, "columns": 25, "germs": 30, "seed": 3},
         N.mosaic_tess(25, 20, germs=30, seed=3, nlmr_compat=False))
    case("gibbs", "nlm.mosaic_gibbs", {"rows": 20, "columns": 25, "germs": 15, "r": 2.0, "patch_classes": 4,
                                       "seed": 3, "rescale": False},
         N.mosaic_gibbs(25, 20, germs=15, R=2.0, patch_classes=4, seed=3, nlmr_compat=False, rescale=False))
    case("random_cluster", "nlm.random_cluster", {**size, "seed": 8, "p": 0.4, "weights": [0.3, 0.7]},
         N.random_cluster(17, 13, p=0.4, ai=[0.3, 0.7], seed=8, rescale=False))
    case("gaussian", "nlm.gaussian_field", {**size, "seed": 2, "range": 5.0},
         N.gaussian_field(17, 13, autocorr_range=5.0, seed=2))
    case("fbm", "nlm.fbm", {**size, "seed": 2, "dimension": 1.3}, N.fbm(17, 13, fract_dim=1.3, seed=2))

    # the utilities, on maps given as inputs (their values in the npz under "in/...")
    x = np.asarray(N.random(17, 13, seed=21).values, dtype=np.float64)
    x[2, 3:7] = np.nan
    real = np.asarray(N.random_cluster(17, 13, p=0.5, ai=[0.2, 0.3, 0.5], seed=22, rescale=False).values,
                      dtype=np.float64)
    y = np.asarray(N.planar_gradient(17, 13, direction=60.0).values, dtype=np.float64)
    out["in/x"], out["in/real"], out["in/y"] = x, real, y
    case("classify_n", "nlm.classify", {"n": 4}, N.classify(x, n=4), {"map": "x"})
    case("classify_weights", "nlm.classify", {"weights": [0.2, None, 0.5]}, N.classify(x, weighting=[0.2, None, 0.5]),
         {"map": "x"})
    case("classify_real", "nlm.classify", {"mask_value": 3.0}, N.classify(x, real_land=real, mask_val=3.0),
         {"map": "x", "real": "real"})
    case("binarize", "nlm.binarize", {"breaks": 0.3}, N.binarize(x, 0.3), {"map": "x"})
    case("binarize_many", "nlm.binarize", {"breaks": [0.2, 0.5, 0.8]}, N.binarize(x, [0.2, 0.5, 0.8]), {"map": "x"})
    case("merge", "nlm.merge", {"scaling": 0.5}, N.merge(y, [x, real], scaling=0.5),
         {"primary": "y", "secondary": ["x", "real"]})
    case("merge_one", "nlm.merge", {"rescale": False}, N.merge(y, real, rescale=False),
         {"primary": "y", "secondary": "real"})
    case("rescale", "nlm.rescale", {}, N.rescale(x), {"map": "x"})

    np.savez_compressed(REF / "nlm.npz", **out)
    (REF / "nlm.json").write_text(json.dumps(cases, indent=1) + "\n", encoding="utf-8")
    print(REF / "nlm.npz", len(out))


if __name__ == "__main__":
    main()
