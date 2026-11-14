# SPDX-License-Identifier: GPL-3.0-or-later
"""What landschaft's SWAT+ project building gives, for Terraflux's tests (swat.from_tables, swat.wgn): the weather
(stations and daily series, with gaps and different periods), the weather generator statistics
(``wgn_from_series``), and the project ``from_tables`` writes (every file of it) in the cases of landschaft's tests
(its synthetic valley, with its crop rotations and with the editor's other options) and in the example landscape;
the run of two of the projects as ``from_rasters`` builds them (the printed tables of a year). Written to
tests/data/reference/swat_project.npz (the inputs and the numbers) and swat_project.json (the cases, the text):

    python tools/make_swat_project_reference.py

The files the editor writes the same for every project are not stored: their hashes are (the data of landschaft's
``swatplus_editor.zip``); the first line of each file, which carries the date it was written, is left out. Run with
the landschaft the submodule points at installed.
"""

from __future__ import annotations

import hashlib
import json
import sys
from pathlib import Path

import numpy as np
import pandas as pd

import landschaft as ls
from landschaft.hydro.swat import TABLES
from landschaft.hydro.swat._editor import _editor_data
from landschaft.hydro.swat._weather import wgn_from_series

ROOT = Path(__file__).resolve().parents[1]
REF = ROOT / "tests" / "data" / "reference"
sys.path.insert(0, str(ROOT / "extern" / "landschaft" / "tests"))
import swat_landscape as syn  # noqa: E402

CROPS = {1: "CORN", 2: "WWHT", 3: "URMD"}
RUN_TABLES = ["hru_wb", "hru_ls", "basin_wb", "channel_sd", "aquifer"]
VARS = ("pcp", "tmax", "tmin", "slr", "hmd", "wnd")


def date_int(index: pd.DatetimeIndex) -> np.ndarray:
    return (index.year * 10000 + index.month * 100 + index.day).to_numpy(np.float64)


def series_array(df: pd.DataFrame) -> np.ndarray:
    """A daily frame as a matrix: the date as yyyymmdd, then a column per station (NaN for a missing day)."""
    return np.column_stack([date_int(df.index), df.to_numpy(np.float64)])


def full_weather():
    """Three stations, series of different periods with gaps, in an order that is not the stations'."""
    rng = np.random.default_rng(11)
    names = ["south", "north", "east-3"]
    stations = pd.DataFrame({"lat": [43.340, 43.352, 43.346], "lon": [-79.745, -79.761, -79.7522],
                             "elev": [215.0, 250.0, 233.3333]}, index=["north", "south", "east-3"])

    def days(a, b):
        return pd.date_range(a, b)

    def gaps(df, p=0.03):
        mask = rng.random(df.shape) < p
        return df.mask(mask)

    d = days("1999-11-15", "2001-12-31")
    season = lambda ix: 10 * np.sin(2 * np.pi * (ix.dayofyear.to_numpy() - 110) / 365)  # noqa: E731
    wet = rng.random((len(d), 3)) < 0.3
    pcp = pd.DataFrame(np.where(wet, rng.gamma(0.8, 9.0, (len(d), 3)), 0.0).round(1), index=d, columns=names)
    d2 = days("2000-01-01", "2001-12-31")
    tmax = pd.DataFrame((12 + season(d2)[:, None] + rng.normal(0, 3, (len(d2), 3))).round(1), index=d2, columns=names)
    tmin = (tmax - 9 - (rng.random(tmax.shape) * 3).round(1)).round(1)
    d3 = days("2000-02-01", "2001-11-30")
    slr = pd.DataFrame((14 + 8 * np.sin(2 * np.pi * (d3.dayofyear.to_numpy()[:, None] - 80) / 365)
                        + rng.normal(0, 2, (len(d3), 3))).round(2).clip(0.1), index=d3, columns=names)
    d4 = days("2000-01-01", "2001-12-15")
    hmd = pd.DataFrame(rng.uniform(0.35, 0.97, (len(d4), 3)).round(3), index=d4, columns=names)
    wnd = pd.DataFrame(rng.gamma(4.0, 0.8, (len(d2), 3)).round(2), index=d2, columns=names)
    pcp, tmax, tmin, slr, hmd, wnd = (gaps(x) for x in (pcp, tmax, tmin, slr, hmd, wnd))
    return stations, {"pcp": pcp, "tmax": tmax, "tmin": tmin, "slr": slr, "hmd": hmd, "wnd": wnd}, {"pcp_hhr": 12.0}


def small_wgn():
    """landschaft's test of the statistics (a periodic series), with the three forms of monthly values."""
    days = pd.date_range("2000-01-01", "2003-12-31")
    pcp = pd.DataFrame({"a": np.where(days.day % 3 == 0, 6.0, 0.0), "b": np.where(days.day % 5 == 0, 3.5, 0.0)},
                       index=days)
    tx = pd.DataFrame({"a": np.where(days.day % 2 == 0, 20.0, 10.0), "b": np.where(days.day % 7 == 0, 25.0, 14.0)},
                      index=days)
    return pcp, tx, tx - 10


def table_json(df: pd.DataFrame) -> dict:
    rows = [[None if (isinstance(v, float) and np.isnan(v)) or v is None else (v.item() if hasattr(v, "item") else v)
             for v in r] for r in df.itertuples(index=False, name=None)]
    return {"columns": list(df.columns), "rows": rows}


def frame(df, obj) -> np.ndarray:
    """year, day of the year, the object (not for the basin) and the columns, as the Terraflux table."""
    dates = df.index.get_level_values("date")
    cols = [dates.year.to_numpy(np.float64), dates.dayofyear.to_numpy(np.float64)]
    if obj != "basin":
        cols.append(df.index.get_level_values(obj).to_numpy(np.float64))
    return np.column_stack(cols + [df[c].to_numpy(np.float64) for c in df.columns])


def wgn_matrix(wgn: pd.DataFrame):
    """The statistics as a matrix (the station column as its number in order of appearance) and the names."""
    stations = list(dict.fromkeys(wgn.index.get_level_values(0)))
    cols = [np.array([stations.index(s) + 1 for s in wgn.index.get_level_values(0)], np.float64),
            wgn.index.get_level_values(1).to_numpy(np.float64)]
    cols += [wgn[c].to_numpy(np.float64) for c in wgn.columns]
    return np.column_stack(cols), stations, list(wgn.columns)


def body(b: bytes) -> bytes:
    return b.split(b"\n", 1)[1]


def main() -> None:
    static, _ = _editor_data()
    out: dict[str, np.ndarray] = {}
    meta: dict = {"landschaft": ls.__version__, "cases": [], "wgn_cases": []}

    # the watersheds of the cases: landschaft's synthetic valley (two land use lookups) and the example landscape
    dem, lu, so = syn.rasters()
    ex = ls.hydro.swat.example_landscape()
    sets = {
        "syn": dict(dem=dem, landuse=lu, soils=so, landuse_lookup=syn.LANDUSE, soil_lookup=syn.SOILS,
                    soil_table=syn.soil_table(), **syn.OPTIONS),
        "syn_crops": dict(dem=dem, landuse=lu, soils=so, landuse_lookup=CROPS, soil_lookup=syn.SOILS,
                          soil_table=syn.soil_table(), **syn.OPTIONS),
        "ex": dict(dem=ex["dem"], landuse=ex["landuse"], soils=ex["soils"], landuse_lookup=ex["landuse_lookup"],
                   soil_lookup=ex["soil_lookup"], soil_table=ex["soil_table"], stream_threshold=1.0),
    }
    watersheds = {k: ls.hydro.swat.watershed(**v) for k, v in sets.items()}
    meta["watersheds"] = {}
    for k, w in watersheds.items():
        tables = {}
        for t, df in w.tables.items():
            text, numeric = {}, []
            for c in df.columns:
                col = df[c]
                if col.map(lambda v: isinstance(v, str)).any():
                    text[c] = [None if v is None or (isinstance(v, float) and np.isnan(v)) else str(v) for v in col]
                    numeric.append(np.full(len(df), np.nan))
                else:
                    numeric.append(col.astype(float).to_numpy())
            out[f"w/{k}/{t}"] = np.column_stack(numeric) if len(df) else np.zeros((0, len(df.columns)))
            tables[t] = {"columns": list(df.columns), "text": text}
        meta["watersheds"][k] = tables

    syn_st, syn_pcp, syn_tx, syn_tn, syn_monthly = (*syn.weather(),)
    full_st, full_series, full_monthly = full_weather()
    ex_series = {"pcp": ex["pcp"], "tmax": ex["tmax"], "tmin": ex["tmin"]}

    given = wgn_from_series(syn_pcp, syn_tx, syn_tn, monthly={**syn_monthly, "pcp_hhr": 7.5})
    given = given.loc[["south"]].copy()
    given["tmp_max_ave"] += 1.0
    cases = [
        {"name": "syn", "watershed": "syn", "stations": syn_st,
         "series": {"pcp": syn_pcp, "tmax": syn_tx, "tmin": syn_tn}, "monthly": syn_monthly, "run": True},
        {"name": "syn_dated", "watershed": "syn", "stations": syn_st,
         "series": {"pcp": syn_pcp, "tmax": syn_tx, "tmin": syn_tn}, "monthly": syn_monthly,
         "start": "2000-03-05", "end": "2001-06-30", "codes": {"pet": 2, "rte_cha": 1, "lapse": 1.5}, "name_": "my name!"},
        {"name": "syn_crops", "watershed": "syn_crops", "stations": syn_st,
         "series": {"pcp": syn_pcp, "tmax": syn_tx, "tmin": syn_tn}, "monthly": syn_monthly,
         "start": "2000-01-01", "end": "2000-12-31", "run": True},
        {"name": "syn_full", "watershed": "syn", "stations": full_st, "series": full_series, "monthly": full_monthly},
        {"name": "syn_wgn_given", "watershed": "syn", "stations": syn_st,
         "series": {"pcp": syn_pcp, "tmax": syn_tx, "tmin": syn_tn}, "wgn": given},
        {"name": "ex", "watershed": "ex", "stations": ex["stations"], "series": ex_series, "monthly": ex["monthly"],
         "run": True},
    ]
    for c in cases:
        n = c["name"]
        kwargs = {"wgn": c.get("wgn"), "monthly": c.get("monthly")}
        wx = ls.hydro.swat.climate(c["stations"], **c["series"], **kwargs)
        w = watersheds[c["watershed"]]
        opt = {k: c[k] for k in ("start", "end", "codes") if k in c}
        if "name_" in c:
            opt["name"] = c["name_"]
        project = ls.hydro.swat.from_tables(w, wx, **opt)
        project.watershed = w
        # the inputs
        rec = {"name": n, "watershed": c["watershed"], "stations": table_json(c["stations"].reset_index(names="name")),
               "series": {k: list(v.columns) for k, v in c["series"].items()}, "monthly": c.get("monthly"),
               "opt": opt, "files": {}, "static": {}}
        for k, v in c["series"].items():
            out[f"{n}/in/{k}"] = series_array(v)
        if c.get("wgn") is not None:
            m, st, cols = wgn_matrix(c["wgn"])
            out[f"{n}/in/wgn"] = m
            rec["wgn_in"] = {"stations": st, "columns": cols}
        # the weather generator the project has
        m, st, cols = wgn_matrix(wx.wgn)
        out[f"{n}/wgn"] = m
        rec["wgn"] = {"stations": st, "columns": cols}
        # the files
        changed = set(opt.get("codes") and ["codes.bsn"] or [])
        for fname, b in project.files.items():
            if fname in static and fname not in changed:
                rec["static"][fname] = hashlib.sha256(body(b)).hexdigest()
            else:
                out[f"{n}/file/{fname}"] = np.frombuffer(body(b), np.uint8)
        rec["files"] = sorted(project.files)
        if c.get("run"):
            res = ls.hydro.swat.run(project, tables={t: (["year"] if t in RUN_TABLES else []) for t in TABLES},
                                    nyskip=0)
            for t in RUN_TABLES:
                out[f"{n}/run/{t}"] = frame(res.table(t, "year"), TABLES[t][1])
        meta["cases"].append(rec)

    # the statistics alone: the weather above, and the periodic series with the forms of the monthly values
    for c in cases:
        if "wgn" in c:
            continue
        n = c["name"]
        if c["name"] not in ("syn_full", "ex"):
            continue
        s = c["series"]
        wgn = wgn_from_series(s["pcp"], s["tmax"], s["tmin"], slr=s.get("slr"), hmd=s.get("hmd"), wnd=s.get("wnd"),
                              monthly=c.get("monthly"))
        m, st, cols = wgn_matrix(wgn)
        out[f"wgn/{n}"] = m
        meta["wgn_cases"].append({"name": n, "case": n, "stations": st, "columns": cols, "monthly": c.get("monthly")})
    pcp, tx, tn = small_wgn()
    forms = {"pcp_hhr": 5.0, "slr_ave": [10, 11, 12, 13, 14, 15, 16, 15, 14, 12, 11, 10],
             "dew_ave": {"a": [1.0] * 12, "b": [2 + i / 10 for i in range(12)]}, "wnd_ave": 2.0}
    monthly = {k: (pd.DataFrame(v, index=range(1, 13)) if isinstance(v, dict) else v) for k, v in forms.items()}
    wgn = wgn_from_series(pcp, tx, tn, monthly=monthly)
    m, st, cols = wgn_matrix(wgn)
    out["wgn/small"] = m
    out["wgn/small_pcp"], out["wgn/small_tmax"], out["wgn/small_tmin"] = (series_array(x) for x in (pcp, tx, tn))
    meta["wgn_cases"].append({"name": "small", "stations": st, "columns": cols, "monthly": forms,
                              "series": list(pcp.columns)})
    np.savez_compressed(REF / "swat_project.npz", **out)
    (REF / "swat_project.json").write_text(json.dumps(meta, indent=1) + "\n", encoding="utf-8")
    print(REF / "swat_project.npz", len(out))


if __name__ == "__main__":
    main()
