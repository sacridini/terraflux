"""Figures of "Validating a simulation" (land-change/validating-a-simulation).

Dinamica EGO's model of Sibuyan (`terraflux new dinamica`, the ranges of part 1) simulated from 1997 to 2011 and
compared with the observed map of 2011 by Terraflux's validate.* operators; the same with two drivers only
(elevation and slope) and persistence (the map of 1997 as the "simulation") as the yardsticks; and, on Plum
Island (three dates), the model fitted on 1985-1991 and validated on 1991-1999. Plus two drawings: the four
outcomes of a cell on a toy grid, and the fuzzy membership of a near miss.

    python website/figures/land-change/validation.py
"""

import pathlib
import shutil
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import dinamica_common as dc  # noqa: E402
from dinamica_common import figs, np, plt  # noqa: E402

SLUG = "land-change/validating-a-simulation"
out = figs.img_dir(SLUG)
data = figs.data_dir(SLUG)
t0, t1 = dc.landuse()

VALIDATE = """format = "terraflux/1"

[inputs.t0]
kind = "map"
path = "START"
band = "BAND0"

[inputs.t1]
kind = "map"
path = "START"
band = "BAND1"

[inputs.simulated]
kind = "map"
path = "SIM"
band = "SIMBAND"

[[nodes]]
id = "agreement"
op = "validate.agreement"
inputs = { start = "$t0", observed = "$t1", simulated = "$simulated" }

[[nodes]]
id = "similarity"
op = "validate.similarity"
inputs = { start = "$t0", observed = "$t1", simulated = "$simulated" }
params = { windows = [1, 3, 5, 7, 9, 11] }

[[nodes]]
id = "three_map"
op = "validate.three_map"
inputs = { t0 = "$t0", t1 = "$t1", simulated = "$simulated" }
params = { factors = [1, 2, 4, 8, 16] }

[[nodes]]
id = "disagreement"
op = "validate.futures"
inputs = { reference = "$t1", simulated = "$simulated", original = "$t0" }

[[nodes]]
id = "kappa"
op = "validate.kappa"
inputs = { reference = "$t1", simulated = "$simulated" }

[outputs.agreement]
from = "agreement.table"

[outputs.similarity]
from = "similarity.table"

[outputs.fom]
from = "three_map.figure_of_merit"

[outputs.budget]
from = "three_map.budget"

[outputs.disagreement]
from = "disagreement.metrics"

[outputs.classes]
from = "disagreement.classes"

[outputs.kappa]
from = "kappa.table"
"""


def validate(w, start, b0, b1, sim, simband):
    m = (VALIDATE.replace("SIMBAND", str(simband)).replace("START", start).replace("BAND0", b0)
         .replace("BAND1", b1).replace('"SIM"', f'"{sim}"'))
    o = dc.run(m, w, name="validate.toml", out="validation")
    res = {"agreement": figs.read_csv(o / "agreement.csv")[0],
           "similarity": {int(float(r["window"])): float(r["minimum"]) for r in figs.read_csv(o / "similarity.csv")},
           "fom": {int(float(r["factor"])): float(r["overall"] or 0) for r in figs.read_csv(o / "fom.csv")},
           "disagreement": figs.read_csv(o / "disagreement.csv")[0],
           "kappa": figs.read_csv(o / "kappa.csv")[0],
           "budget": [r for r in figs.read_csv(o / "budget.csv") if r["level"] == "0"]}
    if (o / "classes.csv").exists():
        res["classes"] = figs.read_csv(o / "classes.csv")
    return res


def show(name, r):
    a = r["agreement"]
    print(f"{name}: overall {float(a['overall']):.4f} fom {float(a['figure_of_merit']):.4f} hits {a['hits']} "
          f"misses {a['misses']} false {a['false_alarms']} wrong {a['wrong_hits']}")
    print("   similarity", {k: round(v, 3) for k, v in r["similarity"].items()})
    print("   fom by block", {k: round(v, 3) for k, v in r["fom"].items()})
    d = r["disagreement"]
    print(f"   quantity {float(d['total_quantity'] or 0):.4f} allocation {float(d['total_allocation'] or 0):.4f} "
          f"kappa {float(d['kappa'] or 0):.3f} kappa simulation {d['kappasimulation']}")
    print("   budget", r["budget"])


# ---- the full model (13 drivers)
w = dc.sibuyan("valid_full")
full = dc.woe_settings(dc.new_dinamica(w))
dc.run(full, w)
r_full = validate(w, "sibuyan_landuse.tif", "1997", "2011", "output/maps.tif", 14)
show("13 drivers", r_full)
sim = dc.read_stack(w / "output" / "maps.tif")[-1]
sim[sim == 255] = np.nan

# ---- two drivers: elevation and slope
w2 = dc.sibuyan("valid_two")
import rasterio  # noqa: E402

with rasterio.open(w2 / "sibuyan_drivers.tif") as ds:
    names = list(ds.descriptions)
    keep = [names.index("elevation") + 1, names.index("slope") + 1]
    prof = ds.profile
    arr = ds.read(keep)
prof.update(count=2)
with rasterio.open(w2 / "two_drivers.tif", "w", **prof) as dst:
    dst.write(arr)
    dst.set_band_description(1, "elevation")
    dst.set_band_description(2, "slope")
dc.cli("new", "dinamica", "--t0", "sibuyan_landuse.tif:1997", "--t1", "sibuyan_landuse.tif:2011", "--drivers",
       "two_drivers.tif", "--steps", 14, "-o", "model.toml", cwd=w2)
two = dc.set_params((w2 / "model.toml").read_text(encoding="utf-8"), "woe",
                    {"ranges": {"elevation": 50, "slope": 2}})
dc.run(two, w2)
r_two = validate(w2, "sibuyan_landuse.tif", "1997", "2011", "output/maps.tif", 14)
show("2 drivers", r_two)
sim2 = dc.read_stack(w2 / "output" / "maps.tif")[-1]
sim2[sim2 == 255] = np.nan

# ---- persistence: nothing changes
wp = dc.sibuyan("valid_persistence")
r_pers = validate(wp, "sibuyan_landuse.tif", "1997", "2011", "sibuyan_landuse.tif", 1997)
show("persistence", r_pers)

# ---- chart data
figs.write_csv(data / "similarity.csv", [{"window": k, "13 drivers": round(r_full["similarity"][k], 4),
                                           "2 drivers": round(r_two["similarity"][k], 4)}
                                          for k in sorted(r_full["similarity"])])
figs.write_csv(data / "fom_blocks.csv", [{"block": k, "13 drivers": round(r_full["fom"][k], 4),
                                          "2 drivers": round(r_two["fom"][k], 4),
                                          "persistence": round(r_pers["fom"][k], 4)} for k in sorted(r_full["fom"])])

# ---- maps: hits, misses, false alarms, wrong hits
def outcomes(start, obs, s):
    ok = np.isfinite(start) & np.isfinite(obs) & np.isfinite(s)
    oc, sc = obs != start, s != start
    h = np.full(start.shape, np.nan)
    h[ok & ~oc & ~sc] = 0
    h[ok & oc & sc & (obs == s)] = 1
    h[ok & oc & sc & (obs != s)] = 2
    h[ok & oc & ~sc] = 3
    h[ok & ~oc & sc] = 4
    return h


HP = {0: ("Persistence simulated as persistence", "#e6e8e3"), 1: ("Hit: change, right class", "#2a9d8f"),
      2: ("Wrong hit: change, wrong class", "#e9c46a"), 3: ("Miss: change not simulated", "#e76f51"),
      4: ("False alarm: change where none", "#6d597a")}
figs.raster(None, out / "outcomes.png", array=outcomes(t0, t1, sim), palette=HP, counts=True, width=900,
            title="Simulated against observed change, Sibuyan 1997-2011")
CROP = (slice(4, 62), slice(8, 84))
figs.raster(None, out / "outcomes_13.png", array=outcomes(t0, t1, sim)[CROP], palette=HP, legend=False, width=560)
figs.raster(None, out / "outcomes_2.png", array=outcomes(t0, t1, sim2)[CROP], palette=HP, legend=False, width=560)
dc.draw_change(t0, t1, out / "change_observed.png", legend=False, width=760)
dc.draw_change(t0, sim, out / "change_simulated.png", legend=False, width=760)
dc.legend_strip(out / "change_legend.png", [("Unchanged", dc.UNCHANGED)] + [(f"to {n.lower()}", c) for k, (n, c) in
                                                                             dc.SIB.items() if k in (2, 3, 4)],
                width=640)

# ---- Plum Island: fitted on 1985-1991, validated on 1991-1999
wpie = dc.sibuyan("valid_pie", pie=True)
dc.cli("new", "dinamica", "--t0", "pie_landuse.tif:1985", "--t1", "pie_landuse.tif:1991", "--drivers",
       "pie_drivers.tif", "--steps", 6, "-o", "calibration.toml", cwd=wpie)
dc.cli("new", "dinamica", "--t0", "pie_landuse.tif:1985", "--t1", "pie_landuse.tif:1991", "--drivers",
       "pie_drivers.tif", "--steps", 6, "--start", "pie_landuse.tif:1991", "--simulate", 8, "-o", "validation.toml",
       cwd=wpie)
pie_ranges = {"ranges": {"elevation": 5, "slope": 1, "dist_built_1985": 100}}
cal = dc.set_params((wpie / "calibration.toml").read_text(encoding="utf-8"), "woe", pie_ranges)
val = dc.set_params((wpie / "validation.toml").read_text(encoding="utf-8"), "woe", pie_ranges)
(data / "pie_validation.toml").write_text(val, encoding="utf-8")
dc.run(cal, wpie, name="calibration.toml", out="cal")
dc.run(val, wpie, name="validation.toml", out="val")
r_cal = validate(wpie, "pie_landuse.tif", "1985", "1991", "cal/maps.tif", 6)
show("PIE calibration period 1985-1991", r_cal)
r_val = validate(wpie, "pie_landuse.tif", "1991", "1999", "val/maps.tif", 8)
show("PIE validation period 1991-1999", r_val)
r_pp = validate(wpie, "pie_landuse.tif", "1991", "1999", "pie_landuse.tif", 1991)
show("PIE persistence 1991-1999", r_pp)
figs.write_csv(data / "pie_periods.csv", [{"block": k, "calibration period (1985-1991)": round(r_cal["fom"][k], 4),
                                           "validation period (1991-1999)": round(r_val["fom"][k], 4)}
                                          for k in sorted(r_cal["fom"])])
PIE = {1: ("Forest", "#3b8a5a"), 2: ("Built", "#b4508a"), 3: ("Other", "#d9a441")}
p0 = figs.read(wpie / "pie_landuse.tif", 2)[0].filled(np.nan)
p1 = figs.read(wpie / "pie_landuse.tif", 3)[0].filled(np.nan)
ps = figs.read(wpie / "val" / "maps.tif", 8)[0].filled(np.nan)
figs.raster(None, out / "pie_outcomes.png", array=outcomes(p0, p1, ps), palette=HP, counts=True, width=900,
            title="Plum Island: simulated against observed change, 1991-1999")

# ---- drawing 1: the four outcomes on a toy grid
F, C, G = 1, 2, 3
start = np.full((8, 10), F, float)
start[0:3, 0:4] = G
obs = start.copy()
obs[3, 0:3] = G           # grassland grew south
obs[1:3, 4] = G           # and east
obs[6, 7] = C             # a new coconut field
obs[6, 8] = C
sim_t = start.copy()
sim_t[3, 0:2] = G         # two hits
sim_t[0:2, 4] = G         # one hit (1,4) and a false alarm (0,4)
sim_t[6, 7] = G           # a wrong hit (coconut simulated as grassland)
sim_t[4, 5] = G           # a false alarm
cols = {F: dc.SIB[1][1], C: dc.SIB[2][1], G: dc.SIB[3][1]}
fig, axs = plt.subplots(1, 4, figsize=(11.2, 2.9), dpi=150)


def grid(ax, a, title, colours):
    rgb = np.ones(a.shape + (3,))
    for v, c in colours.items():
        rgb[a == v] = plt.matplotlib.colors.to_rgb(c)
    ax.imshow(rgb, interpolation="nearest")
    ax.set_xticks(np.arange(-0.5, a.shape[1], 1), [])
    ax.set_yticks(np.arange(-0.5, a.shape[0], 1), [])
    ax.tick_params(length=0)
    ax.grid(True, color="white", linewidth=0.6, linestyle="-")
    ax.set_title(title, loc="left", fontsize=10)


grid(axs[0], start, "1. Start", cols)
grid(axs[1], obs, "2. Observed end", cols)
grid(axs[2], sim_t, "3. Simulated end", cols)
oc = outcomes(start, obs, sim_t)
grid(axs[3], oc, "4. Compared", {k: v[1] for k, v in HP.items()})
for (r, c), lab in {(3, 0): "H", (3, 1): "H", (1, 4): "H", (3, 2): "M", (2, 4): "M", (6, 8): "M", (0, 4): "F",
                    (4, 5): "F", (6, 7): "W"}.items():
    axs[3].text(c, r, lab, ha="center", va="center", fontsize=8, color="white", fontweight="bold")
fig.tight_layout()
fig.savefig(out / "toy_outcomes.png", dpi=150, bbox_inches="tight", pad_inches=0.05)
plt.close(fig)
figs._shrink(out / "toy_outcomes.png")
h, m, f_, wh = (oc == 1).sum(), (oc == 3).sum(), (oc == 4).sum(), (oc == 2).sum()
print("toy: hits", h, "misses", m, "false alarms", f_, "wrong hits", wh, "fom", h / (h + m + f_ + wh))

# ---- drawing 2: the fuzzy membership of a cell (2^(-d/2)) in windows of 3 and 5
fig, axs = plt.subplots(1, 2, figsize=(8.2, 3.6), dpi=150)
for ax, win in zip(axs, (3, 5)):
    n = 7
    a = np.full((n, n), np.nan)
    c = n // 2
    for i in range(n):
        for j in range(n):
            if abs(i - c) <= win // 2 and abs(j - c) <= win // 2:
                a[i, j] = 2 ** (-np.hypot(i - c, j - c) / 2)
    cm = plt.get_cmap("viridis").copy()
    cm.set_bad("#f3f4f1")
    ax.imshow(a, cmap=cm, vmin=0.2, vmax=1, interpolation="nearest")
    for i in range(n):
        for j in range(n):
            if np.isfinite(a[i, j]):
                ax.text(j, i, f"{a[i, j]:.2f}", ha="center", va="center", fontsize=7.5,
                        color="white" if a[i, j] < 0.75 else figs.INK)
    ax.set_xticks(np.arange(-0.5, n, 1), [])
    ax.set_yticks(np.arange(-0.5, n, 1), [])
    ax.tick_params(length=0)
    ax.grid(True, color="white", linewidth=0.8, linestyle="-")
    ax.set_title(f"Window {win} x {win}", loc="left", fontsize=10.5)
fig.tight_layout()
fig.savefig(out / "membership.png", dpi=150, bbox_inches="tight", pad_inches=0.05)
plt.close(fig)
figs._shrink(out / "membership.png")
print("done")
