"""Figures of "Regular steps and track summaries" (website/content/tutorials/animal-movement/steps-and-summaries.md).

    python website/figures/animal-movement/steps-and-summaries.py
"""

import numpy as np
import matplotlib.pyplot as plt
import pandas as pd

import movefig as mf
from movefig import figs

SLUG = "steps-and-summaries"
OUT = figs.img_dir(f"{mf.CAT}/{SLUG}")
DATA = figs.data_dir(f"{mf.CAT}/{SLUG}")
RATES = [("2min", "20s"), ("10min", "1min"), ("30min", "3min"), ("1h", "6min"), ("2h", "12min")]


def chain(i: int, rate: str, tol: str) -> list[dict]:
    """The projected tracks resampled at rate (tolerance tol), bursts of 3 locations and more, their steps."""
    return [
        {"id": f"res{i}", "op": "move.resample", "inputs": {"tracks": "aeqd.tracks"},
         "params": {"rate": rate, "tolerance": tol}},
        {"id": f"bursts{i}", "op": "move.filter_min_n_burst", "inputs": {"tracks": f"res{i}.tracks"},
         "params": {"min_n": 3}},
        {"id": f"steps{i}", "op": "move.steps", "inputs": {"tracks": f"bursts{i}.tracks"}},
    ]


def main():
    w = mf.work(SLUG)
    nodes = list(mf.CLEAN) + [{"id": "aeqd", "op": "move.aeqd_crs", "inputs": {"tracks": "unique.tracks"}}]
    outs = {}
    for i, (r, tol) in enumerate(RATES):
        nodes += chain(i, r, tol)
        outs[f"res{i}"] = f"res{i}.tracks"
        outs[f"steps{i}"] = f"steps{i}.steps"
    nodes += [
        {"id": "tod", "op": "move.time_of_day", "inputs": {"tracks": "steps1.steps"},
         "params": {"include_crepuscule": True}},
        {"id": "nsd", "op": "move.nsd", "inputs": {"tracks": "aeqd.tracks"}},
        {"id": "summary", "op": "move.track_summary", "inputs": {"tracks": "bursts1.tracks"}},
        {"id": "summary_all", "op": "move.track_summary", "inputs": {"tracks": "aeqd.tracks"}},
    ]
    outs.update({"tod": "tod.tracks", "nsd": "nsd.tracks", "summary": "summary.tracks",
                 "summary_all": "summary_all.tracks", "aeqd": "aeqd.tracks"})
    got = mf.run(w, nodes, outs)
    st = got["steps1"]
    print(st.columns.tolist())
    print(st.head(3).drop(columns="geometry").to_string())

    # how the rate changes the data
    rows = []
    for i, (r, tol) in enumerate(RATES):
        res, s = got[f"res{i}"], got[f"steps{i}"]
        nb = res.groupby(mf.ID)["burst_"].nunique().sum()
        rows.append({"rate": r.replace("min", " min").replace("h", " h"), "tolerance": tol, "locations": len(res),
                     "bursts": int(nb), "steps": len(s),
                     "animals": int(s[mf.ID].nunique()),
                     "M4's median step (m)": round(s.loc[s[mf.ID] == "M4", "sl_"].median(), 1)})
        print(rows[-1])
    figs.write_csv(DATA / "rates.csv", rows)

    # M4's bursts of one day at 10 minutes
    r1 = got["res1"]
    r1["t"] = pd.to_datetime(r1["timestamp"], format="ISO8601")
    a = got["aeqd"]
    a["t"] = pd.to_datetime(a["timestamp"], format="ISO8601")
    day0, day1 = pd.Timestamp("2010-03-05 17:00"), pd.Timestamp("2010-03-06 17:00")
    d = r1[(r1[mf.ID] == "M4") & (r1.t >= day0) & (r1.t < day1)]
    full = a[(a[mf.ID] == "M4") & (a.t >= day0) & (a.t < day1)]
    fig, ax = plt.subplots(figsize=(7.6, 4.6), dpi=150)
    ax.plot(full.geometry.x, full.geometry.y, color="#d3d8d3", lw=0.8, zorder=1)
    for k, (b, s) in enumerate(d.groupby("burst_")):
        c = figs.CLASSES[k % len(figs.CLASSES)]
        ax.plot(s.geometry.x, s.geometry.y, color=c, lw=1.6, zorder=2)
        ax.scatter(s.geometry.x, s.geometry.y, s=14, color=c, zorder=3, edgecolor="white", lw=0.4)
    ax.set_aspect(1)
    ax.ticklabel_format(useOffset=False)
    ax.set_xlabel("m east of the centre")
    ax.set_ylabel("m north of the centre")
    fig.tight_layout()
    mf.save(fig, OUT / "bursts.png")
    print("M4 that day:", len(d), "locations in", d.burst_.nunique(), "bursts")

    # step lengths and turn angles at 10 minutes
    def lengths(ax):
        v = st.sl_.dropna()
        ax.hist(v, bins=np.arange(0, 1500, 25), color=figs.FOREST, alpha=0.85)
        ax.set_xlabel("Step length sl_ (m), 10-minute steps")
        ax.set_ylabel("Steps")
        ax.axvline(v.median(), color=figs.EMBER, lw=1.2)
        ax.text(v.median() + 20, ax.get_ylim()[1] * 0.9, f"median {v.median():.0f} m", color=figs.EMBER, fontsize=9)

    figs.chart(OUT / "lengths.png", lengths, width=900, height=380)

    def turns(ax):
        v = np.degrees(st.ta_.dropna())
        ax.hist(v, bins=np.arange(-180, 181, 10), color=figs.WATER, alpha=0.85)
        ax.set_xticks(range(-180, 181, 45))
        ax.set_xlabel("Turn angle ta_ (degrees), 10-minute steps")
        ax.set_ylabel("Steps")

    figs.chart(OUT / "turns.png", turns, width=900, height=380)

    # net squared displacement over the days
    n = got["nsd"]
    n["t"] = pd.to_datetime(n["timestamp"], format="ISO8601")
    fig, axs = plt.subplots(2, 4, figsize=(10, 4.8), dpi=150, sharey=False)
    for ax, k in zip(axs.flat, mf.ANIMALS):
        s = n[n[mf.ID] == k]
        days = (s.t - s.t.iloc[0]).dt.total_seconds() / 86400
        ax.plot(days, s.nsd / 1e6, color=mf.COLORS[k], lw=0.7)
        ax.set_title(k, loc="left", fontsize=10)
        ax.tick_params(labelsize=8)
    for ax in axs[1]:
        ax.set_xlabel("Days", fontsize=9)
    for ax in axs[:, 0]:
        ax.set_ylabel("NSD (km²)", fontsize=9)
    fig.tight_layout()
    mf.save(fig, OUT / "nsd.png")

    # day and night
    tod = got["tod"]
    rows = []
    for k in ("day", "dusk", "night", "dawn"):
        s = tod[tod.tod == k]
        rows.append({"time of day": k, "steps": len(s), "share (%)": round(100 * len(s) / len(tod), 1),
                     "median step (m)": round(s.sl_.median(), 1), "mean step (m)": round(s.sl_.mean(), 1)})
        print(rows[-1])
    figs.write_csv(DATA / "time-of-day.csv", rows)

    def tod_chart(ax):
        # the mean step of each hour of the local day (New York in winter: UTC - 5)
        s = tod.dropna(subset=["sl_"]).copy()
        s["hour"] = (pd.to_datetime(s["t1_"], format="ISO8601") - pd.Timedelta(hours=5)).dt.hour
        g = s.groupby("hour").agg(mean=("sl_", "mean"), n=("sl_", "size"),
                                  night=("tod", lambda x: (x == "night").mean()))
        cols = [("#2f3e5c" if f > 0.5 else "#e8901f") for f in g.night]
        ax.bar(g.index, g["mean"], color=cols, width=0.8)
        ax.set_xticks(range(0, 24, 3), [f"{h:02d}:00" for h in range(0, 24, 3)])
        ax.set_xlabel("Hour of the local day (UTC - 5) at the start of the step")
        ax.set_ylabel("Mean step length (m)")
        from matplotlib.patches import Patch
        ax.legend(handles=[Patch(color="#2f3e5c", label="mostly night"), Patch(color="#e8901f", label="mostly day")],
                  frameon=False, fontsize=9, loc="upper center")

    figs.chart(OUT / "time-of-day.png", tod_chart, width=900, height=400)

    # the summaries of each track
    def summary_rows(s):
        out = []
        for _, r in s.iterrows():
            out.append({"animal": r[mf.ID], "locations": int(r.n_locations),
                        "cum_dist (km)": round(r.cum_dist / 1000, 1), "tot_dist (km)": round(r.tot_dist / 1000, 2),
                        "straightness": round(r.straightness, 4), "msd (km²)": round(r.msd / 1e6, 2),
                        "sinuosity": round(r.sinuosity, 3), "tac": round(r.tac, 3)})
        return out

    rows = summary_rows(got["summary"])
    for r in rows:
        print(r)
    figs.write_csv(DATA / "summary.csv", rows)
    figs.write_csv(DATA / "summary-all.csv", summary_rows(got["summary_all"]))


if __name__ == "__main__":
    main()
