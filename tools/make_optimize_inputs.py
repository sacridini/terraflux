# SPDX-License-Identifier: GPL-3.0-or-later
"""The rasters prioritizr's reference models were compiled from, for Terraflux's tests of prioritize.optimize.

landschaft keeps prioritizr 8.0.6's compiled models and optima (extern/landschaft/tests/data/reference/
optimize) of rasters its tests draw with numpy (tests/optimize_cases.py); this writes them (cost, features,
locked units, penalty data; NaN where no value) into tests/data/reference/optimize_inputs.npz:

    python tools/make_optimize_inputs.py
"""

from __future__ import annotations

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
    }
    out = ROOT / "tests" / "data" / "reference" / "optimize_inputs.npz"
    np.savez_compressed(out, **{k: np.ascontiguousarray(v, dtype=np.float64) for k, v in arrays.items()})
    print(out, {k: v.shape for k, v in arrays.items()})


if __name__ == "__main__":
    main()
