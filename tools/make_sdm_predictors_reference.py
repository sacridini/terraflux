# SPDX-License-Identifier: GPL-3.0-or-later
"""What landschaft's choice of predictors and range of autocorrelation give, for Terraflux's tests, written to
tests/data/reference/sdm_predictors.npz, .json and .gpkg:

    python tools/make_sdm_predictors_reference.py

Five predictors, some collinear, on a projected grid large enough to be sampled and on a lon/lat one (its rows
weighted by latitude), and as a table: the VIFs, the selections by VIF, by correlation (Pearson and Spearman,
with a predictor kept) and within an area, and the variograms' ranges.
"""

from __future__ import annotations

import json
from pathlib import Path

import geopandas as gpd
import numpy as np
import pandas as pd
import shapely
import xarray as xr
from affine import Affine

import landschaft as ls

ROOT = Path(__file__).resolve().parents[1]
REF = ROOT / "tests" / "data" / "reference"
S = ls.sdm
NAMES = ["p1", "p2", "p3", "p4", "p5"]


def stack(ny, nx, t, crs=None, seed=0):
    rng = np.random.default_rng(seed)
    a = ls.nlm.gaussian_field(nx, ny, autocorr_range=8, seed=71 + seed).values
    b = ls.nlm.gaussian_field(nx, ny, autocorr_range=5, seed=72 + seed).values
    c = ls.nlm.gaussian_field(nx, ny, autocorr_range=12, seed=73 + seed).values
    layers = [a, 0.9 * a + 0.1 * rng.uniform(size=a.shape), b, a + b, c]
    layers[2][3:6, 4:9] = np.nan
    da = xr.DataArray(np.round(np.stack(layers), 6), dims=("band", "y", "x"),
                      coords={"band": NAMES, "y": t[3] + (np.arange(ny) + 0.5) * t[5],
                              "x": t[0] + (np.arange(nx) + 0.5) * t[1]})
    da = da.rio.write_transform(Affine(t[1], 0, t[0], 0, t[5], t[3]))
    return da.rio.write_crs(crs) if crs else da


def main() -> None:
    big_t = (0.0, 100.0, 0.0, 9000.0, 0.0, -100.0)
    big = stack(90, 100, big_t, "EPSG:32722", seed=0)
    geo_t = (-60.0, 0.1, 0.0, -10.0, 0.0, -0.1)
    geo = stack(80, 100, geo_t, "EPSG:4326", seed=1)
    small_t = (0.0, 100.0, 0.0, 5000.0, 0.0, -100.0)
    small = stack(50, 60, small_t, seed=2)
    table = pd.DataFrame({n: small.sel(band=n).values.ravel()[::3] for n in NAMES})
    area = gpd.GeoDataFrame(geometry=[shapely.Polygon([(1000, 1000), (8000, 1500), (7000, 8000), (1500, 7000)])],
                            crs="EPSG:32722")
    (REF / "sdm_predictors.gpkg").unlink(missing_ok=True)
    area.to_file(REF / "sdm_predictors.gpkg", layer="area", driver="GPKG")
    area = gpd.read_file(REF / "sdm_predictors.gpkg", layer="area")
    envs = {"big": big, "geo": geo, "small": small, "table": table}
    out: dict[str, np.ndarray] = {f"in/{k}": (v.values if isinstance(v, xr.DataArray) else v.to_numpy())
                                  for k, v in envs.items()}
    cases = []
    for name in ("big", "geo", "small", "table"):
        out[f"vif_{name}"] = S.vif(envs[name], seed=3)["vif"].to_numpy()
        cases.append({"name": f"vif_{name}", "op": "sdm.vif", "env": name, "params": {"seed": 3}})
    sels = {"sel_vif": ("big", {"vif": 5.0, "seed": 4}),
            "sel_cor": ("small", {"vif": None, "cor": 0.7, "keep": ["p1"]}),
            "sel_spearman": ("geo", {"cor": 0.8, "method": "spearman", "vif": 3.0, "seed": 5}),
            "sel_table": ("table", {"vif": 4.0, "cor": 0.95}),
            "sel_area": ("big", {"vif": 5.0, "area": True, "seed": 6})}
    for name, (env, kw) in sels.items():
        args = dict(kw)
        if args.pop("area", False):
            args["area"] = area
        r = S.select_predictors(envs[env], **args)
        sel = r.attrs["selection"]
        out[name] = np.array(list(sel["vif"].values()))
        cases.append({"name": name, "op": "sdm.select_predictors", "env": env, "params": kw,
                      "kept": sel["kept"], "excluded": sel["excluded"]})
    for name, env, kw in (("auto_small", "small", {"n": 400, "seed": 7}), ("auto_geo", "geo", {"n": 300, "seed": 8})):
        r = S.autocorrelation_range(envs[env].sel(band=["p1", "p3", "p5"]), **kw)
        out[name] = np.array([r["range"], r["block_size"]])
        out[f"{name}/layers"] = r["layers"][["range", "sill", "nugget", "kappa"]].to_numpy(dtype=np.float64)
        cases.append({"name": name, "op": "sdm.autocorrelation_range", "env": env, "params": kw,
                      "models": r["layers"]["model"].tolist()})
    np.savez_compressed(REF / "sdm_predictors.npz", **out)
    meta = {"cases": cases, "transforms": {"big": big_t, "geo": geo_t, "small": small_t}}
    (REF / "sdm_predictors.json").write_text(json.dumps(meta, indent=1) + "\n", encoding="utf-8")
    print(REF / "sdm_predictors.npz", len(out))


if __name__ == "__main__":
    main()
