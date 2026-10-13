# SPDX-License-Identifier: GPL-3.0-or-later
"""What landschaft's partitions for cross-validation give, for Terraflux's tests, written to
tests/data/reference/sdm_folds.npz and .json:

    python tools/make_sdm_folds_reference.py

Presences and background points in lon/lat (and a projected copy): random folds, jackknife, the four quadrant
orientations, checkerboards of one and two aggregations, spatial blocks (hexagons pointy and flat, squares of a
size or of rows and columns; random, systematic and checkerboard selection), clusters of the coordinates and of
the predictors, and buffered folds.
"""

from __future__ import annotations

import json
from pathlib import Path

import geopandas as gpd
import numpy as np
import pandas as pd
import xarray as xr
from affine import Affine

import landschaft as ls
from landschaft.sdm import SDMData

ROOT = Path(__file__).resolve().parents[1]
REF = ROOT / "tests" / "data" / "reference"
S = ls.sdm


def main() -> None:
    rng = np.random.default_rng(21)
    n_p, n_b = 60, 90
    lon = np.round(np.r_[rng.uniform(-59, -52, n_p), rng.uniform(-60, -50, n_b)], 4)
    lat = np.round(np.r_[rng.uniform(-19, -14, n_p), rng.uniform(-20, -12, n_b)], 4)
    y = np.r_[np.ones(n_p), np.zeros(n_b)].astype(np.int8)
    coords = gpd.GeoDataFrame({"y": y}, geometry=gpd.points_from_xy(lon, lat), crs="EPSG:4326")
    data = SDMData(pd.DataFrame({"a": lon}), y, coords)
    t = (-60.5, 0.25, 0.0, -11.5, 0.0, -0.25)
    layers = [ls.nlm.gaussian_field(48, 40, autocorr_range=6 + 4 * k, seed=61 + k).values * 10 for k in range(2)]
    env = xr.DataArray(np.stack(layers), dims=("band", "y", "x"),
                       coords={"band": ["a", "b"], "y": t[3] + (np.arange(40) + 0.5) * t[5],
                               "x": t[0] + (np.arange(48) + 0.5) * t[1]})
    env = env.rio.write_transform(Affine(t[1], 0, t[0], 0, t[5], t[3])).rio.write_crs("EPSG:4326")
    px, py = lon * 100000.0, lat * 100000.0
    pcoords = gpd.GeoDataFrame({"y": y}, geometry=gpd.points_from_xy(px, py))
    pdata = SDMData(pd.DataFrame({"a": px}), y, pcoords)

    out: dict[str, np.ndarray] = {"in/lon": lon, "in/lat": lat, "in/y": y.astype(float), "in/env": env.values}
    cases: list[dict] = []

    def case(name, op, params, folds, projected=False, env_used=False):
        f = folds.folds if isinstance(folds, SDMData) else folds
        out[name] = np.asarray(f, dtype=np.float64)
        cases.append({"name": name, "op": op, "params": params, "projected": projected, "env": env_used})

    case("random", "sdm.random_folds", {"k": 5, "seed": 3}, S.random_folds(data, 5, seed=3))
    case("jackknife", "sdm.jackknife_folds", {}, S.jackknife_folds(data))
    for o in ("lat_lon", "lon_lat", "lon_lon", "lat_lat"):
        case(f"quadrant_{o}", "sdm.quadrant_folds", {"orientation": o}, S.quadrant_folds(data, o))
    case("checker", "sdm.checkerboard_folds", {"aggregation": 4}, S.checkerboard_folds(data, env, 4), env_used=True)
    case("checker2", "sdm.checkerboard_folds", {"aggregation": [2, 3]}, S.checkerboard_folds(data, env, [2, 3]),
         env_used=True)
    blocks = {"hex": {"size": 150000.0, "k": 4, "seed": 5},
              "hex_flat": {"size": 120000.0, "k": 3, "flat_top": True, "seed": 6, "offset": [0.3, 0.1]},
              "hex_systematic": {"size": 150000.0, "k": 4, "selection": "systematic", "seed": 11},
              "squares": {"size": 200000.0, "k": 4, "hexagon": False, "seed": 8, "iteration": 50},
              "squares_rc": {"rows_cols": [6, 5], "k": 3, "hexagon": False, "seed": 9, "extend": 2.0},
              "squares_checker": {"size": 150000.0, "hexagon": False, "selection": "checkerboard", "seed": 12},
              "hex_rows": {"rows_cols": [7, 7], "k": 3, "seed": 10}}
    for name, kw in blocks.items():
        args = dict(kw)
        if "rows_cols" in args:
            args["rows_cols"] = tuple(args["rows_cols"])
        if "offset" in args:
            args["offset"] = tuple(args["offset"])
        case(f"blocks_{name}", "sdm.spatial_blocks", kw, S.spatial_blocks(data, **args))
    case("clusters", "sdm.spatial_clusters", {"k": 4, "seed": 7}, S.spatial_clusters(data, 4, seed=7))
    case("clusters_env", "sdm.spatial_clusters", {"k": 3, "seed": 8}, S.spatial_clusters(data, 3, env=env, seed=8),
         env_used=True)
    case("clusters_lloyd", "sdm.spatial_clusters", {"k": 3, "seed": 9, "algorithm": "Lloyd"},
         S.spatial_clusters(pdata, 3, seed=9, algorithm="Lloyd"), projected=True)
    for name, d, kw, dd in (("buffer", data, {}, 150000.0), ("buffer_bg", data, {"add_background": True}, 150000.0),
                            ("buffer_planar", pdata, {"presence_background": False}, 100000.0)):
        folds = S.buffer_folds(d, dd, **kw)
        out[name] = np.array([len(tr) for tr, _ in folds] + [len(te) for _, te in folds]
                             + [int(tr.sum()) for tr, _ in folds] + [int(te.sum()) for _, te in folds], dtype=np.float64)
        cases.append({"name": name, "op": "sdm.buffer_folds", "params": {"size": dd, **kw},
                      "projected": d is pdata, "env": False})
    np.savez_compressed(REF / "sdm_folds.npz", **out)
    (REF / "sdm_folds.json").write_text(json.dumps({"cases": cases, "transform": t}, indent=1) + "\n", encoding="utf-8")
    print(REF / "sdm_folds.npz", len(out))


if __name__ == "__main__":
    main()
