"""Figures of "Taking a stock" (land-change/taking-a-stock): Dinamica EGO's Patcher of a continuous state
(dinamica.continuous_patcher) logging the timber of a forest drawn by chance, beside a road, for 30 years in a
control.repeat loop that carries the stock and the probability, with a quota per year and regrowth by map algebra.

    python website/figures/land-change/taking-a-stock.py
"""

import pathlib
import sys
import time

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[2] / "tools"))
import figs  # noqa: E402
import numpy as np  # noqa: E402
from matplotlib.patches import Patch, Rectangle  # noqa: E402

plt = figs.plt
SLUG = "land-change/taking-a-stock"
out = figs.img_dir(SLUG)
data = figs.data_dir(SLUG)

# the model of the page (the last graph), as it is shown there
MODEL = """format = "terraflux/1"

[inputs.quota]
kind = "value"
value = 15000

[inputs.seed]
kind = "value"
value = 1

[[nodes]]
id = "land"
op = "nlm.random_cluster"
params = { rows = 200, columns = 200, resolution = 100, p = 0.55, weights = [0.8, 0.2], seed = 7 }
note = "1 forest, 2 cleared land; cells of 100 m (1 ha)."

[[nodes]]
id = "richness"
op = "nlm.gaussian_field"
params = { rows = 200, columns = 200, resolution = 100, range = 25, seed = 8 }

[[nodes]]
id = "road"
op = "nlm.distance_gradient"
params = { rows = 200, columns = 200, resolution = 100, origin = [60, 60, 1, 200], nlmr_compat = false, rescale = false }
note = "The distance to a road along column 60, in metres."

[[nodes]]
id = "timber"
op = "map.calc"
inputs = { land = "land.map", r = "richness.map" }
params = { expression = "if land == 1 then 10 + 30 * r else 0" }
note = "Commercial timber, m3 per hectare."

[[nodes]]
id = "access"
op = "map.calc"
inputs = { timber = "timber.map", d = "road.map" }
params = { expression = "if timber > 0 then exp(-d / 3000) else 0" }
note = "The probability: high beside the road."

[[nodes]]
id = "years"
op = "control.repeat"
inputs = { times = 30, stock = "timber.map", probability = "access.map" }

[nodes.body]
carry = { stock = "regrowth.map", probability = "logging.probability" }
collect = { taken = "logging.taken", info = "logging.info", stocks = "regrowth.map" }

[[nodes.body.nodes]]
id = "logging"
op = "dinamica.continuous_patcher"
inputs = { stock = "@stock", probability = "@probability", quota = "$quota", step = "@step", seed = "$seed" }
params = { mean = 25, variance = 200, isometry = 1.5 }

[[nodes.body.nodes]]
id = "regrowth"
op = "map.calc"
inputs = { s = "logging.stock", full = "timber.map" }
params = { expression = "min(full, s + full / 30)" }
note = "The timber grows back in 30 years."
"""

WRITES = {"years.taken": "taken.tif", "years.stocks": "stocks.tif", "timber.map": "timber.tif",
          "access.map": "access.tif", "road.map": "road.tif", "land.map": "land.tif", "years.info": "info.csv"}

LOG = {0: ("Cleared land", "#e9e4d3"), 1: ("Forest", "#3b8a5a"), 2: ("Logged before", "#c9b27c"),
       3: ("Logged this year", "#e8901f"), 4: ("Road", "#1c2420")}


def run(model: str, name: str, *args) -> pathlib.Path:
    work = figs.workdir(f"land-change/taking-a-stock/{name}")
    figs.run(figs.add_writes(model, WRITES), work, *args)
    return work


def arr(path, band=1):
    return figs.read(path, band)[0].filled(np.nan)


def info(work):
    return [{k: float(v) for k, v in r.items()} for r in figs.read_csv(work / "output" / "info.csv")]


def classes(land, road, logged_before, logged_now):
    c = np.where(land == 1, 1, 0).astype(float)
    c[logged_before] = 2
    c[logged_now] = 3
    c[road == 0] = 4
    return c


def legend_strip(png, items, width=900):
    fig = plt.figure(figsize=(width / 150, 0.42), dpi=150)
    handles = [Patch(facecolor=c, edgecolor="#00000022", label=n) for n, c in items]
    fig.legend(handles=handles, loc="center", ncol=len(items), fontsize=9.5, handlelength=1.1, handleheight=1.1,
               columnspacing=1.4)
    fig.savefig(png, dpi=150)
    plt.close(fig)
    figs._shrink(png)


# ---------------------------------------------------------------------------------------------- the base run

t0 = time.time()
base = run(MODEL, "base")
print(f"base run: {time.time() - t0:.2f} s")
land, road = arr(base / "land.tif"), arr(base / "road.tif")
timber, access = arr(base / "timber.tif"), arr(base / "access.tif")
n = figs.bands(base / "taken.tif")
taken = [arr(base / "taken.tif", b) for b in range(1, n + 1)]
stocks = [arr(base / "stocks.tif", b) for b in range(1, n + 1)]
rows = info(base)
forest = land == 1
print(f"forest {forest.sum()} ha of {forest.size}; stock {np.nansum(timber):.0f} m3; "
      f"mean {np.nanmean(timber[forest]):.1f} m3/ha")

figs.raster(None, out / "timber.png", array=np.where(forest, timber, np.nan), cmap="YlGn", vmin=0, vmax=40,
            unit="m³ per hectare", width=620)
figs.raster(None, out / "access.png", array=np.where(forest, access, np.nan), cmap="magma", vmin=0, vmax=1,
            unit="probability", width=620)

# step 1, and the 30 years every two years
legend_strip(out / "legend.png", [LOG[k] for k in (0, 1, 2, 3, 4)], width=900)
first = taken[0] > 0
figs.raster(None, out / "step1.png", array=classes(land, road, np.zeros_like(first), first), palette=LOG,
            legend=False, width=620)
print("step 1:", rows[0], "mean patch", first.sum() / rows[0]["patches"])

done = np.zeros_like(first)
steps, frames = [], []
total0 = float(np.nansum(timber))
for k in range(n):
    now = taken[k] > 0
    d = road[now]
    r = rows[k]
    steps.append({"step": k + 1, "distance": round(float(d.mean()) / 1000, 3), "patches": int(r["patches"]),
                  "standing": round(float(np.nansum(stocks[k])) / 1000, 1),
                  "logged": int((done | now).sum())})
    if (k + 1) % 2 == 0 or k == 0:
        figs.raster(None, out / f"year_{k + 1}.png", array=classes(land, road, done, now), palette=LOG,
                    legend=False, width=620)
        frames.append((k + 1, r, d, int((done | now).sum())))
    done |= now
figs.write_csv(data / "years.csv", steps)
for k, r, d, logged in frames:
    print(f"  - {{ value: {k}, src: /img/tutorials/land-change/taking-a-stock/year_{k}.png, caption: \"Year {k}\", "
          f"stats: {{ \"Taken\": \"{r['taken']:,.0f} m³\", \"Patches\": \"{int(r['patches'])}\", "
          f"\"Mean distance from the road\": \"{d.mean() / 1000:.1f} km\", \"Logged so far\": \"{logged:,} ha\" }} }}")
print("standing stock (thousand m3) start", total0 / 1000, "end", steps[-1]["standing"])
print("distance by year", [s["distance"] for s in steps])

# ---------------------------------------------------------------------------------------------- a toy, explained

TOY = """format = "terraflux/1"

[[nodes]]
id = "g1"
op = "nlm.gaussian_field"
params = { rows = 24, columns = 24, range = 6, seed = 3 }

[[nodes]]
id = "g2"
op = "nlm.gaussian_field"
params = { rows = 24, columns = 24, range = 8, seed = 4 }

[[nodes]]
id = "stock"
op = "map.calc"
inputs = { g = "g1.map" }
params = { expression = "round(2 + 8 * g)" }

[[nodes]]
id = "prob"
op = "map.calc"
inputs = { g = "g2.map" }
params = { expression = "g * g" }

[[nodes]]
id = "cut"
op = "dinamica.continuous_patcher"
inputs = { stock = "stock.map", probability = "prob.map", quota = 150, seed = 2 }
params = { mean = 6, variance = 4, isometry = 1.5, unit = "cells", prune_factor = 3 }
"""
tw = figs.workdir("land-change/taking-a-stock/toy")
figs.run(figs.add_writes(TOY, {"stock.map": "stock.tif", "prob.map": "prob.tif", "cut.patch": "patch.tif",
                               "cut.taken": "taken.tif", "cut.info": "info.csv"}), tw)
ts, tp, tpatch = arr(tw / "stock.tif"), arr(tw / "prob.tif"), arr(tw / "patch.tif")
tinfo = info(tw)[0]
print("toy:", tinfo)
order = np.argsort(-tp.ravel(), kind="stable")
cum = np.cumsum(ts.ravel()[order])
cand = np.zeros(ts.size, bool)
cand[order[cum <= 3 * 150]] = True
cand = cand.reshape(ts.shape)

fig, axes = plt.subplots(1, 3, figsize=(10.5, 3.9), dpi=150)
for ax in axes:
    ax.set_axis_off()
im = axes[0].imshow(ts, cmap="YlGn", vmin=0, vmax=10)
for (i, j), v in np.ndenumerate(ts):
    axes[0].text(j, i, f"{v:.0f}", ha="center", va="center", fontsize=5, color="#1c2420")
axes[0].set_title("Stock of each cell", loc="left")
axes[1].imshow(tp, cmap="magma", vmin=0, vmax=1)
for (i, j), c in np.ndenumerate(cand):
    if c:
        axes[1].add_patch(Rectangle((j - 0.5, i - 0.5), 1, 1, fill=False, ec="#5ad1ff", lw=0.9))
axes[1].set_title("Probability; candidates outlined", loc="left")
bg = np.where(tpatch > 0, np.nan, ts)
axes[2].imshow(bg, cmap="Greys", vmin=-6, vmax=20)
ids = sorted(int(v) for v in np.unique(tpatch) if v > 0)
cols = [figs.CLASSES[i % len(figs.CLASSES)] for i in range(len(ids))]
from matplotlib import colors as mcolors  # noqa: E402
rgba = np.zeros(tpatch.shape + (4,))
for k, pid in enumerate(ids):
    rgba[tpatch == pid] = mcolors.to_rgba(cols[k])
axes[2].imshow(rgba)
for pid in ids:
    ii, jj = np.nonzero(tpatch == pid)
    axes[2].text(jj.mean(), ii.mean(), str(pid), ha="center", va="center", fontsize=8, fontweight="bold",
                 color="white")
axes[2].set_title(f"{len(ids)} patches take {tinfo['taken']:.0f} of a quota of 150", loc="left")
fig.tight_layout()
fig.savefig(out / "toy.png", dpi=150)
plt.close(fig)
figs._shrink(out / "toy.png")
print("toy candidates", int(cand.sum()), "cells; patch sizes", [int((tpatch == p).sum()) for p in ids])

# ---------------------------------------------------------------------------------------------- settings, step 1


def one_step(params: str, name: str):
    m = MODEL.replace("params = { mean = 25, variance = 200, isometry = 1.5 }", f"params = {{ {params} }}")
    assert m != MODEL or params == "mean = 25, variance = 200, isometry = 1.5"
    m = m.replace("times = 30", "times = 1")
    w = run(m, name)
    t = arr(w / "taken.tif") > 0
    return w, t, info(w)[0]


def clumps(mask):
    from scipy import ndimage

    lab, k = ndimage.label(mask, structure=np.ones((3, 3)))
    return k


def frame_line(value, src, stats):
    s = ", ".join(f'"{k}": "{v}"' for k, v in stats.items())
    print(f"  - {{ value: {value}, src: {src}, stats: {{ {s} }} }}")


print("mean patch size")
for mean in (5, 25, 100):
    w, t, r = one_step(f"mean = {mean}, variance = {mean * mean / 3:g}, isometry = 1.5", f"mean_{mean}")
    figs.raster(None, out / f"mean_{mean}.png", array=classes(land, road, np.zeros_like(t), t), palette=LOG,
                legend=False, width=620)
    frame_line(mean, f"/img/tutorials/land-change/taking-a-stock/mean_{mean}.png",
               {"Patches": int(r["patches"]), "Cells taken": f"{int(r['cells'])} ha",
                "Mean patch": f"{r['cells'] / r['patches']:.1f} ha"})

print("isometry")
for iso in (0, 0.5, 1, 1.5, 2):
    w, t, r = one_step(f"mean = 25, variance = 200, isometry = {iso}", f"iso_{iso}")
    figs.raster(None, out / f"iso_{iso}.png", array=classes(land, road, np.zeros_like(t), t), palette=LOG,
                legend=False, width=620)
    frame_line(iso, f"/img/tutorials/land-change/taking-a-stock/iso_{iso}.png",
               {"Patches": int(r["patches"]), "Clumps of logged cells": clumps(t), "Cells taken": f"{int(r['cells'])} ha"})

print("prune factor")
for pf in (1, 3, 10, 30, 100):
    w, t, r = one_step(f"mean = 25, variance = 200, isometry = 1.5, prune_factor = {pf}", f"prune_{pf}")
    figs.raster(None, out / f"prune_{pf}.png", array=classes(land, road, np.zeros_like(t), t), palette=LOG,
                legend=False, width=620)
    d = road[t]
    frame_line(pf, f"/img/tutorials/land-change/taking-a-stock/prune_{pf}.png",
               {"Mean distance from the road": f"{d.mean() / 1000:.1f} km",
                "Farthest logged cell": f"{d.max() / 1000:.1f} km", "Patches": int(r["patches"])})

# ---------------------------------------------------------------------------------------------- quotas, 30 years

print("quota")
qrows = []
for q in (5000, 15000, 30000, 45000):
    w = run(MODEL, f"quota_{q}", "--set", f"quota={q}")
    rr = info(w)
    tk = [arr(w / "taken.tif", b) > 0 for b in range(1, n + 1)]
    allt = np.logical_or.reduce(tk)
    met = sum(1 for r in rr if r["remaining"] <= 0)
    figs.raster(None, out / f"quota_{q}.png", array=classes(land, road, allt, np.zeros_like(allt)), palette=LOG,
                legend=False, width=620)
    frame_line(q, f"/img/tutorials/land-change/taking-a-stock/quota_{q}.png",
               {"Years the quota was met": f"{met} of 30", "Timber taken": f"{sum(r['taken'] for r in rr) / 1e3:,.0f} thousand m³",
                "Forest logged": f"{allt.sum() / forest.sum() * 100:.0f}%"})
    for k, r in enumerate(rr):
        if len(qrows) <= k:
            qrows.append({"step": k + 1})
        qrows[k][f"q{q}"] = round(r["taken"] / 1000, 2)
figs.write_csv(data / "quotas.csv", qrows)

# ---------------------------------------------------------------------------------------------- a cutting cycle

CYCLE = MODEL.replace('carry = { stock = "regrowth.map", probability = "logging.probability" }',
                      'carry = { stock = "regrowth.map", probability = "reopen.map" }').rstrip() + """

[[nodes.body.nodes]]
id = "reopen"
op = "map.calc"
inputs = { s = "regrowth.map", full = "timber.map", p = "logging.probability", p0 = "access.map" }
params = { expression = "if full > 0 and s >= full then p0 else p" }
note = "A cell may be logged again once its timber is back."
"""
assert CYCLE.count("reopen.map") == 1
crows = []
for name, m in (("depleted", MODEL), ("cycle", CYCLE)):
    m2 = m.replace("times = 30", "times = 80")
    w = run(m2, f"long_{name}", "--set", "quota=25000")
    rr = info(w)
    for k, r in enumerate(rr):
        if len(crows) <= k:
            crows.append({"step": k + 1})
        crows[k][name] = round(r["taken"] / 1000, 2)
    met = sum(1 for r in rr if r["remaining"] <= 0)
    print(f"{name}: quota met {met} of {len(rr)} years; last years", [round(r["taken"]) for r in rr[-5:]])
figs.write_csv(data / "cycle.csv", crows)
print("images:", sum(p.stat().st_size for p in out.glob("*.png")) / 1e6, "MB")
