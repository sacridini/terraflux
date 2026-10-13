# SPDX-License-Identifier: GPL-3.0-or-later
"""What landschaft's extract and geopandas' buffer, clip and dissolve give, for Terraflux's tests of vector.extract,
vector.buffer, vector.clip and vector.dissolve: on the grid and features of landschaft's optimize cases
(tests/optimize_cases.py) and its planning-unit polygons, at points drawn over the grid (some outside it), written to
tests/data/reference/vector_ops.json:

    python tools/make_vector_ops_reference.py

Run with the landschaft the submodule points at installed.
"""

from __future__ import annotations

import json
import math
import sys
from pathlib import Path

import numpy as np

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "extern" / "landschaft" / "tests"))

import geopandas as gpd  # noqa: E402
import landschaft as ls  # noqa: E402
import optimize_cases as oc  # noqa: E402
import shapely  # noqa: E402
from shapely.geometry import box  # noqa: E402

REF = ROOT / "tests" / "data" / "reference"


def num(v):
    v = float(v)
    return None if math.isnan(v) else v


def main() -> None:
    feats = oc.features()
    crs = feats.rio.crs
    rng = np.random.default_rng(11)
    x = rng.uniform(-150, 1650, 40)
    y = rng.uniform(-150, 1350, 40)
    points = gpd.GeoDataFrame({"site": np.arange(1, 41, dtype=float)}, geometry=gpd.points_from_xy(x, y), crs=crs)
    out: dict = {"landschaft": ls.__version__, "geos": ".".join(map(str, shapely.geos_version)),
                 "x": x.tolist(), "y": y.tolist()}
    for method in ("nearest", "bilinear"):
        t = ls.extract(feats, points, method=method)
        out[f"extract_{method}"] = [[num(v) for v in t[str(b)]] for b in feats.band.values]
        out["bands"] = [str(b) for b in feats.band.values]
    t = ls.extract(feats.isel(band=0), points, buffer=120.0, stats=["mean", "max", "count"])
    out["extract_buffer"] = {s: [num(v) for v in t[s]] for s in ("mean", "max", "count")}
    pu = oc.pu_polygons()
    out["buffer_area"] = [float(a) for a in pu.geometry.buffer(35.0).area]
    out["buffer_points_area"] = [float(a) for a in points.geometry.buffer(50.0).area]
    mask = gpd.GeoDataFrame(geometry=[box(250, 150, 950, 850), box(900, 800, 1300, 1100)], crs=crs)
    clipped = gpd.clip(pu, mask)
    out["clip_index"] = [int(i) for i in clipped.index]
    out["clip_area"] = [float(a) for a in clipped.geometry.area]
    pu = pu.assign(group=(np.arange(len(pu)) % 3).astype(float))
    d = pu.dissolve(by="group")
    out["dissolve_group"] = [float(g) for g in d.index]
    out["dissolve_area"] = [float(a) for a in d.geometry.area]
    out["dissolve_all_area"] = float(pu.union_all().area)
    path = REF / "vector_ops.json"
    path.write_text(json.dumps(out, indent=1) + "\n", encoding="utf-8")
    print(path)


if __name__ == "__main__":
    main()
