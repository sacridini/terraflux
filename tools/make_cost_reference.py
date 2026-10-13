# SPDX-License-Identifier: GPL-3.0-or-later
"""What landschaft's grid distances (gdistance's) give, for Terraflux's tests, written to
tests/data/reference/cost.npz and cost.json:

    python tools/make_cost_reference.py

On maungawhau (a projected grid): the transitions of every built-in function, 4, 8 and 16 directions,
symmetric and not; their geoCorrection (c and r, scaled); the accumulated cost from points and from a mask; the
least costs among points and from points to others; the least-cost paths. On europe (longitude/latitude): the
same over its populations (pop_coord.csv). On a global grid that wraps around (cost_global.tif, written here):
the accumulated cost, the least costs and the paths across the 180th meridian.

Run with the landschaft the submodule points at installed.
"""

from __future__ import annotations

import csv
import json
from pathlib import Path

import numpy as np

import landschaft as ls

ROOT = Path(__file__).resolve().parents[1]
REF = ROOT / "tests" / "data" / "reference"
C = ls.cost
BUILTINS = ["mean", "min", "max", "sum", "prod", "from", "to", "diff", "harmonic", "geometric"]

# maungawhau: x 2667400..2668010, y 6478700..6479570
M_POINTS = [[2667405.0, 6479565.0], [2667905.3, 6478900.1], [2667700.0, 6479200.0], [2668009.0, 6478701.0],
            [2667555.5, 6479401.2], [2667000.0, 6479000.0]]
M_TO = [[2667900.0, 6479500.0], [2667450.0, 6478750.0], [2667705.0, 6479195.0]]
G_POINTS = [[175.5, 10.2], [-175.3, 12.7], [-20.0, -40.0], [100.1, 60.3], [179.9, -5.0], [-179.9, -5.0]]


def main() -> None:
    out: dict[str, np.ndarray] = {}
    cases: dict = {"landschaft": ls.__version__, "maungawhau": {"points": M_POINTS, "to": M_TO},
                   "global": {"points": G_POINTS}}
    m = C.example_data("maungawhau")
    for f in BUILTINS:
        out[f"m/tr/{f}/8/1"] = C.transition(m, f, 8, True).values
    for d in (4, 16):
        out[f"m/tr/mean/{d}/1"] = C.transition(m, "mean", d, True).values
    for d in (4, 8, 16):
        for f in ("mean", "diff"):
            out[f"m/tr/{f}/{d}/0"] = C.transition(m, f, d, False).values
    out["m/recip/mean/8/1"] = (1 / C.transition(m, "mean", 8, True)).values
    for d, s in ((8, True), (16, False)):
        tr = 1 / C.transition(m, "mean", d, s)
        for kind in ("c", "r"):
            for scale in (False, True):
                out[f"m/geo/{d}/{int(s)}/{kind}/{int(scale)}"] = C.geo_correction(tr, kind, scale=scale).values
    sym = C.geo_correction(1 / C.transition(m, "mean", 8, True))
    asym = C.geo_correction(1 / C.transition(m, "from", 16, False))
    mask = np.zeros((87, 61))
    mask[10, 5] = 1
    mask[60:62, 40] = 3
    out["m/mask"] = mask
    for name, tr in (("sym", sym), ("asym", asym)):
        out[f"m/{name}/acc/points"] = np.asarray(C.acc_cost(tr, np.array(M_POINTS[:3])), dtype=np.float64)
        out[f"m/{name}/acc/mask"] = np.asarray(C.acc_cost(tr, mask), dtype=np.float64)
        out[f"m/{name}/dist"] = C.cost_distance(tr, np.array(M_POINTS))
        out[f"m/{name}/dist_to"] = C.cost_distance(tr, np.array(M_POINTS), np.array(M_TO))
        for k, p in enumerate(C.shortest_path(tr, M_POINTS[0], np.array(M_POINTS[1:]), output="cells")):
            out[f"m/{name}/path/{k}"] = np.asarray(p, dtype=np.float64)

    e = C.example_data("europe")
    with open(C.example_data("europe").parent / "pop_coord.csv", newline="") as f:
        pops = [[float(r["x"]), float(r["y"])] for r in csv.DictReader(f)]
    cases["europe"] = {"points": pops}
    tr = 1 / C.transition(e, "mean", 8, True)
    for kind in ("c", "r"):
        out[f"e/geo/{kind}"] = C.geo_correction(tr, kind).values
    g = C.geo_correction(tr, "c")
    out["e/acc"] = np.asarray(C.acc_cost(g, np.array(pops[:2])), dtype=np.float64)
    out["e/dist"] = C.cost_distance(g, np.array(pops))
    for k, p in enumerate(C.shortest_path(g, pops[0], np.array(pops[1:]), output="cells")):
        out[f"e/path/{k}"] = np.asarray(p, dtype=np.float64)

    # a global grid (wraps around): 18 x 36 cells of 10 degrees
    import rasterio
    from rasterio.transform import from_origin

    rng = np.random.default_rng(7)
    z = rng.uniform(1.0, 10.0, (18, 36))
    z[4:6, 10:14] = np.nan
    path = REF / "cost_global.tif"
    with rasterio.open(path, "w", driver="GTiff", width=36, height=18, count=1, dtype="float64", crs="EPSG:4326",
                       transform=from_origin(-180, 90, 10, 10), nodata=np.nan) as dst:
        dst.write(z, 1)
    tr = C.transition(path, "mean", 8, True)
    assert tr.grid.wrap and tr.lonlat
    out["g/tr"] = tr.values
    gg = C.geo_correction(1 / tr, "c")
    out["g/geo"] = gg.values
    out["g/acc"] = np.asarray(C.acc_cost(gg, np.array(G_POINTS[:1])), dtype=np.float64)
    out["g/dist"] = C.cost_distance(gg, np.array(G_POINTS))
    for k, p in enumerate(C.shortest_path(gg, G_POINTS[0], np.array(G_POINTS[1:]), output="cells")):
        out[f"g/path/{k}"] = np.asarray(p, dtype=np.float64)

    np.savez_compressed(REF / "cost.npz", **{k: np.ascontiguousarray(v, dtype=np.float64) for k, v in out.items()})
    (REF / "cost.json").write_text(json.dumps(cases, indent=1) + "\n", encoding="utf-8")
    print(REF / "cost.npz", len(out))


if __name__ == "__main__":
    main()
