# SPDX-License-Identifier: GPL-3.0-or-later
"""What landschaft's SWAT+ results on the map give, for Terraflux's tests (swat.hru_map, swat.channel_map,
swat.reach_discharge): the projects ``from_rasters`` builds in the cases of swat_project's reference (landschaft's
synthetic valley with its land use and with crop rotations (the whole weather, not swat_project's year), and the example landscape), and for each case a call of
``hru_map``, ``channel_map`` or ``reach_discharge`` with no run given (the project runs as its print.prt says).
Written to tests/data/reference/swat_maps.npz (the maps and the tables) and swat_maps.json (the cases):

    python tools/make_swat_maps_reference.py

Terraflux builds the same projects from the inputs of swat_project.npz (tools/make_swat_project_reference.py).
Run with the landschaft the submodule points at installed.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

import numpy as np
import pandas as pd

import landschaft as ls

ROOT = Path(__file__).resolve().parents[1]
REF = ROOT / "tests" / "data" / "reference"
sys.path.insert(0, str(ROOT / "extern" / "landschaft" / "tests"))
import swat_landscape as syn  # noqa: E402

CROPS = {1: "CORN", 2: "WWHT", 3: "URMD"}


def main() -> None:
    dem, lu, so = syn.rasters()
    ex = ls.hydro.swat.example_landscape()
    st, pcp, tx, tn, monthly = syn.weather()
    sets = {
        "syn": (dict(dem=dem, landuse=lu, soils=so, landuse_lookup=syn.LANDUSE, soil_lookup=syn.SOILS,
                     soil_table=syn.soil_table(), **syn.OPTIONS),
                dict(stations=st, pcp=pcp, tmax=tx, tmin=tn, monthly=monthly), {}),
        "syn_crops": (dict(dem=dem, landuse=lu, soils=so, landuse_lookup=CROPS, soil_lookup=syn.SOILS,
                           soil_table=syn.soil_table(), **syn.OPTIONS),
                      dict(stations=st, pcp=pcp, tmax=tx, tmin=tn, monthly=monthly), {}),
        "ex": (dict(dem=ex["dem"], landuse=ex["landuse"], soils=ex["soils"], landuse_lookup=ex["landuse_lookup"],
                    soil_lookup=ex["soil_lookup"], soil_table=ex["soil_table"], stream_threshold=1.0),
               dict(stations=ex["stations"], pcp=ex["pcp"], tmax=ex["tmax"], tmin=ex["tmin"], monthly=ex["monthly"]),
               {}),
    }
    projects = {}
    for k, (land, weather, opt) in sets.items():
        wx = ls.hydro.swat.climate(**weather)
        projects[k] = ls.hydro.swat.from_rasters(**land, climate=wx, **opt)

    def by_hru(p):
        n = len(p.watershed.tables["gis_hrus"])
        return {k: round(0.5 * k + 0.25, 3) for k in range(1, n + 1) if k % 4 != 0} | {n + 40: 7.0}

    def by_channel(p):
        n = len(p.watershed.tables["gis_channels"])
        return {k: round(1.5 * k, 3) for k in range(1, n + 1) if k % 3 != 0} | {n + 9: 2.0}

    # (name, project, kind, settings)
    cases = [
        ("syn_hru_sedyld", "syn", "hru", dict(variable="sedyld")),
        ("syn_hru_sedyld_mean", "syn", "hru", dict(variable="sedyld", how="mean")),
        ("syn_hru_sedyld_max", "syn", "hru", dict(variable="sedyld", how="max")),
        ("syn_hru_sedyld_sum", "syn", "hru", dict(variable="sedyld", how="sum")),
        ("syn_hru_surq_annual", "syn", "hru", dict(variable="surq_gen")),
        ("syn_hru_et_sum", "syn", "hru", dict(variable="et", how="sum")),
        ("syn_hru_wateryld_mean", "syn", "hru", dict(variable="wateryld", how="mean")),
        ("syn_hru_bioms", "syn", "hru", dict(variable="bioms")),
        ("syn_hru_lai_max", "syn", "hru", dict(variable="lai", how="max")),
        ("syn_hru_denit", "syn", "hru", dict(variable="denit")),
        ("syn_hru_plant", "syn", "hru", dict(variable="sedyld", c_factor="plant")),
        ("syn_hru_values", "syn", "hru", dict(variable="series")),
        ("syn_crops_hru_bioms", "syn_crops", "hru", dict(variable="bioms")),
        ("syn_crops_hru_yield_max", "syn_crops", "hru", dict(variable="yield", how="max")),
        ("syn_crops_hru_sedyld_mean", "syn_crops", "hru", dict(variable="sedyld", how="mean")),
        ("ex_hru_sedyld_plant", "ex", "hru", dict(variable="sedyld", c_factor="plant")),
        ("ex_hru_wateryld_sum", "ex", "hru", dict(variable="wateryld", how="sum")),
        ("syn_channel_flow", "syn", "channel", {}),
        ("syn_channel_flow_max", "syn", "channel", dict(values="flo_out", how="max")),
        ("syn_channel_sed_annual", "syn", "channel", dict(values="sed_out", how="annual")),
        ("syn_channel_sed_sum", "syn", "channel", dict(values="sed_out", how="sum")),
        ("syn_channel_values", "syn", "channel", dict(values="series")),
        ("ex_channel_flow", "ex", "channel", {}),
        ("ex_channel_no3_mean", "ex", "channel", dict(values="no3_out", how="mean")),
        ("ex_channel_flow_annual", "ex", "channel", dict(values="flo_out", how="annual")),
        ("syn_reach", "syn", "reach", {}),
        ("syn_crops_reach", "syn_crops", "reach", {}),
        ("ex_reach", "ex", "reach", {}),
    ]
    out: dict[str, np.ndarray] = {}
    meta: dict = {"landschaft": ls.__version__, "cases": []}
    for name, key, kind, kw in cases:
        p = projects[key]
        rec = {"name": name, "project": key, "kind": kind, "settings": dict(kw)}
        if kind == "hru":
            kw = dict(kw)
            if kw.get("variable") == "series":
                s = by_hru(p)
                rec["series"] = [[k, v] for k, v in s.items()]
                kw["variable"] = pd.Series(s, name="mine")
            da = ls.hydro.swat.hru_map(p, **kw)
            out[name] = da.values.astype(np.float64)
        elif kind == "channel":
            kw = dict(kw)
            v = kw.pop("values", None)
            if v == "series":
                s = by_channel(p)
                rec["series"] = [[k, x] for k, x in s.items()]
                v = pd.Series(s, name="mine")
            da = ls.hydro.swat.channel_map(p, v, **kw)
            out[name] = da.values.astype(np.float64)
        else:
            q = ls.hydro.swat.reach_discharge(p)
            dates = q.index
            out[name] = np.column_stack([dates.year.to_numpy(np.float64), dates.dayofyear.to_numpy(np.float64),
                                         q.to_numpy(np.float64)])
            rec["channels"] = [int(c) for c in q.columns]
        rec["shape"] = list(out[name].shape)
        meta["cases"].append(rec)
        print(name, out[name].shape, float(np.nanmax(out[name])) if out[name].size else None)
    np.savez_compressed(REF / "swat_maps.npz", **out)
    (REF / "swat_maps.json").write_text(json.dumps(meta, indent=1) + "\n", encoding="utf-8")
    print(REF / "swat_maps.npz", len(out))


if __name__ == "__main__":
    main()
