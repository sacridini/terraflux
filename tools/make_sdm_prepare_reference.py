# SPDX-License-Identifier: GPL-3.0-or-later
"""What landschaft's preparation of a species distribution model gives, for Terraflux's tests, written to
tests/data/reference/sdm_prepare.npz and .json (the records are sdm_area_bradypus.csv, the predictors landschaft's
bioclim):

    python tools/make_sdm_prepare_reference.py

Bradypus' records over the bioclimatic layers: the steps of landschaft's own test (one per cell, thinning, the
area of a buffer, the predictors, background points, spatial blocks), the defaults without cleaning, cleaning
without the sea, an area given as polygons (landschaft's own buffer), the other kinds of folds, blocks of the
autocorrelation range. Per case the records (x, y, presence, the predictors, the fold), the predictors kept and
the report.
"""

from __future__ import annotations

import json
from pathlib import Path

import geopandas as gpd
import numpy as np
import pandas as pd

import landschaft as ls

ROOT = Path(__file__).resolve().parents[1]
REF = ROOT / "tests" / "data" / "reference"


def main() -> None:
    occ = pd.read_csv(REF / "sdm_area_bradypus.csv", float_precision="round_trip")
    env = ls.load_raster(ls.sdm.example_data("bioclim"))
    given = gpd.read_file(REF / "sdm_area.gpkg", layer="buffer")
    cases = {
        "steps": {"clean": False, "thin": 50_000, "area": 800_000, "predictors": {"vif": 10}, "background": 1500,
                  "folds": 4, "seed": 3},
        "defaults": {"clean": False, "background": 800, "folds": False},
        "clean": {"clean": {"seas": False}, "background": 600, "folds": 3, "seed": 2},
        "given_area": {"clean": False, "_area": True, "background": 700, "folds": {"method": "random", "k": 4}},
        "quadrants": {"clean": False, "background": 800, "folds": {"method": "quadrants"}, "predictors": False},
        "checkerboard": {"clean": False, "background": 800, "folds": {"method": "checkerboard", "aggregation": 4}},
        "clusters": {"clean": False, "background": 500, "folds": {"method": "clusters", "k": 4}, "seed": 5},
        "jackknife": {"clean": False, "thin": {"distance": 200_000, "reps": 3}, "background": 300,
                      "folds": {"method": "jackknife"}, "predictors": 5},
        "auto": {"clean": False, "background": 900, "folds": {"size": "auto", "k": 3}, "seed": 4},
    }
    out: dict[str, np.ndarray] = {"in/env": env.values.astype(np.float64)}
    meta = {"env": {"names": [str(b) for b in env.band.values], "transform": list(env.rio.transform())[:6]},
            "cases": {}}
    for name, kw in cases.items():
        args = {k: v for k, v in kw.items() if not k.startswith("_")}
        if kw.get("_area"):
            args["area"] = given
        data = ls.sdm.prepare(occ, env, **args)
        t = data.table()
        cols = ["x", "y_coord", "y", *data.predictors] + (["fold"] if data.folds is not None else [])
        out[name] = t[cols].to_numpy(dtype=np.float64)
        rep = data.report[["step", "presences", "removed", "detail"]]
        meta["cases"][name] = {"settings": kw, "predictors": list(data.predictors),
                               "report": [[r.step, int(r.presences), int(r.removed), r.detail]
                                          for r in rep.itertuples(index=False)]}
        print(name, out[name].shape, data.predictors)
        print(rep.to_string(index=False))
    np.savez_compressed(REF / "sdm_prepare.npz", **out)
    (REF / "sdm_prepare.json").write_text(json.dumps(meta, indent=1) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
