# SPDX-License-Identifier: GPL-3.0-or-later
"""landschaft's calibration of FUTURES' patches (ls.simulate.calibrate_patches, r.futures.calib), for Terraflux's
tests of futures.calibrate.

The case is landschaft's own (tests/simulate_cases.py's futures_calib_py: its world, a PatchGrowing model with one
subregion, the observed development at the start and three steps later, every combination of two compactness
means, two ranges and two discount factors, two runs each). The model is saved by landschaft (Model.save) into
tests/data/reference/futures_calib/model.json, its simulation node made a futures.calibrate node of the same
inputs (calibrate.json, with end.tif, the observed end); landschaft's table and patch library go into
calib.json:

    python tools/make_futures_calib_reference.py
"""

from __future__ import annotations

import json
import shutil
import sys
import warnings
from pathlib import Path

import numpy as np
import pandas as pd

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "extern" / "landschaft" / "tests"))

import landschaft as ls  # noqa: E402
import simulate_cases as sc  # noqa: E402

S = ls.simulate
OUT = ROOT / "tests" / "data" / "reference" / "futures_calib"


def main() -> None:
    shutil.rmtree(OUT, ignore_errors=True)
    OUT.mkdir(parents=True)
    world = sc.futures_world()
    r = sc._calib_rasters(world, True)
    pot = S.DevelopmentPotential(pd.DataFrame(sc._FUT_POT, columns=["ID", "Intercept", "development_pressure",
                                                                     "slope", "road", "forest"]).set_index("ID"))
    years = list(range(2021, 2021 + sc._FUT_STEPS))
    demand = S.DevelopmentDemand(pd.DataFrame({1: sc._FUT_DEMAND[1]}, index=years))
    model = S.Model(pot, demand, S.PatchGrowing([1], subregions=r["sub"], neighborhood=8))
    drivers = {k: r[k] for k in ("slope", "road", "forest")}
    c = sc._CALIB
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        t = S.calibrate_patches(model, r["dev0f"], r["dev3f"], drivers, compactness_means=c["means"],
                                compactness_ranges=c["ranges"], discount_factors=c["discounts"], repeat=c["repeat"],
                                seed=c["seed"])
        # the model as calibrate_patches runs it: its library the observed one
        model.allocation.patch_sizes = t.attrs["library"]
        model.save(OUT / "model.json", r["dev0f"], drivers, seed=0)
    r["dev3f"].astype("float32").rio.to_raster(OUT / "end.tif")
    g = json.loads((OUT / "model.json").read_text(encoding="utf-8"))
    g["inputs"]["end"] = {"kind": "map", "path": "end.tif", "note": "the development observed at the end"}
    del g["inputs"]["seed"]
    node = next(n for n in g["nodes"] if n["id"] == "simulation")
    node["op"] = "futures.calibrate"
    node["inputs"].pop("seed")
    node["inputs"]["end"] = "$end"
    for k in ("patch_sizes", "compactness_mean", "compactness_range", "discount_factor"):
        node["params"].pop(k)
    node["params"].update({"compactness_means": list(c["means"]), "compactness_ranges": list(c["ranges"]),
                           "discount_factors": list(c["discounts"]), "repeat": c["repeat"], "seed": c["seed"]})
    g["outputs"] = {"table": {"from": "simulation.table"}, "library": {"from": "simulation.library"}}
    (OUT / "calibrate.json").write_text(json.dumps(g, indent=1) + "\n", encoding="utf-8")
    (OUT / "model.json").unlink()
    (OUT / "calib.json").write_text(json.dumps({"columns": list(t.columns), "rows": t.to_numpy(np.float64).tolist(),
                                                "library": [float(v) for v in t.attrs["library"]]}, indent=1)
                                    + "\n", encoding="utf-8")
    print(t)


if __name__ == "__main__":
    main()
