# SPDX-License-Identifier: GPL-3.0-or-later
"""What landschaft's analysis of land change gives, for Terraflux's tests: the transitions between consecutive
maps, the intensity analysis (OpenLand's, Aldwaik & Pontius 2012) in km2 and in cells, the components of the
difference (diffeR's), the matrices and the change count of Terraflux's pie_landuse (three dates) and
sibuyan_landuse (two), written to tests/data/reference/change.json:

    python tools/make_change_reference.py

Run with the landschaft the submodule points at installed. NaN written null.
"""

from __future__ import annotations

import json
import math
from pathlib import Path

import numpy as np

import landschaft as ls

ROOT = Path(__file__).resolve().parents[1]
REF = ROOT / "tests" / "data" / "reference"
INPUTS = ROOT / "tests" / "data" / "inputs"

# (file, gain, loss) of the transition level
CASES = [("pie_landuse.tif", 2, 1), ("pie_landuse.tif", 1, 3), ("sibuyan_landuse.tif", 2, 1)]


def num(v):
    if isinstance(v, str):
        return v
    v = float(v)
    return None if math.isnan(v) else v


def frame(df) -> dict:
    return {"columns": [str(c) for c in df.columns], "rows": [[num(v) for v in row] for row in df.itertuples(index=False)]}


def main() -> None:
    out: dict = {"landschaft": ls.__version__, "cases": []}
    for name, gain, loss in CASES:
        lu = ls.load_raster(INPUTS / name)
        ch = ls.simulate.changes(lu)
        case = {"file": name, "gain": gain, "loss": loss, "times": [float(t) for t in ch.times],
                "cell_area": ch.cell_area, "classes": [float(c) for c in ch.classes]}
        case["table"] = frame(ch.table)
        case["one_step"] = frame(ch.one_step)
        case["areas"] = [[float(v) for v in row] for row in np.asarray(ch.areas, dtype=float)]
        m = ch.matrix()
        case["matrix"] = {"classes": [float(c) for c in m.index], "counts": np.asarray(m, dtype=float).tolist()}
        for area in (True, False):
            ia = ch.intensity(gain, loss, area=area)
            key = "intensity_km2" if area else "intensity_cells"
            case[key] = {k: frame(getattr(ia, k)) for k in (
                "interval", "category_gain", "category_loss", "transition_gain", "transition_loss",
                "category_gain_stationarity", "category_loss_stationarity", "transition_gain_stationarity",
                "transition_loss_stationarity")}
        case["components"] = frame(ch.components())
        case["components_error"] = frame(ls.simulate.difference_components(m, analysis="error"))
        case["overall"] = ls.simulate.overall_difference(m)
        count, table = ch.change_count()
        counts = np.asarray(count)
        case["change_count"] = {"values": [int(v) for v in counts.ravel()], "table": frame(table)}
        out["cases"].append(case)
    path = REF / "change.json"
    path.write_text(json.dumps(out) + "\n", encoding="utf-8")
    print(path)


if __name__ == "__main__":
    main()
