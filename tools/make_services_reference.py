# SPDX-License-Identifier: GPL-3.0-or-later
"""What landschaft's InVEST models give, for Terraflux's tests: every case of landschaft's InVEST references
(extern/landschaft/tests/data/reference/invest: carbon, habitat quality, annual water yield, sediment delivery
ratio), run as landschaft's tests run them; the inputs as landschaft reads them (missing cells NaN) and the
outputs written to tests/data/reference/services.npz (and the cases' settings to services.json):

    python tools/make_services_reference.py

Run with the landschaft the submodule points at installed.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

import numpy as np

import landschaft as ls
from landschaft.surface._grid import read_surface

ROOT = Path(__file__).resolve().parents[1]
REF = ROOT / "tests" / "data" / "reference"
sys.path.insert(0, str(ROOT / "extern" / "landschaft" / "tests"))

import test_services as T  # noqa: E402
import test_services_sdr as S  # noqa: E402
import test_services_water as W  # noqa: E402
from landschaft.services._water import _wide  # noqa: E402


def values(x) -> np.ndarray:
    return np.asarray(read_surface(x).values, np.float64)


def table(df) -> dict:
    return {"columns": [str(c) for c in df.columns], "rows": df.to_numpy(np.float64).tolist()}


def main() -> None:
    out: dict[str, np.ndarray] = {}
    meta: dict = {"landschaft": ls.__version__, "pixel": T.PIXEL, "origin": list(T.ORIGIN), "cases": {}}

    for name in ("carbon_storage", "carbon_sequestration", "carbon_valuation", "carbon_valuation_flat",
                 "carbon_minus_one_patched"):
        inputs, _, m = T._case(name)
        kw = {}
        case = {"model": "carbon", "pools": table(T._pools(inputs))}
        out[f"{name}/in/lulc"] = values(T._raster(inputs["bas"], m["nodata"]))
        if "alt" in inputs:
            kw["future"] = T._raster(inputs["alt"], m["nodata"])
            out[f"{name}/in/future"] = values(kw["future"])
        v = m["valuation"]
        if v:
            kw.update(price=v["price"], discount_rate=v["discount"], rate_change=v["change"], year=v["years"][0],
                      future_year=v["years"][1])
            case["params"] = {k: kw[k] for k in ("price", "discount_rate", "rate_change", "year", "future_year")}
        c = ls.services.carbon(T._raster(inputs["bas"], m["nodata"]), T._pools(inputs), **kw)
        out[f"{name}/storage"] = values(c.storage)
        for p in ("c_above", "c_below", "c_soil", "c_dead"):
            out[f"{name}/{p}"] = values(c.pools[p])
        if c.change is not None:
            out[f"{name}/storage_future"] = values(c.storage_future)
            out[f"{name}/change"] = values(c.change)
        if c.npv is not None:
            out[f"{name}/npv"] = values(c.npv)
        case["totals"] = [float(t) for t in c.totals["total"]]
        meta["cases"][name] = case

    for name in ("hq_current", "hq_scenarios", "hq_access", "hq_empty_threat_patched"):
        inputs, _, m = T._case(name)
        hq, _, _ = T._run_hq(name)
        case = {"model": "habitat_quality", "threats": m["threats"], "k": m["k"],
                "sensitivity": table(T._sensitivity(inputs, m)), "polygons": m["polygons"]}
        out[f"{name}/in/lulc"] = values(T._raster(inputs["cur"], m["nodata"]))
        for sc, key in (("cur", "threat_maps"), ("fut", "future_threat_maps")):
            if f"threat_crops_{sc}" in inputs:
                for t, r in T._hq_inputs(inputs, m, sc).items():
                    out[f"{name}/in/{key}/{t}"] = values(r)
        for sc in ("fut", "bas"):
            if sc in inputs:
                out[f"{name}/in/{'future' if sc == 'fut' else 'base'}"] = values(T._raster(inputs[sc], m["nodata"]))
        for sc in hq.quality:
            suffix = "" if sc == "current" else f"_{sc}"
            out[f"{name}/quality{suffix}"] = values(hq.quality[sc])
            out[f"{name}/degradation{suffix}"] = values(hq.degradation[sc])
        for t, r in hq.threats["current"].items():
            out[f"{name}/threat/{t}"] = values(r)
        for sc, r in hq.rarity.items():
            out[f"{name}/rarity{'' if sc == 'current' else '_future'}"] = values(r)
        if hq.rarity:
            rt = hq.rarity_table.assign(scenario=hq.rarity_table["scenario"].map({"current": 0, "future": 1}))
            case["rarity_table"] = table(rt)
        meta["cases"][name] = case

    for name in ("wy_basic", "wy_full", "wy_int_inputs", "wy_float64"):
        inputs, _, m = T._case(name)
        wy, _, _ = W._run_wy(name)
        args = W._args(inputs, m)
        case = {"model": "water_yield", "z": m["z"], "biophysical": table(W._bio(inputs)),
                "watersheds": m["watersheds"],
                "precision": "float64" if any(_wide(x) for x in args[2:]) else "float32"}
        for k, x in zip(("lulc", "precipitation", "eto", "depth", "pawc"), (args[0], *args[2:])):
            out[f"{name}/in/{k}"] = values(x)
        if "subwatersheds" in m:
            case["subwatersheds"] = m["subwatersheds"]
        if "demand" in inputs:
            case["demand"] = {"columns": ["lucode", "demand"],
                              "rows": np.column_stack([inputs["codes"], inputs["demand"]]).tolist()}
        if "valuation" in m:
            case["valuation"] = {"columns": ["ws_id", "efficiency", "fraction", "height", "kw_price", "cost",
                                             "time_span", "discount"], "rows": m["valuation"]}
        for k in ("wyield", "aet", "pet", "fractp"):
            out[f"{name}/{k}"] = values(getattr(wy, k))
        if wy.demand is not None:
            out[f"{name}/demand_map"] = values(wy.demand)
        case["watershed_table"] = table(wy.watersheds.drop(columns="geometry"))
        if wy.subwatersheds is not None:
            case["subwatershed_table"] = table(wy.subwatersheds.drop(columns="geometry"))
        meta["cases"][name] = case

    for name in ("sdr_mfd", "sdr_d8", "sdr_drainage"):
        inputs, _, m = T._case(name)
        s, _, _ = S._run(name)
        nd = m["input_nodata"]
        p = m["params"]
        case = {"model": "sdr", "watersheds": m["watersheds"],
                "biophysical": {"columns": ["lucode", "usle_c", "usle_p"],
                                "rows": np.column_stack([inputs["codes"], inputs["usle_c"], inputs["usle_p"]]).tolist()},
                "params": {"threshold": p["threshold_flow_accumulation"], "k": p["k_param"], "ic_0": p["ic_0_param"],
                           "sdr_max": p["sdr_max"], "l_max": p["l_max"], "method": p["flow_dir_algorithm"]}}
        for k in ("dem", "erosivity", "erodibility"):
            out[f"{name}/in/{k}"] = values(T._raster(inputs[k], nd))
        out[f"{name}/in/lulc"] = values(T._raster(inputs["lulc"], m["nodata"]))
        if "drainage" in inputs:
            out[f"{name}/in/drainage"] = values(T._raster(inputs["drainage"], 255))
        for k in ("usle", "sed_export", "sed_deposition", "avoided_export", "avoided_erosion", "rkls", "sdr", "ic",
                  "stream", "drains_to_stream"):
            out[f"{name}/{k}"] = np.asarray(getattr(s, k).values, np.float64)
        case["watershed_table"] = table(s.watersheds.drop(columns="geometry"))
        meta["cases"][name] = case

    np.savez_compressed(REF / "services.npz", **out)
    (REF / "services.json").write_text(json.dumps(meta, indent=1) + "\n", encoding="utf-8")
    print(REF / "services.npz", len(out))


if __name__ == "__main__":
    main()
