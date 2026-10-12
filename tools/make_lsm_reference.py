# SPDX-License-Identifier: GPL-3.0-or-later
"""What landschaft's lsm gives for the analyses landscapemetrics has no reference of, for Terraflux's tests: the
square zones of grid_zones and the series of SpatioTemporalAnalysis (whole maps and a grid of zones), on
landschaft's augusta.tif and a changed copy of it, written to tests/data/reference/lsm_series.json:

    python tools/make_lsm_reference.py

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
AUGUSTA = ROOT / "extern" / "landschaft" / "tests" / "data" / "lsm" / "augusta.tif"

LANDSCAPE = ["np", "shdi", "ed", "ta", "contag"]
CLASS = ["pland", "np", "ca", "enn_mn", "ai"]


def num(v):
    v = float(v)
    return None if math.isnan(v) else v


def later_of(a):
    """augusta changed: class 42 of the top-left quarter turned 41, class 82 (one cell) gone into 81."""
    b = a.copy()
    v = b.values
    q = v[:60, :80]
    q[q == 42] = 41
    v[v == 82] = 81
    return b


def rows(table, keys: int):
    """A wide table as [keys..., values...] rows."""
    out = []
    for index, row in table.iterrows():
        index = index if isinstance(index, tuple) else (index,)
        out.append([num(k) for k in index[:keys]] + [num(v) for v in row.to_numpy()])
    return out


def main() -> None:
    a = ls.load_raster(AUGUSTA)
    b = later_of(a)
    out: dict = {"landschaft": ls.__version__}
    out["grid_zones"] = [{"size": s, "zones": [[[float(x), float(y)] for x, y in z["coordinates"][0]]
                                               for z in ls.grid_zones(a, s)]} for s in (1000.0, 1500.0, 700.0)]
    dates = [2001, 2011]
    series = ls.SpatioTemporalAnalysis([a, b], dates=dates)
    zonal = ls.SpatioTemporalAnalysis([a, b], dates=dates, grid=1500)
    out["series"] = {
        "dates": dates, "landscape_metrics": LANDSCAPE, "class_metrics": CLASS,
        "landscape": rows(series.landscape_metrics(LANDSCAPE)[LANDSCAPE], 1),
        "class": rows(series.class_metrics(CLASS)[CLASS], 2),
        "class_na": rows(series.class_metrics(CLASS, fillna=False)[CLASS], 2),
        "grid": 1500,
        "zonal_landscape": rows(zonal.landscape_metrics(LANDSCAPE)[LANDSCAPE], 2),
        "zonal_class": rows(zonal.class_metrics(CLASS)[CLASS], 3),
    }
    path = REF / "lsm_series.json"
    path.write_text(json.dumps(out, indent=1) + "\n", encoding="utf-8")
    print(path)


if __name__ == "__main__":
    main()
