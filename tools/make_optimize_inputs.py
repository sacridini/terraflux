# SPDX-License-Identifier: GPL-3.0-or-later
"""The rasters prioritizr's reference models were compiled from, for Terraflux's tests of prioritize.optimize.

landschaft keeps prioritizr 8.0.6's compiled models and optima (extern/landschaft/tests/data/reference/
optimize) of rasters its tests draw with numpy (tests/optimize_cases.py); this writes them (cost, features,
locked units, penalty data, solutions; NaN where no value) into tests/data/reference/optimize_inputs.npz, and
its tables (planning units, features, amounts, boundary, connectivity and adjacency matrices, a solution,
manual bounds) as Terraflux's tables into optimize_tables.json; the zone problems' rasters into
optimize_zones.npz and their tables (zones as numbers 1..) into optimize_zone_tables.json:

    python tools/make_optimize_inputs.py
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

import numpy as np

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "extern" / "landschaft" / "tests"))

import optimize_cases as oc  # noqa: E402


def main() -> None:
    arrays = {
        "pu_cost": oc.pu_cost().values,
        "features": oc.features().values,
        "locked_in": oc.locked_in().values,
        "locked_out": oc.locked_out().values,
        "penalty_raster": oc.penalty_raster().values,
        "sol_raster": oc.sol_raster().values,
        "solp_raster": oc.solp_raster().values,
    }
    out = ROOT / "tests" / "data" / "reference" / "optimize_inputs.npz"
    np.savez_compressed(out, **{k: np.ascontiguousarray(v, dtype=np.float64) for k, v in arrays.items()})
    print(out, {k: v.shape for k, v in arrays.items()})

    def table(df) -> dict:
        return {"columns": [str(c) for c in df.columns],
                "rows": [[None if np.isnan(v) else float(v) for v in row]
                         for row in df.to_numpy(dtype=np.float64)]}  # NaN: null

    spec = oc.spec_table()[["id"]]
    tables = {"pu_table": table(oc.pu_table()), "spec_table": table(spec), "rij_table": table(oc.rij_table()),
              "sol_table": table(oc.sol_table())}
    for k in ("bnd_table", "conn_table", "asym_table", "adj_table", "bounds_cells", "status_cells", "bounds_rows"):
        tables[k] = table(getattr(oc, k)())
    path = out.with_name("optimize_tables.json")
    path.write_text(json.dumps(tables) + "\n", encoding="utf-8")
    print(path, list(tables))

    # management zones (protect, use, restore): every zone's cost and features (zone after zone), the restoration
    # gain (a contribution), the locks, values per zone, fixed allocations
    zones = {
        "zone_cost": oc.zone_cost().values,
        "zone_features": np.concatenate([oc.zf_protect().values, oc.zf_use().values, oc.zf_restore().values]),
        "zone_gain": oc.zone_contributions()[2],
        "zone_locked_in": oc.zone_locked_in().values,
        "zone_locked_out": oc.zone_locked_out().values,
        "zone_linear": oc.zone_linear().values,
        "zone_sol": oc.zone_sol().values,
        "zone_solp": oc.zone_solp().values,
    }
    zout = out.with_name("optimize_zones.npz")
    np.savez_compressed(zout, **{k: np.ascontiguousarray(v, dtype=np.float64) for k, v in zones.items()})
    print(zout, {k: v.shape for k, v in zones.items()})

    def zone_index(df):
        df = df.copy()
        df["zone"] = [float(oc.ZONES.index(z) + 1) for z in df["zone"]]
        return df

    ztables = {"zone_table": table(oc.zone_table()), "zone_rij": table(oc.zone_rij()),
               "zone_bounds_cells": table(zone_index(oc.zone_bounds_cells())),
               "zone_status_cells": table(zone_index(oc.zone_status_cells())),
               "cell_conn": table(oc.cell_conn()), "cell_asym": table(oc.cell_asym()), "zone_bnd": table(oc.zone_bnd())}
    zpath = out.with_name("optimize_zone_tables.json")
    zpath.write_text(json.dumps(ztables) + "\n", encoding="utf-8")
    print(zpath, list(ztables))


if __name__ == "__main__":
    main()
