"""Figures of "Clean tracks and measure them" (website/content/tutorials/animal-movement/cleaning-and-measures.md).

    python website/figures/animal-movement/cleaning-and-measures.py
"""

import numpy as np
import matplotlib.pyplot as plt
import pandas as pd
from matplotlib.collections import LineCollection

import movefig as mf
from movefig import figs

SLUG = "cleaning-and-measures"
OUT = figs.img_dir(f"{mf.CAT}/{SLUG}")
DATA = figs.data_dir(f"{mf.CAT}/{SLUG}")

MEASURES = [
    {"id": "lags", "op": "move.time_lags", "inputs": {"tracks": "unique.tracks"}, "params": {"units": "min"}},
    {"id": "dist", "op": "move.distance", "inputs": {"tracks": "lags.tracks"}},
    {"id": "speed", "op": "move.speed", "inputs": {"tracks": "dist.tracks"}, "params": {"units": "km/h"}},
    {"id": "heading", "op": "move.azimuth", "inputs": {"tracks": "speed.tracks"}, "params": {"units": "deg"}},
    {"id": "turn", "op": "move.turn_angle", "inputs": {"tracks": "heading.tracks"}, "params": {"units": "deg"}},
]
UNITS = ["2 mins", "10 mins", "30 mins", "hour", "2 hours"]


def thinned(i: int, unit: str) -> list[dict]:
    """A location per interval of the cleaned tracks, then their turn angles and step lengths."""
    return [
        {"id": f"thin{i}", "op": "move.filter_per_interval", "inputs": {"tracks": "unique.tracks"},
         "params": {"unit": unit}},
        {"id": f"tdist{i}", "op": "move.distance", "inputs": {"tracks": f"thin{i}.tracks"}},
        {"id": f"tturn{i}", "op": "move.turn_angle", "inputs": {"tracks": f"tdist{i}.tracks"},
         "params": {"units": "deg"}},
    ]


def main():
    w = mf.work(SLUG)
    nodes = list(mf.CLEAN) + MEASURES + [
        {"id": "segments", "op": "move.segments", "inputs": {"tracks": "turn.tracks"}},
    ]
    outs = {"located": "located.vector", "visible": "visible.tracks", "m": "turn.tracks",
            "segments": "segments.segments"}
    for i, u in enumerate(UNITS):
        nodes += thinned(i, u)
        outs[f"thin{i}"] = f"tturn{i}.tracks"
    got = mf.run(w, nodes, outs)
    m, seg = got["m"], got["segments"]
    print(len(got["located"]), "located,", len(got["visible"]), "visible,", len(m), "unique")
    m["t"] = pd.to_datetime(m["timestamp"])

    # the time lags: three schedules
    def lags(ax):
        bins = np.logspace(np.log10(0.3), np.log10(3000), 70)
        for a in mf.ANIMALS:
            v = m.loc[m[mf.ID] == a, "time_lag"].dropna()
            ax.hist(v, bins=bins, histtype="step", lw=1.3, color=mf.COLORS[a], label=a)
        ax.set_xscale("log")
        ax.set_xticks([0.5, 1, 2, 5, 10, 15, 30, 60, 120, 600, 1440], ["0.5", "1", "2", "5", "10", "15", "30", "60",
                                                                       "120", "600", "1440"])
        ax.set_xlabel("Time lag to the next location (minutes, log scale)")
        ax.set_ylabel("Steps")
        ax.legend(ncol=4, fontsize=8.5, frameon=False)

    figs.chart(OUT / "lags.png", lags, width=950, height=430)

    # speed against the time lag: the fast steps are the short ones
    def speed_lag(ax):
        s = m.dropna(subset=["speed", "time_lag"])
        s = s[s.time_lag > 0]
        ax.scatter(s.time_lag, s.speed, s=2, alpha=0.25, color=figs.FOREST, rasterized=True)
        ax.set_xscale("log")
        ax.set_yscale("log")
        ax.set_xlim(0.2, 3000)
        ax.set_ylim(1e-3, 60)
        ax.set_xticks([0.5, 1, 2, 5, 10, 30, 60, 600], ["0.5", "1", "2", "5", "10", "30", "60", "600"])
        ax.set_yticks([0.001, 0.01, 0.1, 1, 10], ["0.001", "0.01", "0.1", "1", "10"])
        ax.axhline(10, color=figs.EMBER, lw=1, ls="--")
        ax.text(1500, 12, "10 km/h", color=figs.EMBER, ha="right", fontsize=9)
        ax.set_xlabel("Time lag (minutes, log scale)")
        ax.set_ylabel("Speed (km/h, log scale)")

    figs.chart(OUT / "speed-lag.png", speed_lag, width=900, height=460)

    # the steps of M4's night, coloured by speed
    day0, day1 = pd.Timestamp("2010-03-05 17:00"), pd.Timestamp("2010-03-06 17:00")
    seg["t"] = pd.to_datetime(seg["timestamp"])
    s = seg[(seg[mf.ID] == "M4") & (seg.t >= day0) & (seg.t < day1) & (seg.geometry.geom_type == "LineString")]
    fig, ax = plt.subplots(figsize=(7.2, 4.6), dpi=150)
    lines = [np.array(g.coords) for g in s.geometry]
    lc = LineCollection(lines, array=np.clip(s.speed.values, 0, 6), cmap="viridis", linewidths=2.2)
    ax.add_collection(lc)
    ax.autoscale()
    mf.lonlat(ax)
    ax.locator_params(nbins=5)
    mf.scalebar(ax, 0.5)
    cb = fig.colorbar(lc, ax=ax, shrink=0.85)
    cb.set_label("Speed (km/h; 6 and more in yellow)", fontsize=9)
    fig.tight_layout()
    mf.save(fig, OUT / "segments.png")

    # turn angles at 2 minutes and at 2 hours, M4 and M5
    def roses(png, which):
        fig, axs = plt.subplots(1, len(which), figsize=(3.1 * len(which), 3.3), dpi=150,
                                subplot_kw={"projection": "polar"})
        bins = np.radians(np.arange(-180, 181, 15))
        for ax, (i, title) in zip(axs, which):
            d = got[f"thin{i}"]
            d = d[d[mf.ID].isin(["M4", "M5"])]
            v = np.radians(d.turn_angle.dropna().values)
            h, _ = np.histogram(v, bins=bins)
            ax.bar(bins[:-1] + np.diff(bins) / 2, h / h.sum(), width=np.diff(bins), color=figs.WATER, alpha=0.8,
                   edgecolor="white", lw=0.5)
            ax.set_theta_zero_location("N")
            ax.set_theta_direction(-1)
            ax.set_yticklabels([])
            ax.set_xticks(np.radians([0, 90, 180, 270]), ["0°", "90°", "180°", "-90°"], fontsize=8.5)
            ax.set_title(title, fontsize=10.5, pad=12)
        fig.tight_layout()
        mf.save(fig, png)

    roses(OUT / "turns.png", [(0, "every 2 minutes"), (2, "every 30 minutes"), (4, "every 2 hours")])

    # the slider: M4's night thinned to one location per interval
    stats = []
    for i, u in enumerate(UNITS):
        d = got[f"thin{i}"]
        d["t"] = pd.to_datetime(d["timestamp"])
        n = d[(d[mf.ID] == "M4") & (d.t >= day0) & (d.t < day1)]
        fig, ax = plt.subplots(figsize=(6.4, 4.4), dpi=150)
        full = m[(m[mf.ID] == "M4") & (m.t >= day0) & (m.t < day1)]
        ax.plot(full.geometry.x, full.geometry.y, color="#c9cfc9", lw=0.8)
        ax.plot(n.geometry.x, n.geometry.y, color=figs.INK, lw=1.4)
        ax.scatter(n.geometry.x, n.geometry.y, s=14, color=figs.EMBER, zorder=3, edgecolor="white", lw=0.4)
        ax.set_xlim(full.geometry.x.min() - 0.002, full.geometry.x.max() + 0.002)
        ax.set_ylim(full.geometry.y.min() - 0.002, full.geometry.y.max() + 0.002)
        mf.lonlat(ax)
        ax.locator_params(nbins=5)
        mf.scalebar(ax, 0.5)
        fig.tight_layout()
        mf.save(fig, OUT / f"thin_{i}.png")
        km = n["distance"].iloc[:-1].sum() / 1000
        stats.append({"value": i, "unit": u, "locations": len(n), "km": round(km, 2)})
        print(u, len(n), "locations", round(km, 2), "km")
    figs.write_csv(DATA / "thin.csv", stats)

    # per animal, at its own rate and at a common one (one location per 10 minutes)
    t10 = got["thin1"]
    rows = []
    for a in mf.ANIMALS:
        s = m[m[mf.ID] == a]
        q = t10[t10[mf.ID] == a]
        days = (s.t.max() - s.t.min()).total_seconds() / 86400
        rows.append({"animal": a, "median lag (min)": round(s.time_lag.median(), 1),
                     "median speed (km/h)": round(s.speed.median(), 2),
                     "99th percentile (km/h)": round(s.speed.quantile(0.99), 1),
                     "km per day": round(s["distance"].sum() / 1000 / days, 2),
                     "km per day at 10 min": round(q["distance"].sum() / 1000 / days, 2)})
        print(rows[-1])
    figs.write_csv(DATA / "animals.csv", rows)
    print("short lags", int((m.time_lag < 1).sum()), "speeds over 10 km/h", int((m.speed > 10).sum()),
          "of which lag < 1 min", int(((m.speed > 10) & (m.time_lag < 1)).sum()))


if __name__ == "__main__":
    main()
