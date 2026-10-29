# SPDX-License-Identifier: GPL-3.0-or-later
"""What landschaft's ls.move gives, for Terraflux's tests, written to tests/data/reference/move.npz and .json:

    python tools/make_move_reference.py

The track sets of landschaft's own tests (tests/move_cases.py of its checkout: longitude / latitude from
millimetre to continental steps, the antimeridian, near a pole; UTM; no CRS; duplicated times; gaps with empty
locations) and move2's fishers (tests/data/move/fishers.csv.gz, copied here): move2's steps, filters,
interpolation, projection and Brownian tracks, amt's steps, bursts, summaries, fits, random numbers and steps, time
of day and covariates.
"""

from __future__ import annotations

import json
import shutil
import sys
import warnings
from pathlib import Path

import geopandas as gpd
import numpy as np
import pandas as pd
import xarray as xr
from affine import Affine
from shapely import Point

import landschaft as ls

ROOT = Path(__file__).resolve().parents[1]
REF = ROOT / "tests" / "data" / "reference"
LS = Path(ls.__file__).resolve().parents[2]
sys.path.insert(0, str(LS / "tests"))
from move_cases import EXTRACT_RASTER, track_sets  # noqa: E402

M = ls.move


def frame(s: dict) -> gpd.GeoDataFrame:
    t = pd.to_datetime(np.round(s["t"] * 1e9).astype(np.int64), utc=True)
    geom = [Point() if np.isnan(x) else Point(x, y) for x, y in zip(s["x"], s["y"])]
    cols = {"time": t, "track": s["track"]}
    for k in ("v1", "v2"):
        if k in s:
            cols[k] = s[k]
    return M.as_tracks(gpd.GeoDataFrame(cols, geometry=geom, crs=s["crs"]), "time", "track")


def main() -> None:
    sets = track_sets()
    fishers_src = LS / "tests" / "data" / "move" / "fishers.csv.gz"
    (ROOT / "tests" / "data" / "move").mkdir(parents=True, exist_ok=True)
    shutil.copyfile(fishers_src, ROOT / "tests" / "data" / "move" / "fishers.csv.gz")
    out: dict[str, np.ndarray] = {}
    meta: dict = {"sets": {}}
    tr = {}
    for name, s in sets.items():
        tr[name] = frame(s)
        for k in ("x", "y", "t", "v1", "v2"):
            if k in s:
                out[f"in/{name}/{k}"] = np.asarray(s[k], np.float64)
        meta["sets"][name] = {"track": [str(v) for v in s["track"]], "crs": s["crs"]}
    fishers = ls.load_tracks(fishers_src)
    fishers_ok = fishers[~fishers.geometry.is_empty].reset_index(drop=True)

    def put(key, v):
        out[key] = np.asarray(v, dtype=np.float64)

    # move2: the measures of the steps
    for name, t in [("ll", tr["ll"]), ("utm", tr["utm"]), ("plain", tr["plain"]), ("fishers", fishers_ok)]:
        put(f"measures_{name}/distance", M.distance(t))
        put(f"measures_{name}/azimuth", M.azimuth(t))
        put(f"measures_{name}/turn_angle", M.turn_angle(t))
        put(f"measures_{name}/time_lags", M.time_lags(t))
        lags = M.time_lags(t)
        if np.all(lags[~np.isnan(lags)] > 0):
            put(f"measures_{name}/speed", M.speed(t))
    put("units/km", M.distance(tr["ll"], "km"))
    put("units/deg", M.azimuth(tr["ll"], "deg"))
    put("units/turn_deg", M.turn_angle(tr["ll"], "deg"))
    put("units/utm_km", M.distance(tr["utm"], "km"))
    put("units/utm_kmh", M.speed(tr["utm"], "km/h"))
    put("units/lag_h", M.time_lags(tr["ll"], "h"))
    # duplicates: the records kept
    dup = tr["dup"]
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        for crit, kw in [("first", {}), ("last", {}), ("subsets", {}), ("subsets_equal", {}), ("sample", {"seed": 7})]:
            put(f"unique/{crit}", M.unique(dup, crit, **kw))
    # one per interval and Movebank's outliers, on the fishers (every record, empty ones too)
    for key, unit, crit, kw in [("hour", "hour", "first", {}), ("last", "hour", "last", {}),
                                ("m15", "15 mins", "first", {}), ("h2", "2 hours", "first", {}),
                                ("h5", "5 hours", "first", {}), ("day", "day", "first", {}),
                                ("d3", "3 days", "first", {}), ("week", "week", "first", {}),
                                ("month", "month", "first", {}), ("random", "day", "random", {"seed": 3})]:
        put(f"per_interval/{key}", M.per_interval(fishers, unit, crit, **kw))
    put("visible", M.movebank_visible(fishers))
    meta["fishers"] = {"n": len(fishers), "track": fishers.attrs["landschaft.move"]["track"]}
    # interpolation
    gap, gap_plain = tr["gap"], tr["gap_plain"]

    def coords(df, key, with_time=True):
        g = df.geometry
        put(f"{key}/x", [p.x if not p.is_empty else np.nan for p in g])
        put(f"{key}/y", [p.y if not p.is_empty else np.nan for p in g])
        if with_time:
            put(f"{key}/t", df["time"].array.asi8 / 1e9)

    coords(M.interpolate(gap), "interpolate_empty")
    tt = pd.date_range(gap["time"].min(), gap["time"].max(), freq="45min")
    meta["interpolate_times"] = [str(v.tz_convert(None)) for v in tt]
    coords(M.interpolate(gap, tt, max_time_lag=7200), "interpolate_times")
    coords(M.interpolate(gap, "1 hour", omit=True), "interpolate_hour_omit")
    tp = pd.date_range(gap_plain["time"].min(), gap_plain["time"].max(), freq="20min")
    meta["interpolate_plain"] = [str(v.tz_convert(None)) for v in tp]
    coords(M.interpolate(gap_plain, tp), "interpolate_plain")
    # a local projection
    meta["aeqd"] = {"centroid": M.aeqd_crs(fishers).srs, "center": M.aeqd_crs(fishers, "center").srs}
    # Brownian tracks
    b1 = M.simulate_brownian([0, 1, 2.5, 4, 8], [1, 2, 0, 0.5], tracks=3, seed=11)
    b2 = M.simulate_brownian(seed=5)
    put("brownian/x1", b1.geometry.x)
    put("brownian/y1", b1.geometry.y)
    put("brownian/x2", b2.geometry.x)
    put("brownian/y2", b2.geometry.y)

    # amt: steps
    for name, t, kw in [("utm", tr["utm"], {}), ("plain", tr["plain"], {}), ("ll", tr["ll"], {"lonlat": True})]:
        s = M.steps(t, **kw)
        for c in ("sl_", "direction_p", "ta_", "x2_", "y2_"):
            put(f"steps_{name}/{c}", s[c])
        put(f"steps_{name}/dt_", s["dt_"].dt.total_seconds())
    # bursts
    utm = tr["utm"]
    a = M.resample(utm, 60, 15)
    b = M.resample(utm, "15min", "2min", start=3)
    f = M.filter_min_n_burst(a, 4)
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        s = M.steps(a)
    for key, d in (("a", a), ("b", b), ("f", f)):
        put(f"resample/t_{key}", d["time"].array.asi8 / 1e9)
        put(f"resample/burst_{key}", d["burst_"])
    for c in ("sl_", "direction_p", "ta_", "burst_"):
        put(f"resample/steps_{c}", s[c])
    # summaries
    for name in ("utm", "ll"):
        t = tr[name]
        put(f"summary_{name}/nsd", M.nsd(t))
        for fn in ("msd", "cum_dist", "tot_dist", "straightness", "sinuosity", "tac"):
            put(f"summary_{name}/{fn}", getattr(M, fn)(t))
    # fits of each track's steps
    su = M.steps(utm)
    fits = []
    for k in pd.unique(su["track"]):
        one = su[su["track"] == k]
        sl, ta = one["sl_"].to_numpy(), one["ta_"].to_numpy()
        g = M.fit_distr(sl, "gamma")
        v = M.fit_distr(ta, "vonmises")
        e = M.fit_distr(sl[sl > 0], "exp")
        ln = M.fit_distr(sl[sl > 0], "lnorm")
        h = M.fit_distr(sl, "hnorm")
        u = M.fit_distr(ta, "unif")
        fits.append([g.params["shape"], g.params["scale"], *g.vcov.to_numpy().ravel(), v.params["kappa"],
                     float(v.vcov.iloc[0, 0]), e.params["rate"], ln.params["meanlog"], ln.params["sdlog"],
                     h.params["sd"], u.params["min"], u.params["max"]])
    put("fits", fits)
    # random numbers
    dists = [("gamma", {"shape": 0.4, "scale": 120}), ("gamma", {"shape": 3.5, "scale": 20}), ("exp", {"rate": 0.02}),
             ("lnorm", {"meanlog": 3, "sdlog": 0.7}), ("hnorm", {"sd": 50}), ("unif", {}),
             ("vonmises", {"kappa": 0}), ("vonmises", {"kappa": 2.5})]
    meta["random_numbers"] = dists
    put("random_numbers", [M.random_numbers(M.make_distribution(n, **p), 300, seed=17) for n, p in dists])
    # random steps
    r = M.random_steps(su, n_control=5, seed=42)
    for c in ("x2_", "y2_", "sl_", "ta_", "case_", "step_id_"):
        put(f"random_steps/{c}", r[c].astype(np.float64))
    u = M.random_steps(su, n_control=3, sl_distr=M.make_distribution("exp", rate=0.01),
                       ta_distr=M.make_distribution("unif"), seed=3)
    for c in ("x2_", "y2_", "sl_", "ta_"):
        put(f"random_steps_given/{c}", u[c])
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        rb = M.random_steps(M.steps(a), seed=7)
    for c in ("x2_", "y2_", "sl_", "ta_", "case_", "step_id_", "burst_"):
        put(f"random_steps_bursts/{c}", rb[c].astype(np.float64))
    # time of day
    ll = tr["ll"]
    sll = M.steps(ll)
    codes = {"day": 1, "dusk": 2, "night": 3, "dawn": 4}

    def tod(c):
        return [codes[v] if isinstance(v, str) else np.nan for v in c]

    put("time_of_day/tod", tod(M.time_of_day(ll)))
    put("time_of_day/crep", tod(M.time_of_day(ll, include_crepuscule=True)))
    put("time_of_day/end", tod(M.time_of_day(sll, include_crepuscule=True)))
    put("time_of_day/start", tod(M.time_of_day(sll, where="start")))
    put("time_of_day/fishers", tod(M.time_of_day(fishers_ok, include_crepuscule=True)))
    # covariates
    e = EXTRACT_RASTER
    xs, ys = utm.geometry.x.to_numpy(), utm.geometry.y.to_numpy()
    x0, y1 = xs[e["x_from"]], ys[e["y_from"]]
    v = np.arange(1, e["nrows"] * e["ncols"] + 1, dtype=np.float64)
    v[v % e["na_every"] == 0] = np.nan
    res = e["res"]
    cover = xr.DataArray(v.reshape(e["nrows"], e["ncols"]), dims=("y", "x"),
                         coords={"y": y1 - (np.arange(e["nrows"]) + 0.5) * res,
                                 "x": x0 + (np.arange(e["ncols"]) + 0.5) * res}, name="cover")
    cover = cover.rio.write_transform(Affine(res, 0, x0, 0, -res, y1)).rio.write_crs("EPSG:32723")
    meta["extract"] = {"transform": [x0, res, 0.0, y1, 0.0, -res], "nrows": e["nrows"], "ncols": e["ncols"],
                       "na_every": e["na_every"]}
    put("extract/track", M.extract_covariates(utm, {"cover": cover})["cover"])
    both = M.extract_covariates(su, {"cover": cover}, where="both")
    put("extract/both_start", both["cover_start"])
    put("extract/both_end", both["cover_end"])
    put("extract/start", M.extract_covariates(su, {"cover": cover}, where="start")["cover"])
    put("extract/random", M.extract_covariates(r, {"cover": cover})["cover"])

    np.savez_compressed(REF / "move.npz", **out)
    (REF / "move.json").write_text(json.dumps(meta, indent=1) + "\n", encoding="utf-8")
    print(REF / "move.npz", len(out))


if __name__ == "__main__":
    main()
