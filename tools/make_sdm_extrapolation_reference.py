# SPDX-License-Identifier: GPL-3.0-or-later
"""What landschaft's MESS, MOP and ExDet give, for Terraflux's tests, written to
tests/data/reference/sdm_extrapolation.npz and .json:

    python tools/make_sdm_extrapolation_reference.py

Three predictors (Gaussian fields with missing cells) and a reference sample from a part of the map; MESS (each
predictor's and their least, on the stack and on a table), MOP (basic, simple and detailed; Euclidean, scaled
and centered, Mahalanobis, without distances, rescaled, over the in-range, out-of-range or all conditions) and
ExDet. The json holds each case's operator, settings and the result's names; the npz the inputs and results.
"""

from __future__ import annotations

import json
from pathlib import Path

import numpy as np
import pandas as pd
import xarray as xr

import landschaft as ls

ROOT = Path(__file__).resolve().parents[1]
REF = ROOT / "tests" / "data" / "reference"
S = ls.sdm


def main() -> None:
    layers = []
    for k, name in enumerate(["temp", "prec", "soil"]):
        f = ls.nlm.gaussian_field(40, 30, autocorr_range=8 + 3 * k, resolution=1000.0, seed=31 + k)
        v = f.values * (10 + 5 * k) + k
        if k == 1:
            v[4, 5:9] = np.nan
        layers.append(f.copy(data=v))
    env = xr.concat(layers, dim="band").assign_coords(band=["temp", "prec", "soil"])
    env = env.rio.write_transform(layers[0].rio.transform())
    rng = np.random.default_rng(3)
    rows, cols = rng.integers(0, 15, 160), rng.integers(0, 25, 160)
    ref = pd.DataFrame({n: env.sel(band=n).values[rows, cols] for n in ["temp", "prec", "soil"]})
    table = pd.DataFrame({n: env.sel(band=n).values[::3, ::4].ravel() for n in ["temp", "prec", "soil"]})

    out: dict[str, np.ndarray] = {"in/env": env.values, "in/reference": ref.to_numpy(), "in/table": table.to_numpy()}
    cases: list[dict] = []

    def case(name: str, op: str, params: dict, result, target: str = "env") -> None:
        if isinstance(result, xr.DataArray):
            out[name] = np.asarray(result.values, dtype=np.float64)
            names = [str(b) for b in result.band.values]
        else:
            out[name] = result.to_numpy(dtype=np.float64).T
            names = list(result.columns)
        cases.append({"name": name, "op": op, "params": params, "names": names, "target": target})

    case("mess", "sdm.mess", {}, S.mess(ref, env))
    case("mess_full", "sdm.mess", {"full": True}, S.mess(ref, env, full=True))
    case("mess_table", "sdm.mess", {"full": True}, S.mess(ref, table, full=True), "table")
    case("mop", "sdm.mop", {}, S.mop(ref, env))
    case("mop_simple", "sdm.mop", {"type": "simple", "where": "all", "percentage": 5.0},
         S.mop(ref, env, type="simple", where="all", percentage=5.0))
    case("mop_detailed", "sdm.mop", {"type": "detailed", "scale": True, "center": True, "where": "out_range",
                                     "rescale": True},
         S.mop(ref, env, type="detailed", scale=True, center=True, where="out_range", rescale=True))
    case("mop_none", "sdm.mop", {"type": "detailed", "distance": "none"}, S.mop(ref, env, type="detailed",
                                                                              distance=None))
    case("mop_mahalanobis", "sdm.mop", {"distance": "mahalanobis", "percentage": 10.0, "where": "all"},
         S.mop(ref, env, distance="mahalanobis", percentage=10.0, where="all"))
    case("mop_table", "sdm.mop", {"type": "simple"}, S.mop(ref, table, type="simple"), "table")
    case("exdet", "sdm.exdet", {}, S.exdet(ref, env))
    case("exdet_table", "sdm.exdet", {}, S.exdet(ref, table), "table")

    np.savez_compressed(REF / "sdm_extrapolation.npz", **out)
    (REF / "sdm_extrapolation.json").write_text(json.dumps(cases, indent=1) + "\n", encoding="utf-8")
    print(REF / "sdm_extrapolation.npz", len(out))


if __name__ == "__main__":
    main()
