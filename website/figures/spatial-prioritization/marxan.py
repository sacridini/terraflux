"""Figures of 'Reserves with Marxan, and irreplaceability' (spatial-prioritization/marxan).

Marxan 4 (prioritize.marxan) on Serra Verde: a toy of the boundary, calibrating the species penalty factor, the
number of iterations against the proven optimum, a slider over the boundary length modifier, selection frequency,
and ProxIRR's irreplaceability (prioritize.proxirr) on cells and on blocks of 5 x 5 cells.

    python website/figures/spatial-prioritization/marxan.py [toy spf iterations blm versus_optimum proxirr]
"""

import numpy as np

import sp_common as sp
from sp_common import figs, plt

SLUG = "marxan"
OUT = sp.img(SLUG)
DAT = sp.data(SLUG)
PROP = "[0.3, 0.3, 0.3, 0.3, 0.3, 0.3, 0.3, 0.3]"
ITERS = 10_000_000
OPTIMUM = 37141.1  # the proven optimum of the 30 % minimum set (prioritize.optimize, gap 0), thousand USD


def marxan(name, params, writes=None, iters=None):
    body = f'''
[[nodes]]
id = "marxan"
op = "prioritize.marxan"
inputs = {{ features = "features.stack", cost = "cost.map" }}
params = {{ prop = {PROP}, num_reps = 10, num_iters = {iters or ITERS}, seed = 1{params} }}

[outputs.summary]
from = "marxan.summary"
[outputs.penalties]
from = "marxan.penalties"
'''
    w = sp.run(SLUG, name, body, writes or {"marxan.map": "best.tif", "marxan.frequency_map": "freq.tif"})
    return w, sp.table(w, "summary")


def best_run(summary):
    return min(summary, key=lambda r: r["score"])


# ------------------------------------------------------------------------------------------------ a toy
def toy():
    """Two selections of eight cells: scattered and compact, with their boundaries."""
    scattered = [(0, 0), (0, 4), (1, 2), (2, 5), (3, 1), (4, 3), (5, 0), (5, 5)]
    compact = [(2, 1), (2, 2), (2, 3), (2, 4), (3, 1), (3, 2), (3, 3), (3, 4)]
    fig, axs = plt.subplots(1, 2, figsize=(7.6, 3.6), dpi=150)
    for ax, cells, title in ((axs[0], scattered, "Scattered: boundary 32"), (axs[1], compact, "Compact: boundary 12")):
        s = set(cells)
        for r in range(6):
            for c in range(6):
                col = sp.SELECTED if (r, c) in s else sp.UNSELECTED
                ax.add_patch(plt.Rectangle((c, r), 1, 1, facecolor=col, edgecolor="white", lw=1.2))
        # the boundary: edges between a selected cell and an unselected one or the outside
        n = 0
        for (r, c) in s:
            for dr, dc, seg in ((-1, 0, ((c, r), (c + 1, r))), (1, 0, ((c, r + 1), (c + 1, r + 1))),
                                (0, -1, ((c, r), (c, r + 1))), (0, 1, ((c + 1, r), (c + 1, r + 1)))):
                if (r + dr, c + dc) not in s:
                    n += 1
                    ax.plot([seg[0][0], seg[1][0]], [seg[0][1], seg[1][1]], color=figs.EMBER, lw=2.4,
                            solid_capstyle="round")
        print(title, "edges", n)
        ax.set_xlim(-0.1, 6.1)
        ax.set_ylim(6.1, -0.1)
        ax.set_aspect("equal")
        ax.set_axis_off()
        ax.set_title(title, loc="left", fontsize=11)
    fig.tight_layout()
    sp.save(fig, OUT / "boundary-toy.png")


# ------------------------------------------------------------------------------------------------ SPF
def spf():
    rows = []
    for v in (None, 2, 5, 10, 20, 50):
        p = "" if v is None else f", spf = {v}"
        w, s = marxan(f"spf_{v}", p, {"marxan.map": "best.tif"})
        b = best_run(s)
        missed = [int(r["missing"]) for r in s]
        rows.append({"spf": "1 (default)" if v is None else str(v), "Best run (M USD)": round(b["cost"] / 1000, 1),
                     "Targets missed by the best run": int(b["missing"]),
                     "Runs meeting every target": sum(m == 0 for m in missed)})
        print(rows[-1])
    sp.write_csv(DAT / "spf.csv", rows)
    return rows


# ------------------------------------------------------------------------------------------------ iterations
def iterations():
    rows = []
    for it in (100_000, 500_000, 2_000_000, 10_000_000, 50_000_000):
        w, s = marxan(f"it_{it}", ", spf = 10", {"marxan.map": "best.tif"}, iters=it)
        costs = [r["cost"] for r in s]
        b = best_run(s)
        rows.append({"iterations": it / 1e6, "Best run": round(b["cost"] / 1000, 2),
                     "Worst run": round(max(costs) / 1000, 2), "Proven optimum": round(OPTIMUM / 1000, 2)})
        print(rows[-1], "missing", [int(r["missing"]) for r in s])
    sp.write_csv(DAT / "iterations.csv", rows)
    return rows


# ------------------------------------------------------------------------------------------------ BLM
BLMS = [0, 0.002, 0.005, 0.01, 0.02, 0.05, 0.1]


def blm():
    rows = []
    for v in BLMS:
        w, s = marxan(f"blm_{v}", f", spf = 10, blm = {v}")
        b = best_run(s)
        sel = sp.grid(w / "best.tif")
        sp.plan_map(sel, OUT / f"blm_{v}.png", legend=[], width=760)
        freq = sp.grid(w / "freq.tif")
        if v == 0.01:
            figs.raster(None, OUT / "frequency.png", array=np.where(sp.land(), freq, np.nan), cmap="viridis",
                        vmin=0, vmax=1, unit="Share of the 10 runs selecting the cell")
            sp.plan_map(sel, OUT / "best.png")
            np.save(figs.WEB / ".cache" / "work" / sp.CATEGORY / SLUG / "freq.npy", freq)
        land = sp.land()
        always = int(np.nansum(freq[land] >= 0.999))
        some = int(np.nansum((freq[land] > 0) & (freq[land] < 0.999)))
        r = {"blm": v, "cost": b["cost"] / 1000, "boundary": b["connectivity"] / 1000, "units": int(b["planning_units"]),
             "missing": int(b["missing"]), "always": always, "some": some}
        rows.append(r)
        print({k: (round(x, 2) if isinstance(x, float) else x) for k, x in r.items()})
    # the BLM as text ("none" for 0) so that the chart spaces the values evenly
    sp.write_csv(DAT / "blm.csv", [{"blm": str(r["blm"]) if r["blm"] else "none",
                                    "Cost (% above the optimum)": round(100 * (r["cost"] * 1000 / OPTIMUM - 1), 1),
                                    "Boundary (% of BLM 0)": round(100 * r["boundary"] / rows[0]["boundary"], 1)}
                                   for r in rows])
    sp.write_csv(DAT / "tradeoff.csv", [{"Boundary (km)": round(r["boundary"]), "Cost (million USD)": round(r["cost"], 2)}
                                        for r in rows])
    return rows


def versus_optimum():
    w, s = marxan("vs_opt", ", spf = 10, blm = 0")
    rows = [{"run": int(r["run"]), "Marxan run": round(r["cost"] / 1000, 2), "Proven optimum": round(OPTIMUM / 1000, 2)}
            for r in s]
    sp.write_csv(DAT / "vs-optimum.csv", rows)
    print("marxan runs", [r["Marxan run"] for r in rows], "missing", [int(r["missing"]) for r in s])
    print("penalties", sp.table(w, "penalties"))
    return rows


# ------------------------------------------------------------------------------------------------ ProxIRR
def proxirr():
    w = sp.run(SLUG, "proxirr", '''
[[nodes]]
id = "irr"
op = "prioritize.proxirr"
inputs = { features = "features.stack" }
params = { prop = 0.3 }

[[nodes]]
id = "blocks"
op = "prioritize.coarsen"
inputs = { cost = "cost.map", features = "features.stack" }
params = { factor = 5 }

[[nodes]]
id = "irr5"
op = "prioritize.proxirr"
inputs = { features = "blocks.features" }
params = { prop = 0.3 }
''', {"irr.beta": "beta.tif", "irr.gamma": "gamma.tif", "irr5.beta": "beta5.tif", "irr5.gamma": "gamma5.tif",
      "blocks.cost": "cost5.tif"})
    beta, gamma = sp.grid(w / "beta.tif"), sp.grid(w / "gamma.tif")
    beta5, gamma5, cost5 = sp.grid(w / "beta5.tif"), sp.grid(w / "gamma5.tif"), sp.grid(w / "cost5.tif")
    m = sp.land()
    m5 = np.isfinite(cost5)

    def draw(arrs, png, titles, masks):
        fig, axs = plt.subplots(1, 2, figsize=(9.6, 3.3), dpi=150)
        for ax, a, t, mk in zip(axs, arrs, titles, masks):
            cm = plt.get_cmap("magma_r").copy()
            cm.set_bad((1, 1, 1, 0))
            vmax = float(np.nanmax(np.where(mk, a, np.nan)))
            im = ax.imshow(np.ma.masked_invalid(np.where(mk, a, np.nan)), cmap=cm, vmin=0, vmax=vmax,
                           interpolation="nearest")
            ax.set_axis_off()
            ax.set_title(t, loc="left", fontsize=11)
            cb = fig.colorbar(im, ax=ax, fraction=0.035, pad=0.02)
            cb.outline.set_visible(False)
            cb.ax.grid(False)
            cb.ax.tick_params(labelsize=8.5)
        fig.tight_layout()
        sp.save(fig, png)

    draw((beta, beta5), OUT / "proxirr.png", ("beta, cells of 1 km²", "beta, blocks of 5 x 5 km"), (m, m5))
    print("cells: beta max", np.nanmax(beta[m]), "gamma max", np.nanmax(gamma[m]))
    print("blocks:", int(m5.sum()), "beta max", np.nanmax(beta5[m5]), "beta>0.5", int(np.nansum(beta5[m5] > 0.5)),
          "beta>0.9", int(np.nansum(beta5[m5] > 0.9)), "gamma max", np.nanmax(gamma5[m5]))
    f = sp.features()
    for k in sp.FEATURES:
        print(k, "max beta in its cells", round(float(np.nanmax(np.where(np.nan_to_num(f[k]) > 0, beta, 0))), 3))
    try:
        freq = np.load(figs.WEB / ".cache" / "work" / sp.CATEGORY / SLUG / "freq.npy")
        ok = m & np.isfinite(freq)
        from scipy.stats import spearmanr
        print("spearman beta vs frequency", spearmanr(beta[ok], freq[ok]))
        # the frequency in blocks: the mean of the cells' frequencies
        h, wd = freq.shape
        fb = np.nanmean(np.where(m, freq, np.nan).reshape(h // 5, 5, wd // 5, 5), axis=(1, 3))
        ok5 = m5 & np.isfinite(fb)
        print("spearman beta5 vs block frequency", spearmanr(beta5[ok5], fb[ok5]))
    except FileNotFoundError:
        pass


if __name__ == "__main__":
    import sys
    for t in sys.argv[1:] or ["toy", "spf", "iterations", "blm", "versus_optimum", "proxirr"]:
        print(t, globals()[t]())
