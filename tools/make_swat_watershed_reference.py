# SPDX-License-Identifier: GPL-3.0-or-later
"""What landschaft's QSWAT+ watershed delineation gives, for Terraflux's tests (swat.watershed): landschaft's
synthetic valley of its tests (tests/swat_landscape.py) and the example landscape (``example_landscape()``) divided
by ``ls.hydro.swat.watershed`` in the cases its tests use (each HRU method, thresholds, outlets and snap, slope bands,
areas in ha and in percent, exempt land uses, a usersoil table, land use on another grid); the inputs, every table
(numbers in the matrix, text columns in the json) and every grid, written to tests/data/reference/swat_watershed.npz
(the cases to swat_watershed.json):

    python tools/make_swat_watershed_reference.py

Run with the landschaft the submodule points at installed.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

import numpy as np
import pandas as pd
import xarray as xr

import landschaft as ls
from landschaft.surface._grid import read_surface

ROOT = Path(__file__).resolve().parents[1]
REF = ROOT / "tests" / "data" / "reference"
sys.path.insert(0, str(ROOT / "extern" / "landschaft" / "tests"))
import swat_landscape as syn  # noqa: E402

THRESHOLDS = {"stream_threshold": 0.15}


def usersoil(layered: pd.DataFrame) -> pd.DataFrame:
    """The same soils in ArcSWAT / QSWAT+'s usersoil layout (one row per soil, the layers in columns)."""
    wide = []
    for name, g in layered.groupby("name", sort=False):
        r = {"SNAM": name, "NLAYERS": len(g), "HYDGRP": g["hyd_grp"].iloc[0], "SOL_ZMX": g["dp_tot"].iloc[0],
             "ANION_EXCL": 0.5, "SOL_CRK": 0.5, "TEXTURE": g["texture"].iloc[0]}
        for k, (_, lay) in enumerate(g.iterrows(), 1):
            for src, dst in (("SOL_Z", "dp"), ("SOL_BD", "bd"), ("SOL_AWC", "awc"), ("SOL_K", "soil_k"),
                             ("SOL_CBN", "carbon"), ("CLAY", "clay"), ("SILT", "silt"), ("SAND", "sand"),
                             ("ROCK", "rock"), ("SOL_ALB", "alb"), ("USLE_K", "usle_k"), ("SOL_EC", "ec"),
                             ("SOL_CAL", "caco3"), ("SOL_PH", "ph")):
                r[f"{src}{k}"] = lay[dst]
        wide.append(r)
    out = pd.DataFrame(wide)
    # a layer the soil lacks has no value
    return out


def regrid(da: xr.DataArray, k: int) -> xr.DataArray:
    """The raster on cells k times as wide (the top left cell of each block's value)."""
    res = float(abs(da.x[1] - da.x[0])) * k
    v = da.values[::k, ::k]
    x0, y0 = float(da.x[0]) - res / k / 2, float(da.y[0]) + res / k / 2
    xs = x0 + (np.arange(v.shape[1]) + 0.5) * res
    ys = y0 - (np.arange(v.shape[0]) + 0.5) * res
    out = xr.DataArray(v, dims=("y", "x"), coords={"y": ys, "x": xs}, name=da.name)
    return out.rio.write_crs(da.rio.crs).rio.write_transform()


def table_json(df: pd.DataFrame) -> dict:
    """A table as columns and rows (text as it is, none as null)."""
    rows = [[None if (isinstance(v, float) and np.isnan(v)) or v is None else (v.item() if hasattr(v, "item") else v)
             for v in r] for r in df.itertuples(index=False, name=None)]
    return {"columns": list(df.columns), "rows": rows}


def landscape(name: str):
    if name == "syn":
        dem, lu, so = syn.rasters()
        return {"dem": dem, "landuse": lu, "soils": so, "landuse_lookup": syn.LANDUSE, "soil_lookup": syn.SOILS,
                "soil_table": syn.soil_table()}
    return ls.hydro.swat.example_landscape()


def case_list(base_points: list) -> list[dict]:
    """The cases: the landscape, the arguments of watershed and what changes in the inputs."""
    syn_ = {"landscape": "syn", **THRESHOLDS}
    ex = {"landscape": "ex", "stream_threshold": 1.0}
    return [
        {"name": "syn_dominant_hru", **syn_, "hru_method": "dominant_hru"},
        {"name": "syn_dominant", **syn_, "hru_method": "dominant", "slope_bands": [5, 20]},
        {"name": "syn_filter_all", **syn_, "hru_method": "filter", "slope_bands": [5, 20]},
        {"name": "syn_filter_20", **syn_, "hru_method": "filter", "thresholds": [20, 20, 20], "slope_bands": [5, 20]},
        {"name": "syn_filter_options", **syn_, "hru_method": "filter", "thresholds": [5.0, 5.0, 5.0],
         "slope_bands": [5, 20]},
        {"name": "syn_filter_exempt", **syn_, "hru_method": "filter", "thresholds": [40, 30, 30],
         "slope_bands": [5, 12.5, 20], "exempt": ["RNGE"]},
        {"name": "syn_area_ha", **syn_, "hru_method": "area", "min_area": 3.0, "slope_bands": [5, 20]},
        {"name": "syn_area_percent", **syn_, "hru_method": "area", "min_area": 10.0, "area_in_ha": False,
         "slope_bands": [5, 20]},
        {"name": "syn_area_exempt", **syn_, "hru_method": "area", "min_area": 8.0, "slope_bands": [5, 20],
         "exempt": ["PAST"]},
        {"name": "syn_default_threshold", "landscape": "syn", "hru_method": "filter", "slope_bands": [10]},
        {"name": "syn_outlets", **syn_, "hru_method": "filter", "thresholds": [5, 5, 5], "slope_bands": [5, 20],
         "outlets": base_points, "snap": 100.0},
        {"name": "syn_outlet_default_snap", **syn_, "hru_method": "dominant_hru", "outlets": base_points[:1]},
        {"name": "syn_usersoil", **syn_, "hru_method": "filter", "thresholds": [5, 5, 5], "slope_bands": [5, 20],
         "soil_table": "usersoil"},
        {"name": "syn_names", **syn_, "hru_method": "filter", "thresholds": [5, 5, 5], "slope_bands": [5, 20],
         "landuse_lookup": {1: "WATR", 2: "PAST", 3: "BARR", 4: "past"}},
        {"name": "syn_same_names", **syn_, "hru_method": "filter", "slope_bands": [5, 20],
         "landuse_lookup": {1: "FRSD", 2: "PAST", 3: "PAST"}},
        {"name": "syn_regrid", **syn_, "hru_method": "filter", "thresholds": [5, 5, 5], "slope_bands": [5, 20],
         "regrid": ["landuse"]},
        {"name": "ex_default", **ex},
        {"name": "ex_filter", **ex, "hru_method": "filter", "thresholds": [10, 10, 10], "slope_bands": [15, 35]},
        {"name": "ex_area", **ex, "hru_method": "area", "min_area": 40.0, "slope_bands": [15, 35]},
    ]


def main() -> None:
    lands = {n: landscape(n) for n in ("syn", "ex")}
    # outlet points near the streams of a first run: a little off the outlets of two subbasins
    first = ls.hydro.swat.watershed(lands["syn"]["dem"], lands["syn"]["landuse"], lands["syn"]["soils"],
                                    landuse_lookup=syn.LANDUSE, soil_lookup=syn.SOILS, soil_table=syn.soil_table(),
                                    **THRESHOLDS).tables["gis_points"]
    base_points = [[float(r.xpr) + 7.0, float(r.ypr) - 5.0] for r in first.sort_values("id").itertuples()][::-1][:2]
    out: dict[str, np.ndarray] = {}
    meta: dict = {"landschaft": ls.__version__, "landscapes": {}, "cases": []}
    for name, ex in lands.items():
        s = read_surface(ex["dem"])
        meta["landscapes"][name] = {
            "shape": list(s.shape), "extent": [float(v) for v in s.extent], "epsg": int(ex["dem"].rio.crs.to_epsg()),
            "landuse_lookup": [[int(k), v] for k, v in ex["landuse_lookup"].items()],
            "soil_lookup": [[int(k), v] for k, v in ex["soil_lookup"].items()],
            "soil_table": table_json(ex["soil_table"]), "usersoil": table_json(usersoil(ex["soil_table"])),
        }
        for r in ("dem", "landuse", "soils"):
            out[f"{name}/{r}"] = np.asarray(ex[r].values, dtype=np.float64)
        if name == "syn":
            for r in ("landuse",):
                c = regrid(ex[r], 2)
                s2 = read_surface(c)
                out[f"{name}/regrid_{r}"] = np.asarray(c.values, dtype=np.float64)
                meta["landscapes"][name]["regrid"] = {"shape": list(s2.shape), "extent": [float(v) for v in s2.extent]}
    for case in case_list(base_points):
        ex = lands[case["landscape"]]
        kw = {k: v for k, v in case.items() if k not in ("name", "landscape", "soil_table", "landuse_lookup", "regrid")}
        if "landuse_lookup" in case:
            kw["landuse_lookup"] = {int(k): v for k, v in case["landuse_lookup"].items()}
        else:
            kw["landuse_lookup"] = ex["landuse_lookup"]
        kw["soil_lookup"] = ex["soil_lookup"]
        kw["soil_table"] = usersoil(ex["soil_table"]) if case.get("soil_table") == "usersoil" else ex["soil_table"]
        if "thresholds" in kw:
            kw["thresholds"] = tuple(kw["thresholds"])
        if "slope_bands" in kw:
            kw["slope_bands"] = tuple(kw["slope_bands"])
        if "exempt" in kw:
            kw["exempt"] = tuple(kw["exempt"])
        landuse = regrid(ex["landuse"], 2) if "regrid" in case else ex["landuse"]
        w = ls.hydro.swat.watershed(ex["dem"], landuse, ex["soils"], **kw)
        tables = {}
        for t, df in w.tables.items():
            text = {}
            numeric = []
            for c in df.columns:
                col = df[c]
                if col.map(lambda v: isinstance(v, str)).any():
                    text[c] = [None if v is None or (isinstance(v, float) and np.isnan(v)) else str(v) for v in col]
                    numeric.append(np.full(len(df), np.nan))
                else:
                    numeric.append(col.astype(float).to_numpy())
            out[f"{case['name']}/{t}"] = np.column_stack(numeric) if len(df) else np.zeros((0, len(df.columns)))
            tables[t] = {"columns": list(df.columns), "text": text}
        out[f"{case['name']}/subbasins"] = np.asarray(w.subbasins.values, dtype=np.float64)
        out[f"{case['name']}/hrus"] = np.asarray(w.hrus.values, dtype=np.float64)
        out[f"{case['name']}/channels"] = np.asarray(w.channels.values, dtype=np.float64)
        meta["cases"].append({**case, "tables": tables})
    np.savez_compressed(REF / "swat_watershed.npz", **out)
    (REF / "swat_watershed.json").write_text(json.dumps(meta, indent=1) + "\n", encoding="utf-8")
    print(REF / "swat_watershed.npz", len(out))


if __name__ == "__main__":
    main()
