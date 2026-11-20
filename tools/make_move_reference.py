# SPDX-License-Identifier: GPL-3.0-or-later
"""What the library's tf.move gives, for Terraflux's tests, written to tests/data/reference/move.npz and .json:

    python tools/make_move_reference.py

The track sets of the library's own tests (tests/move_cases.py of its checkout: longitude / latitude from
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

import terraflux as tf

ROOT = Path(__file__).resolve().parents[1]
REF = ROOT / "tests" / "data" / "reference"
LS = Path(tf.__file__).resolve().parents[2]
sys.path.insert(0, str(LS / "tests"))
from move_cases import (ALONG_CASES, ALONG_LAYERS, ALONG_RASTERS, EXTRACT_RASTER, M2U_SYNTHETIC, SPEED,  # noqa: E402
                        SPEED_MASS, SPEED_MODE, THIN_DISTANCE, THIN_TIME, along_raster, along_track_sets,
                        m2u_track_sets, track_sets)

M = tf.move
ALONG_STATS = ("mean", "min", "max", "ascent", "descent", "delta")


def frame(s: dict) -> gpd.GeoDataFrame:
    t = pd.to_datetime(np.round(s["t"] * 1e9).astype(np.int64), utc=True)
    geom = [Point() if np.isnan(x) else Point(x, y) for x, y in zip(s["x"], s["y"])]
    cols = {"time": t, "track": s["track"]}
    for k in ("v1", "v2"):
        if k in s:
            cols[k] = s[k]
    return M.as_tracks(gpd.GeoDataFrame(cols, geometry=geom, crs=s["crs"]), "time", "track")


def m2u_frame(s: dict) -> gpd.GeoDataFrame:
    """A set of the cleaning cases: a missing time is NaT, a missing coordinate an empty point."""
    t = np.asarray(s["t"], np.float64)
    geom = [Point() if (np.isnan(a) or np.isnan(b)) else Point(a, b) for a, b in zip(s["x"], s["y"])]
    time = pd.Series(pd.to_datetime((np.nan_to_num(t) * 1e9).astype(np.int64), utc=True)).where(~np.isnan(t))
    cols = {"time": time, "track": s["track"]}
    for k in s:
        if k not in ("x", "y", "t", "track", "crs"):
            cols[k] = s[k]
    return M.as_tracks(gpd.GeoDataFrame(cols, geometry=geom, crs=s["crs"]), "time", "track", sort=False)


# the GPS quality calls of the library's tests (tests/test_move_clean.py)
QUALITY = {"default": {}, "nosat": {"sat_min": None}, "strict": {"sat_min": 8, "dop_max": 3, "hacc_max": 40},
           "keepempty": {"drop_empty": False}, "nodop": {"dop_max": None, "hacc_max": 60},
           "none": {"sat_min": None, "dop_max": None, "hacc_max": None, "drop_empty": False}}
GPS = [("gps_cl_q", "cl_q"), ("gps_cl_qd", "cl_qd"), ("gps_synth", "m2u_synthetic"), ("gps_fishers", "fishers")]


def clean_reference(out: dict, meta: dict, fishers_src: Path, synth_src: Path) -> None:
    """The cleaning and thinning of the library (move2utils' v_phys_estimate, mt_flag_speed_cap with a hard cap,
    mt_filter_gps_quality, mt_thin_time and mt_thin_distance): the sets of tests/move_cases.py's m2u_track_sets, move2's
    fishers and move2utils' synthetic tracks (tests/data/move/m2u_synthetic.csv.gz, GPL-3.0-or-later). Everything is
    new keys (`in/m2u/...`, `clean/...`); the cases' arguments are in the json (`clean`)."""
    shutil.copyfile(synth_src, ROOT / "tests" / "data" / "move" / "m2u_synthetic.csv.gz")
    sets = m2u_track_sets()
    tracks = {}
    meta["m2u_sets"] = {}
    for name, s in sets.items():
        tracks[name] = m2u_frame(s)
        for k in s:
            if k not in ("track", "crs"):
                out[f"in/m2u/{name}/{k}"] = np.asarray(s[k], np.float64)
        meta["m2u_sets"][name] = {"track": [str(v) for v in s["track"]], "crs": s["crs"]}
    tracks["fishers"] = tf.load_tracks(fishers_src, sort=False)
    tracks["m2u_synthetic"] = tf.load_tracks(synth_src, sort=False)

    def put(key, v):
        out[key] = np.asarray(v, dtype=np.float64)

    def as_float(flags):
        return pd.Series(flags).astype("Float64").to_numpy(np.float64, na_value=np.nan)

    cases = meta["clean"] = {"speed": {}, "thin_time": {}, "thin_distance": {}, "gps": {}}
    # v_phys_estimate
    masses = [1e-9, 3e-8, 0.001, 0.02, 0.5, 1, 2, 5, 6, 70, 250, 1000, 25000, 1.084e5, 2e5]
    cases["v_phys"] = {"masses": masses, "levels": [0.95, 0.9, 0.99], "modes": ["flying", "running", "swimming"]}
    got = {"v": [], "lo": [], "hi": [], "kmh": []}
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        for lv in (0.95, 0.9, 0.99):
            for mode in ("flying", "running", "swimming"):
                r = M.v_phys_estimate(masses, mode, ci_level=lv)
                got["v"] += list(r.v_max)
                got["lo"] += list(r.lower)
                got["hi"] += list(r.upper)
                got["kmh"] += list(r.kmh)
    for k, v in got.items():
        put(f"clean/v_phys/{k}", v)
    # the hard speed cap
    for case, (data, v_max, jitter, _) in SPEED.items():
        tr = tracks[data]
        if data == "fishers":
            tr = tr[~tr.geometry.is_empty]
        kw = {"jitter": jitter}
        kw.update({"mass": SPEED_MASS, "mode": SPEED_MODE} if v_max is None else {"v_max": v_max})
        r = M.flag_speed(tr, **kw)
        put(f"clean/{case}/outlier", r["is_outlier"])
        put(f"clean/{case}/speed", r["step_speed"])
        put(f"clean/{case}/above", as_float(r["is_speed_above_cap"]))
        if jitter is not None:
            put(f"clean/{case}/below", as_float(r["is_step_below_jitter"]))
        put(f"clean/{case}/flagged", r["flagged_by_speed"])
        put(f"clean/{case}/loglr", r["loglr_speed"])
        put(f"clean/{case}/vmax", [r.attrs["v_max_used"]])
        cases["speed"][case] = {"data": data, "v_max": v_max, "jitter": jitter,
                                "mass": SPEED_MASS if v_max is None else None, "mode": SPEED_MODE}
    # thinning in time and in distance
    for case, (data, interval, tolerance, criterion) in THIN_TIME.items():
        put(f"clean/{case}/selected", M.thin_time(tracks[data], interval, tolerance, criterion))
        cases["thin_time"][case] = {"data": data, "interval": interval, "tolerance": tolerance, "criterion": criterion}
    for case, (data, distance, tolerance, criterion, method) in THIN_DISTANCE.items():
        tr = tracks[data]
        if data == "fishers":
            tr = tr[~tr.geometry.is_empty]
        tol = tolerance if method == "interval" else None
        put(f"clean/{case}/selected", M.thin_distance(tr, distance, method, tol, criterion))
        cases["thin_distance"][case] = {"data": data, "distance": distance, "tolerance": tol, "criterion": criterion,
                                        "method": method}
    # GPS quality
    for case, data in GPS:
        for key, kw in QUALITY.items():
            if data == "m2u_synthetic" and key == "strict":
                kw = {"sat_min": 8, "dop_max": 3}
            put(f"clean/{case}/{key}", M.gps_quality(tracks[data], **kw))
            cases["gps"].setdefault(case, {"data": data, "calls": {}})["calls"][key] = kw


def along_reference(out: dict, meta: dict, fishers_src: Path) -> None:
    """The values along the steps (extract_covariates_along) and their summaries (along_summary) of the library's
    cases (tests/move_cases.py: along_track_sets, ALONG_RASTERS, along_raster) and of the fishers' steps, on the rasters
    of ALONG_RASTERS (three layers made of integers). New keys only: `in/along/<case>/{x1,y1,x2,y2}` the steps,
    `along/<case>/off` the cells per step (n + 1 offsets), `along/<case>/v<k>` the values of layer k (cover, lvl,
    hill) of every cell in terra's order and `along/<case>/<layer>_<stat>` the summaries per step; the rasters and
    the cases are in the json (`along`)."""
    sets = along_track_sets()
    f = tf.load_tracks(fishers_src, sort=False)
    fs = M.steps(f[~f.geometry.is_empty])
    steps = {name: pd.DataFrame({"x1_": s["x"], "y1_": s["y"], "x2_": s["x2"], "y2_": s["y2"]})
             for name, s in sets.items()}
    steps["along_fishers"] = fs[["x1_", "y1_", "x2_", "y2_"]].reset_index(drop=True)
    cases = dict(ALONG_CASES, along_fishers="fishers")
    crs = {name: sets[name]["crs"] for name in sets}
    crs["along_fishers"] = ALONG_RASTERS["fishers"]["crs"]
    meta["along"] = {"rasters": {k: dict(v) for k, v in ALONG_RASTERS.items()}, "layers": list(ALONG_LAYERS),
                     "stats": list(ALONG_STATS), "cases": {}}
    for case, name in cases.items():
        r = along_raster(name)
        layers = {}
        for k in ALONG_LAYERS:
            da = xr.DataArray(r["values"][k], dims=("y", "x"), name=k,
                              coords={"y": r["ymax"] - r["yres"] * (np.arange(r["nrows"]) + 0.5),
                                      "x": r["xmin"] + r["xres"] * (np.arange(r["ncols"]) + 0.5)})
            if r["crs"]:
                da = da.rio.write_crs(r["crs"])
            # the transform the library reads back exactly (its _grid), not the one rioxarray derives from the coordinates
            layers[k] = da.rio.write_transform(Affine(r["xres"], 0, r["xmin"], 0, -r["yres"], r["ymax"]))
        s = steps[case]
        for c in ("x1_", "y1_", "x2_", "y2_"):
            out[f"in/along/{case}/{c[:2]}"] = s[c].to_numpy(np.float64)
        got = M.extract_covariates_along(s, layers)
        out[f"along/{case}/off"] = np.r_[0, np.cumsum([len(g) for g in got])].astype(np.float64)
        table = np.concatenate([g.to_numpy() for g in got])
        for j, k in enumerate(ALONG_LAYERS):
            out[f"along/{case}/v{j + 1}"] = table[:, j]
        summary = M.along_summary(s, layers)
        for k in ALONG_LAYERS:
            for st in ALONG_STATS:
                out[f"along/{case}/{k}_{st}"] = summary[f"{k}_along_{st}"].to_numpy(np.float64)
        meta["along"]["cases"][case] = {"raster": name, "crs": crs[case], "n": len(s)}


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
    fishers = tf.load_tracks(fishers_src)
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
    meta["fishers"] = {"n": len(fishers), "track": fishers.attrs["terraflux.move"]["track"]}
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

    clean_reference(out, meta, fishers_src, LS / M2U_SYNTHETIC)
    along_reference(out, meta, fishers_src)

    np.savez_compressed(REF / "move.npz", **out)
    (REF / "move.json").write_text(json.dumps(meta, indent=1) + "\n", encoding="utf-8")
    print(REF / "move.npz", len(out))


if __name__ == "__main__":
    main()
