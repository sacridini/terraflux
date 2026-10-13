# SPDX-License-Identifier: GPL-3.0-or-later
"""What landschaft's h3_units gives, for Terraflux's tests: the H3 hexagons over the cost raster of landschaft's
optimize cases (tests/optimize_cases.py, EPSG:32723) at resolutions 9 and 10, their indexes and costs, written to
tests/data/reference/h3_units.json:

    python tools/make_h3_reference.py

Run with the landschaft the submodule points at installed (and h3).
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "extern" / "landschaft" / "tests"))

import h3  # noqa: E402
import landschaft as ls  # noqa: E402
import optimize_cases as oc  # noqa: E402

REF = ROOT / "tests" / "data" / "reference"


def main() -> None:
    cost = oc.pu_cost()
    out: dict = {"landschaft": ls.__version__, "h3": h3.__version__, "crs": cost.rio.crs.to_string(), "cases": []}
    for res in (9, 10):
        hexes = ls.prioritize.h3_units(cost, res)
        out["cases"].append({"resolution": res, "h3": list(hexes["h3"]), "cost": [float(c) for c in hexes["cost"]]})
    path = REF / "h3_units.json"
    path.write_text(json.dumps(out, indent=1) + "\n", encoding="utf-8")
    print(path)


if __name__ == "__main__":
    main()
