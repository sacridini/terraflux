"""Figures of "Movement on the relief" (website/content/tutorials/animal-movement/movement-on-the-relief.md).

The part's models are run by the terraflux binary: the Copernicus DEM downloaded and put on UTM 32N, its slope, its
ruggedness and the energy of walking over it, the four ibex's steps with their random steps, the relief at their ends and
along them, the length on the relief, the integrated step selection function (move.fit_issf), its log-RSS, paths
simulated from it, their density, the resistance and Omniscape's current. The same computation is made by the
terraflux library (tf.move, tf.cost) and the numbers are compared; what the page tells about them is printed here.

    PYTHONPATH=extern/terraflux-py/src python website/figures/animal-movement/movement-on-the-relief.py
"""

import json
import os
import shutil
import time

import matplotlib.pyplot as plt
import numpy as np
import pandas as pd
import terraflux as tf
from matplotlib.collections import LineCollection

import movefig as mf
from movefig import figs

SLUG = "movement-on-the-relief"
OUT = figs.img_dir(f"{mf.CAT}/{SLUG}")
DATA = figs.data_dir(f"{mf.CAT}/{SLUG}")
ID = mf.ID
NT = min(8, int(os.environ.get("HEAVY_CORES", "4")))
TRACKS = DATA / "bargy_ibex_2013.csv"
EXT = [6.38, 45.94, 6.52, 46.08]  # west, south, east, north: the Bargy massif
UTM = "EPSG:32632"
MASS = 80  # kg: the middle of the 60 to 100 kg of an adult ibex (the sex of the four animals is not in the file)
RESULTS = {}

MOVE = "log(sl_) + cos(ta_) + log(sl_):slope"
FORMULAS = {
    # the relief at the end of the step
    "end": f"case_ ~ energy + slope + tri + {MOVE}",
    # and along it: the climb and the net height gained or lost
    "full": f"case_ ~ energy + slope + tri + dem_along_ascent + dem_along_delta + {MOVE}",
    # what a simulation can read: the end of the step, and the start (the height changed is |height_end - height_start|)
    "sim": ("case_ ~ energy + I(energy^2) + slope + I(height_end/1000) + I((height_end/1000)^2) + "
            f"abs(height_end - height_start) + {MOVE}"),
}
PATHS = dict(n_steps=300, n_paths=10, n_control=5000, seed=11)

BASE = [
    {"id": "read", "op": "move.read_tracks", "params": {"path": "bargy_ibex_2013.csv"}},
    {"id": "utm", "op": "vector.reproject", "inputs": {"vector": "read.tracks"}, "params": {"crs": UTM}},
    {"id": "resample", "op": "move.resample", "inputs": {"tracks": "utm.vector"},
     "params": {"rate": "1h", "tolerance": "5min"}},
    {"id": "bursts", "op": "move.filter_min_n_burst", "inputs": {"tracks": "resample.tracks"}, "params": {"min_n": 3}},
    {"id": "steps", "op": "move.steps", "inputs": {"tracks": "bursts.tracks"}},
    {"id": "sl_fit", "op": "move.fit_distribution", "inputs": {"values": "steps.steps"}, "params": {"name": "gamma"}},
    {"id": "ta_fit", "op": "move.fit_distribution", "inputs": {"values": "steps.steps"}, "params": {"name": "vonmises"}},
]
RELIEF = [
    {"id": "dem_ll", "op": "data.download",
     "params": {"source": "copernicus_dem", "dem_resolution": 30, "extent": EXT, "cellsize": 0.0003}},
    {"id": "dem", "op": "raster.reproject", "inputs": {"raster": "dem_ll.raster"},
     "params": {"crs": UTM, "resolution": 30, "resampling": "bilinear", "options": "-dstnodata nan"}},
    {"id": "slope", "op": "surface.terrain", "inputs": {"map": "dem.raster"}, "params": {"v": "slope", "unit": "degrees"}},
    {"id": "tri", "op": "surface.tri", "inputs": {"map": "dem.raster"}, "params": {"s": 3}},
    {"id": "energy", "op": "cost.energy_landscape", "inputs": {"dem": "dem.raster"},
     "params": {"mass": MASS, "unit": "kcal", "neigh": 8, "model": "animal"}},
]
COVARIATES = [
    {"id": "random", "op": "move.random_steps",
     "inputs": {"steps": "steps.steps", "sl_distr": "sl_fit.distribution", "ta_distr": "ta_fit.distribution"},
     "params": {"n_control": 10, "seed": 1}},
    {"id": "ends", "op": "move.extract_covariates",
     "inputs": {"tracks": "random.steps", "energy": "energy.energy", "slope": "slope.map", "tri": "tri.map"},
     "params": {"where": "end"}},
    {"id": "climb", "op": "move.extract_covariates", "inputs": {"tracks": "ends.tracks", "dem": "dem.raster"},
     "params": {"where": "along", "stats": ["ascent", "delta"]}},
    {"id": "both", "op": "move.extract_covariates", "inputs": {"tracks": "climb.tracks", "height": "dem.raster"},
     "params": {"where": "both"}},
    {"id": "distance", "op": "move.surface_distance", "inputs": {"steps": "both.tracks", "dem": "dem.raster"}},
]


def fit_node(name, formula):
    return {"id": name, "op": "move.fit_issf",
            "inputs": {"steps": "distance.steps", "sl_distr": "sl_fit.distribution", "ta_distr": "ta_fit.distribution"},
            "params": {"formula": formula}}


def column(v):
    return [float(round(x, 2)) for x in v]


def phase1(w):
    """The relief, the steps with their covariates and the three fits."""
    nodes = BASE + RELIEF + COVARIATES + [fit_node(f"ssf_{k}", f) for k, f in FORMULAS.items()] + [
        {"id": f"w_{k}", "op": "raster.write", "inputs": {"value": v}, "params": {"path": f"{k}.tif"}}
        for k, v in [("dem", "dem.raster"), ("slope", "slope.map"), ("tri", "tri.map"), ("energy", "energy.energy")]
    ]
    outs = {"steps": "steps.steps", "data": "distance.steps", "sl": "sl_fit.table", "ta": "ta_fit.table"}
    outs |= {f"fit_{k}": f"ssf_{k}.table" for k in FORMULAS} | {f"info_{k}": f"ssf_{k}.fit" for k in FORMULAS}
    return nodes, outs


def phase2(medians, energy_by_slope, slopes):
    """The log-RSS, the simulated paths, the resistance and the current, from the fits of phase 1."""
    sl0, en0, tri0 = medians["sl"], medians["energy"], medians["tri"]
    ref_slope = medians["slope"]
    ref_energy = float(np.interp(ref_slope, slopes, energy_by_slope))
    h = list(range(0, 101, 10))
    nodes = [
        {"id": "rss_slope", "op": "move.log_rss", "inputs": {"model": "ssf_sim.model"},
         "params": {"x1": {"slope": column(slopes), "energy": column(energy_by_slope), "height_end": [1915.0] * len(slopes),
                           "height_start": [1900.0] * len(slopes), "sl_": [sl0] * len(slopes), "ta_": [0.0] * len(slopes)},
                    "x2": {"slope": round(ref_slope, 1), "energy": round(ref_energy, 2), "height_end": 1915.0,
                           "height_start": 1900.0, "sl_": sl0, "ta_": 0.0},
                    "ci": "se", "level": 0.95}},
        {"id": "rss_climb", "op": "move.log_rss", "inputs": {"model": "ssf_full.model"},
         "params": {"x1": {"dem_along_delta": h, "dem_along_ascent": h, "energy": [round(en0, 2)] * len(h),
                           "slope": [round(ref_slope, 1)] * len(h), "tri": [round(tri0, 1)] * len(h),
                           "sl_": [sl0] * len(h), "ta_": [0.0] * len(h)},
                    "x2": {"dem_along_delta": 0, "dem_along_ascent": 0, "energy": round(en0, 2),
                           "slope": round(ref_slope, 1), "tri": round(tri0, 1), "sl_": sl0, "ta_": 0.0},
                    "ci": "se", "level": 0.95}},
        {"id": "rss_undulating", "op": "move.log_rss", "inputs": {"model": "ssf_full.model"},
         "params": {"x1": {"dem_along_delta": [0] * len(h), "dem_along_ascent": h, "energy": [round(en0, 2)] * len(h),
                           "slope": [round(ref_slope, 1)] * len(h), "tri": [round(tri0, 1)] * len(h),
                           "sl_": [sl0] * len(h), "ta_": [0.0] * len(h)},
                    "x2": {"dem_along_delta": 0, "dem_along_ascent": 0, "energy": round(en0, 2),
                           "slope": round(ref_slope, 1), "tri": round(tri0, 1), "sl_": sl0, "ta_": 0.0},
                    "ci": "se", "level": 0.95}},
        # the movement alone, to see what the relief adds to the paths
        {"id": "plain", "op": "move.fit_issf",
         "inputs": {"steps": "distance.steps", "sl_distr": "sl_fit.distribution", "ta_distr": "ta_fit.distribution"},
         "params": {"formula": "case_ ~ log(sl_) + cos(ta_)"}},
        {"id": "walk", "op": "move.simulate_path",
         "inputs": {"model": "ssf_sim.model", "start": "bursts.tracks", "energy": "energy.energy", "slope": "slope.map",
                    "height": "dem.raster"}, "params": dict(PATHS)},
        {"id": "walk_plain", "op": "move.simulate_path",
         "inputs": {"model": "plain.model", "start": "bursts.tracks", "energy": "energy.energy"}, "params": dict(PATHS)},
        {"id": "use", "op": "move.path_density", "inputs": {"paths": "walk.paths", "like": "dem.raster"}},
        {"id": "use_plain", "op": "move.path_density", "inputs": {"paths": "walk_plain.paths", "like": "dem.raster"}},
        {"id": "resistance", "op": "move.resistance", "inputs": {"model": "ssf_sim.model", "energy": "energy.energy",
                                                                 "slope": "slope.map", "height": "dem.raster"},
         "params": {"transform": "negexp", "c": 8, "r_max": 100}},
        {"id": "current", "op": "circuit.omniscape", "inputs": {"resistance": "resistance.resistance"},
         "params": {"radius": 40, "block_size": 11}},
        {"id": "v_res", "op": "move.validate", "inputs": {"use": "bursts.tracks", "surface": "resistance.resistance"},
         "params": {"kind": "resistance", "quantile": 0.9}},
        {"id": "v_cur", "op": "move.validate", "inputs": {"use": "bursts.tracks", "surface": "current.current"},
         "params": {"kind": "current", "quantile": 0.9}},
        {"id": "w_resistance", "op": "raster.write", "inputs": {"value": "resistance.resistance"},
         "params": {"path": "resistance.tif"}},
        {"id": "w_current", "op": "raster.write", "inputs": {"value": "current.current"}, "params": {"path": "current.tif"}},
        {"id": "w_use", "op": "raster.write", "inputs": {"value": "use.density"}, "params": {"path": "use.tif"}},
        {"id": "w_use_plain", "op": "raster.write", "inputs": {"value": "use_plain.density"},
         "params": {"path": "use_plain.tif"}},
    ]
    outs = {"rss_slope": "rss_slope.table", "rss_climb": "rss_climb.table", "rss_undulating": "rss_undulating.table",
            "sim": "walk.paths", "sim_plain": "walk_plain.paths", "use": "use.density", "use_plain": "use_plain.density",
            "resistance": "resistance.resistance", "current": "current.current", "v_res": "v_res.table",
            "v_cur": "v_cur.table"}
    return nodes, outs




# ---------------------------------------------------------------------------------------------- helpers
def cell_values(da, x, y):
    """The value of the cell holding each point (NaN outside)."""
    from rasterio.transform import rowcol

    a = np.asarray(da.values, float)
    x, y = np.asarray(x, float), np.asarray(y, float)
    r, c = rowcol(da.rio.transform(), x, y)
    r, c = np.asarray(r), np.asarray(c)
    ok = (r >= 0) & (r < a.shape[0]) & (c >= 0) & (c < a.shape[1])
    out = np.full(len(x), np.nan)
    out[ok] = a[r[ok], c[ok]]
    return out


def extent_of(da):
    rs = da.rio.resolution()
    return [float(da.x.min()) - rs[0] / 2, float(da.x.max()) + rs[0] / 2, float(da.y.min()) - abs(rs[1]) / 2,
            float(da.y.max()) + abs(rs[1]) / 2]


def hillshade(da, azdeg=315, altdeg=40):
    from matplotlib.colors import LightSource

    z = np.asarray(da.values, float)
    z = np.where(np.isfinite(z), z, np.nanmin(z))
    return LightSource(azdeg=azdeg, altdeg=altdeg).hillshade(z, vert_exag=1.4, dx=30, dy=30)


def save(fig, png):
    """The figure as it is (maps with continuous colours band when quantized to a palette, as mf.save does)."""
    fig.savefig(png, dpi=120)
    plt.close(fig)


def km_axes(ax, x0, y0, grid=False, step=2000):
    xl, yl = ax.get_xlim(), ax.get_ylim()
    ax.set_xticks(x0 + np.arange(0, xl[1] - x0 + 1, step)[np.arange(0, xl[1] - x0 + 1, step) >= xl[0] - x0])
    ax.set_yticks(y0 + np.arange(0, yl[1] - y0 + 1, step)[np.arange(0, yl[1] - y0 + 1, step) >= yl[0] - y0])
    ax.set_aspect(1)
    ax.grid(grid)
    ax.set_xlabel("km east")
    ax.set_ylabel("km north")
    ax.tick_params(labelsize=8.5)
    ax.xaxis.set_major_formatter(lambda v, _: f"{(v - x0) / 1000:g}")
    ax.yaxis.set_major_formatter(lambda v, _: f"{(v - y0) / 1000:g}")


def relief_base(ax, dem, crop, shade=True, alpha=1.0):
    """The DEM as a hillshade tinted by height, in a map of UTM metres."""
    ext = extent_of(dem)
    z = np.asarray(dem.values, float)
    ax.imshow(z, extent=ext, cmap="YlGnBu_r", vmin=np.nanpercentile(z, 1), vmax=np.nanpercentile(z, 99.5),
              alpha=0.55 * alpha, interpolation="nearest")
    if shade:
        ax.imshow(hillshade(dem), extent=ext, cmap="gray", vmin=0.15, vmax=1.0, alpha=0.5 * alpha,
                  interpolation="nearest")
    ax.set_xlim(crop[0], crop[1])
    ax.set_ylim(crop[2], crop[3])


def coefficient_rows(tab, labels):
    rows = []
    for r in tab.itertuples():
        rows.append({"term": labels.get(r.term, r.term), "coefficient": round(r.coef, 4), "se": round(r.se, 4),
                     "lower": round(r.lower, 4), "upper": round(r.upper, 4), "exp(coef)": round(r.exp_coef, 3),
                     "z": round(r.z, 2), "p": float(f"{r.p:.2g}")})
    return rows


LABELS = {"energy": "energy at the end (per kcal)", "slope": "slope at the end (per degree)",
          "tri": "TRI at the end (per m)", "dem_along_ascent": "ascent along the step (per m)",
          "dem_along_delta": "net height change along the step (per m)", "log(sl_)": "log(step length)",
          "cos(ta_)": "cos(turn angle)", "slope:log(sl_)": "log(step length) x slope",
          "abs(height_end - height_start)": "net height change, end less start (per m)",
          "I(energy^2)": "energy squared (per kcal squared)", "I(height_end/1000)": "height at the end (per km)",
          "I((height_end/1000)^2)": "height at the end, squared (per km squared)"}


# ---------------------------------------------------------------------------------------------- the library's side
def lib_data(rnd, dem, slope, tri, energy):
    """The random steps with the covariates of the model (the library)."""
    cov = tf.move.extract_covariates(rnd, {"energy": energy, "slope": slope, "tri": tri})
    cov = tf.move.along_summary(cov, {"dem": dem}, stats=("ascent", "delta"))
    cov = tf.move.extract_covariates(cov, {"height": dem}, where="both")
    cov = tf.move.surface_distance(cov, dem)
    cov["stratum"] = cov[ID].astype(str) + "_" + cov["step_id_"].astype(int).astype(str)
    return cov


def library_side(w, d, fits, paths):
    """The same computation with the terraflux library, from the same file; compares it to the operators'."""
    out = {}
    tr = tf.load_tracks(TRACKS).to_crs(UTM)
    bursts = tf.move.filter_min_n_burst(tf.move.resample(tr, "1h", "5min"), 3)
    st = tf.move.steps(bursts)
    sl = tf.move.fit_distr(st["sl_"], "gamma")
    ta = tf.move.fit_distr(st["ta_"], "vonmises")
    rnd = tf.move.random_steps(st, n_control=10, sl_distr=sl, ta_distr=ta, seed=1)
    dem = tf.load_raster(w / "dem.tif")
    slope, tri = tf.surface.terrain(dem, "slope"), tf.surface.tri(dem, 3)
    energy = tf.cost.energy_landscape(dem, MASS, unit="kcal")
    cov = lib_data(rnd, dem, slope, tri, energy)
    out["distr"] = (sl, ta)
    out["data"], out["steps"], out["dem"], out["slope"], out["energy"] = cov, st, dem, slope, energy
    print("library: steps", len(st), "rows", len(cov), "operators:", len(d))
    same = {c: float(np.nanmax(np.abs(cov[c].to_numpy(float) - d[c].to_numpy(float))))
            for c in ["sl_", "ta_", "energy", "slope", "tri", "dem_along_ascent", "dem_along_delta", "height_start",
                      "height_end", "sl_surface_"]}
    print("library vs operators, largest difference of each column:", same)
    out["parity_columns"] = max(same.values())
    models = {}
    for k, f in FORMULAS.items():
        m = tf.move.fit_issf(cov, f + " + strata(stratum)")
        models[k] = m
        tab = fits[k].set_index("term")
        dif = float(np.max(np.abs(m.coef.to_numpy() - tab.coef.to_numpy())))
        dse = float(np.max(np.abs(np.sqrt(np.diag(m.vcov.to_numpy())) - tab.se.to_numpy())))
        print(f"library vs operator, fit {k}: coefficients {dif:.2e}, standard errors {dse:.2e}")
        out[f"fit_{k}"] = max(dif, dse)
    out["models"] = models
    # one path, compared to the operators' first path of the first track
    m = models["sim"]
    first = bursts[bursts[ID] == bursts[ID].iloc[0]].reset_index(drop=True)
    p0, p1 = first.geometry.iloc[0], first.geometry.iloc[1]
    ta0 = float(np.arctan2(p1.y - p0.y, p1.x - p0.x))
    start = tf.move.make_start((p0.x, p0.y), ta0, time=first["timestamp"].iloc[0], dt=3600)
    kern = tf.move.redistribution_kernel(m, start, {"energy": energy, "slope": slope, "height": dem},
                                         n_control=PATHS["n_control"], seed=PATHS["seed"])
    lp = tf.move.simulate_path(kern, PATHS["n_steps"])
    op = paths[paths.track == f"{first[ID].iloc[0]}_path_1"].reset_index(drop=True)
    n = min(len(lp), len(op))
    dd = float(np.nanmax(np.hypot(lp.x_.to_numpy()[:n] - op.x_.to_numpy()[:n],
                                  lp.y_.to_numpy()[:n] - op.y_.to_numpy()[:n])))
    print(f"library vs operator, the first simulated path: {len(lp)} and {len(op)} points, largest distance {dd:.2e} m")
    out["path_diff"] = dd
    return out


# ---------------------------------------------------------------------------------------------- main
def main():
    w = mf.work(SLUG)
    shutil.copy(TRACKS, w / TRACKS.name)
    n1, o1 = phase1(w)
    t = time.time()
    got = mf.run(w, n1, o1)
    print(f"phase 1 run in {time.time() - t:.1f} s")
    d = got["data"]
    obs_all = d[d.case_ == 1]
    print(len(d), int(d.case_.sum()), d.groupby(ID).case_.sum().to_dict())
    RESULTS["steps"], RESULTS["rows"] = int(d.case_.sum()), len(d)
    fits = {k: got[f"fit_{k}"] for k in FORMULAS}
    for k in FORMULAS:
        print(k, FORMULAS[k], got[f"info_{k}"][["loglik_null", "loglik", "aic", "n", "nevent"]].round(2).to_dict("records"))
        print(fits[k].round(4).to_string())
    # the medians of the observed steps, and the energy of a cell of each slope
    obs = obs_all.dropna(subset=["slope"])
    medians = {"sl": round(float(obs.sl_.median()), 0), "slope": round(float(obs.slope.median()), 1),
               "energy": round(float(obs.energy.median()), 2), "tri": round(float(obs.tri.median()), 1)}
    print("medians of the observed steps:", medians)
    import rasterio

    with rasterio.open(w / "slope.tif") as a, rasterio.open(w / "energy.tif") as b:
        sl_, en_ = a.read(1).astype(float), b.read(1).astype(float)
    ok = np.isfinite(sl_) & np.isfinite(en_)
    slopes = np.arange(0, 61, 5.0)
    by = [float(np.median(en_[ok & (np.abs(sl_ - s) <= 2.5)])) for s in slopes]
    print("median energy of the cells by slope:", dict(zip(slopes.tolist(), np.round(by, 2))))
    n2, o2 = phase2(medians, np.array(by), slopes)
    t = time.time()
    g2 = mf.run(w, n1 + n2, o1 | o2, name="model2.toml")
    print(f"phase 2 run in {time.time() - t:.1f} s")
    paths = g2["sim"]
    plain = g2["sim_plain"]

    # ------------------------------------------------------------------ the library
    lib = library_side(w, d, fits, paths)
    RESULTS.update({k: lib[k] for k in ("parity_columns", "fit_end", "fit_full", "fit_sim", "path_diff")})
    dem, slope, energy = lib["dem"], lib["slope"], lib["energy"]

    # ------------------------------------------------------------------ tables
    cs = d[["slope", "tri", "energy", "dem_along_ascent", "dem_along_delta"]].dropna()
    corr = cs.corr()
    print("correlations of the relief covariates (all steps):\n", corr.round(2))
    figs.write_csv(DATA / "correlations.csv", [{"": a, **{b: round(float(corr.loc[a, b]), 2) for b in cs.columns}}
                                               for a in cs.columns])
    ua = []
    for case, name in ((1, "observed"), (0, "random")):
        q = d[d.case_ == case].dropna(subset=["slope"])
        ua.append({"steps": name, "n": len(q), "slope at the end (deg)": round(q.slope.mean(), 1),
                   "TRI at the end (m)": round(q.tri.mean(), 1), "energy at the end (kcal)": round(q.energy.mean(), 2),
                   "ascent along the step (m)": round(q.dem_along_ascent.mean(), 1),
                   "net height change (m)": round(q.dem_along_delta.mean(), 1),
                   "median net height change (m)": round(q.dem_along_delta.median(), 1),
                   "planar length (m)": round(q.sl_.mean(), 1), "length on the relief (m)": round(q.sl_surface_.mean(), 1),
                   "longer on the relief (%)": round((q.sl_surface_.sum() / q.sl_.sum() - 1) * 100, 1)})
        print(ua[-1])
    figs.write_csv(DATA / "used-available.csv", ua)
    for k in FORMULAS:
        figs.write_csv(DATA / f"coefficients-{k}.csv", coefficient_rows(fits[k], LABELS))
    info = {k: got[f"info_{k}"].iloc[0] for k in FORMULAS}
    RESULTS["loglik"] = {k: [float(info[k].loglik_null), float(info[k].loglik)] for k in FORMULAS}

    # a term at a time, and all of them (library)
    cov = lib["data"]
    one = []
    base = " + log(sl_) + cos(ta_) + strata(stratum)"
    for label, f in [("slope", "slope"), ("TRI", "tri"), ("energy", "energy"), ("ascent along the step", "dem_along_ascent"),
                     ("net height change along the step", "dem_along_delta")]:
        m = tf.move.fit_issf(cov, f"case_ ~ {f}" + base)
        se_ = float(m.summary().loc[f, "se"])
        one.append({"term": label, "coefficient": round(float(m.coef[f]), 4), "se": round(se_, 4),
                    "z": round(float(m.coef[f]) / se_, 2), "log-likelihood": round(m.loglik[1], 1)})
        print(one[-1])
    m0 = tf.move.fit_issf(cov, "case_ ~ log(sl_) + cos(ta_) + strata(stratum)")
    one.insert(0, {"term": "(movement only)", "coefficient": "", "se": "", "z": "", "log-likelihood": round(m0.loglik[1], 1)})
    figs.write_csv(DATA / "one-at-a-time.csv", one)
    # by animal and by body mass
    arows = []
    for a, g in cov.groupby(ID):
        m = tf.move.fit_issf(g, FORMULAS["full"] + " + strata(stratum)")
        sm = m.summary()
        arows.append({"animal": a, "steps": int(g.case_.sum()),
                      **{LABELS[t]: f"{sm.loc[t, 'coef']:.4f} ({sm.loc[t, 'coef'] - 1.96 * sm.loc[t, 'se']:.4f} to "
                                    f"{sm.loc[t, 'coef'] + 1.96 * sm.loc[t, 'se']:.4f})"
                         for t in ("energy", "dem_along_ascent", "dem_along_delta")}})
        print(arows[-1])
    figs.write_csv(DATA / "animals.csv", arows)
    mrows = []
    for mass in (40, 60, 80, 100, 120):
        en_m = tf.cost.energy_landscape(dem, mass, unit="kcal")
        cm = tf.move.extract_covariates(cov.drop(columns=["energy"]), {"energy": en_m})
        m = tf.move.fit_issf(cm, FORMULAS["full"] + " + strata(stratum)")
        sm = m.summary()
        mrows.append({"mass (kg)": mass, "mean energy at the end (kcal)": round(float(cm[cm.case_ == 1].energy.mean()), 2),
                      "energy coefficient": round(sm.loc["energy", "coef"], 4), "se": round(sm.loc["energy", "se"], 4),
                      "net height change": round(sm.loc["dem_along_delta", "coef"], 4),
                      "log-likelihood": round(m.loglik[1], 1)})
        print(mrows[-1])
    figs.write_csv(DATA / "body-mass.csv", mrows)
    crows = []
    sl_, ta_ = lib["distr"]
    tri = tf.surface.tri(dem, 3)
    for n_control in (2, 5, 10, 20, 50):
        cn = lib_data(tf.move.random_steps(lib["steps"], n_control=n_control, sl_distr=sl_, ta_distr=ta_, seed=1),
                      dem, slope, tri, energy)
        m = tf.move.fit_issf(cn, FORMULAS["full"] + " + strata(stratum)")
        sm = m.summary()
        crows.append({"random steps": n_control, "rows": len(cn),
                      "net height change": round(sm.loc["dem_along_delta", "coef"], 4),
                      "lower": round(sm.loc["dem_along_delta", "coef"] - 1.96 * sm.loc["dem_along_delta", "se"], 4),
                      "upper": round(sm.loc["dem_along_delta", "coef"] + 1.96 * sm.loc["dem_along_delta", "se"], 4),
                      "ascent": round(sm.loc["dem_along_ascent", "coef"], 4),
                      "energy": round(sm.loc["energy", "coef"], 4)})
        print(crows[-1])
    figs.write_csv(DATA / "controls.csv", crows)

    # the log-RSS tables
    figs.write_csv(DATA / "log-rss-slope.csv", [
        {"slope (deg)": r.slope_x1, "energy (kcal)": r.energy_x1, "log-RSS": round(r.log_rss, 3), "lower": round(r.lwr, 3),
         "upper": round(r.upr, 3), "RSS": round(float(np.exp(r.log_rss)), 3)} for r in g2["rss_slope"].itertuples()])
    figs.write_csv(DATA / "log-rss-height.csv", [
        {"ascent (m)": b.dem_along_ascent_x1,
         "straight up": round(a.log_rss, 3), "straight up, lower": round(a.lwr, 3), "straight up, upper": round(a.upr, 3),
         "up and down": round(b.log_rss, 3), "up and down, lower": round(b.lwr, 3), "up and down, upper": round(b.upr, 3)}
        for a, b in zip(g2["rss_climb"].itertuples(), g2["rss_undulating"].itertuples())])

    # ------------------------------------------------------------------ the simulated paths
    paths = paths[np.isfinite(paths.x_)].reset_index(drop=True)
    plain = plain[np.isfinite(plain.x_)].reset_index(drop=True)
    print("points per path: relief", paths.groupby("track").size().describe()[["min", "50%", "max"]].tolist(),
          "plain", plain.groupby("track").size().describe()[["min", "50%", "max"]].tolist())
    obs_x, obs_y = obs_all.x2_.to_numpy(float), obs_all.y2_.to_numpy(float)
    rows = []
    for name, (px, py, track) in {"observed": (obs_x, obs_y, None),
                                  "simulated, with the relief": (paths.x_.to_numpy(), paths.y_.to_numpy(), paths.track),
                                  "simulated, movement only": (plain.x_.to_numpy(), plain.y_.to_numpy(), plain.track)}.items():
        h, sl_c, en_c = cell_values(dem, px, py), cell_values(slope, px, py), cell_values(energy, px, py)
        if track is None:
            step_len, dh = obs_all.sl_.to_numpy(float), obs_all.dem_along_delta.to_numpy(float)
            n_paths, steps_n = 4, len(obs_all)
        else:
            tr_ = track.to_numpy()
            same = tr_[1:] == tr_[:-1]
            step_len = np.hypot(np.diff(px), np.diff(py))[same]
            dh = np.abs(np.diff(h))[same]
            n_paths, steps_n = len(set(tr_)), int(same.sum())
        rows.append({"positions": name, "paths": n_paths, "steps": steps_n,
                     "mean step (m)": round(float(np.nanmean(step_len)), 1),
                     "net height change per step (m)": round(float(np.nanmean(dh)), 1),
                     "median height change (m)": round(float(np.nanmedian(dh)), 1),
                     "slope (deg)": round(float(np.nanmean(sl_c)), 1), "energy (kcal)": round(float(np.nanmean(en_c)), 2),
                     "height (m)": round(float(np.nanmean(h)), 0), "height sd (m)": round(float(np.nanstd(h)), 0)})
        print(rows[-1])
    figs.write_csv(DATA / "simulation.csv", rows)

    # the use, on the grid of the DEM: observed positions, the paths with the relief and without it
    use_obs = tf.move.path_density(pd.DataFrame({"x_": obs_x, "y_": obs_y}), dem)
    use_sim = tf.load_raster(w / "use.tif")
    use_plain = tf.load_raster(w / "use_plain.tif")
    from scipy.stats import spearmanr

    def coarse(da, k=10):
        a = np.nan_to_num(np.asarray(da.values, float))
        r, c = (a.shape[0] // k) * k, (a.shape[1] // k) * k
        return a[:r, :c].reshape(r // k, k, c // k, k).sum(axis=(1, 3))

    co = coarse(use_obs)
    comp = []
    for name, u in (("with the relief", use_sim), ("movement only", use_plain)):
        cu = coarse(u)
        keep = (co > 0) | (cu > 0)
        rho = float(spearmanr(co[keep], cu[keep]).statistic)
        in_range = float(cu[co > 0].sum() / cu.sum())
        comp.append({"paths": name, "Spearman's rho of the use in cells of 300 m": round(rho, 3),
                     "share of the use in cells the animals used (%)": round(in_range * 100, 1)})
        print(comp[-1])
    print("cells of 300 m used by the animals:", int((co > 0).sum()), "of", co.size)
    figs.write_csv(DATA / "use-comparison.csv", comp)
    RESULTS["use_cells"] = int((co > 0).sum())
    # the connectivity: the animals' positions against the surfaces
    for k in ("v_res", "v_cur"):
        print(k, g2[k].round(3).to_string())
    vrows = []
    for k, nm in (("v_res", "resistance (lower is better)"), ("v_cur", "Omniscape's current")):
        tab = g2[k]
        r = tab[tab.iloc[:, 0].astype(str) == "pooled"].iloc[0] if tab.iloc[:, 0].astype(str).eq("pooled").any() else tab.iloc[-1]
        vrows.append({"surface": nm, "cells": int(r.n_cells), "Spearman's rho": round(float(r.rho), 3),
                      "use in the best 10 %": round(float(r.use_fraction) * 100, 1),
                      "area of the best 10 %": round(float(r.area_fraction) * 100, 1), "use ratio": round(float(r.use_ratio), 2)})
    figs.write_csv(DATA / "validation.csv", vrows)

    # ------------------------------------------------------------------ figures
    res = tf.load_raster(w / "resistance.tif")
    cur = tf.load_raster(w / "current.tif")
    figures(w, d, dem, slope, energy, res, cur, use_obs, use_sim, use_plain, paths, plain, g2, obs_x, obs_y)
    (w / "results.json").write_text(json.dumps(RESULTS, indent=1, default=float))
    print("RESULTS", RESULTS)


def figures(w, d, dem, slope, energy, res, cur, use_obs, use_sim, use_plain, paths, plain, g2, obs_x, obs_y):
    ext = extent_of(dem)
    obs_all = d[d.case_ == 1]
    pad = 1200
    crop = (obs_x.min() - pad, obs_x.max() + pad, obs_all.y2_.min() - pad, obs_all.y2_.max() + pad)
    crop = (max(crop[0], ext[0]), min(crop[1], ext[1]), max(crop[2], ext[2]), min(crop[3], ext[3]))
    x0, y0 = crop[0], crop[2]
    animals = sorted(obs_all[ID].unique())
    colors = dict(zip(animals, [figs.CLASSES[i] for i in (0, 1, 2, 3)]))

    # 1. the area: the relief with the ibex, the energy of walking and its relation to the slope
    fig, axs = plt.subplots(1, 3, figsize=(12.4, 4.7), dpi=150, gridspec_kw={"width_ratios": [1, 1, 0.8]})
    ax = axs[0]
    relief_base(ax, dem, crop)
    for a in animals:
        q = obs_all[obs_all[ID] == a]
        ax.scatter(q.x2_, q.y2_, s=2.2, color=colors[a], alpha=0.55, lw=0, label=a)
    ax.set_title("The relief and the four ibex", fontsize=10, loc="left")
    km_axes(ax, x0, y0)
    ax.plot([crop[0] + 400, crop[0] + 2400], [crop[2] + 500] * 2, color=figs.INK, lw=2.5, solid_capstyle="butt")
    ax.text(crop[0] + 1400, crop[2] + 560, "2 km", ha="center", va="bottom", fontsize=8.5, color=figs.INK)
    lg = ax.legend(loc="upper right", frameon=True, facecolor="white", framealpha=0.85, fontsize=8, markerscale=4, ncol=2)
    for h in lg.legend_handles:
        h.set_alpha(1)
    zr = np.asarray(dem.values, float)
    cb = fig.colorbar(plt.cm.ScalarMappable(cmap="YlGnBu_r", norm=plt.Normalize(np.nanpercentile(zr, 1), np.nanpercentile(zr, 99.5))),
                      ax=ax, shrink=0.78, pad=0.02)
    cb.set_label("Height (m)", fontsize=8.5)
    ax = axs[1]
    e = np.log10(np.asarray(energy.values, float))
    im = ax.imshow(e, extent=ext, cmap="magma_r", vmin=np.nanpercentile(e, 2), vmax=np.nanpercentile(e, 99.5),
                   interpolation="nearest")
    ax.imshow(hillshade(dem), extent=ext, cmap="gray", vmin=0.15, vmax=1.0, alpha=0.35, interpolation="nearest")
    ax.set_xlim(crop[0], crop[1])
    ax.set_ylim(crop[2], crop[3])
    ax.set_title("Energy of walking onto a cell (log10 kcal, 80 kg)", fontsize=10, loc="left")
    km_axes(ax, x0, y0)
    ax.set_ylabel("")
    fig.colorbar(im, ax=ax, shrink=0.78, pad=0.02)
    ax = axs[2]
    sv, ev = np.asarray(slope.values, float).ravel(), np.asarray(energy.values, float).ravel()
    ok = np.isfinite(sv) & np.isfinite(ev)
    ax.hexbin(sv[ok], ev[ok], gridsize=60, bins="log", cmap="Greys", mincnt=1, extent=(0, 70, 0, 80))
    bins = np.arange(0, 66, 5)
    mid = (bins[:-1] + bins[1:]) / 2
    med = [np.median(ev[ok][(sv[ok] >= a_) & (sv[ok] < b_)]) for a_, b_ in zip(bins[:-1], bins[1:])]
    ax.plot(mid, med, color=figs.EMBER, lw=2)
    ax.set_xlim(0, 65)
    ax.set_ylim(0, 60)
    ax.set_xlabel("Slope (degrees)")
    ax.set_ylabel("Energy (kcal)")
    ax.set_title("The cells: energy by slope", fontsize=10, loc="left")
    fig.tight_layout()
    save(fig, OUT / "area.png")

    # 2. one stratum over the contour lines
    dd = d[d.case_ == 0].groupby([ID, "step_id_"]).dem_along_delta.mean().rename("rand_delta")
    cand = obs_all.join(dd, on=[ID, "step_id_"])
    cand = cand[(cand.sl_ > 110) & (cand.sl_ < 180) & (cand.dem_along_delta <= 2) & (cand.rand_delta > 30)]
    print("candidates for the stratum figure:", len(cand))
    pick = cand.iloc[0] if len(cand) else obs_all.iloc[0]
    s_ = d[(d[ID] == pick[ID]) & (d.step_id_ == pick.step_id_)]
    fig, ax = plt.subplots(figsize=(6.6, 5.2), dpi=150)
    cx, cy = (pick.x1_ + pick.x2_) / 2, (pick.y1_ + pick.y2_) / 2
    half = 330
    box = (cx - half, cx + half, cy - half, cy + half)
    relief_base(ax, dem, box, alpha=0.8)
    zz = np.asarray(dem.values, float)
    cs_ = ax.contour(np.asarray(dem.x), np.asarray(dem.y), zz, levels=np.arange(0, 3000, 10), colors="#3b4a42",
                     linewidths=0.45, alpha=0.75)
    ax.clabel(cs_, levels=np.arange(0, 3000, 50), fmt="%d", fontsize=7)
    cm_ = plt.get_cmap("GnBu")
    vmax = max(60.0, float(s_.dem_along_delta.max()))
    for r in s_[s_.case_ == 0].itertuples():
        col = cm_(0.3 + 0.7 * min(r.dem_along_delta / vmax, 1))
        ax.plot([r.x1_, r.x2_], [r.y1_, r.y2_], color=col, lw=1.6, alpha=0.95, zorder=3)
        ax.scatter([r.x2_], [r.y2_], s=22, color=col, edgecolor="white", lw=0.6, zorder=4)
    ob = s_[s_.case_ == 1].iloc[0]
    ax.plot([ob.x1_, ob.x2_], [ob.y1_, ob.y2_], color=figs.EMBER, lw=3.4, zorder=5)
    ax.scatter([ob.x2_], [ob.y2_], s=60, color=figs.EMBER, edgecolor="white", zorder=6)
    ax.scatter([ob.x1_], [ob.y1_], s=50, color=figs.INK, zorder=6)
    ax.set_xlim(box[0], box[1])
    ax.set_ylim(box[2], box[3])
    km_axes(ax, box[0], box[2], step=200)
    ax.set_xlabel("m east")
    ax.set_ylabel("m north")
    ax.xaxis.set_major_formatter(lambda v, _: f"{v - box[0]:g}")
    ax.yaxis.set_major_formatter(lambda v, _: f"{v - box[2]:g}")
    sm_ = plt.cm.ScalarMappable(cmap=plt.matplotlib.colors.LinearSegmentedColormap.from_list("g", [cm_(0.3), cm_(1.0)]), norm=plt.Normalize(0, vmax))
    fig.colorbar(sm_, ax=ax, shrink=0.8).set_label("Net height change of a random step (m)", fontsize=9)
    fig.tight_layout()
    save(fig, OUT / "stratum.png")
    RESULTS["stratum"] = {"animal": pick[ID], "step_id": int(pick.step_id_), "length_m": float(pick.sl_),
                          "observed_delta_m": float(pick.dem_along_delta),
                          "random_deltas": [round(float(v), 1) for v in s_[s_.case_ == 0].dem_along_delta]}
    print(RESULTS["stratum"])

    # 3. the net height change and the ascent of the observed and random steps
    def comp(ax, col, xlabel, hi):
        bins = np.linspace(0, hi, 31)
        for case, color, lab in ((1, figs.EMBER, "observed"), (0, figs.WATER, "random")):
            v = d[d.case_ == case][col].dropna().clip(upper=hi)
            ax.hist(v, bins=bins, density=True, histtype="step", lw=1.9, color=color, label=lab)
        ax.set_xlabel(xlabel)
        ax.set_ylabel("Density")
        ax.legend(frameon=False, fontsize=9)

    fig, axs = plt.subplots(1, 2, figsize=(9.6, 3.6), dpi=150)
    comp(axs[0], "dem_along_delta", "Net height change along the step (m; 150 and more in the last class)", 150)
    comp(axs[1], "dem_along_ascent", "Ascent along the step (m; 150 and more in the last class)", 150)
    fig.tight_layout()
    save(fig, OUT / "height-change.png")

    # 4. the log-RSS: the slope (with its energy), and the height change
    fig, axs = plt.subplots(1, 2, figsize=(9.8, 3.8), dpi=150)
    r = g2["rss_slope"]
    ax = axs[0]
    ax.fill_between(r.slope_x1, r.lwr, r.upr, color=figs.FOREST, alpha=0.18, lw=0)
    ax.plot(r.slope_x1, r.log_rss, color=figs.FOREST, lw=2)
    ax.axhline(0, color=figs.INK2, lw=0.9, ls="--")
    ax.set_xlabel("Slope at the end of the step (degrees; its energy goes with it)")
    ax.set_ylabel("log-RSS against the median slope (33 degrees)")
    ax = axs[1]
    for key, color, lab in (("rss_climb", figs.EMBER, "straight up (ascent = net change)"),
                            ("rss_undulating", figs.WATER, "up and down (net change 0)")):
        r = g2[key]
        x = r.dem_along_ascent_x1
        ax.fill_between(x, r.lwr, r.upr, color=color, alpha=0.18, lw=0)
        ax.plot(x, r.log_rss, color=color, lw=2, label=lab)
    ax.axhline(0, color=figs.INK2, lw=0.9, ls="--")
    ax.set_xlabel("Ascent along the step (m)")
    ax.set_ylabel("log-RSS against a flat step")
    ax.legend(frameon=False, fontsize=8.5, loc="upper center")
    fig.tight_layout()
    save(fig, OUT / "log-rss.png")

    # 5. the use: the animals and the simulated paths
    from scipy.ndimage import gaussian_filter

    def smooth(da, sigma=4):
        return gaussian_filter(np.nan_to_num(np.asarray(da.values, float)), sigma)

    fig, axs = plt.subplots(1, 3, figsize=(12.4, 4.4), dpi=150)
    titles = ["The four ibex (positions)", "Simulated from the model with the relief", "Simulated, movement only"]
    for ax, da, ti in zip(axs, (use_obs, use_sim, use_plain), titles):
        relief_base(ax, dem, crop, alpha=0.7)
        u = smooth(da)
        ax.imshow(np.where(u > u.max() * 0.02, u, np.nan), extent=ext, cmap="YlOrRd", vmin=0, vmax=u.max() * 0.7, alpha=0.85,
                  interpolation="nearest")
        ax.set_title(ti, fontsize=10, loc="left")
        km_axes(ax, x0, y0)
    for ax in axs[1:]:
        ax.set_ylabel("")
    for sub, ax in ((paths, axs[1]), (plain, axs[2])):
        for _, g in sub.groupby("track"):
            ax.plot(g.x_, g.y_, color="#1c2420", lw=0.25, alpha=0.35)
    fig.tight_layout()
    save(fig, OUT / "paths.png")

    # 6. the resistance and the current
    fig, axs = plt.subplots(1, 2, figsize=(9.8, 4.6), dpi=150)
    ax = axs[0]
    im = ax.imshow(np.asarray(res.values, float), extent=ext, cmap="viridis_r", vmin=1, vmax=100)
    ax.set_title("Resistance of the habitat terms", fontsize=10, loc="left")
    fig.colorbar(im, ax=ax, shrink=0.78, pad=0.02).set_label("1 (best) to 100 (least selected)", fontsize=8.5)
    ax = axs[1]
    cv = np.asarray(cur.values, float)
    im = ax.imshow(np.log10(np.where(cv > 0, cv, np.nan)), extent=ext, cmap="magma")
    ax.set_title("Omniscape's current (log10)", fontsize=10, loc="left")
    fig.colorbar(im, ax=ax, shrink=0.78, pad=0.02)
    for ax in axs:
        ax.scatter(obs_x, obs_y, s=1.2, color="white", alpha=0.5, lw=0)
        ax.set_xlim(crop[0], crop[1])
        ax.set_ylim(crop[2], crop[3])
        km_axes(ax, x0, y0)
    axs[1].set_ylabel("")
    fig.tight_layout()
    save(fig, OUT / "connectivity.png")


if __name__ == "__main__":
    main()
