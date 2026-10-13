# SPDX-License-Identifier: GPL-3.0-or-later
"""What landschaft's thinning, one record per cell and background points give, for Terraflux's tests, written
to tests/data/reference/sdm_points.npz and .json:

    python tools/make_sdm_points_reference.py

Records in lon/lat (a table) over two predictors on a lon/lat grid (with missing cells) and on a projected one:
spThin's thinning, a record per cell (with and without the missing cells), and background points (weighted by
the cells' widths, within a band of distances from the records, outside their envelope, with a bias, within an
area, on the projected grid).
"""

from __future__ import annotations

import json
from pathlib import Path

import geopandas as gpd
import numpy as np
import pandas as pd
import shapely
import xarray as xr

import landschaft as ls

ROOT = Path(__file__).resolve().parents[1]
REF = ROOT / "tests" / "data" / "reference"
S = ls.sdm


def stack(transform, crs=None) -> xr.DataArray:
    layers = []
    for k in range(2):
        f = ls.nlm.gaussian_field(48, 40, autocorr_range=6 + 4 * k, seed=51 + k)
        v = f.values * 10.0
        if k == 0:
            v[30:34, 5:12] = np.nan
        layers.append(v)
    ny, nx = 40, 48
    x = transform[0] + (np.arange(nx) + 0.5) * transform[1]
    y = transform[3] + (np.arange(ny) + 0.5) * transform[5]
    da = xr.DataArray(np.stack(layers), dims=("band", "y", "x"), coords={"band": ["a", "b"], "y": y, "x": x})
    from affine import Affine
    da = da.rio.write_transform(Affine(transform[1], 0, transform[0], 0, transform[5], transform[3]))
    return da.rio.write_crs(crs) if crs else da


def main() -> None:
    geo_t = (-60.0, 0.25, 0.0, -10.0, 0.0, -0.25)
    env = stack(geo_t, "EPSG:4326")
    proj_t = (500000.0, 1000.0, 0.0, 8000000.0, 0.0, -1000.0)
    env_p = stack(proj_t)
    rng = np.random.default_rng(12)
    occ = pd.DataFrame({"lon": np.round(rng.uniform(-59, -49, 120), 4), "lat": np.round(rng.uniform(-19.5, -11, 120), 4)})
    occ.loc[5, ["lon", "lat"]] = occ.loc[6, ["lon", "lat"]].to_numpy() + 0.01
    out: dict[str, np.ndarray] = {"in/env": env.values, "in/env_p": env_p.values, "in/occ": occ.to_numpy()}
    t = S.thin(occ, 60000.0, reps=5, seed=3)
    out["thin"] = t.to_numpy()
    out["thin_kept"] = np.asarray(t.attrs["thin"]["kept"], dtype=np.float64)
    out["one"] = S.one_per_cell(occ, env).to_numpy()
    out["one_drop"] = S.one_per_cell(occ, env, drop_missing=True).to_numpy()
    bias = env.isel(band=1).copy(data=np.abs(env.values[1]) + 0.1)
    out["in/bias"] = bias.values
    area = gpd.GeoDataFrame(geometry=[shapely.Polygon([(-58, -12), (-50, -13), (-51, -18), (-57, -19)])],
                            crs="EPSG:4326")
    (REF / "sdm_points.gpkg").unlink(missing_ok=True)
    area.to_file(REF / "sdm_points.gpkg", layer="area", driver="GPKG")
    area = gpd.read_file(REF / "sdm_points.gpkg", layer="area")
    cases = {"bg": {"n": 300, "seed": 4},
             "bg_band": {"n": 150, "seed": 5, "occ": True, "distance": [50000.0, 400000.0]},
             "bg_envelope": {"n": 120, "seed": 6, "occ": True, "envelope": 0.1},
             "bg_bias": {"n": 200, "seed": 7, "bias": True, "occ": True},
             "bg_area": {"n": 100, "seed": 8, "area": True, "tryf": 2},
             "bg_proj": {"n": 250, "seed": 9, "projected": True}}
    for name, kw in cases.items():
        args = {"n": kw["n"], "seed": kw["seed"]}
        for k in ("distance", "envelope", "tryf"):
            if k in kw:
                args[k] = kw[k]
        if kw.get("occ"):
            args["occ"] = occ
        if kw.get("bias"):
            args["bias"] = bias
        if kw.get("area"):
            args["area"] = area
        b = S.background(env_p if kw.get("projected") else env, **args)
        out[name] = b["cell"].to_numpy(dtype=np.float64)
    np.savez_compressed(REF / "sdm_points.npz", **out)
    (REF / "sdm_points.json").write_text(json.dumps({"background": cases, "geo": geo_t, "proj": proj_t}, indent=1)
                                         + "\n", encoding="utf-8")
    print(REF / "sdm_points.npz", len(out), out["thin_kept"])


if __name__ == "__main__":
    main()
