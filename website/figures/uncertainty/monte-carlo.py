"""Figures of 'Monte Carlo error propagation': the template 'Carbon and its uncertainty' run with 10 to 5,000 draws
(the maps of the standard deviation, and how its summaries settle as draws grow, over 10 seeds each), the draws of
one cell, the delta method against Monte Carlo, and the uncertainty of the total carbon (cells' errors that are
not independent), checked by drawing the same parameters with numpy."""

import pathlib
import shutil
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[2] / "tools"))
import figs  # noqa: E402
import numpy as np  # noqa: E402
import matplotlib.pyplot as plt  # noqa: E402

SLUG = "uncertainty/monte-carlo"
work = figs.workdir(SLUG)
out = figs.img_dir(SLUG)
data = figs.data_dir(SLUG)

BASE = figs.template("carbon-and-its-uncertainty")
assert "n = 500, seed = 1" in BASE


def variant(n, seed):
    m = BASE.replace("n = 500, seed = 1", f"n = {n}, seed = {seed}")
    return figs.add_writes(m, {"carbon.mean": "mean.tif", "carbon.sd": "sd.tif", "carbon.quantiles": "q.tif",
                               "canopy.map": "canopy.tif", "interval.low": "low.tif", "interval.high": "high.tif"})


CELL = (50, 50)  # a cell followed through the runs
NS = [10, 20, 50, 100, 200, 500, 1000, 2000, 5000]
SEEDS = range(1, 11)
rows, spread = [], []
for n in NS:
    vals = []
    for s in SEEDS:
        d = figs.workdir(f"{SLUG}/n{n}_s{s}")
        figs.run(variant(n, s), d)
        sd, _ = figs.read(d / "sd.tif")
        mean, _ = figs.read(d / "mean.tif")
        q, _ = figs.read(d / "q.tif", 2)
        vals.append((float(sd.mean()), float(sd[CELL]), float(mean[CELL]), float(q[CELL])))
        if s == 1:
            figs.raster(d / "sd.tif", out / f"sd_{n}.png", cmap="magma", vmin=0, vmax=16, unit="sd of carbon (t/ha)",
                        width=620)
    v = np.array(vals)
    rows.append({"draws": n, "sd_mean": round(v[0, 0], 3), "cell_sd": round(v[0, 1], 3), "cell_mean": round(v[0, 2], 3),
                 "cell_q975": round(v[0, 3], 3)})
    spread.append({"draws": n, "n": f"n={n}", "low": round(v[:, 1].min(), 3), "high": round(v[:, 1].max(), 3),
                   "seed1": round(v[0, 1], 3), "q_low": round(v[:, 3].min(), 2), "q_high": round(v[:, 3].max(), 2)})
    print(n, rows[-1], spread[-1])
figs.write_csv(data / "draws.csv", rows)
figs.write_csv(data / "spread.csv", spread)

# ------------------------------------------------------------------ the maps of the template (500 draws, seed 1)
d500 = work.parent / "monte-carlo" / "n500_s1"
figs.raster(d500 / "canopy.tif", out / "canopy.png", cmap="YlGn", unit="canopy cover (0-1)", width=620)
figs.raster(d500 / "mean.tif", out / "mean.png", cmap="YlGn", unit="carbon, mean (t/ha)", width=620)
figs.raster(d500 / "q.tif", out / "q_low.png", band=1, cmap="YlGn", vmin=0, vmax=160, unit="2.5% quantile (t/ha)", width=620)
figs.raster(d500 / "q.tif", out / "q_high.png", band=2, cmap="YlGn", vmin=0, vmax=160, unit="97.5% quantile (t/ha)", width=620)
canopy, _ = figs.read(d500 / "canopy.tif")
mean, _ = figs.read(d500 / "mean.tif")
sd, _ = figs.read(d500 / "sd.tif")
print("canopy", canopy.min(), canopy.max(), "mean", mean.min(), mean.max(), "sd", sd.min(), sd.max())
rel = sd / mean * 100
print("relative sd", float(rel.min()), float(rel.max()), "at cell", float(rel[CELL]))

# ------------------------------------------------------------------ the draws of one cell, reproduced with numpy
c = float(canopy[CELL])
params = np.array([[250, 25], [1.5, 0.15], [0.47, 0.02]])
rng = np.random.default_rng(1)
z = rng.standard_normal(500 * 3).reshape(500, 3)
draws = params[:, 0] + z * params[:, 1]
cell = draws[:, 0] * c ** draws[:, 1] * draws[:, 2]
print("cell", CELL, "canopy", c, "numpy mean", cell.mean(), "terraflux", float(mean[CELL]), "numpy sd", cell.std(ddof=1),
      "terraflux", float(sd[CELL]))
(work / "check.txt").write_text(f"canopy {c}\nnumpy {cell.mean()} {cell.std(ddof=1)}\nterraflux {float(mean[CELL])} {float(sd[CELL])}\n",
                                encoding="utf-8")


def hist(ax):
    ax.hist(cell, bins=30, color=figs.FOREST, alpha=0.8, edgecolor="white")
    lo, hi = np.quantile(cell, [0.025, 0.975])
    for x, lab in ((cell.mean(), "mean"), (lo, "2.5%"), (hi, "97.5%")):
        ax.axvline(x, color=figs.EMBER if lab == "mean" else figs.INK2, lw=1.5, ls="-" if lab == "mean" else "--")
        ax.text(x, ax.get_ylim()[1] * 0.95, f" {lab}\n {x:.1f}", fontsize=8, va="top", color=figs.INK)
    ax.set_xlabel("carbon of the cell (t/ha)")
    ax.set_ylabel("draws")


figs.chart(out / "cell_draws.png", hist, width=900, height=380)

# ------------------------------------------------------------------ the delta method against Monte Carlo (5,000 draws)
DELTA = BASE.replace('''[[nodes]]
id = "interval"''', '''[[nodes]]
id = "delta"
op = "uncertainty.delta"
inputs = { canopy = "canopy.map" }
params = { expression = "a * canopy ^ b * fraction", parameters = [["a", 250, 25], ["b", 1.5, 0.15], ["fraction", 0.47, 0.02]] }

[[nodes]]
id = "interval"''').replace("n = 500, seed = 1", "n = 5000, seed = 1")
dd = figs.workdir(f"{SLUG}/delta")
figs.run(figs.add_writes(DELTA, {"delta.sd": "delta_sd.tif", "delta.value": "delta_value.tif", "carbon.sd": "mc_sd.tif",
                                 "carbon.mean": "mc_mean.tif", "canopy.map": "canopy.tif"}), dd)
ds, _ = figs.read(dd / "delta_sd.tif")
ms, _ = figs.read(dd / "mc_sd.tif")
dv, _ = figs.read(dd / "delta_value.tif")
mm, _ = figs.read(dd / "mc_mean.tif")
cv, _ = figs.read(dd / "canopy.tif")
order = np.argsort(cv.compressed())
pts = []
cc = cv.compressed()[order]
for k in np.linspace(0, len(cc) - 1, 60).astype(int):
    pts.append({"canopy": round(float(cc[k]), 4), "monte_carlo": round(float(ms.compressed()[order][k]), 3),
                "delta": round(float(ds.compressed()[order][k]), 3)})
figs.write_csv(data / "delta.csv", pts)
print("delta vs MC sd: max abs diff", float(np.abs(ds - ms).max()), "mean rel diff %", float((np.abs(ds - ms) / ms).mean() * 100))
print("value at the means vs MC mean, mean diff", float((dv - mm).mean()))

# ------------------------------------------------------------------ the total carbon: errors that are not independent
SUM = BASE.replace("n = 500, seed = 1", "n = 5000, seed = 1") + '''
[[nodes]]
id = "total_independent"
op = "uncertainty.sum_sd"
inputs = { sd = "carbon.sd" }
params = { method = "quadratic" }

[[nodes]]
id = "total_shared"
op = "uncertainty.sum_sd"
inputs = { sd = "carbon.sd" }
params = { method = "linear" }

[outputs.independent]
from = "total_independent.sd"

[outputs.shared]
from = "total_shared.sd"
'''
ds2 = figs.workdir(f"{SLUG}/total")
figs.run(figs.add_writes(SUM, {"carbon.mean": "mean.tif", "canopy.map": "canopy.tif"}), ds2)
print(sorted(p.name for p in (ds2 / "output").iterdir()))
txt = {p.stem: p.read_text(encoding="utf-8").strip() for p in (ds2 / "output").iterdir()}
print(txt)
mean5, _ = figs.read(ds2 / "mean.tif")
cv = canopy.filled(np.nan).ravel()
rng = np.random.default_rng(1)
z = rng.standard_normal(5000 * 3).reshape(5000, 3)
dr = params[:, 0] + z * params[:, 1]
totals = np.array([np.nansum(a * cv ** b * f) for a, b, f in dr])
print("total: mean", totals.mean(), "Terraflux mean map summed", float(mean5.sum()), "sd of the total", totals.std(ddof=1),
      "q", np.quantile(totals, [0.025, 0.975]))
(work / "total.txt").write_text(f"{txt}\nnumpy total mean {totals.mean()} sd {totals.std(ddof=1)} q {np.quantile(totals, [0.025, 0.975])}\n"
                                f"sum of mean {float(mean5.sum())}\n", encoding="utf-8")


def thist(ax):
    ax.hist(totals / 1000, bins=40, color=figs.WATER, alpha=0.8, edgecolor="white")
    m = totals.mean() / 1000
    for s_, lab, col in ((float(txt["independent"]) / 1000, "if the cells' errors were independent", figs.EMBER),
                         (float(txt["shared"]) / 1000, "if they are shared (linear sum)", figs.FOREST)):
        ax.errorbar([m], [ax.get_ylim()[1] * (0.55 if col == figs.EMBER else 0.4)], xerr=[[1.96 * s_], [1.96 * s_]],
                    fmt="o", color=col, capsize=4, label=f"mean ± 1.96 sd, {lab}")
    ax.set_xlabel("total carbon of the 10,000 cells (thousand t, cells of 1 ha)")
    ax.set_ylabel("draws")
    ax.legend(fontsize=8, loc="upper left")


figs.chart(out / "total.png", thist, width=900, height=400)
