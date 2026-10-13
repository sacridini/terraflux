# SPDX-License-Identifier: GPL-3.0-or-later
"""What landschaft's surface metrics give, for Terraflux's tests: every metric of its example surfaces
(normforest, orelevation, orforest: geographic grids), and the moving window of some on the top-left 40 x 40
cells of each, square and circular, written to tests/data/reference/surface.json and surface_window.npz:

    python tools/make_surface_reference.py

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
WINDOW = ["sa", "sq", "sku", "sv", "sdq", "sdr", "sds", "sbi", "svk", "sal", "std", "sfd"]


def num(v):
    v = float(v)
    return None if math.isnan(v) else v


def main() -> None:
    out: dict = {"landschaft": ls.__version__, "metrics": {}, "window": WINDOW}
    maps: dict[str, np.ndarray] = {}
    for name in ("normforest", "orelevation", "orforest"):
        z = ls.load_raster(ls.surface.example_data(name))
        m = ls.surface.metrics(z)
        out["metrics"][name] = {k: num(v) for k, v in m.items()}
        crop = z[:40, :40]
        for shape in ("square", "circle"):
            ds = ls.surface.window_metrics(crop, WINDOW, window=7, shape=shape)
            for k in WINDOW:
                maps[f"{name}/{shape}/{k}"] = np.asarray(ds[k].values, dtype=np.float64)
    (REF / "surface.json").write_text(json.dumps(out, indent=1) + "\n", encoding="utf-8")
    np.savez(REF / "surface_window.npz", **maps)
    print(REF / "surface.json", len(maps))


TERRAIN = [
    ("terrain_slope", "terrain", {"v": "slope"}),
    ("terrain_aspect4", "terrain", {"v": "aspect", "unit": "radians", "neighbors": 4}),
    ("tpi", "tpi", {"scale": 3}),
    ("tpi_circle", "tpi", {"scale": 0.0012, "win": "circle", "normalize": True}),
    ("tri", "tri", {"s": 5}),
    ("vrm", "vrm", {"s": 3}),
    ("srr", "srr", {"s": 5}),
    ("dissection", "dissection", {"s": [3, 5]}),
    *[(f"curvature_{t}", "curvature", {"type": t}) for t in ("planform", "profile", "total", "mcnab", "bolstad")],
    ("spherical_sd", "spherical_sd", {"d": 3}),
    ("spherical_var", "spherical_sd", {"d": 5, "variance": True}),
    ("hsp", "hsp", {"min_scale": 3, "max_scale": 15, "inc": 4}),
    ("trasp", "trasp", {}),
]


def terrain_cases() -> None:
    """The terrain of orelevation (a geographic grid): every function with some options, the planes, peaks and
    valleys, the bearing area curve and the autocorrelation, written to tests/data/reference/surface_terrain.npz
    (and the cases to surface_terrain.json)."""
    z = ls.load_raster(ls.surface.example_data("orelevation"))
    out: dict[str, np.ndarray] = {}
    for key, fn, kw in TERRAIN:
        out[key] = np.asarray(getattr(ls.surface, fn)(z, **kw).values, dtype=np.float64)
    slope = ls.surface.terrain(z, "slope")
    aspect = ls.surface.terrain(z, "aspect")
    for k, kw in enumerate([{}, {"direct": True, "equation": 2}, {"scaled": True, "equation": 3, "latitude": -20.0,
                                                                  "force_hemisphere": False}]):
        out[f"hli_{k}"] = np.asarray(ls.surface.hli(aspect, slope, **kw).values, dtype=np.float64)
    for order in (0, 2, 5):
        out[f"plane_{order}"] = np.asarray(ls.surface.fit_plane(z, order).values, dtype=np.float64)
    out["plane_best"] = np.asarray(ls.surface.best_fit_plane(z).values, dtype=np.float64)
    out["residuals_best"] = np.asarray(ls.surface.remove_plane(z).values, dtype=np.float64)
    for which in ("peaks", "valleys"):
        t = getattr(ls.surface, f"find_{which}")(z)
        out[which] = t[["x", "y", "value", "row", "col"]].to_numpy(dtype=np.float64)
    b = ls.surface.bearing_area(z)
    out["bearing"] = b[["bearing", "height"]].to_numpy(dtype=np.float64)
    nf = ls.load_raster(ls.surface.example_data("normforest"))
    a = ls.surface.aacf(nf)
    out["aacf"] = np.asarray(a.values, dtype=np.float64)
    np.savez(REF / "surface_terrain.npz", **out)
    cases = {"terrain": [[k, fn, kw] for k, fn, kw in TERRAIN],
             "hli": [{}, {"direct": True, "equation": 2}, {"scaled": True, "equation": 3, "latitude": -20.0,
                                                           "force_hemisphere": False}]}
    (REF / "surface_terrain.json").write_text(json.dumps(cases) + "\n", encoding="utf-8")
    print(REF / "surface_terrain.npz", len(out))


if __name__ == "__main__":
    main()
    terrain_cases()
