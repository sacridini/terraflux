"""Figures of "Clean a track of GPS errors and thin it" (website/content/tutorials/animal-movement/gps-errors-and-thinning.md).

The fishers' file has no GPS quality columns and no gross errors, so this page makes some: M4's 8,958 locations get
simulated satellite counts and dilution of precision, and 80 of them are spoiled (seeded: the same every run). The
errors are written beside the page's data (m4-with-errors.csv) with the truth in a column, so every number about what
a filter caught is checked against what was put in. The operators (move.filter_gps_quality, move.max_speed,
move.flag_speed, move.thin_time, move.thin_distance) are run by the terraflux binary and checked against the library.

    PYTHONPATH=extern/terraflux-py/src TERRAFLUX_BIN=target/debug/terraflux.exe \\
        python website/figures/animal-movement/cleaning-and-thinning.py
"""

import os

import geopandas as gpd
import matplotlib.pyplot as plt
import numpy as np
import pandas as pd
import terraflux as tf

import movefig as mf
from movefig import figs

SLUG = "cleaning-and-thinning"
OUT = figs.img_dir(f"{mf.CAT}/{SLUG}")
DATA = figs.data_dir(f"{mf.CAT}/{SLUG}")
ID = mf.ID
MASS = 4.5  # kg, a round number inside what an adult male fisher weighs
MODE = "running"
RESULTS = {}


def make_errors():
    """M4's locations with simulated GPS quality and 80 spoiled fixes (seeded)."""
    tr = mf.clean_tracks()
    m4 = tr[tr[ID] == "M4"].reset_index(drop=True)
    p, crs = mf.projected(m4)
    n = len(p)
    rng = np.random.default_rng(11)
    x, y = p.geometry.x.to_numpy().copy(), p.geometry.y.to_numpy().copy()
    # a good fix sees 5 to 12 satellites and a dilution of precision of 0.8 to 8
    sat = rng.choice(np.arange(5, 13), size=n, p=[0.08, 0.12, 0.20, 0.20, 0.18, 0.12, 0.06, 0.04])
    dop = np.round(np.clip(rng.gamma(6.0, 0.45, size=n), 0.8, 8.0), 1)
    kind = np.full(n, "", dtype=object)
    idx = rng.choice(np.arange(5, n - 5), size=80, replace=False)
    loud, silent, poor = idx[:30], idx[30:60], idx[60:]

    def jump(i, lo, hi):
        d = np.exp(rng.uniform(np.log(lo), np.log(hi)))
        a = rng.uniform(0, 2 * np.pi)
        x[i] += d * np.cos(a)
        y[i] += d * np.sin(a)

    for i in loud:  # a wrong position with the signs of a poor fix
        jump(i, 300, 5000)
        if rng.random() < 0.6:
            sat[i] = rng.choice([3, 4])
        else:
            dop[i] = np.round(rng.uniform(12, 30), 1)
        kind[i] = "jump, poor fix"
    for i in silent:  # a wrong position that looks like a good fix
        jump(i, 200, 3000)
        kind[i] = "jump, good-looking fix"
    for i in poor:  # a poor fix that happens to be right
        if rng.random() < 0.5:
            sat[i] = rng.choice([3, 4])
        else:
            dop[i] = np.round(rng.uniform(11, 20), 1)
        kind[i] = "poor fix, right position"
    ll = gpd.GeoSeries(gpd.points_from_xy(x, y), crs=crs).to_crs(4326)
    df = pd.DataFrame({
        "event-id": np.arange(1, n + 1),
        "timestamp": m4.timestamp.dt.strftime("%Y-%m-%d %H:%M:%S.%f").str[:-3],
        "location-long": np.round(ll.x.to_numpy(), 6), "location-lat": np.round(ll.y.to_numpy(), 6),
        "gps:satellite-count": sat, "gps:dop": dop, "sensor-type": "gps",
        "individual-local-identifier": "M4", "injected": kind})
    df.to_csv(DATA / "m4-with-errors.csv", index=False)
    return df, p.crs


def load(path):
    return tf.load_tracks(str(path))


def metrics(t, crs, label, truth=None):
    """Distance, the fastest step and the minimum convex polygons of a track (lon/lat, in order of time)."""
    d = tf.move.distance(t)
    sp = tf.move.speed(t)
    pr = t.to_crs(crs)
    return {"stage": label, "locations": len(t), "distance (km)": round(float(np.nansum(d)) / 1000, 1),
            "fastest step (m/s)": round(float(np.nanmax(sp)), 1),
            "MCP 100 % (km²)": round(float(tf.move.hr_area(tf.move.hr_mcp(pr, levels=1.0)).area.iloc[0]) / 1e6, 2),
            "MCP 95 % (km²)": round(float(tf.move.hr_area(tf.move.hr_mcp(pr, levels=0.95)).area.iloc[0]) / 1e6, 2)}


def main():
    w = mf.work(SLUG)
    df, crs = make_errors()
    csv = DATA / "m4-with-errors.csv"
    print("written", csv.name, len(df), "rows;", df.injected.value_counts().to_dict())
    orig = mf.clean_tracks()
    orig = orig[orig[ID] == "M4"].reset_index(drop=True)
    raw = load(csv)
    raw["injected"] = raw["injected"].fillna("")
    truth_ids = {k: set(raw.loc[raw.injected == k, "event-id"]) for k in raw.injected.unique() if k}

    # ------------------------------------------------------------------ the limit
    vp = tf.move.v_phys_estimate(MASS, MODE)
    print("v_max", vp)
    RESULTS["vmax"] = [vp.lower, vp.v_max, vp.upper, vp.kmh]

    # ------------------------------------------------------------------ the filters, one after the other
    q = tf.move.filter_gps_quality(raw)  # sat_min 5, dop_max 10
    dropped_q = set(raw["event-id"]) - set(q["event-id"])
    print("GPS quality drops", len(dropped_q))
    f = tf.move.flag_speed(q, mass=MASS, mode=MODE)
    flagged = set(f.loc[f.is_outlier.astype(bool), "event-id"])
    clean = f[~f.is_outlier.astype(bool)].reset_index(drop=True)
    print("speed flags", len(flagged), "limit used", f.attrs.get("v_max_used"))

    # detection against the truth
    rows = []
    for k, ids in truth_ids.items():
        by_q = len(ids & dropped_q)
        by_s = len(ids & flagged)
        left = len(ids) - by_q - by_s
        rows.append({"what was put in": k, "locations": len(ids), "dropped by the GPS quality": by_q,
                     "flagged by the speed": by_s, "left in the track": left})
    good = set(raw["event-id"]) - set().union(*truth_ids.values())
    rows.append({"what was put in": "untouched fixes", "locations": len(good),
                 "dropped by the GPS quality": len(good & dropped_q), "flagged by the speed": len(good & flagged),
                 "left in the track": len(good) - len(good & dropped_q) - len(good & flagged)})
    figs.write_csv(DATA / "detection.csv", rows)
    for r in rows:
        print(r)
    RESULTS["detection"] = rows

    # the stages of the clean-up and what each does to what is measured
    mrows = [metrics(orig, crs, "M4 as recorded (no errors put in)"),
             metrics(raw, crs, "with the errors"),
             metrics(q, crs, "after the GPS quality"),
             metrics(clean, crs, "after the speed limit")]
    figs.write_csv(DATA / "stages.csv", mrows)
    for r in mrows:
        print(r)

    # the limit: lower, estimate, +25 %, upper
    lrows = []
    for label, v in [("lower end of the interval", vp.lower), ("Hirt et al.'s estimate", vp.v_max),
                     ("upper end of the interval", vp.upper), ("the estimate + 25 %", vp.v_max * 1.25),
                     ("3 times the estimate", vp.v_max * 3)]:
        ff = tf.move.flag_speed(q, v_max=v)
        fl = set(ff.loc[ff.is_outlier.astype(bool), "event-id"])
        err = set().union(*[ids for k, ids in truth_ids.items() if k != "poor fix, right position"])
        lrows.append({"limit": label, "m/s": round(float(v), 2), "km/h": round(float(v) * 3.6, 1),
                      "flagged": len(fl), "errors among them": len(fl & err),
                      "untouched fixes among them": len(fl & good),
                      "errors left in": len(err - dropped_q - fl)})
        print(lrows[-1])
    figs.write_csv(DATA / "limit.csv", lrows)

    # ------------------------------------------------------------------ figures
    # the speeds against the limit
    ff = tf.move.flag_speed(q, v_max=vp.v_max)
    spd = ff.step_speed.to_numpy()
    nxt_bad = np.isin(ff["event-id"].to_numpy(), list(truth_ids["jump, good-looking fix"]))
    prev_bad = np.r_[False, nxt_bad[:-1]]
    step_bad = nxt_bad | prev_bad  # a step to or from a spoiled fix

    def speeds(ax):
        bins = np.logspace(-4, 2, 90)
        ax.hist(spd[~step_bad & np.isfinite(spd)], bins=bins, color=figs.FOREST, alpha=0.75,
                label="steps of untouched fixes")
        ax.hist(spd[step_bad & np.isfinite(spd)], bins=bins, color=figs.EMBER, alpha=0.85,
                label="steps to or from a good-looking spoiled fix")
        ax.set_xscale("log")
        ax.set_yscale("log")
        ax.axvline(vp.v_max, color=figs.INK, lw=1.3, ls="--")
        ax.text(vp.v_max * 1.1, ax.get_ylim()[1] * 0.45, f"limit {vp.v_max:.1f} m/s\n({vp.kmh:.0f} km/h)",
                fontsize=9, color=figs.INK, va="top")
        ax.set_xlabel("Speed of the step (m/s, log scale)")
        ax.set_ylabel("Steps (log scale)")
        ax.legend(frameon=False, fontsize=9, loc="upper left")

    figs.chart(OUT / "speeds.png", speeds, width=900, height=420)
    RESULTS["steps_over_limit"] = int(np.nansum(spd > vp.v_max))

    # Hirt et al.'s law
    mass = np.logspace(-3, 5, 200)

    def hirt(ax):
        for mode, col in [("flying", figs.WATER), ("running", figs.FOREST), ("swimming", figs.EMBER)]:
            v = tf.move.v_phys_estimate(mass, mode)
            ax.plot(mass, v.v_max, color=col, lw=2, label=mode)
            ax.fill_between(mass, v.lower, v.upper, color=col, alpha=0.18, lw=0)
        ax.scatter([MASS], [vp.v_max], s=48, color=figs.INK, zorder=5)
        ax.annotate("M4 (4.5 kg, running)", (MASS, vp.v_max), (MASS * 2.2, vp.v_max * 0.45), fontsize=9,
                    arrowprops=dict(arrowstyle="-", color=figs.INK, lw=0.8))
        ax.set_xscale("log")
        ax.set_yscale("log")
        ax.set_ylim(0.2, 100)
        ax.set_xlabel("Body mass (kg, log scale)")
        ax.set_ylabel("Maximum speed (m/s, log scale)")
        ax.legend(frameon=False, fontsize=9, loc="lower right")

    figs.chart(OUT / "hirt.png", hirt, width=900, height=420)

    # the track before and after
    box = None
    fig, axs = plt.subplots(1, 2, figsize=(9.4, 4.3), dpi=150, sharex=True, sharey=True)
    pr = raw.to_crs(crs)
    for ax, title in zip(axs, ["With the errors", "After the two filters"]):
        ax.set_title(title, fontsize=10.5, loc="left")
    axs[0].plot(pr.geometry.x, pr.geometry.y, color=figs.INK2, lw=0.25, alpha=0.55)
    colors = {"jump, poor fix": figs.WATER, "jump, good-looking fix": figs.EMBER}
    for k, col in colors.items():
        s = pr[pr["event-id"].isin(truth_ids[k])]
        axs[0].scatter(s.geometry.x, s.geometry.y, s=14, color=col, lw=0, label=k, zorder=4)
    axs[0].legend(loc="upper left", fontsize=8.5, frameon=False)
    cl = clean.to_crs(crs)
    axs[1].plot(cl.geometry.x, cl.geometry.y, color=figs.INK2, lw=0.25, alpha=0.55)
    left = cl[cl["event-id"].isin(truth_ids["jump, good-looking fix"] | truth_ids["jump, poor fix"])]
    axs[1].scatter(left.geometry.x, left.geometry.y, s=18, color=figs.EMBER, lw=0, label="spoiled fixes left in",
                   zorder=4)
    axs[1].legend(loc="upper left", fontsize=8.5, frameon=False)
    for ax in axs:
        mf.kmaxes(ax)
    axs[1].set_ylabel("")
    fig.tight_layout()
    mf.save(fig, OUT / "cleaned.png")

    # ------------------------------------------------------------------ the operators against the library
    nodes = [
        {"id": "read", "op": "move.read_tracks", "params": {"path": "m4-with-errors.csv"}},
        {"id": "quality", "op": "move.filter_gps_quality", "inputs": {"tracks": "read.tracks"},
         "params": {"sat_min": 5, "dop_max": 10}},
        {"id": "limit", "op": "move.max_speed", "params": {"mass": MASS, "mode": MODE}},
        {"id": "speed", "op": "move.flag_speed", "inputs": {"tracks": "quality.tracks"},
         "params": {"mass": MASS, "mode": MODE}},
        {"id": "clean", "op": "vector.filter", "inputs": {"vector": "speed.tracks"},
         "params": {"expression": "is_outlier == 0"}},
        {"id": "thin", "op": "move.thin_time", "inputs": {"tracks": "clean.vector"}, "params": {"interval": "10 mins"}},
        {"id": "by_distance", "op": "move.thin_distance", "inputs": {"tracks": "clean.vector"},
         "params": {"distance": 100}},
        {"id": "by_interval", "op": "move.thin_distance", "inputs": {"tracks": "clean.vector"},
         "params": {"distance": 100, "method": "interval"}},
    ]
    outs = {"quality": "quality.tracks", "limit": "limit.table", "speed": "speed.tracks", "clean": "clean.vector",
            "thin": "thin.tracks", "by_distance": "by_distance.tracks", "by_interval": "by_interval.tracks"}
    import shutil

    shutil.copy(csv, w / csv.name)
    try:
        got = mf.run(w, nodes, outs)
        print("operator limit table:", got["limit"].to_dict("records"))
        print("operators: after the GPS quality", len(got["quality"]), "library", len(q))
        print("operators: flagged", int(got["speed"].is_outlier.sum()), "library", len(flagged))
        print("operators: clean", len(got["clean"]), "library", len(clean))
        same = bool((got["clean"]["event-id"].to_numpy() == clean["event-id"].to_numpy()).all()) if len(
            got["clean"]) == len(clean) else False
        print("the same locations kept:", same)
        RESULTS["operators_same"] = same
        RESULTS["operators_thin"] = [len(got["thin"]), len(got["by_distance"]), len(got["by_interval"])]
    except SystemExit as e:
        print("operator model:", str(e)[:1500])
        got = None

    # ------------------------------------------------------------------ the thinning
    base = clean
    d0 = base.timestamp.diff().dt.total_seconds() / 60
    print("clean track: median lag", float(d0.median()), "min")

    def lag_stats(t, target, tol):
        lag = t.timestamp.diff().dt.total_seconds().to_numpy()[1:] / 60
        inside = np.abs(lag - target) <= tol + 1e-9
        return lag, inside

    trows = []
    series = {}
    for label, t, target, tol in [
            ("one per clock interval (move.filter_per_interval)", tf.move.filter_per_interval(base, "10 mins"), 10, 1),
            ("move.thin_time, 10 min ± 1", tf.move.filter_thin_time(base, "10 mins"), 10, 1),
            ("move.thin_time, 10 min ± 3", tf.move.filter_thin_time(base, "10 mins", "3 mins"), 10, 3)]:
        lag, inside = lag_stats(t, target, tol)
        series[label] = lag
        trows.append({"method": label, "locations": len(t), "median lag (min)": round(float(np.median(lag)), 1),
                      "lags within ± 1 min of 10": round(float((np.abs(lag - 10) <= 1).mean()) * 100, 1),
                      "lags under 5 min": round(float((lag < 5).mean()) * 100, 1),
                      "lags over 60 min (gaps)": int((lag > 60).sum())})
        print(trows[-1])
    figs.write_csv(DATA / "thin-time.csv", trows)

    def lagchart(ax):
        bins = np.arange(0, 41, 1.0)
        for (label, lag), col in zip(series.items(), [figs.EMBER, figs.FOREST, figs.WATER]):
            ax.hist(np.clip(lag, 0, 40), bins=bins, histtype="step", lw=1.8, color=col, label=label)
        ax.set_xlabel("Time between consecutive kept locations (minutes; 40 and more in the last class)")
        ax.set_ylabel("Steps")
        ax.legend(frameon=False, fontsize=8.5)

    figs.chart(OUT / "thin-lags.png", lagchart, width=900, height=400)

    # thinning by distance, on a night
    night0, night1 = pd.Timestamp("2010-03-05 17:00", tz="UTC"), pd.Timestamp("2010-03-06 17:00", tz="UTC")
    nb = base[(base.timestamp >= night0) & (base.timestamp < night1)].reset_index(drop=True)
    variants = [("All the locations", nb, None),
                ("move.thin_time, 30 minutes", tf.move.filter_thin_time(nb, "30 mins"), None),
                ("move.thin_distance, 100 m (step)", tf.move.filter_thin_distance(nb, 100), None),
                ("move.thin_distance, 100 m ± 10 (interval)", tf.move.filter_thin_distance(nb, 100, "interval"), None)]
    drows = []
    for label, t, _ in variants:
        dist = tf.move.distance(t)
        d = dist[np.isfinite(dist)]
        lag = t.timestamp.diff().dt.total_seconds().to_numpy()[1:] / 60
        drows.append({"night of M4": label, "locations": len(t), "median step (m)": round(float(np.median(d)), 1),
                      "shortest step (m)": round(float(d.min()), 1), "longest step (m)": round(float(d.max()), 1),
                      "median time lag (min)": round(float(np.median(lag)), 1)})
        print(drows[-1])
    figs.write_csv(DATA / "thin-distance.csv", drows)
    fig, axs = plt.subplots(1, 4, figsize=(11.0, 2.9), dpi=150, sharex=True, sharey=True)
    xs, ys = [], []
    for ax, (label, t, _) in zip(axs, variants):
        ax.plot(nb.geometry.x, nb.geometry.y, color="#c9cfc9", lw=0.7)
        ax.plot(t.geometry.x, t.geometry.y, color=figs.INK, lw=0.9)
        ax.scatter(t.geometry.x, t.geometry.y, s=7, color=figs.EMBER, zorder=3, edgecolor="white", lw=0.3)
        ax.set_title(f"{label}\n{len(t)} locations", fontsize=8.5)
        mf.lonlat(ax)
        ax.locator_params(nbins=3)
        ax.set_xlabel("")
    axs[0].set_ylabel("Latitude")
    for ax in axs[1:]:
        ax.set_ylabel("")
    ax = axs[0]
    ax.set_xlim(nb.geometry.x.min() - 0.002, nb.geometry.x.max() + 0.002)
    ax.set_ylim(nb.geometry.y.min() - 0.002, nb.geometry.y.max() + 0.002)
    fig.tight_layout()
    mf.save(fig, OUT / "thin-distance.png")

    print("RESULTS", RESULTS)


if __name__ == "__main__":
    main()
