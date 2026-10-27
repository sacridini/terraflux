# SPDX-License-Identifier: GPL-3.0-or-later
"""What landschaft's nutrient delivery ratio, crop pollination and seasonal water yield models give, for
Terraflux's tests: every case of landschaft's InVEST references of these models
(extern/landschaft/tests/data/reference/invest: ndr_*, pollination_*, swy_*, those compared bit for bit there),
run as landschaft's tests run them; the inputs as landschaft reads them (missing cells NaN) and the outputs
written to tests/data/reference/services_invest.npz (and the cases' settings to services_invest.json):

    python tools/make_services_invest_reference.py

Run with the landschaft the submodule points at installed (1.54 or later).
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
import test_services_ndr as N  # noqa: E402
import test_services_pollination as P  # noqa: E402
import test_services_swy as S  # noqa: E402

NDR = ["ndr_mfd", "ndr_d8", "ndr_p_only", "ndr_tall", "ndr_wide"]
POLLINATION = ["pollination_guilds", "pollination_farms", "pollination_blocks", "pollination_wide_kernel",
               "pollination_faint"]
SWY = ["swy_mfd_patched", "swy_d8_patched", "swy_float64_patched", "swy_user_recharge_patched",
       "swy_user_recharge_d8_patched"]


def compact(a: np.ndarray) -> np.ndarray:
    """float32 when that holds every value (NaN too): the same numbers, a smaller file."""
    a = np.asarray(a, np.float64)
    b = a.astype(np.float32)
    return b if np.array_equal(b.astype(np.float64), a, equal_nan=True) else a


def values(x) -> np.ndarray:
    return compact(read_surface(x).values)


def array(x) -> np.ndarray:
    return compact(getattr(x, "values", x))


def version() -> str:
    import tomllib

    with open(ROOT / "extern" / "landschaft" / "pyproject.toml", "rb") as f:
        return tomllib.load(f)["project"]["version"]


def table(df) -> dict:
    """A table as JSON: missing values null."""
    rows = [[None if np.isnan(v) else v for v in r] for r in df.to_numpy(np.float64).tolist()]
    return {"columns": [str(c) for c in df.columns], "rows": rows}


def json_value(v):
    if isinstance(v, str):
        try:
            return json.loads(v)
        except json.JSONDecodeError:
            import ast

            return ast.literal_eval(v)
    return v


def main() -> None:
    out: dict[str, np.ndarray] = {}
    meta: dict = {"landschaft": version(), "pixel": T.PIXEL, "origin": list(T.ORIGIN), "cases": {}}

    for name in NDR:
        inputs, _, m = T._case(name)
        r, _, _ = N._run(name)
        nd = m["input_nodata"]
        p = json_value(m["params"])
        bio = N._table(inputs, m).drop(columns=["load_type_n", "load_type_p"])
        case = {"model": "ndr", "watersheds": json_value(m["watersheds"]), "biophysical": table(bio),
                "load_type_n": m["load_type_n"], "load_type_p": m["load_type_p"],
                "params": {"threshold": p["threshold_flow_accumulation"], "k": p["k_param"],
                           "nitrogen": p["calc_n"], "phosphorus": p["calc_p"], "method": p["flow_dir_algorithm"]},
                "ic_0": r.ic_0, "runoff_proxy_mean": r.runoff_proxy_mean}
        if p.get("runoff_proxy_av") is not None:
            case["params"]["runoff_proxy_mean"] = p["runoff_proxy_av"]
        if p["calc_n"]:
            case["params"]["subsurface_critical_length"] = p["subsurface_critical_length_n"]
            case["params"]["subsurface_efficiency"] = p["subsurface_eff_n"]
        out[f"{name}/in/dem"] = values(T._raster(inputs["dem"], nd))
        out[f"{name}/in/lulc"] = values(T._raster(inputs["lulc"], m["nodata"]))
        out[f"{name}/in/runoff_proxy"] = values(T._raster(inputs["runoff"], nd))
        for k in ("n_surface_export", "n_subsurface_export", "n_total_export", "p_surface_export", "ndr_n", "ndr_p",
                  "effective_retention_n", "effective_retention_p", "ic", "stream", "drains_to_stream"):
            v = getattr(r, k)
            if v is not None:
                out[f"{name}/{k}"] = array(v)
        case["watershed_table"] = table(r.watersheds.drop(columns="geometry"))
        meta["cases"][name] = case

    for name in POLLINATION:
        inputs, _, m = P._case(name)
        res, _, _ = P._run(name)
        guild = json_value(m["guild"])
        case = {"model": "pollination", "guilds": [dict(zip(guild, row)) for row in zip(*guild.values())],
                "biophysical": table(__import__("pandas").DataFrame(json_value(m["bio"])))}
        out[f"{name}/in/lulc"] = values(P._raster(inputs["lulc"], m["nodata"]))
        for sp, da in res.supply.items():
            out[f"{name}/supply/{sp}"] = array(da)
        for sp, d in res.abundance.items():
            for se, da in d.items():
                out[f"{name}/abundance/{sp}_{se}"] = array(da)
        for se, da in res.total_abundance.items():
            out[f"{name}/total_abundance/{se}"] = array(da)
        if "farms" in m:
            case["farms"] = json_value(m["farms"])
            for k in ("farm_pollinators", "total_yield", "wild_yield"):
                out[f"{name}/{k}"] = array(getattr(res, k))
            case["farm_table"] = table(res.farms[["p_abund", "y_tot", "pdep_y_w", "y_wild"]])
        case["species"] = sorted(res.supply)
        case["seasons"] = list(res.total_abundance)
        meta["cases"][name] = case

    for name in SWY:
        inputs, _, m = T._case(name)
        s, _, _ = S._run(name)
        nd = m["input_nodata"]
        p = m["params"]
        case = {"model": "seasonal_water_yield", "watersheds": json_value(m["watersheds"]),
                "params": {"threshold": p["threshold"], "method": p["flow"], "beta": float(S.ls.services._swy._number(
                    p["beta"], "beta")), "gamma": float(S.ls.services._swy._number(p["gamma"], "gamma"))}}
        out[f"{name}/in/dem"] = values(T._raster(inputs["dem"], nd))
        if "recharge" in inputs:
            out[f"{name}/in/recharge"] = values(T._raster(inputs["recharge"], nd))
            case["precision"] = "float64" if inputs["recharge"].dtype == np.float64 else "float32"
        else:
            case["biophysical"] = table(S._bio(inputs))
            out[f"{name}/in/lulc"] = values(T._raster(inputs["lulc"], 0))
            out[f"{name}/in/soil_group"] = values(T._raster(inputs["soil"], 255))
            for mo in range(12):
                out[f"{name}/in/precipitation/{mo + 1}"] = values(T._raster(inputs["precip"][mo], nd))
                out[f"{name}/in/et0/{mo + 1}"] = values(T._raster(inputs["et0"][mo], nd))
            case["precision"] = "float64" if inputs["precip"].dtype == np.float64 else "float32"
            if "zones" in inputs:
                out[f"{name}/in/climate_zones"] = values(T._raster(inputs["zones"], -1))
                case["climate_zone_table"] = {
                    "columns": ["cz_id", *S.MONTHS],
                    "rows": np.column_stack([inputs["zone_ids"], inputs["zone_events"]]).astype(float).tolist()}
            else:
                case["rain_events"] = [float(e) for e in inputs["events"]]
            if "alpha" in inputs:
                case["params"]["alpha"] = [float(a) for a in inputs["alpha"]]
            else:
                case["params"]["alpha"] = float(S.ls.services._swy._number(p["alpha"], "alpha"))
            for k in ("quickflow", "l_sum_avail", "aet", "precipitation"):
                out[f"{name}/{k}"] = array(getattr(s, k))
            for mo in range(12):
                out[f"{name}/quickflow_monthly/{mo + 1}"] = array(s.intermediate["quickflow_monthly"].values[mo])
            case["monthly_table"] = table(s.monthly)
        for k in ("local_recharge", "l_avail", "l_sum", "baseflow", "b_sum", "vri", "stream"):
            out[f"{name}/{k}"] = array(getattr(s, k))
        case["watershed_table"] = table(s.watersheds.drop(columns="geometry"))
        meta["cases"][name] = case

    np.savez_compressed(REF / "services_invest.npz", **out)
    (REF / "services_invest.json").write_text(json.dumps(meta, indent=1) + "\n", encoding="utf-8")
    print(REF / "services_invest.npz", len(out))


if __name__ == "__main__":
    main()
