# SPDX-License-Identifier: GPL-3.0-or-later
"""landschaft's multi-action problems from rasters (ls.prioritize.actions_raster_data), for Terraflux's tests of
prioritize.actions_raster.

The rasters of landschaft's tests (tests/test_prioritize_actions.py's _rasters: a 9 x 8 grid with cells without a
cost, three features, two threats and a raster of action costs) go into tests/data/reference/actions_raster.npz;
per case its settings, the tables landschaft builds (pu, features, dist_features, threats, dist_threats, boundary)
and the optimum of its problem (HiGHS through landschaft's closure engine, gap 0) into actions_raster.json. The
example case is landschaft's own example (prioritize/data: cost.tif, features.tif, threats.tif), its tables and
its optimum at gap 0.01:

    python tools/make_actions_raster_reference.py
"""

from __future__ import annotations

import json
import sys
import warnings
from pathlib import Path

import numpy as np
import pandas as pd

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "extern" / "landschaft" / "tests"))

import landschaft as ls  # noqa: E402
import optimize_cases  # noqa: E402

P = ls.prioritize
OUT = ROOT / "tests" / "data" / "reference"


def rasters(seed=11, ny=9, nx=8):
    """test_prioritize_actions.py's _rasters."""
    rng = np.random.default_rng(seed)
    cost = rng.integers(1, 4, (ny, nx)).astype(float)
    cost[0, :3] = np.nan
    feats = np.where(rng.random((3, ny, nx)) < 0.4, rng.integers(1, 4, (3, ny, nx)), 0).astype(float)
    feats[1, 4, 4] = np.nan
    thr = np.where(rng.random((2, ny, nx)) < 0.5, rng.random((2, ny, nx)).round(2) + 0.01, 0.0)
    acost = rng.integers(1, 6, (ny, nx)).astype(float)
    return cost, feats, thr, acost


def table(df: pd.DataFrame | None) -> dict | None:
    """A table's numeric columns (the names left out)."""
    if df is None:
        return None
    df = df[[c for c in df.columns if c != "name" and pd.api.types.is_numeric_dtype(df[c])]]
    return {"columns": list(df.columns), "rows": df.to_numpy(np.float64).tolist()}


def main() -> None:
    cost, feats, thr, acost = rasters()
    locked_in = np.zeros(cost.shape)
    locked_in[2, 2] = 1
    locked_in[6, 1] = 1
    locked_out = np.zeros(cost.shape)
    locked_out[5, 5] = 1
    locked_out[2, 2] = np.nan
    arrays = {"cost": cost, "features": feats, "threats": thr, "action_cost": acost, "locked_in": locked_in,
              "locked_out": locked_out}
    g = optimize_cases.grid
    grids = {"cost": g(cost), "features": g(feats, names=["sp1", "sp2", "sp3"]),
             "threats": g(thr, names=["fire", "cattle"]), "action_cost": g(acost), "locked_in": g(locked_in),
             "locked_out": g(locked_out)}
    sens = pd.DataFrame({"feature": ["sp1", "sp2", 3, "sp3"], "threat": ["fire", "fire", "cattle", 2],
                         "delta1": [0.0, 0.1, 0.0, 0.2], "delta2": [1.0, 0.9, 0.8, 1.0]})
    # name: (actions_raster_data's keyword arguments in Terraflux's form, its own, the problem's settings, the gap)
    cases = {
        "absolute": ({"action_cost": {"fire": 2.0, "cattle": "action_cost"}, "target_recovery": [3, 2, 4],
                      "target_conservation": 1, "relative": False},
                     {"action_cost": {"fire": 2.0, "cattle": grids["action_cost"]}, "target_recovery": [3, 2, 4],
                      "target_conservation": 1, "relative": False}, {}),
        "relative": ({"action_cost": 1.0, "target_recovery": 0.5, "target_conservation": 0.25},
                     {"action_cost": 1.0, "target_recovery": 0.5, "target_conservation": 0.25}, {}),
        "boundary": ({"action_cost": "action_cost", "target_recovery": 0.4, "boundary": True,
                      "blm_actions": [0.5, 0.0]},
                     {"action_cost": grids["action_cost"], "target_recovery": 0.4, "boundary": True,
                      "blm_actions": [0.5, 0.0]}, {"blm": 0.1}),
        "locked": ({"action_cost": [1.5, "action_cost"], "target_recovery": {"sp1": 0.3, "sp2": 0.2, "sp3": 0.5},
                    "locked_in": True, "locked_out": True, "sensitivity": True},
                   {"action_cost": [1.5, grids["action_cost"]], "target_recovery": {"sp1": 0.3, "sp2": 0.2, "sp3": 0.5},
                    "locked_in": grids["locked_in"], "locked_out": grids["locked_out"], "sensitivity": sens}, {}),
        "benefits": ({"action_cost": "action_cost", "target_recovery": 0.2, "boundary": True},
                     {"action_cost": grids["action_cost"], "target_recovery": 0.2, "boundary": True},
                     {"model": "maximizeBenefits", "budget": 40.0, "blm": 0.05, "curve": 2}),
    }
    out = {}
    for name, (ours, theirs, problem) in cases.items():
        with warnings.catch_warnings():
            warnings.simplefilter("ignore")
            d = P.actions_raster_data(grids["cost"], grids["features"], grids["threats"], **theirs)
            model = problem.get("model", "minimizeCosts")
            kw = {k: v for k, v in problem.items() if k != "model"}
            s = d.problem(model, **kw).add_highs_solver(gap=0).solve()
        out[name] = {"settings": ours, "problem": problem, "gap": 0.0,
                     "tables": {k: table(getattr(d, k)) for k in ("pu", "features", "dist_features", "threats",
                                                                   "dist_threats", "boundary")},
                     "objective": float(s.objective), "units": s.units.tolist(), "actions": s.actions.tolist()}
        print(name, s.status, s.objective)
    # landschaft's example
    data = ROOT / "extern" / "landschaft" / "src" / "landschaft" / "prioritize" / "data"
    cost_e = ls.load_raster(data / "cost.tif")
    feats_e = ls.load_raster(data / "features.tif")
    thr_e = ls.load_raster(data / "threats.tif")
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        d = P.actions_raster_data(cost_e, feats_e, thr_e, action_cost=[20, 5], target_recovery=0.2)
        s = d.problem("minimizeCosts").add_highs_solver(gap=0.01).solve()
    out["example"] = {"settings": {"action_cost": [20, 5], "target_recovery": 0.2}, "problem": {}, "gap": 0.01,
                      "tables": {k: table(getattr(d, k)) for k in ("pu", "features", "dist_features", "threats",
                                                                    "dist_threats", "boundary")},
                      "objective": float(s.objective), "units": s.units.tolist(), "actions": s.actions.tolist()}
    print("example", s.status, s.objective, d.n_units, d.n_actions)
    np.savez_compressed(OUT / "actions_raster.npz", **arrays)
    (OUT / "actions_raster.json").write_text(json.dumps(out) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
