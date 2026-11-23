# SPDX-License-Identifier: GPL-3.0-or-later
"""What the library's tf.move gives, for Terraflux's tests, written to tests/data/reference/move.npz and .json:

    python tools/make_move_reference.py

The track sets of the library's own tests (tests/move_cases.py of its checkout: longitude / latitude from
millimetre to continental steps, the antimeridian, near a pole; UTM; no CRS; duplicated times; gaps with empty
locations) and move2's fishers (tests/data/move/fishers.csv.gz, copied here): move2's steps, filters,
interpolation, projection and Brownian tracks, amt's steps, bursts, summaries, fits, random numbers and steps, time
of day and covariates, the cleaning and thinning, the values along the steps, the corridors and the distances between
UDs, amt's home ranges (MCP, KDE with its bandwidths, LoCoH, isopleths, UDs, overlaps) and the dynamic Brownian
bridges (the dBBMM and the dBGB: variances, distributions, volumes, outer probabilities, window suggestions, masks)
the length of steps on the relief (surface_distance), the step selection functions (fit_issf and log_rss) and
the simulation from a step selection model (make_issf_model, simulate_path, resistance, path_density).
`--only-relief`, `--only-issf` and `--only-sim` add just those keys to the files written.
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
from move_issf_cases import (BOOT_A, BOOT_B, F_A, F_B, F_BOOT, F_BOOT_LOG, F_COL, F_FAC, F_ONE, F_TIES_NUM,  # noqa: E402
                             F_TIES_TXT, LR_A, LR_BOOT, LR_BOOT1, LR_COL, LR_FAC, issf_track_sets)
from move_cases import (ALONG_CASES, ALONG_LAYERS, ALONG_RASTERS, CORRIDOR, EXTRACT_RASTER, M2U_SYNTHETIC,  # noqa: E402
                        SPEED, SPEED_MASS, SPEED_MODE, THIN_DISTANCE, THIN_TIME, along_raster, along_track_sets,
                        corridor_track_sets, hr_track_sets, m2u_track_sets, track_sets, ud_track_sets)

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


COR_SETS = ("cor_grid", "cor_lattice", "cor_zero")
# the distance between UDs: the coarse UDs of three fishers (the library's emd_fishers case, 30 x 20 cells) and blobs on a
# longitude / latitude grid (emd_lonlat), in the library's own move2utils reference (tests/data/reference/move)
EMD_UDS = {"fishers_c": ("emd_fishers", "_c", ("F1", "F2", "M1"), None),
           "lonlat": ("emd_lonlat", "", ("a", "b", "c"), "EPSG:4326")}
EMD_CASES = {  # key -> (data, arguments of ud_distance)
    "sink": ("fishers_c", {}), "mask09": ("fishers_c", {"mask_quantile": 0.9}),
    "mask05": ("fishers_c", {"mask_quantile": 0.5}), "mask1": ("fishers_c", {"mask_quantile": 1}),
    "reg01": ("fishers_c", {"reg": 0.1}), "reg002": ("fishers_c", {"reg": 0.02, "max_iter": 2000}),
    "tol13": ("fishers_c", {"tol": 1e-13}), "thr": ("fishers_c", {"threshold": 1500}),
    "iter3": ("fishers_c", {"max_iter": 3}),
    "exact": ("fishers_c", {"method": "exact"}),
    "exact_q": ("fishers_c", {"method": "exact", "mask_quantile": 0.9}),
    "exact_iter3": ("fishers_c", {"method": "exact", "exact_max_iter": 3}),
    "gc": ("lonlat", {"gc": True}), "gc_q": ("lonlat", {"gc": True, "mask_quantile": 0.9}),
    "gc_thr": ("lonlat", {"gc": True, "threshold": 20000}),
    "gc_reg": ("lonlat", {"gc": True, "reg": 0.02, "max_iter": 1500}),
    "deg": ("lonlat", {}), "deg_thr": ("lonlat", {"threshold": 0.1}),
}


def corridor_reference(out: dict, meta: dict, tr: dict, fishers_src: Path) -> None:
    """LaPoint's corridors (move2utils' mt_corridor) of the library and the earth mover's distance between UDs
    (emd), on the cases of the library's tests (tests/move_cases.py: CORRIDOR, corridor_track_sets) except the whole
    of the fishers (47 thousand locations: the track M1 stays). New keys only: `in/cor/<set>/{x,y,t}` the lattices
    and the dwelling animals, `corridor/<case>/{flag,speed,azimuth,circvar,nb,thr}` per location (`thr`: the
    thresholds used), `in/emd/<data>/ud` the UDs (band, row, column) and `emd/<data>/<key>` the distances of the pairs
    (1, 2), (1, 3), (2, 3); the cases, the lattices' tracks and the UDs' grids are in the json (`corridor`, `emd`)."""
    sets = corridor_track_sets()
    meta["corridor"] = {"sets": {}, "cases": {}}
    frames = {"utm": tr["utm"], "ll": tr["ll"]}
    for name in COR_SETS:
        s = sets[name]
        frames[name] = m2u_frame(s)
        for k in ("x", "y", "t"):
            out[f"in/cor/{name}/{k}"] = np.asarray(s[k], np.float64)
        meta["corridor"]["sets"][name] = {"track": [str(v) for v in s["track"]], "crs": s["crs"]}
    frames["cl_ft"] = m2u_frame(m2u_track_sets()["cl_ft"])
    fishers = tf.load_tracks(fishers_src, sort=False)
    frames["fishers"] = fishers[fishers[M._tracks.columns(fishers)[1]] == "M1"]
    for case, (data, one, args, _) in CORRIDOR.items():
        if data == "fishers" and one is None:
            continue
        with warnings.catch_warnings():
            warnings.simplefilter("ignore")
            r = M.corridor(frames[data], **args)
        used = r.attrs["corridor"]
        out[f"corridor/{case}/flag"] = (r["corridor"] == "corridor").to_numpy().astype(np.float64)
        out[f"corridor/{case}/speed"] = r["corridor_speed"].to_numpy(np.float64)
        out[f"corridor/{case}/azimuth"] = r["corridor_azimuth"].to_numpy(np.float64)
        out[f"corridor/{case}/circvar"] = r["corridor_circvar"].to_numpy(np.float64)
        out[f"corridor/{case}/nb"] = r["corridor_n_neighbours"].astype("Float64").to_numpy(np.float64, na_value=np.nan)
        out[f"corridor/{case}/thr"] = np.array([used["speed_threshold"], used["circvar_threshold"]], np.float64)
        meta["corridor"]["cases"][case] = {"data": data, "track": one, "args": args}

    ref = np.load(LS / "tests" / "data" / "reference" / "move" / "move2utils_corridor.npz")
    meta["emd"] = {"uds": {}, "cases": {}}
    stacks = {}
    for data, (case, suffix, names, crs) in EMD_UDS.items():
        ny, nx, nl = (int(v) for v in ref[f"{case}/dim{suffix}"])
        xs, ys = ref[f"{case}/xs{suffix}"], ref[f"{case}/ys{suffix}"]
        xres, yres = (xs[-1] - xs[0]) / (nx - 1), (ys[0] - ys[-1]) / (ny - 1)
        # the grid Terraflux rebuilds: cell centres from the corner and the size, as its operator computes them
        xmin, ymax = float(xs[0] - xres / 2), float(ys[0] + yres / 2)
        x = xmin + (np.arange(nx) + 0.5) * xres
        y = ymax + (np.arange(ny) + 0.5) * -yres
        ud = np.asarray(ref[f"{case}/ud{suffix}"], np.float64).reshape(nl, ny, nx)
        out[f"in/emd/{data}/ud"] = ud.ravel()
        meta["emd"]["uds"][data] = {"nx": nx, "ny": ny, "bands": list(names), "xmin": xmin, "xres": float(xres),
                                    "ymax": ymax, "yres": float(yres), "crs": crs}
        da = xr.DataArray(ud, coords={"band": list(names), "y": y, "x": x}, dims=("band", "y", "x"))
        stacks[data] = da.rio.write_crs(crs) if crs else da
    for key, (data, args) in EMD_CASES.items():
        with warnings.catch_warnings():
            warnings.simplefilter("ignore")
            table = M.ud_distance(stacks[data], **args)
        out[f"emd/{data}/{key}"] = table["distance"].to_numpy(np.float64)
        meta["emd"]["cases"][key] = {"data": data, "args": args}
    # one UD against the other (a number), and a UD against itself
    f = stacks["fishers_c"]
    out["emd/fishers_c/pair"] = np.array([M.ud_distance(f.sel(band="F1"), f.sel(band="M1"))], np.float64)
    out["emd/fishers_c/same"] = np.array([M.ud_distance(f.sel(band="F1"), f.sel(band="F1"))], np.float64)
    out["emd/fishers_c/same_exact"] = np.array(
        [M.ud_distance(f.sel(band="F1"), f.sel(band="F1"), method="exact")], np.float64)

# amt's home ranges (the library's tests/move_cases.py: hr_track_sets) on its track sets: UTM (three tracks of
# different shapes), the same without a CRS, three small tracks, the fishers' first 150 locations each, and a set of
# degenerate tracks. Each case is (set, method, settings of Terraflux's move.home_range); the library is called with the
# same.
HR_LV5 = [0.1, 0.5, 0.75, 0.95, 1.0]
HR_SETS = ("hr_utm", "hr_plain", "hr_small", "hr_fishers_s", "hr_edge")
HR_CASES = {
    "mcp_utm": ("hr_utm", "mcp", {"levels": HR_LV5}),
    "mcp_plain": ("hr_plain", "mcp", {"levels": HR_LV5}),
    "mcp_small": ("hr_small", "mcp", {"levels": HR_LV5}),
    "mcp_fishers": ("hr_fishers_s", "mcp", {"levels": HR_LV5}),
    "kde_small": ("hr_small", "kde", {"levels": [0.5, 0.9, 0.95], "res": 6.0, "ud": True}),
    "kde_small_h": ("hr_small", "kde", {"levels": [0.5, 0.95], "res": 6.0, "bandwidth": [25.0, 40.0], "ud": True}),
    "kde_small_pi": ("hr_small", "kde", {"levels": [0.5, 0.95], "res": 6.0, "bandwidth": "pi"}),
    "kde_small_piu": ("hr_small", "kde", {"levels": [0.9], "res": 6.0, "bandwidth": "pi", "rescale": "unitvar"}),
    "kde_small_lscv": ("hr_small", "kde", {"levels": [0.5, 0.95], "res": 6.0, "bandwidth": "lscv"}),
    "kde_small_lscv_local": ("hr_small", "kde", {"levels": [0.9], "res": 6.0, "bandwidth": "lscv",
                                                 "which_min": "local"}),
    "kde_small_lscv_unit": ("hr_small", "kde", {"levels": [0.5], "res": 6.0, "bandwidth": "lscv",
                                                "rescale": "unitvar"}),
    "kde_small_refu": ("hr_small", "kde", {"levels": [0.9], "res": 6.0, "bandwidth": "ref", "rescale": "unitvar"}),
    "kde_small_refx": ("hr_small", "kde", {"levels": [0.9], "res": 6.0, "bandwidth": "ref", "rescale": "xvar"}),
    "kde_utm": ("hr_utm", "kde", {"levels": [0.5, 0.9, 0.95], "ud_track": "b"}),
    "kde_utm_scaled": ("hr_utm", "kde", {"levels": [0.95], "bandwidth": "ref_scaled"}),
    "kde_utm_scaled2": ("hr_utm", "kde", {"levels": [0.9], "bandwidth": "ref_scaled", "num_parts": 2}),
    "kde_utm_pi": ("hr_utm", "kde", {"levels": [0.5, 0.95], "bandwidth": "pi"}),
    "kde_plain": ("hr_plain", "kde", {"levels": [0.5, 0.9, 0.95]}),
    "kde_fishers": ("hr_fishers_s", "kde", {"levels": [0.5, 0.95]}),
    "kde_like": ("hr_small", "kde", {"levels": [0.5, 0.9], "like": {"xmin": 189800.0, "ymax": 8250200.0, "res": 4.0,
                                                                     "ncol": 100, "nrow": 120}, "ud": True}),
    "locoh_k_utm": ("hr_utm", "locoh", {"n": 8, "type": "k", "levels": [0.5, 0.9, 0.95]}),
    "locoh_k_small": ("hr_small", "locoh", {"n": 4, "type": "k", "levels": [0.3, 0.8, 1.0]}),
    "locoh_k_fishers": ("hr_fishers_s", "locoh", {"n": 10, "type": "k", "levels": [0.5, 0.95]}),
    "locoh_r_utm": ("hr_utm", "locoh", {"n": 60.0, "type": "r", "levels": [0.5, 0.9, 0.95]}),
    "locoh_a_utm": ("hr_utm", "locoh", {"n": 400.0, "type": "a", "levels": [0.5, 0.9, 0.95]}),
    "locoh_plain": ("hr_plain", "locoh", {"n": 6, "type": "k", "levels": [0.95]}),
}
# the overlaps: (set, the home ranges (method and settings of move.home_range), overlap type, conditional, which)
HR_OVERLAPS = {
    "hr_kde": ("hr_utm", ("kde", {"levels": [0.5, 0.9]}), "hr", False, "all"),
    "hr_mcp": ("hr_utm", ("mcp", {"levels": [0.5, 0.9]}), "hr", False, "all"),
    "hr_locoh": ("hr_utm", ("locoh", {"levels": [0.5, 0.9], "n": 6, "type": "k"}), "hr", False, "all"),
    "hr_cons": ("hr_utm", ("mcp", {"levels": [0.5, 0.9]}), "hr", False, "consecutive"),
    "hr_one": ("hr_utm", ("mcp", {"levels": [0.5, 0.9]}), "hr", False, "one_to_all"),
    "hr_fishers": ("hr_fishers_s", ("mcp", {"levels": [0.5, 0.9]}), "hr", False, "all"),
    **{t: ("hr_utm", ("kde", {"levels": [0.5, 0.9]}), t, False, "all") for t in ("phr", "vi", "ba", "udoi", "hd")},
    **{"c" + t: ("hr_utm", ("kde", {"levels": [0.5, 0.9]}), t, True, "all") for t in ("phr", "vi", "ba", "udoi", "hd")},
    "cons": ("hr_utm", ("kde", {"levels": [0.5, 0.9]}), "vi", False, "consecutive"),
    "one": ("hr_utm", ("kde", {"levels": [0.5, 0.9]}), "udoi", False, "one_to_all"),
    "fishers_vi": ("hr_fishers_s", ("kde", {"levels": [0.5, 0.95]}), "vi", True, "consecutive"),
}


def hr_enc(g) -> list:
    """A Polygon or MultiPolygon as numbers: the polygons, their rings, the vertices (x, y pairs)."""
    polys = list(g.geoms) if g.geom_type in ("MultiPolygon", "GeometryCollection") else [g]
    polys = [p for p in polys if p.geom_type == "Polygon" and not p.is_empty]
    out = [len(polys)]
    for p in polys:
        rings = [p.exterior, *p.interiors]
        out.append(len(rings))
        for r in rings:
            c = np.asarray(r.coords, np.float64)
            out.append(len(c))
            out.extend(c.reshape(-1).tolist())
    return out


def hr_frame(s: dict) -> gpd.GeoDataFrame:
    t = pd.to_datetime((np.asarray(s["t"]) * 1e9).astype(np.int64), utc=True)
    geom = [Point(a, b) for a, b in zip(s["x"], s["y"])]
    df = gpd.GeoDataFrame({"time": t, "track": s["track"]}, geometry=geom, crs=s["crs"])
    return M.as_tracks(df, "time", "track", sort=False)


def hr_like(spec: dict, crs):
    """The template of a case: cells of a given size from a corner (all the numbers exact in binary)."""
    xs = spec["xmin"] + (np.arange(spec["ncol"]) + 0.5) * spec["res"]
    ys = spec["ymax"] - (np.arange(spec["nrow"]) + 0.5) * spec["res"]
    da = xr.DataArray(np.zeros((spec["nrow"], spec["ncol"])), dims=("y", "x"), coords={"y": ys, "x": xs})
    return da.rio.write_crs(crs) if crs else da


def hr_library(tr, method: str, p: dict, crs=None):
    """The library's home range for the settings of Terraflux's operator."""
    levels = p.get("levels", [0.95])
    if method == "mcp":
        return M.hr_mcp(tr, levels=levels)
    if method == "locoh":
        return M.hr_locoh(tr, n=p.get("n", 10), type=p.get("type", "k"), levels=levels)
    trast = None
    if "like" in p:
        trast = hr_like(p["like"], crs)
    elif "res" in p or "factor" in p:
        trast = M.make_trast(tr, factor=p.get("factor", 1.5), res=p.get("res"))
    bw = p.get("bandwidth", "ref")
    rescale = p.get("rescale", "none")
    h = None
    if isinstance(bw, list):
        h = bw
    elif bw == "ref" and rescale != "none":
        h = M.hr_kde_ref(tr, rescale=rescale)
    elif bw == "pi":
        h = M.hr_kde_pi(tr, rescale=rescale)
    elif bw == "lscv":
        h = M.hr_kde_lscv(tr, which_min=p.get("which_min", "global"), rescale=rescale).h
    elif bw == "ref_scaled":
        sc = M.hr_kde_ref_scaled(tr, trast=trast, levels=max(levels), num_parts=p.get("num_parts", 1))
        h = {k: float(v) for k, v in sc["h"].items()}
    return M.hr_kde(tr, h=h, trast=trast, levels=levels)


def hr_reference(out: dict, meta: dict) -> None:
    """amt's home ranges of the library (hr_mcp, hr_kde with its bandwidths, hr_locoh, hr_isopleths, hr_area, hr_ud,
    hr_overlap, intensity_use) on the sets of its tests (tests/move_cases.py: hr_track_sets; the fishers' whole set is
    left out, its first 150 locations each stay). New keys only: `in/hr/<set>/{x,y}` the locations,
    `hr/<case>/{level,area,glen,geom}` the polygons as hr_isopleths lists them (`geom`: each one's vertices as hr_enc,
    `glen` their lengths), `hr/<case>/h` (hx, hy of each track), `hr/<case>/ud` the UD of each track (hr_ud),
    `hr/overlap/<case>/{from,to,level,overlap}`, `hr/iu/<set>` the intensity of use and `hr/edge/...` the degenerate
    tracks one by one; the cases and the sets' tracks are in the json (`hr`)."""
    sets = hr_track_sets()
    meta["hr"] = {"sets": {}, "cases": {}, "overlaps": {}, "edge": {}}
    frames = {}
    for name in HR_SETS:
        s = sets[name]
        frames[name] = hr_frame(s)
        out[f"in/hr/{name}/x"] = np.asarray(s["x"], np.float64)
        out[f"in/hr/{name}/y"] = np.asarray(s["y"], np.float64)
        meta["hr"]["sets"][name] = {"track": [str(v) for v in s["track"]], "crs": s["crs"]}
    for name in ("hr_utm", "hr_plain", "hr_small", "hr_fishers_s"):
        out[f"hr/iu/{name}"] = M.intensity_use(frames[name]).to_numpy(np.float64)

    def polygons(key, iso):
        geoms = [hr_enc(g) for g in iso.geometry]
        out[f"hr/{key}/level"] = iso["level"].to_numpy(np.float64)
        out[f"hr/{key}/area"] = iso["area"].to_numpy(np.float64)
        out[f"hr/{key}/glen"] = np.array([len(g) for g in geoms], np.float64)
        out[f"hr/{key}/geom"] = np.concatenate([np.asarray(g, np.float64) for g in geoms])

    for key, (name, method, p) in HR_CASES.items():
        tr = frames[name]
        with warnings.catch_warnings():
            warnings.simplefilter("ignore")
            hr = hr_library(tr, method, p, sets[name]["crs"])
            try:
                polygons(key, M.hr_isopleths(hr))
            except ValueError as e:
                raise ValueError(f"{key}: {e}") from e
        if method == "kde":
            out[f"hr/{key}/h"] = hr.h[["hx", "hy"]].to_numpy(np.float64).reshape(-1)
            ud = M.hr_ud(hr)
            if p.get("ud"):
                out[f"hr/{key}/ud"] = ud.values.reshape(-1)
            if p.get("ud_track"):
                out[f"hr/{key}/ud"] = ud.sel(track=p["ud_track"]).values.reshape(-1)
            meta["hr"]["cases"].setdefault(key, {})["grid"] = [int(ud.sizes["x"]), int(ud.sizes["y"])]
        meta["hr"]["cases"].setdefault(key, {}).update({"set": name, "method": method, "params": p})
    # the bandwidths by themselves (hr_kde_ref, hr_kde_pi and hr_kde_lscv are compared through the kde cases above;
    # here the reference bandwidths of the fishers, pooled template or not)
    for key, (name, p) in {"ref_fishers": ("hr_fishers_s", {"levels": [0.5, 0.95]})}.items():
        out[f"hr/{key}"] = M.hr_kde_ref(frames[name]).to_numpy(np.float64).reshape(-1)
    # the overlaps
    for key, (name, (method, p), kind, cond, which) in HR_OVERLAPS.items():
        with warnings.catch_warnings():
            warnings.simplefilter("ignore")
            hr = hr_library(frames[name], method, p, sets[name]["crs"])
            ov = M.hr_overlap(hr, type=kind, conditional=cond, which=which)
        labels = list(hr.tracks)
        out[f"hr/overlap/{key}/from"] = np.array([labels.index(v) + 1 for v in ov["from"]], np.float64)
        out[f"hr/overlap/{key}/to"] = np.array([labels.index(v) + 1 for v in ov["to"]], np.float64)
        out[f"hr/overlap/{key}/level"] = ov["level"].to_numpy(np.float64)
        out[f"hr/overlap/{key}/overlap"] = ov["overlap"].to_numpy(np.float64)
        meta["hr"]["overlaps"][key] = {"set": name, "method": method, "params": p, "type": kind, "conditional": cond,
                                        "which": which}
    # degenerate tracks, one at a time: whether each estimate succeeds and its areas
    tr = frames["hr_edge"]
    meta["hr"]["edge"] = {"tracks": [str(v) for v in pd.unique(tr["track"])],
                          "cases": {"mcp": {"levels": [0.5, 1.0]}, "kde": {"levels": [0.9]},
                                    "locoh": {"n": 2, "type": "k", "levels": [0.5, 0.9]}}}
    for label in pd.unique(tr["track"]):
        one = tr[tr["track"] == label]
        for method, p in meta["hr"]["edge"]["cases"].items():
            key = f"hr/edge/{method}/{label}"
            try:
                with warnings.catch_warnings():
                    warnings.simplefilter("ignore")
                    iso = M.hr_isopleths(hr_library(one, method, p, sets["hr_edge"]["crs"]))
                out[key] = np.concatenate([[1.0], iso["area"].to_numpy(np.float64), iso["level"].to_numpy(np.float64)])
            except Exception as e:   # noqa: BLE001 - the library refuses it; Terraflux must too
                out[key] = np.array([0.0])
                print("edge", method, label, "->", type(e).__name__, str(e)[:100])


# the dynamic Brownian bridges (group "move2utils_ud" of the library's cases): the sets are the library's own
# (tests/move_cases.py: ud_track_sets) and the fishers' M1, on a coarse grid so that the reference stays small. The
# settings are those of the operators (the library's names): a case is (model, set, prep, settings).
UD_SETS = ("ud_m1", "ud_m1_ll", "ud_utm", "ud_ll", "ud_err", "ud_burst")
UD_LIKE = {"xmin": -9000.0, "ymax": 8700.0, "res": 300.0, "ncol": 60, "nrow": 58}
UD_LIKE_G = {"xmin": -12000.0, "ymax": 11700.0, "res": 400.0, "ncol": 60, "nrow": 59}
UD_VAR_CASES = {
    "var_m1": ("dbbmm", "ud_m1", None, {"location_error": 25}),
    "var_m1_w21": ("dbbmm", "ud_m1", None, {"location_error": 15, "window_size": 21, "margin": 5}),
    "var_utm": ("dbbmm", "ud_utm", None, {"location_error": 10, "window_size": 21, "margin": 7}),
    "var_noerr": ("dbbmm", "ud_utm", None, {"location_error": None, "window_size": 21, "margin": 7}),
    "var_ll": ("dbbmm", "ud_ll", None, {"location_error": 5}),
    "var_m1_ll": ("dbbmm", "ud_m1_ll", None, {"location_error": 25}),
    "var_burst_err": ("dbbmm", "ud_burst", "burst", {"location_error": "le", "window_size": 21, "margin": 7,
                                                      "location_error_na": "approx"}),
    "var_err_median": ("dbbmm", "ud_err", None, {"location_error": "le", "location_error_na": "median"}),
    "var_err_mean": ("dbbmm", "ud_err", None, {"location_error": "le", "location_error_na": "mean"}),
    "var_err_zero": ("dbbmm", "ud_err", None, {"location_error": "le", "location_error_na": "zero"}),
    "var_err_approx": ("dbbmm", "ud_err", None, {"location_error": "le", "location_error_na": "approx"}),
    "var_argos": ("dbbmm", "ud_err", "argos", {"location_error": "auto"}),
    "var_eobs": ("dbbmm", "ud_err", None, {"location_error": "auto"}),
    "gvar_m1": ("dbgb", "ud_m1", None, {"location_error": 25}),
    "gvar_m1_w21": ("dbgb", "ud_m1", None, {"location_error": 15, "window_size": 21, "margin": 5}),
    "gvar_utm": ("dbgb", "ud_utm", None, {"location_error": 10, "window_size": 21, "margin": 7}),
    "gvar_noerr": ("dbgb", "ud_utm", None, {"location_error": None, "window_size": 21, "margin": 7}),
    "gvar_burst": ("dbgb", "ud_burst", "burst", {"location_error": "le", "window_size": 21, "margin": 7,
                                                  "location_error_na": "approx"}),
    "gvar_err_median": ("dbgb", "ud_err", None, {"location_error": "le", "location_error_na": "median"}),
    "gvar_argos": ("dbgb", "ud_err", "argos", {"location_error": "auto"}),
}
UD_CASES = {
    "ud_m1_dim": ("dbbmm", "ud_m1", None, {"location_error": 25, "dim_size": 50}),
    "ud_m1_enlarge_dim": ("dbbmm", "ud_m1", None, {"location_error": 25, "dim_size": 40, "ext_factor": 0}),
    "ud_m1_enlarge_res": ("dbbmm", "ud_m1", None, {"location_error": 25, "res": 400.0, "ext_factor": 0}),
    "ud_m1_step": ("dbbmm", "ud_m1", None, {"location_error": 25, "res": 300.0, "time_step": 3}),
    "ud_m1_like": ("dbbmm", "ud_m1", None, {"location_error": 25, "like": UD_LIKE}),
    "ud_utm": ("dbbmm", "ud_utm", None, {"location_error": 10, "dim_size": 50, "window_size": 21, "margin": 7}),
    "ud_burst": ("dbbmm", "ud_burst", "burst", {"location_error": 5, "window_size": 21, "margin": 7, "dim_size": 40}),
    "ud_argos": ("dbbmm", "ud_err", "argos", {"location_error": "auto", "dim_size": 50}),
    "ud_eobs": ("dbbmm", "ud_err", None, {"location_error": "auto", "dim_size": 50}),
    "ud_err_approx": ("dbbmm", "ud_err", None, {"location_error": "le", "location_error_na": "approx", "dim_size": 50}),
    "ud_ll": ("dbbmm", "ud_ll", None, {"location_error": 5, "dim_size": 50}),
    "ud_ll_input": ("dbbmm", "ud_ll", None, {"location_error": 5, "dim_size": 50, "crs": "input"}),
    "ud_m1_ll": ("dbbmm", "ud_m1_ll", None, {"location_error": 25, "dim_size": 50}),
    "gud_m1_dim": ("dbgb", "ud_m1", None, {"location_error": 25, "dim_size": 50}),
    "gud_m1_enlarge_dim": ("dbgb", "ud_m1", None, {"location_error": 25, "dim_size": 40, "ext_factor": 0}),
    "gud_m1_like": ("dbgb", "ud_m1", None, {"location_error": 25, "like": UD_LIKE_G}),
    "gud_m1_step": ("dbgb", "ud_m1", None, {"location_error": 25, "res": 400.0, "time_step": 3}),
    "gud_utm": ("dbgb", "ud_utm", None, {"location_error": 10, "dim_size": 50, "window_size": 21, "margin": 7}),
    "gud_burst": ("dbgb", "ud_burst", "burst", {"location_error": 5, "window_size": 21, "margin": 7, "dim_size": 40}),
    "gud_err_approx": ("dbgb", "ud_err", None, {"location_error": "le", "location_error_na": "approx",
                                                "dim_size": 50}),
}
UD_MASK = [*range(30, 41), 90, 150]
UD_BY_TRACK = {"u1": [5, 6, 7], "u2": [50, 51]}
UD_COLUMNS = ("le", "lc", "eobs_horizontal_accuracy_estimate", "burst")


def ud_frame(s: dict, prep=None) -> gpd.GeoDataFrame:
    """The tracks of a set of the dBBMM cases, as the library's tests make them."""
    t = pd.to_datetime(np.round(np.asarray(s["t"]) * 1e9).astype(np.int64), utc=True)
    cols = {"time": t, "track": s["track"]}
    for k in UD_COLUMNS:
        if k in s:
            cols[k] = s[k]
    df = gpd.GeoDataFrame(cols, geometry=gpd.points_from_xy(s["x"], s["y"]), crs=s["crs"])
    tr = M.as_tracks(df, "time", "track", sort=False)
    if prep == "burst":
        tr = tr.copy()
        tr["burst_"] = tr.pop("burst").astype(int)
    elif prep == "argos":
        tr = tr.drop(columns="eobs_horizontal_accuracy_estimate").copy()
        tr["argos_lc"] = np.array(["3", "2", "1", "0", "A", "B", "Z"])[tr["lc"].to_numpy().astype(int) - 1]
    return tr


def ud_run(kind: str, model: str, tr, p: dict):
    kw = dict(p)
    if "like" in kw:
        kw["like"] = hr_like(kw["like"], tr.crs.to_wkt())
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        return getattr(M, f"{model}_{kind}")(tr, **kw)


def ud_reference(out: dict, meta: dict, fishers_ok) -> None:
    """The dBBMM and the dBGB of the library on its sets and the fishers' M1: `in/ud/<set>/{x,y,t,...}` the
    locations, `ud/var/<case>/<column>` the variances' columns, `ud/<case>/ud` the distributions (the bands one after
    the other, rows from the top) with their grids and bands in the json (`ud`), the projected coordinates of the
    longitude / latitude sets (`ud/proj/<set>/...`), the masks, the volume and the outer probability, the window
    suggestions."""
    from pyproj import CRS  # noqa: F401
    from terraflux.move._clean import _seconds
    from terraflux.move._ud import _transform_xy

    sets = ud_track_sets()
    m1 = fishers_ok[fishers_ok["individual-local-identifier"].astype(str) == "M1"]
    aeqd = M.aeqd_crs(m1, "center")
    g = m1.to_crs(aeqd)
    t = _seconds(m1["timestamp"])
    track = np.array(["M1"] * len(m1))
    sets["ud_m1"] = {"x": g.geometry.x.to_numpy(), "y": g.geometry.y.to_numpy(), "t": t, "track": track,
                     "crs": aeqd.srs}
    sets["ud_m1_ll"] = {"x": m1.geometry.x.to_numpy(), "y": m1.geometry.y.to_numpy(), "t": t, "track": track,
                        "crs": "EPSG:4326"}
    meta["ud"] = {"sets": {}, "var": {}, "cases": {}, "masks": {}, "suggest": {}, "aeqd": {}}
    frames = {}
    for name in UD_SETS:
        s = sets[name]
        for k in ("x", "y", "t", *UD_COLUMNS):
            if k in s:
                out[f"in/ud/{name}/{k}"] = np.asarray(s[k], np.float64)
        meta["ud"]["sets"][name] = {"track": [str(v) for v in s["track"]], "crs": s["crs"],
                                    "columns": [k for k in UD_COLUMNS if k in s]}
        frames[name, None] = ud_frame(s)
    frames["ud_burst", "burst"] = ud_frame(sets["ud_burst"], "burst")
    frames["ud_err", "argos"] = ud_frame(sets["ud_err"], "argos")
    # the coordinates the library computes in, for the longitude / latitude sets
    for name in ("ud_ll", "ud_m1_ll"):
        tr = frames[name, None]
        x, y = tr.geometry.x.to_numpy(np.float64), tr.geometry.y.to_numpy(np.float64)
        for how in ("centroid", "center"):
            px, py = _transform_xy(x, y, tr.crs, M.aeqd_crs(tr, how))
            out[f"ud/proj/{name}/{how}_x"] = px
            out[f"ud/proj/{name}/{how}_y"] = py
            meta["ud"]["aeqd"][f"{name}/{how}"] = M.aeqd_crs(tr, how).srs

    cols = {"dbbmm": ("motion_variance", "in_windows", "interest", "location_error"),
            "dbgb": ("para_sd", "orth_sd", "n_estim", "seg_interest", "dbgb_location_error")}
    for key, (model, name, prep, p) in UD_VAR_CASES.items():
        v = ud_run("variance", model, frames[name, prep], p)
        for c in cols[model]:
            out[f"ud/var/{key}/{c}"] = v[c].to_numpy(np.float64)
        meta["ud"]["var"][key] = {"model": model, "set": name, "prep": prep, "params": p}

    def put_ud(key, ud, **extra):
        out[f"ud/{key}/ud"] = np.asarray(ud.values, np.float64).reshape(-1)
        tr_ = ud.rio.transform()
        meta["ud"]["cases"].setdefault(key, {}).update({
            "shape": [int(v) for v in ud.shape], "bands": [str(v) for v in ud["band"].values],
            "extent": [float(v) for v in ud.attrs["extent"]], "transform": [tr_.c, tr_.a, tr_.b, tr_.f, tr_.d, tr_.e],
            "time_step": [float(v) for v in ud.attrs["time_step"]], **extra})

    uds = {}
    for key, (model, name, prep, p) in UD_CASES.items():
        ud = ud_run("ud", model, frames[name, prep], p)
        uds[key] = ud
        put_ud(key, ud, model=model, set=name, prep=prep, params=p)

    # the same bridge from the variance's columns, masked, with another error given to the distribution
    utm = frames["ud_utm", None]
    u1 = utm[utm["track"] == "u1"]
    u2 = utm[utm["track"] == "u2"]
    for model in ("dbbmm", "dbgb"):
        pre = "" if model == "dbbmm" else "g"
        field = "interest" if model == "dbbmm" else "seg_interest"
        v = ud_run("variance", model, u1, {"location_error": 10, "window_size": 21, "margin": 7})
        out[f"ud/mask/{model}/before"] = v[field].to_numpy(np.float64)
        vm = M.mask_segments(v, UD_MASK)
        out[f"ud/mask/{model}/after"] = vm[field].to_numpy(np.float64)
        put_ud(f"{pre}ud_masked", ud_run("ud", model, vm, {"dim_size": 40}), model=model, kind="masked")
        put_ud(f"{pre}ud_columns", ud_run("ud", model, v, {"dim_size": 40}), model=model, kind="columns")
        put_ud(f"{pre}ud_override", ud_run("ud", model, v, {"dim_size": 40, "location_error": 12.0}), model=model,
               kind="override")
        v2 = ud_run("variance", model, u2, {"location_error": 10, "window_size": 21, "margin": 7})
        lag = np.diff(_seconds(u2["time"]))
        idx = np.flatnonzero(lag > 3 * 3600)
        out[f"ud/mask/{model}/gap_idx"] = idx.astype(np.float64)
        out[f"ud/mask/{model}/gap"] = M.mask_segments(v2, idx)[field].to_numpy(np.float64)
        out[f"ud/mask/{model}/gap_before"] = v2[field].to_numpy(np.float64)
        vall = ud_run("variance", model, utm, {"location_error": 10, "window_size": 21, "margin": 7})
        out[f"ud/mask/{model}/by_track"] = M.mask_segments(vall, UD_BY_TRACK, model=model)[field].to_numpy(np.float64)
    meta["ud"]["masks"] = {"segments": UD_MASK, "gap_hours": 3.0, "by_track": UD_BY_TRACK}

    # the volume and the outer probability
    m1_xy = np.vstack([np.c_[sets["ud_m1"]["x"][::7], sets["ud_m1"]["y"][::7]], [[1e7, 1e7]]])
    out["ud/volume/ud_m1_dim"] = np.asarray(M.ud_volume(uds["ud_m1_dim"]).values, np.float64).reshape(-1)
    out["ud/outer/ud_m1_dim"] = M.ud_outer_probability(m1_xy, uds["ud_m1_dim"])
    out["ud/outer/ud_m1_dim/x"] = m1_xy[:, 0]
    out["ud/outer/ud_m1_dim/y"] = m1_xy[:, 1]
    out["ud/volume/ud_utm"] = np.asarray(M.ud_volume(uds["ud_utm"]).values, np.float64).reshape(-1)
    own = np.asarray(sets["ud_utm"]["track"]) == "u2"
    u_xy = np.vstack([np.c_[np.asarray(sets["ud_utm"]["x"])[own][::5], np.asarray(sets["ud_utm"]["y"])[own][::5]],
                      [[1e7, 1e7]]])
    out["ud/outer/ud_utm/x"] = u_xy[:, 0]
    out["ud/outer/ud_utm/y"] = u_xy[:, 1]
    out["ud/outer/ud_utm"] = M.ud_outer_probability(u_xy, uds["ud_utm"], band="u2")
    assert list(uds["ud_utm"]["band"].values) == ["u1", "u2"]

    # the window of the sampling
    for key, tr_, hours in [("fishers_h1", fishers_ok, 1), ("fishers_h4", fishers_ok, 4), ("fishers_h12", fishers_ok, 12),
                            ("utm_h1", utm, 1), ("utm_h5", utm, 5)]:
        with warnings.catch_warnings():
            warnings.simplefilter("ignore")
            sg = M.suggest_dbbmm_window(tr_, hours)
        sg = sg if isinstance(sg, dict) else {"one": sg}
        rows = [[np.nan] * 6 if v is None else [v.window_size, v.margin, v.median_dt_secs, v.window_hours, v.n,
                                                v.n_windows] for v in sg.values()]
        out[f"ud/suggest/{key}"] = np.array(rows, np.float64).reshape(-1)
        meta["ud"]["suggest"][key] = hours


VAL_NX, VAL_NY, VAL_RES, VAL_XMIN, VAL_YMAX = 40, 30, 10.0, 1000.0, 5300.0
VAL_CRS = "EPSG:32723"
# the validation of a connectivity surface (move.validate): case -> (use, surface, arguments of validate). The use is a
# UD stack ("ud"), one band of it ("ud1"), the UD on another grid ("ud_b"), points ("pts", "pts_one", "pts_ll"), or
# lines ("lines"); the surface "current", "resistance" or "binary".
VAL_CASES = {
    "ud_current": ("ud", "current", {"quantile": 0.8}),
    "ud_default": ("ud", "current", {}),
    "ud_support1": ("ud", "current", {"support": 1, "quantile": 0.7}),
    "ud_support5": ("ud", "current", {"support": 0.5, "quantile": 0.95}),
    "ud_threshold": ("ud", "current", {"threshold": 0.3}),
    "ud_resistance": ("ud", "resistance", {"quantile": 0.8}),
    "ud_resistance_t": ("ud", "resistance", {"threshold": -0.2}),
    "ud_binary": ("ud", "binary", {"kind": "binary"}),
    "ud_one": ("ud1", "current", {"quantile": 0.8}),
    "ud_other_grid": ("ud_b", "current", {"quantile": 0.8}),
    "ud_nan": ("ud_nan", "current", {"quantile": 0.8}),
    "pts": ("pts", "current", {"quantile": 0.8}),
    "pts_default": ("pts", "current", {}),
    "pts_threshold": ("pts", "current", {"threshold": 0.3}),
    "pts_resistance": ("pts", "resistance", {"quantile": 0.8}),
    "pts_binary": ("pts", "binary", {"kind": "binary"}),
    "pts_one": ("pts_one", "current", {"quantile": 0.8}),
    "pts_ll": ("pts_ll", "current", {"quantile": 0.8}),
    "lines": ("lines", "current", {"quantile": 0.8}),
    "lines_spacing": ("lines", "current", {"quantile": 0.8, "spacing": 7.3}),
    "lines_resistance": ("lines", "resistance", {"quantile": 0.7}),
    "lines_binary": ("lines", "binary", {"kind": "binary"}),
}


def validate_reference(out: dict, meta: dict) -> None:
    """The library's `validate` on a synthetic surface, UDs, points and lines (no R: the library is the reference). New
    keys only: `in/validate/<name>` the inputs (rasters row by row from the north, NaN for none; coordinates), and
    `validate/<case>/{table,threshold,corridor}` (`table`: a row per track and pooled, the eight numbers after the
    track); the structure, the notes and the arguments are in the json (`validate`)."""
    rng = np.random.default_rng(20261121)
    x = VAL_XMIN + (np.arange(VAL_NX) + 0.5) * VAL_RES
    y = VAL_YMAX - (np.arange(VAL_NY) + 0.5) * VAL_RES
    gx, gy = np.meshgrid(x, y)

    def ra(a, xs=x, ys=y, crs=VAL_CRS):
        return xr.DataArray(a, coords={"y": ys, "x": xs}, dims=("y", "x")).rio.write_crs(crs)

    cur = np.exp(-((gx - 1200) ** 2) / (2 * 90.0 ** 2)) + 0.5 * np.exp(-((gy - 5100) ** 2) / (2 * 50.0 ** 2))
    cur = cur + 0.05 * rng.normal(size=cur.shape)
    cur[::7, ::5] = np.round(cur[::7, ::5], 1)           # ties
    cur[4:7, 10:14] = np.nan
    res = 10.0 - 8.0 * (cur - np.nanmin(cur)) / (np.nanmax(cur) - np.nanmin(cur))
    binary = np.where(np.isnan(cur), np.nan, (cur > np.nanquantile(cur, 0.85)).astype(float))
    surfaces = {"current": cur, "resistance": res, "binary": binary}
    for k, v in surfaces.items():
        out[f"in/validate/surface_{k}"] = v.ravel()
    names = ["a", "b", "c"]
    bands = []
    for cx, cy, sd in ((1150, 5050, 70.0), (1300, 4900, 90.0), (1220, 5150, 45.0)):
        u = np.exp(-((gx - cx) ** 2 + (gy - cy) ** 2) / (2 * sd ** 2))
        u[u < 1e-6] = 0.0
        bands.append(u / u.sum())
    ud = np.stack(bands)
    out["in/validate/ud"] = ud.ravel()
    ud_nan = ud.copy()
    ud_nan[0, :, 30:] = np.nan
    ud_nan[1, 20:, :] = np.nan
    out["in/validate/ud_nan"] = ud_nan.ravel()
    # a UD on a coarser grid, shifted: its centres fall inside the surface (mostly)
    nxb, nyb, rb = 26, 20, 15.0
    xb0, yb0 = 1013.0, 5290.0
    xb = xb0 + (np.arange(nxb) + 0.5) * rb
    yb = yb0 - (np.arange(nyb) + 0.5) * rb
    gxb, gyb = np.meshgrid(xb, yb)
    ub = np.exp(-((gxb - 1180) ** 2 + (gyb - 5000) ** 2) / (2 * 80.0 ** 2))
    ub = ub / ub.sum()
    out["in/validate/ud_b"] = ub.ravel()

    def stack(a, nm):
        return xr.DataArray(a, coords={"band": nm, "y": y, "x": x}, dims=("band", "y", "x")).rio.write_crs(VAL_CRS)

    # points: three tracks, a few outside the grid and on NaN cells
    n_pts = [60, 45, 30]
    px, py, pt = [], [], []
    for i, n in enumerate(n_pts):
        c = [(1150, 5050), (1300, 4900), (1220, 5150)][i]
        px.append(rng.normal(c[0], 60, n))
        py.append(rng.normal(c[1], 70, n))
        pt += [names[i]] * n
    px, py = np.concatenate(px), np.concatenate(py)
    px[3], py[3] = 5000.0, 5000.0                       # off the grid
    px[70], py[70] = 1135.3, 5265.7                     # on the NaN block
    out["in/validate/pts_x"], out["in/validate/pts_y"] = px, py
    from pyproj import Transformer
    lon, lat = Transformer.from_crs(VAL_CRS, "EPSG:4326", always_xy=True).transform(px, py)
    out["in/validate/pts_lon"], out["in/validate/pts_lat"] = np.asarray(lon), np.asarray(lat)
    # lines: tracks of two- and three-vertex polylines
    verts, counts, lt = [], [], []
    for i in range(18):
        k = 2 + (i % 2)
        x0, y0 = rng.uniform(1050, 1350), rng.uniform(4850, 5200)
        pts = [(x0, y0)]
        for _ in range(k - 1):
            pts.append((pts[-1][0] + rng.uniform(-70, 70), pts[-1][1] + rng.uniform(-70, 70)))
        verts += pts
        counts.append(k)
        lt.append(names[i % 3])
    vx, vy = np.array([p[0] for p in verts]), np.array([p[1] for p in verts])
    out["in/validate/line_x"], out["in/validate/line_y"] = vx, vy
    meta["validate"] = {"grid": {"nx": VAL_NX, "ny": VAL_NY, "xmin": VAL_XMIN, "ymax": VAL_YMAX, "res": VAL_RES,
                                 "crs": VAL_CRS},
                        "grid_b": {"nx": nxb, "ny": nyb, "xmin": xb0, "ymax": yb0, "res": rb},
                        "bands": names, "pts_tracks": pt, "line_counts": counts, "line_tracks": lt, "cases": {}}
    from shapely import LineString

    def use_of(kind):
        if kind == "ud":
            return stack(ud, names)
        if kind == "ud1":
            return ra(ud[0])
        if kind == "ud_nan":
            return stack(ud_nan, names)
        if kind == "ud_b":
            return ra(ub, xb, yb)
        if kind in ("pts", "pts_one"):
            g = gpd.GeoDataFrame({"track": pt}, geometry=[Point(a, b) for a, b in zip(px, py)], crs=VAL_CRS)
            return g if kind == "pts" else g[["geometry"]]
        if kind == "pts_ll":
            return gpd.GeoDataFrame({"track": pt}, geometry=[Point(a, b) for a, b in zip(lon, lat)], crs="EPSG:4326")
        lines, at = [], 0
        for c in counts:
            lines.append(LineString(verts[at:at + c]))
            at += c
        return gpd.GeoDataFrame({"track": lt}, geometry=lines, crs=VAL_CRS)

    for case, (kind, surf, args) in VAL_CASES.items():
        args = dict(args)
        with warnings.catch_warnings():
            warnings.simplefilter("ignore")
            v = M.validate(use_of(kind), ra(surfaces[surf]), **{"kind": surf, **args})
        t = v.table
        out[f"validate/{case}/table"] = t.drop(columns="track").to_numpy(np.float64)
        out[f"validate/{case}/threshold"] = np.array([v.threshold], np.float64)
        out[f"validate/{case}/corridor"] = v.corridor.to_numpy().ravel()
        meta["validate"]["cases"][case] = {"use": kind, "surface": surf, "args": args, "tracks": t["track"].tolist(),
                                           "note": v.note}

RELIEF_ELEVATION = LS / "src" / "terraflux" / "move" / "data" / "elevation.tif"


def relief_dem(z: np.ndarray, dx: float, dy: float, x0: float, ymax: float, crs: str = "EPSG:32633") -> xr.DataArray:
    """A (y, x) DEM of cells dx by dy with its top left corner at (x0, ymax)."""
    ny, nx = z.shape
    da = xr.DataArray(z, dims=("y", "x"), coords={"y": ymax - dy * (np.arange(ny) + 0.5),
                                                   "x": x0 + dx * (np.arange(nx) + 0.5)}, name="dem")
    return da.rio.write_crs(crs).rio.write_transform(Affine(dx, 0, x0, 0, -dy, ymax))


def relief_reference(out: dict, meta: dict) -> None:
    """The length of a step on the relief (surface_distance) on a plane (steps inside and outside the DEM, two
    spacings), a rough DEM with a hole of no data (zero-length, NaN and outside steps), cells that are not square, and
    the fishers' steps over the library's elevation.tif. New keys only: `in/relief/<case>/{x1,y1,x2,y2}` the steps,
    `in/relief/<case>/z` the DEM (rows from the top) and `relief/<case>/<variant>` the lengths; the DEMs and the
    variants are in the json (`relief`)."""
    rng = np.random.default_rng(2024)
    meta["relief"] = {"cases": {}}
    cases = {}
    # a plane: steps from beyond the extent (the outer half cell is clamped, further out is NaN)
    n = 60
    x = 10.0 * (np.arange(n) + 0.5)
    y = n * 10.0 - 10.0 * (np.arange(n) + 0.5)
    cases["plane"] = (0.37 * x[None, :] - 0.52 * y[:, None], 10.0, 10.0, 0.0, 600.0, rng.uniform(-30, 630, (4, 300)),
                      {"default": None, "s3": 3.0, "s40": 40.0})
    # a rough DEM with a hole, on cells of 7.5 by 12 m, with steps of zero length and a missing coordinate
    ny, nx = 50, 70
    z = 200 + 30 * np.sin(np.arange(nx) / 5)[None, :] + 25 * np.cos(np.arange(ny) / 4)[:, None] + rng.normal(0, 4, (ny, nx))
    z[20:24, 30:36] = np.nan
    z[0, 5] = np.nan
    p = rng.uniform(0, 1, (4, 400))
    p[0], p[2] = 1000 + p[0] * 525, 1000 + p[2] * 525
    p[1], p[3] = 500 + p[1] * 600, 500 + p[3] * 600
    p[2, 10], p[3, 10] = p[0, 10], p[1, 10]
    p[0, 11] = np.nan
    cases["rough"] = (z, 7.5, 12.0, 1000.0, 1100.0, p, {"default": None, "s1": 1.0, "s5": 5.0})
    for name, (z, dx, dy, x0, ymax, p, variants) in cases.items():
        dem = relief_dem(z, dx, dy, x0, ymax)
        steps = pd.DataFrame({"x1_": p[0], "y1_": p[1], "x2_": p[2], "y2_": p[3]})
        steps.attrs["terraflux.move"] = {"crs": "EPSG:32633", "lonlat": False}
        for c, k in zip(("x1", "y1", "x2", "y2"), p):
            out[f"in/relief/{name}/{c}"] = np.asarray(k, np.float64)
        out[f"in/relief/{name}/z"] = np.asarray(z, np.float64).ravel()
        for v, sp in variants.items():
            out[f"relief/{name}/{v}"] = M.surface_distance(steps, dem, spacing=sp)["sl_surface_"].to_numpy(np.float64)
        meta["relief"]["cases"][name] = {"nx": z.shape[1], "ny": z.shape[0],
                                         "dx": dx, "dy": dy, "xmin": x0, "ymax": ymax, "crs": "EPSG:32633",
                                         "variants": variants}
    # the fishers over the library's elevation, in the raster's own CRS
    import rioxarray as rx

    elev = rx.open_rasterio(RELIEF_ELEVATION).isel(band=0, drop=True)
    f = tf.load_tracks(LS / "tests" / "data" / "move" / "fishers.csv.gz", sort=False)
    st = M.steps(f[~f.geometry.is_empty].to_crs(elev.rio.crs))
    for c in ("x1", "y1", "x2", "y2"):
        out[f"in/relief/fishers/{c}"] = st[c + "_"].to_numpy(np.float64)
    variants = {"default": None, "s20": 20.0}
    for v, sp in variants.items():
        out[f"relief/fishers/{v}"] = M.surface_distance(st, elev, spacing=sp)["sl_surface_"].to_numpy(np.float64)
    meta["relief"]["cases"]["fishers"] = {"crs": elev.rio.crs.to_string(), "variants": variants}


def issf_put(out: dict, key: str, fit) -> list:
    """The numbers of a fit under `key/...` (the covariance row-major, the scalars in a fixed order) and the names of
    its coefficients."""
    names = [str(n) for n in fit.coef.index]
    s, ci = fit.summary(), fit.confint()
    out[f"{key}/coef"] = fit.coef.to_numpy(np.float64)
    out[f"{key}/vcov"] = fit.vcov.to_numpy(np.float64).ravel()
    out[f"{key}/means"] = fit.means.to_numpy(np.float64)
    out[f"{key}/scalars"] = np.array([fit.loglik[0], fit.loglik[1], fit.iter, fit.n, fit.nevent, fit.n_omitted, fit.wald,
                                      fit.score, fit.aic, fit.edf, float(fit.converged)], np.float64)
    for c in ("exp_coef", "se", "z", "p"):
        out[f"{key}/{c}"] = s[c].to_numpy(np.float64)
    out[f"{key}/lower"], out[f"{key}/upper"] = (ci.iloc[:, k].to_numpy(np.float64) for k in (0, 1))
    return names


def issf_lr(out: dict, key: str, fit, x1: dict, x2: dict, **kw) -> None:
    """The log-RSS of a fit under `key/{log_rss,lwr,upr}`."""
    r = M.log_rss(fit, pd.DataFrame(x1), pd.DataFrame(x2), **kw)
    for c in ("log_rss", "lwr", "upr"):
        if c in r:
            out[f"{key}/{c}"] = r[c].to_numpy(np.float64)


def issf_reference(out: dict, meta: dict) -> None:
    """Step selection functions (fit_issf, log_rss) on the library's synthetic sets (tests/move_issf_cases.py) and on the
    fishers' random steps over the library's elevation.tif. New keys only: `in/issf/<set>/<column>` the inputs,
    `issf/<case>/...` the fits (see issf_put), `issf/<case>/<lr>/...` the log-RSS and `issf/<case>/matrix` the model
    matrix (n x p, row-major); the formulas, names and arguments are in the json (`issf`). The factor column of the
    harness has its levels sorted (Terraflux has no categories)."""
    sets = issf_track_sets()
    meta["issf"] = {"sets": {}, "cases": {}}
    frames = {}
    for name, s in sets.items():
        cols = {"step_id_": s["x"], "case_": s["case_"], "elevation": s["elevation"], "sl_": s["sl_"], "ta_": s["ta_"],
                "landuse": s["landuse"]}
        for c, v in cols.items():
            out[f"in/issf/{name}/{c}"] = np.asarray(v, np.float64)
        meta["issf"]["sets"][name] = {"step_txt": [str(v) for v in s["track"]]}
        frames[name] = pd.DataFrame({**cols, "step_txt": s["track"]})

    def prep(name: str, extra: str | None) -> pd.DataFrame:
        d = frames[name].copy()
        if extra == "lu":
            d["lu"] = d["landuse"].astype(int).astype(str)
        if extra == "col":
            d["elev2"], d["k1"] = 2 * d["elevation"], 1.0
        return d

    def case(key, set_name, formula, x1=None, x2=None, extra=None, lrs=(), matrix=True):
        d = prep(set_name, extra)
        with warnings.catch_warnings():
            warnings.simplefilter("ignore")
            fit = M.fit_issf(d, formula)
            m = {"set": set_name, "formula": formula, "extra": extra, "names": issf_put(out, f"issf/{key}", fit),
                 "x1": x1, "x2": x2, "lr": []}
            if matrix:
                out[f"issf/{key}/matrix"] = fit.model_matrix(d).to_numpy(np.float64).ravel()
            for tag, kw in lrs:
                issf_lr(out, f"issf/{key}/{tag}", fit, x1, x2, **kw)
                m["lr"].append({"tag": tag, **kw})
        meta["issf"]["cases"][key] = m

    ses = (("none", {}), ("se", {"ci": "se"}), ("se90", {"ci": "se", "ci_level": 0.9}))
    case("a", "issf_a", F_A, LR_A["x1"], LR_A["x2"], lrs=ses)
    case("b", "issf_a", F_B)
    case("fac", "issf_a", F_FAC, LR_FAC["x1"], LR_FAC["x2"], "lu", lrs=ses[:2])
    case("ties_t", "issf_ties", F_TIES_TXT)
    case("ties_n", "issf_ties", F_TIES_NUM)
    case("col", "issf_a", F_COL, LR_COL["x1"], LR_COL["x2"], "col", lrs=ses[1:2])
    case("one", "issf_a", F_ONE, extra="col")
    boot_a = ("boot", {"ci": "boot", "seed": BOOT_A["seed"], "n_boot": BOOT_A["n_boot"], "ci_level": BOOT_A["ci_level"]})
    boot_b = ("boot", {"ci": "boot", "seed": BOOT_B["seed"], "n_boot": BOOT_B["n_boot"], "ci_level": BOOT_B["ci_level"]})
    case("boot_a", "issf_a", F_BOOT, LR_BOOT["x1"], LR_BOOT["x2"], lrs=(boot_a,), matrix=False)
    case("boot_b", "issf_a", F_BOOT_LOG, LR_BOOT1["x1"], LR_BOOT["x2"], lrs=(boot_b,), matrix=False)
    # the fishers of three animals over the library's elevation: their steps (as move.steps gives them) are the input,
    # the random steps and the covariate are the Terraflux's own in the test
    import rioxarray as rx

    elev = rx.open_rasterio(RELIEF_ELEVATION).isel(band=0, drop=True)
    f = tf.load_tracks(LS / "tests" / "data" / "move" / "fishers.csv.gz", sort=False)
    st = M.steps(f[~f.geometry.is_empty].to_crs(elev.rio.crs))
    left, bottom, right, top = elev.rio.bounds()
    trk = "individual-local-identifier"
    keep = st.x1_.between(left, right) & st.y1_.between(bottom, top) & st[trk].isin(["F1", "M1", "M3"])
    st = st[keep].reset_index(drop=True)
    for c in ("x1_", "y1_", "x2_", "y2_", "sl_", "direction_p", "ta_"):
        out[f"in/issf/fishers/{c}"] = st[c].to_numpy(np.float64)
    meta["issf"]["fishers"] = {"track": st[trk].tolist(), "crs": elev.rio.crs.to_string(), "n_control": 5, "seed": 11}
    rs = M.random_steps(st, n_control=5, seed=11)
    rs = M.extract_covariates(rs, {"elevation": elev})
    for c in ("x2_", "y2_", "sl_", "ta_", "case_", "step_id_", "elevation"):
        out[f"issf/fishers/in/{c}"] = rs[c].to_numpy(np.float64)
    rs["stratum_"] = rs.groupby([trk, "step_id_"], sort=False).ngroup() + 1
    f1 = "case_ ~ elevation + log(sl_) + cos(ta_) + strata(stratum_)"
    f2 = "case_ ~ elevation + log(sl_) + cos(ta_) + strata(step_id_)"
    q = rs["elevation"].dropna().quantile([0.1, 0.5, 0.9]).tolist()
    x1 = {"elevation": q, "sl_": [30.0, 120.0, 500.0], "ta_": [0.0, 1.0, -2.0]}
    x2 = {"elevation": [float(rs["elevation"].mean())], "sl_": [100.0], "ta_": [0.0]}
    meta["issf"]["cases"]["fishers"] = {"formula": f1, "x1": x1, "x2": x2, "lr": []}
    meta["issf"]["cases"]["fishers_id"] = {"formula": f2, "lr": []}
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        fit = M.fit_issf(rs, f1)
        meta["issf"]["cases"]["fishers"]["names"] = issf_put(out, "issf/fishers", fit)
        for tag, kw in (("none", {}), ("se", {"ci": "se"}),
                        ("boot", {"ci": "boot", "seed": 5, "n_boot": 30, "ci_level": 0.9})):
            issf_lr(out, f"issf/fishers/{tag}", fit, x1, x2, **kw)
            meta["issf"]["cases"]["fishers"]["lr"].append({"tag": tag, **kw})
        meta["issf"]["cases"]["fishers_id"]["names"] = issf_put(out, "issf/fishers_id", M.fit_issf(rs, f2))


SIM_DT = {"1h": 3600.0, "15min": 900.0, "2h": 7200.0, "30min": 1800.0}


def sim_reference(out: dict, meta: dict) -> None:
    """Simulation from a step selection model (make_issf_model, redistribution_kernel, simulate_path, resistance and
    path_density) on the library's cases (tests/move_sim_cases.py: A, C, D, E, F and the fishers' B), a numeric-time
    start with a kernel that stops, a fit of the fishers' random steps over the library's elevation, the resistance in
    its four transformations and two scales and the density of three paths. New keys only: `in/sim/...` the inputs
    (the library's R-made covariate maps in tests/data/reference/move/inputs_sim.npz), `sim/<case>/{x,y,t,dt,first}`
    the path (`t` and `dt` in seconds), `sim/res/...` and `sim/dens/...`; the models, starts and arguments are in the
    json (`sim`). A model's coefficients are a list of [term, value] pairs in the library's order, which the sum of
    the weights follows."""
    import cases_sim as cs  # noqa: F401  (the library's tests/move_sim_cases.py, imported below as `cases`)
    raise NotImplementedError


SIM_DT = {"1h": 3600.0, "15min": 900.0, "2h": 7200.0, "30min": 1800.0}
SIM_EPOCH = pd.Timestamp("1970-01-01", tz="UTC")
# a model with habitat terms of two covariates, terms of movement and one at the start of the step (resistance)
SIM_RES = {"cov_end": 1.5, "wet": -0.4, "cov_end:wet": 0.2, "I(wet^2)": 0.1, "log(wet + 3)": 0.3,
           "exp(cov_end)": -0.05, "sl_": -0.002, "cov_start": 0.1, "cov_end:log_sl_": 0.05}


def sim_reference(out: dict, meta: dict) -> None:
    """Simulation from a step selection model (make_issf_model, redistribution_kernel, simulate_path, resistance and
    path_density) on the library's cases (tests/move_sim_cases.py: A, C, D, E, F and the fishers' B), a numeric-time
    start whose kernel stops, a fit of the fishers' random steps over the library's elevation, the resistance in its
    four transformations and two scales and the density of three paths. New keys only: `in/sim/...` the inputs (the
    library's R-made covariate maps are in its tests/data/reference/move/inputs_sim.npz), `sim/<case>/{x,y,t,dt,first,
    stopped}` the path (`t` and `dt` in seconds, or the numbers they are), `sim/res/<transform>_<scale>` and
    `sim/dens/...`; the models, starts and arguments are in the json (`sim`). A model's coefficients are a list of
    [term, value] pairs in the library's order, which the sum of the weights follows."""
    import rioxarray as rx

    data = LS / "tests" / "data" / "reference" / "move"
    ins = dict(np.load(data / "inputs_sim.npz"))
    ref = dict(np.load(data / "amt_sim.npz"))
    import move_sim_cases as cs

    meta["sim"] = {"cases": {}}
    xs = np.arange(5.0, 1000.0, 10.0)

    def raster_a(values, name="cov"):
        return xr.DataArray(np.asarray(values, float).reshape(100, 100), dims=("y", "x"),
                            coords={"y": xs[::-1], "x": xs}, name=name).rio.write_crs("EPSG:32633")

    map_a = np.asarray(ins["sim_a/in_map"], float)
    hole = map_a.copy()
    hole[150:160] = np.nan
    hole[4000:4100] = np.nan
    wet = map_a.reshape(100, 100)[::-1, ::-1].ravel()
    out["in/sim/map_a"], out["in/sim/map_hole"], out["in/sim/map_wet"] = map_a, hole, wet
    ext = ins["sim_b/in_pd_ext"]
    nr, nc = (int(v) for v in ins["sim_b/in_pd_dim"])
    out["in/sim/pd_vals"], out["in/sim/pd_ext"] = np.asarray(ins["sim_b/in_pd_vals"], float), np.asarray(ext, float)
    meta["sim"]["pd_dim"] = [nr, nc]
    pd_x = ext[0] + (ext[1] - ext[0]) / nc * (np.arange(nc) + 0.5)
    pd_y = ext[3] - (ext[3] - ext[2]) / nr * (np.arange(nr) + 0.5)
    popden = xr.DataArray(out["in/sim/pd_vals"].reshape(nr, nc), dims=("y", "x"), coords={"y": pd_y, "x": pd_x},
                          name="popden").rio.write_crs("EPSG:5070")

    def model_of(c):
        return M.make_issf_model(c["coefs"], M.make_distribution(c["sl"][0], **c["sl"][1]),
                                 M.make_distribution(c["ta"][0], **c["ta"][1]))

    def start_of(c):
        return M.make_start(c["start"], c["ta0"], time=c["time"], dt=c["dt"])

    def seconds(c):
        return SIM_DT[c] if isinstance(c, str) else float(c)

    def put(key, k, p):
        t = p["t_"]
        out[f"sim/{key}/x"], out[f"sim/{key}/y"] = p["x_"].to_numpy(np.float64), p["y_"].to_numpy(np.float64)
        out[f"sim/{key}/t"] = ((t - SIM_EPOCH).dt.total_seconds() if pd.api.types.is_datetime64_any_dtype(t)
                               else t).to_numpy(np.float64)
        d = p["dt"]
        out[f"sim/{key}/dt"] = (d.dt.total_seconds() if pd.api.types.is_timedelta64_dtype(d) else d).to_numpy(np.float64)
        out[f"sim/{key}/first"] = (np.array([k.sample.x_[0], k.sample.y_[0]]) if k.sample is not None
                                   else np.empty(0))
        out[f"sim/{key}/stopped"] = np.array([float(k.stopped is not None)])

    def case(key, c, cov, layer, model=None):
        model = model if model is not None else model_of(c)
        with warnings.catch_warnings():
            warnings.simplefilter("ignore")
            k = M.redistribution_kernel(model, start_of(c), cov, n_control=c["n_control"], tolerance_outside=c["tol"],
                                        seed=c["seed"])
            p = M.simulate_path(k, c["n_steps"])
        put(key, k, p)
        meta["sim"]["cases"][key] = {
            "layer": layer, "coefs": [[a, float(b)] for a, b in c["coefs"].items()] if "coefs" in c else None,
            "sl": [c["sl"][0], c["sl"][1]], "ta": [c["ta"][0], c["ta"][1]], "start": [float(v) for v in c["start"]],
            "ta0": c["ta0"], "time": c["time"], "dt": seconds(c["dt"]), "n_control": c["n_control"], "tol": c["tol"],
            "n_steps": c["n_steps"], "seed": c["seed"]}
        return k, p

    for key, c in (("a", cs.A), ("c", cs.C), ("d", cs.D), ("e", cs.E), ("f", cs.F)):
        case(key, c, raster_a(map_a), "cov")
    b = cs.B
    c_b = {"coefs": dict(zip(["popden_end", "sl_", "log_sl_", "cos_ta_"], (float(v) for v in ref["sim_b/coef"]))),
           "sl": ("gamma", {"shape": float(ref["sim_b/sl_shape"][0]), "scale": float(ref["sim_b/sl_scale"][0])}),
           "ta": ("vonmises", {"kappa": float(ref["sim_b/ta_kappa"][0])}),
           "start": tuple(float(v) for v in ins["sim_b/in_start"]), "ta0": b["ta0"], "time": b["time"], "dt": b["dt"],
           "n_control": b["n_control"], "tol": b["tol"], "n_steps": b["n_steps"], "seed": b["seed"]}
    case("b", c_b, popden, "popden")
    # a kernel that stops (the start at the edge, a tolerance of 1 %), then the path from where it left the generator,
    # with a start in numbers (time 0, a step of 1)
    c_g = {**cs.F, "start": (20.0, 500.0), "ta0": 3.0, "time": 0, "dt": 1, "n_control": 300, "tol": 0.01, "seed": 1,
           "n_steps": 5}
    k, _ = case("g", c_g, raster_a(map_a), "cov")
    assert k.stopped is not None
    # three paths of case A (the seeds seed, seed + 1, seed + 2) and their density over a map with a hole
    paths = []
    cov_a = raster_a(map_a)
    for i in range(3):
        c = {**cs.A, "seed": cs.A["seed"] + i}
        _, p = case(f"a{i + 1}", c, cov_a, "cov")
        paths.append(p.assign(track=f"path_{i + 1}"))
    allp = pd.concat(paths, ignore_index=True)
    like = raster_a(hole)
    out["sim/dens/plain"] = np.asarray(M.path_density(allp, like).values, np.float64).ravel()
    out["sim/dens/start"] = np.asarray(M.path_density(allp, like, include_start=True).values, np.float64).ravel()
    out["sim/dens/by"] = np.asarray(M.path_density(allp, like, by="track").values, np.float64).ravel()
    # the resistance of two covariates (one with a hole), every transformation and scale
    cov2 = {"cov": raster_a(hole), "wet": raster_a(wet, "wet")}
    model = M.make_issf_model(SIM_RES, M.make_distribution("gamma", shape=2, scale=50),
                              M.make_distribution("vonmises", kappa=2))
    meta["sim"]["res"] = {"coefs": [[a, float(v)] for a, v in SIM_RES.items()], "variants": []}
    for transform in ("negexp", "linear", "inverse", "log"):
        for scale in ("minmax", "max"):
            for kw in ({}, {"c": 3.0, "r_max": 20.0}):
                with warnings.catch_warnings(record=True) as w:
                    warnings.simplefilter("always")
                    r = M.resistance(model, cov2, transform=transform, scale=scale, **kw)
                tag = f"{transform}_{scale}" + ("_c3" if kw else "")
                out[f"sim/res/{tag}"] = np.asarray(r.values, np.float64).ravel()
                meta["sim"]["res"]["variants"].append({"tag": tag, "transform": transform, "scale": scale, **kw})
                meta["sim"]["res"]["warning"] = str(w[0].message)
    # a fit of the fishers' random steps (as issf_reference makes them) over the library's elevation, simulated
    elev = rx.open_rasterio(RELIEF_ELEVATION).isel(band=0, drop=True)
    f = tf.load_tracks(LS / "tests" / "data" / "move" / "fishers.csv.gz", sort=False)
    st = M.steps(f[~f.geometry.is_empty].to_crs(elev.rio.crs))
    left, bottom, right, top = elev.rio.bounds()
    trk = "individual-local-identifier"
    keep = st.x1_.between(left, right) & st.y1_.between(bottom, top) & st[trk].isin(["F1", "M1", "M3"])
    st = st[keep].reset_index(drop=True)
    rs = M.extract_covariates(M.random_steps(st, n_control=5, seed=11), {"elevation": elev})
    rs["stratum_"] = rs.groupby([trk, "step_id_"], sort=False).ngroup() + 1
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        fit = M.fit_issf(rs, "case_ ~ elevation + log(sl_) + cos(ta_) + strata(stratum_)")
    # (three tracks: the fit keeps no distributions, as the library says; they are given, fitted to all the steps)
    sl_d, ta_d = M.fit_distr(st["sl_"].dropna(), "gamma"), M.fit_distr(st["ta_"].dropna(), "vonmises")
    c_f = {"sl": (sl_d.name, {k: float(v) for k, v in sl_d.params.items()}),
           "ta": (ta_d.name, {k: float(v) for k, v in ta_d.params.items()}),
           "start": (float(st.x1_[0]), float(st.y1_[0])), "ta0": 0.3, "time": "2015-06-01 06:00:00", "dt": "1h",
           "n_control": 2000, "tol": 1.0, "n_steps": 12, "seed": 17}
    assert fit.sl_distr is None
    fit.sl_distr, fit.ta_distr = sl_d, ta_d
    case("fishers", c_f, {"elevation": elev}, "elevation", model=fit)


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
    corridor_reference(out, meta, tr, fishers_src)
    hr_reference(out, meta)
    ud_reference(out, meta, fishers_ok)
    validate_reference(out, meta)
    relief_reference(out, meta)
    issf_reference(out, meta)

    np.savez_compressed(REF / "move.npz", **out)
    (REF / "move.json").write_text(json.dumps(meta, indent=1) + "\n", encoding="utf-8")
    print(REF / "move.npz", len(out))


def only(*steps) -> None:
    """Adds the keys of the given references to the files already written (the rest unchanged)."""
    with np.load(REF / "move.npz") as z:
        out = {k: z[k] for k in z.files}
    meta = json.loads((REF / "move.json").read_text(encoding="utf-8"))
    for step in steps:
        step(out, meta)
    np.savez_compressed(REF / "move.npz", **out)
    (REF / "move.json").write_text(json.dumps(meta, indent=1) + "\n", encoding="utf-8")
    print(REF / "move.npz", len(out))


if __name__ == "__main__":
    if sys.argv[1:] == ["--only-relief"]:
        only(relief_reference)
    elif sys.argv[1:] == ["--only-issf"]:
        only(issf_reference)
    elif sys.argv[1:] == ["--only-sim"]:
        only(sim_reference)
    else:
        main()
