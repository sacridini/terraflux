# SPDX-License-Identifier: GPL-3.0-or-later
"""The tables Marxan's reference runs were made from, for Terraflux's tests of prioritize.marxan.

landschaft keeps Marxan 4.0.6's runs (extern/landschaft/tests/data/reference/optimize/marxan.npz) of tables its
tests build with numpy (tests/marxan_cases.py); this writes every case's tables (pu, spec, puvspr, bound, as
Terraflux's tables: columns and rows) and options into tests/data/reference/marxan_inputs.json:

    python tools/make_marxan_inputs.py
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

import numpy as np

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "extern" / "landschaft" / "tests"))

import marxan_cases as mc  # noqa: E402


def table(df) -> dict:
    return {"columns": [str(c) for c in df.columns],
            "rows": [[float(v) for v in row] for row in df.to_numpy(dtype=np.float64)]}


def main() -> None:
    out = {}
    for name, c in mc.CASES.items():
        pu, spec, puvspr, bound = mc.tables(c.status, c.spec)
        out[name] = {"tables": {"pu": table(pu), "spec": table(spec), "puvspr": table(puvspr), "bound": table(bound)},
                     "options": c.options, "exe": c.exe}
    path = ROOT / "tests" / "data" / "reference" / "marxan_inputs.json"
    path.write_text(json.dumps(out) + "\n", encoding="utf-8")
    print(path, list(out))


if __name__ == "__main__":
    main()
