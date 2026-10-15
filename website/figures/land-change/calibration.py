"""Figures of "Calibration and many runs" (land-change/calibration-and-many-runs).

Dinamica EGO's model of Sibuyan (`terraflux new dinamica`, the ranges of part 1):

1. 50 replicates (`terraflux submit --replicates 50`): the share of the runs in which each cell changed, the
   distribution of the forest left in 2011 and of the figure of merit;
2. three scenarios (`terraflux submit --scenarios`);
3. a calibration of the isometry and the Expander's share (`terraflux calibrate`, grid, similarity in 5 x 5,
   three seeds each), and the same with the figure of merit;
4. what `--param x=0:1` (whole numbers) tries, against `0.0:1.0`.

    python website/figures/land-change/calibration.py
"""

import json
import pathlib
import sys
import time

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import dinamica_common as dc  # noqa: E402
from dinamica_common import figs, np, plt  # noqa: E402

SLUG = "land-change/calibration-and-many-runs"
out = figs.img_dir(SLUG)
data = figs.data_dir(SLUG)
t0, t1 = dc.landuse()
w = dc.sibuyan("calibration")
model = dc.woe_settings(dc.new_dinamica(w, "sibuyan.toml"))
(w / "sibuyan.toml").write_text(model, encoding="utf-8")


def timed(*args):
    t = time.perf_counter()
    o = dc.cli(*args, cwd=w)
    return time.perf_counter() - t, o


# ---- 1. replicates
s, o = timed("submit", "sibuyan.toml", "--replicates", 50, "--out", "replicates", "--workers", 4)
print(f"50 replicates, 4 workers: {s:.1f} s")
print(o[-1500:])
print("folders:", sorted(p.name for p in (w / "replicates").iterdir())[:5], "...")
tasks = figs.read_csv(w / "replicates" / "tasks.csv")
print("tasks.csv columns:", list(tasks[0].keys()), tasks[0])
runs = sorted((w / "replicates").glob("seed_*"), key=lambda p: int(p.name.split("_")[1]))
changed = np.zeros(t0.shape)
forest = []
for r in runs:
    m = dc.read_stack(r / "maps.tif")[-1]
    m[m == 255] = np.nan
    changed += (m != t0) & np.isfinite(t0)
    forest.append(float((m == 1).sum()))
share = np.where(np.isfinite(t0), changed / len(runs), np.nan)
print("cells that changed in every run:", int((share == 1).sum()), "in some:", int(((share > 0) & (share < 1)).sum()),
      "in none:", int((share == 0).sum()), "observed change:", int(((t1 != t0) & np.isfinite(t0)).sum()))
figs.raster(None, out / "change_frequency.png", array=np.ma.masked_invalid(share),
            cmap="magma_r", vmin=0, vmax=1, unit="share of the 50 runs", width=820,
            title="How often each cell changed, 50 replicates")
hist = np.histogram(share[np.isfinite(share) & (share > 0)], bins=10, range=(0, 1))[0]
figs.write_csv(data / "frequency.csv", [{"share of runs": f"{i * 10}-{i * 10 + 10}%", "cells": int(h)}
                                        for i, h in enumerate(hist)])
print("forest in 2011 over the runs: min", min(forest), "max", max(forest), "observed", (t1 == 1).sum())

# the figure of merit of every replicate (validate.agreement on each)
VAL = """format = "terraflux/1"

[inputs.t0]
kind = "map"
path = "sibuyan_landuse.tif"
band = "1997"

[inputs.t1]
kind = "map"
path = "sibuyan_landuse.tif"
band = "2011"

[[nodes]]
id = "simulated"
op = "raster.read"
params = { path = "SIM", band = "14" }

[[nodes]]
id = "agreement"
op = "validate.agreement"
inputs = { start = "$t0", observed = "$t1", simulated = "simulated.map" }

[[nodes]]
id = "similarity"
op = "validate.similarity"
inputs = { start = "$t0", observed = "$t1", simulated = "simulated.map" }
params = { windows = [5] }

[outputs.agreement]
from = "agreement.table"

[outputs.similarity]
from = "similarity.table"
"""
rows = []
for r in runs:
    o = dc.run(VAL.replace("SIM", (r / "maps.tif").relative_to(w).as_posix()), w, name="val.toml", out="val")
    a = figs.read_csv(o / "agreement.csv")[0]
    sm = figs.read_csv(o / "similarity.csv")[0]
    rows.append({"seed": int(r.name.split("_")[1]), "fom": round(float(a["figure_of_merit"]), 4),
                 "similarity": round(float(sm["minimum"]), 4)})
figs.write_csv(data / "replicates.csv", rows)
f = np.array([r["fom"] for r in rows])
sim = np.array([r["similarity"] for r in rows])
print(f"fom over 50 runs: mean {f.mean():.3f} sd {f.std():.3f} min {f.min():.3f} max {f.max():.3f}")
print(f"similarity 5x5: mean {sim.mean():.3f} sd {sim.std():.3f} min {sim.min():.3f} max {sim.max():.3f}")


def hist_png(png, vals, xlabel, color):
    def draw(ax):
        ax.hist(vals, bins=12, color=color, edgecolor="white")
        ax.axvline(np.mean(vals), color=figs.INK, lw=1, ls=(0, (3, 2)))
        ax.set_xlabel(xlabel)
        ax.set_ylabel("Runs")
    figs.chart(png, draw, width=560, height=330)


hist_png(out / "fom_hist.png", f, "Figure of merit", figs.FOREST)
hist_png(out / "similarity_hist.png", sim, "Fuzzy similarity (5 x 5)", figs.EMBER)

# ---- 2. scenarios
(w / "scenarios.toml").write_text("""[[scenario]]
name = "as_observed"

[[scenario]]
name = "compact"
set = { "patches.isometry" = 2.5, "patches.expansion" = 0.95 }

[[scenario]]
name = "scattered"
set = { "patches.isometry" = 1.0, "patches.expansion" = 0.1 }
""", encoding="utf-8")
s, o = timed("submit", "sibuyan.toml", "--scenarios", "scenarios.toml", "--out", "scenarios", "--workers", 3)
print(f"3 scenarios: {s:.1f} s")
CROP = (slice(4, 62), slice(8, 84))
for name in ("as_observed", "compact", "scattered"):
    m = dc.read_stack(w / "scenarios" / name / "maps.tif")[-1]
    m[m == 255] = np.nan
    dc.draw_change(t0[CROP], m[CROP], out / f"scenario_{name}.png", legend=False, width=520)
    from scipy import ndimage
    new = (m != t0) & np.isfinite(t0)
    lab, k = ndimage.label(new, structure=np.ones((3, 3)))
    print(name, "changed", int(new.sum()), "clumps of change", k)
dc.legend_strip(out / "change_legend.png", [("Unchanged", dc.UNCHANGED)] + [(f"to {n.lower()}", c) for kk, (n, c) in
                                                                             dc.SIB.items() if kk in (2, 3, 4)],
                width=640)

# ---- 3. calibration
s, o = timed("calibrate", "sibuyan.toml", "--param", "patches.isometry=0.5:2.5", "--param",
             "patches.expansion=0.1:0.9", "--observed", "sibuyan_landuse.tif:2011", "--metric", "similarity:5",
             "--seeds", "1,2,3", "--out", "calibration", "--workers", 4)
print(f"calibration (25 candidates x 3 seeds): {s:.1f} s")
print(o[-2000:])
cal = figs.read_csv(w / "calibration" / "calibration.csv")
print("calibration.csv columns:", list(cal[0].keys()))
print(cal[:3])
print("best.json:", (w / "calibration" / "best.json").read_text()[:600])
print("files:", sorted(p.name for p in (w / "calibration").iterdir())[:12], (w / "sibuyan_calibrated.toml").exists())
iso = sorted({float(r["patches.isometry"]) for r in cal})
exp = sorted({float(r["patches.expansion"]) for r in cal})
grid = np.full((len(exp), len(iso)), np.nan)
for r in cal:
    grid[exp.index(float(r["patches.expansion"])), iso.index(float(r["patches.isometry"]))] = float(r["similarity:5"])
fig, ax = plt.subplots(figsize=(6.4, 4.2), dpi=150)
im = ax.imshow(grid, origin="lower", cmap="viridis", aspect="auto")
for i in range(len(exp)):
    for j in range(len(iso)):
        ax.text(j, i, f"{grid[i, j]:.3f}", ha="center", va="center", fontsize=8.5,
                color="white" if grid[i, j] < np.nanmean(grid) else figs.INK)
ax.set_xticks(range(len(iso)), [f"{v:g}" for v in iso])
ax.set_yticks(range(len(exp)), [f"{v:g}" for v in exp])
ax.set_xlabel("Isometry (patches.isometry)")
ax.set_ylabel("Expander's share (patches.expansion)")
ax.grid(False)
cb = fig.colorbar(im, ax=ax)
cb.set_label("Fuzzy similarity, 5 x 5 (mean of 3 seeds)")
cb.outline.set_visible(False)
fig.tight_layout()
fig.savefig(out / "calibration_grid.png", dpi=150)
plt.close(fig)
figs._shrink(out / "calibration_grid.png")
figs.write_csv(data / "calibration.csv", [{k: v for k, v in r.items()} for r in cal])

# the same with the figure of merit
s, o = timed("calibrate", "sibuyan.toml", "--param", "patches.isometry=0.5:2.5", "--param",
             "patches.expansion=0.1:0.9", "--observed", "sibuyan_landuse.tif:2011", "--metric", "fom",
             "--seeds", "1,2,3", "--out", "calibration_fom", "--workers", 4)
cf = figs.read_csv(w / "calibration_fom" / "calibration.csv")
print("by the figure of merit, best:", cf[:3])

# ---- 4. whole numbers
s, o = timed("calibrate", "sibuyan.toml", "--param", "patches.expansion=0:1", "--observed",
             "sibuyan_landuse.tif:2011", "--out", "calibration_int", "--workers", 2)
print("expansion=0:1 tries:", [r["patches.expansion"] for r in figs.read_csv(w / "calibration_int" / "calibration.csv")])
print("done")
