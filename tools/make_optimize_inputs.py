# SPDX-License-Identifier: GPL-3.0-or-later
"""The rasters prioritizr's reference models were compiled from, for Terraflux's tests of prioritize.optimize.

landschaft keeps prioritizr 8.0.6's compiled models and optima (extern/landschaft/tests/data/reference/
optimize) of rasters its tests draw with numpy (tests/optimize_cases.py); this writes them (cost, features,
locked units, penalty data, solutions; NaN where no value) into tests/data/reference/optimize_inputs.npz, and
its tables (planning units, features, amounts, boundary, connectivity and adjacency matrices) as Terraflux's
tables into optimize_tables.json:

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
                "rows": [[float(v) for v in row] for row in df.to_numpy(dtype=np.float64)]}

    spec = oc.spec_table()[["id"]]
    tables = {"pu_table": table(oc.pu_table()), "spec_table": table(spec), "rij_table": table(oc.rij_table())}
    for k in ("bnd_table", "conn_table", "asym_table", "adj_table"):
        tables[k] = table(getattr(oc, k)())
    path = out.with_name("optimize_tables.json")
    path.write_text(json.dumps(tables) + "\n", encoding="utf-8")
    print(path, list(tables))


if __name__ == "__main__":
    main()
