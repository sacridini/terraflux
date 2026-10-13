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


if __name__ == "__main__":
    main()
