# SPDX-License-Identifier: GPL-3.0-or-later
"""The inputs of R proxirr's reference outputs, for Terraflux's tests of prioritize.proxirr.

landschaft keeps proxirr 0.5's outputs (extern/landschaft/tests/data/reference/optimize/proxirr.npz) of inputs
its tests build with numpy (tests/optimize_cases.py); this writes them (the kernel's entries, and Marxan's tables
as Terraflux's tables) into tests/data/reference/proxirr_inputs.json:

    python tools/make_proxirr_inputs.py
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

import numpy as np

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "extern" / "landschaft" / "tests"))

import optimize_cases as oc  # noqa: E402


def table(df) -> dict:
    return {"columns": [str(c) for c in df.columns],
            "rows": [[float(v) for v in row] for row in df.to_numpy(dtype=np.float64)]}


def entries(df) -> dict:
    return {c: [float(v) for v in df[c].to_numpy(dtype=np.float64)] for c in ("local", "global", "target")}


def main() -> None:
    out = {
        "proxirr_entries": entries(oc.proxirr_entries()),
        "proxirr_many": entries(oc.proxirr_many()),
        "proxirr_marxan": {"pu": table(oc.marxan_pu()), "spec": table(oc.marxan_spec()),
                           "puvspr": table(oc.marxan_puvspr())},
        "proxirr_marxan_ids": {"pu": table(oc.marxan_pu_ids()), "spec": table(oc.marxan_spec_ids()),
                               "puvspr": table(oc.marxan_puvspr_ids())},
    }
    path = ROOT / "tests" / "data" / "reference" / "proxirr_inputs.json"
    path.write_text(json.dumps(out) + "\n", encoding="utf-8")
    print(path)


if __name__ == "__main__":
    main()
