# SPDX-License-Identifier: GPL-3.0-or-later
"""What landschaft's preparation of FUTURES' inputs gives, for Terraflux's tests, written to
tests/data/reference/futures_tools.npz and .json:

    python tools/make_futures_tools_reference.py

Developed land on 45 x 60 cells of 30 m (a few excluded cells): the development pressure by gravity, occurrence
and kernel, padded or not, and the patch library of the growth to a later map, whole, by subregions, with a
threshold.
"""

from __future__ import annotations

import json
from pathlib import Path

import numpy as np
import xarray as xr
from affine import Affine

import landschaft as ls

ROOT = Path(__file__).resolve().parents[1]
REF = ROOT / "tests" / "data" / "reference"
S = ls.simulate


def grid(values: np.ndarray) -> xr.DataArray:
    ny, nx = values.shape
    da = xr.DataArray(values, dims=("y", "x"), coords={"y": 5000 + (np.arange(ny) + 0.5) * -30.0,
                                                     "x": 1000 + (np.arange(nx) + 0.5) * 30.0})
    return da.rio.write_transform(Affine(30.0, 0, 1000.0, 0, -30.0, 5000.0))


def main() -> None:
    ny, nx = 45, 60
    start = (ls.nlm.gaussian_field(nx, ny, autocorr_range=9, seed=31).values > 0.68).astype(float)
    end = start.copy()
    end[ls.nlm.gaussian_field(nx, ny, autocorr_range=4, seed=32).values > 0.62] = 1.0
    start[10:12, 20:23] = np.nan
    sub = np.floor(ls.nlm.gaussian_field(nx, ny, autocorr_range=15, seed=33).values * 3)
    out: dict[str, np.ndarray] = {"in/start": start, "in/end": end, "in/sub": sub}
    meta: dict = {"pressure": {}, "library": {}}
    cases = {"gravity": dict(size=6), "gravity_nopad": dict(size=4, pad=False),
             "occurrence": dict(size=5, method="occurrence"), "gamma": dict(size=8, gamma=2.3, scaling_factor=4.0),
             "kernel": dict(size=6, method="kernel", gamma=3.0, scaling_factor=2.0)}
    for name, kw in cases.items():
        out[f"pressure/{name}"] = S.development_pressure(grid(start), **kw).values.astype(np.float64)
        meta["pressure"][name] = kw
    for name, kw in {"whole": {}, "threshold": {"threshold": 4000.0}, "subregions": {"_sub": True},
                     "per_subregion": {"_sub": True, "per_subregion": True}}.items():
        args = {k: v for k, v in kw.items() if not k.startswith("_")}
        if kw.get("_sub"):
            args["subregions"] = grid(sub)
        lib = S.patch_library(grid(start), grid(end), **args)
        meta["library"][name] = {"settings": kw, "library": lib if isinstance(lib, list) else
                                 {str(k): v for k, v in lib.items()}}
    np.savez_compressed(REF / "futures_tools.npz", **out)
    (REF / "futures_tools.json").write_text(json.dumps(meta, indent=1) + "\n", encoding="utf-8")
    print(REF / "futures_tools.npz", {k: (len(v["library"]) if isinstance(v["library"], list) else
                                          list(v["library"])) for k, v in meta["library"].items()})


if __name__ == "__main__":
    main()
