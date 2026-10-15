"""Figures of "The demand" (land-change/land-demand): the linear demand of Plum Island interpolated and
extrapolated by demand.from_maps, a Markov chain's projection (markov.fit + plus.demand), and a counterfactual
scenario written as a table ("forest protected from 1985") run with CLUE-S against the trend (real runs)."""

import json

import matplotlib.pyplot as plt
import numpy as np

from clue_common import CLASSES, NAMES, figs, panels, pie_work

SLUG = "land-change/land-demand"
out = figs.img_dir(SLUG)
data = figs.data_dir(SLUG)
work = pie_work(SLUG)

HEAD = """format = "terraflux/1"

[inputs.observed]
kind = "stack"
path = "pie_landuse.tif"

[inputs.t1985]
kind = "map"
path = "pie_landuse.tif"
band = "1985"

[inputs.t1999]
kind = "map"
path = "pie_landuse.tif"
band = "1999"
"""

DEMANDS = HEAD + """
[[nodes]]
id = "trend"
op = "demand.from_maps"
inputs = { maps = "$observed" }
params = { times = [1985, 1991, 1999], out_times = [OUT] }

[[nodes]]
id = "markov"
op = "markov.fit"
inputs = { t0 = "$t1985", t1 = "$t1999", steps = 14 }

[[nodes]]
id = "projection"
op = "plus.demand"
inputs = { start = "$t1999", markov = "markov.markov" }
params = { steps = 31 }

[outputs.trend]
from = "trend.demand"

[outputs.projection]
from = "projection.demand"

[outputs.markov]
from = "markov.markov"
""".replace("OUT", ", ".join(str(y) for y in range(1985, 2031)))
figs.run(DEMANDS, work, name="demands.toml")
trend = json.loads((work / "output" / "trend.json").read_text())
proj = json.loads((work / "output" / "projection.json").read_text())
mk = json.loads((work / "output" / "markov.json").read_text())
print("markov keys", list(mk.keys()))
print("single", np.round(np.array(mk["single"]), 4).tolist() if "single" in mk else mk)
print("multi", np.round(np.array(mk["multi"]), 5).tolist() if "multi" in mk else "")
obs = {1985: [], 1991: [], 1999: []}
for b, y in enumerate(obs, start=1):
    a = figs.read(work / "pie_landuse.tif", b)[0]
    obs[y] = [int((a == k).sum()) for k in CLASSES]
print("observed", obs)

rows = []
for i, t in enumerate(trend["times"]):
    r = {"year": int(t)}
    for j, k in enumerate(trend["classes"]):
        r[NAMES[int(k)]] = int(trend["table"][i][j])
    rows.append(r)
figs.write_csv(data / "trend.csv", rows)
print("trend 1999, 2030:", rows[14], rows[-1])

# the linear trend against the Markov projection, from 1999 to 2030
rows_cmp = []
for i, t in enumerate(proj["times"]):
    y = 1999 + int(t)
    r = {"year": y}
    lin = trend["table"][trend["times"].index(float(y))]
    for j, k in enumerate(proj["classes"]):
        r[f"{NAMES[int(k)]} (Markov)"] = int(proj["table"][i][j])
        r[f"{NAMES[int(k)]} (linear)"] = int(lin[trend["classes"].index(k)])
    rows_cmp.append(r)
figs.write_csv(data / "trend_vs_markov.csv", rows_cmp)
print("2030:", rows_cmp[-1])


# the static figure: observed counts and the linear demand
def draw(ax):
    years = [r["year"] for r in rows]
    for k in CLASSES:
        n = NAMES[k]
        col = CLASSES[k][1]
        ax.plot(years, [r[n] for r in rows], color=col, lw=2, label=f"{n}: demand")
        ax.scatter(list(obs), [obs[y][k - 1] for y in obs], color=col, s=36, zorder=3, edgecolor="white")
    ax.axvspan(1985, 1999, color="#eef2ee", zorder=0)
    ax.text(1986, 57000, "interpolated", fontsize=9, color="#48534d")
    ax.text(2001, 57000, "extrapolated", fontsize=9, color="#48534d")
    ax.set_xlabel("Year")
    ax.set_ylabel("Cells")
    ax.set_ylim(15000, 59000)
    ax.legend(ncol=3, loc="lower left", fontsize=9)


figs.chart(out / "trend.png", draw, width=900, height=420,
           title="A linear demand through the observed counts (dots)")

# --- a counterfactual scenario: forest kept at its 1985 area, built land growing as observed
start = trend["table"][0]
ncell = sum(start)
prot = []
for i in range(15):
    built = trend["table"][i][1]
    forest = start[0]
    prot.append([int(forest), int(built), int(ncell - forest - built)])
print("protection rows", prot[0], prot[-1])

MODEL = (figs.WEB / "figures" / "land-change" / "clue_model.toml").read_text(encoding="utf-8")
table = "{ classes = [1, 2, 3], rows = [" + ", ".join("[" + ", ".join(str(v) for v in r) + "]" for r in prot) + \
        "], times = [" + ", ".join(str(y) for y in range(1985, 2000)) + "] }"
protect = MODEL.replace('''op = "demand.from_maps"
inputs = { maps = "$observed" }
params = { times = [1985, 1991, 1999] }''', f'''op = "demand.table"
params = {{ table = {table} }}''')
assert protect != MODEL
(data / "protection_table.txt").write_text(table, encoding="utf-8")
runs = {}
for name, m in (("trend", MODEL), ("protection", protect)):
    w = pie_work(f"{SLUG}/{name}")
    figs.run(m, w)
    runs[name] = w
    steps = figs.read_csv(w / "output" / "steps.csv")
    print(name, "converged:", all(float(s["converged"]) == 1 for s in steps), steps[-1])
t0 = figs.read(work / "pie_landuse.tif", 1)[0]
last = {n: figs.read(w / "output" / "maps.tif", 14)[0] for n, w in runs.items()}
for n, a in last.items():
    lost = int(((t0 == 1) & (a != 1)).sum())
    gained = int(((t0 != 1) & (a == 1)).sum())
    print(n, {NAMES[k]: int((a == k).sum()) for k in CLASSES}, "forest lost", lost, "gained", gained,
          "kept", int(((t0 == 1) & (a == 1)).sum()))
    figs.raster(None, out / f"sim1999_{n}.png", array=a, palette=dict(CLASSES), legend=False, width=760)
# where the two scenarios differ
d = np.ma.where(last["trend"] == last["protection"], 0, last["protection"])
DIFF = {0: ("Same in both", "#e6e8e3"), 1: ("Forest only with protection", "#3b8a5a"),
        2: ("Built only with protection", "#b4508a"), 3: ("Other only with protection", "#d9a441")}
figs.raster(None, out / "scenario_difference.png", array=d, palette=DIFF, counts=True, width=860,
            title="Simulated 1999: where the protection scenario differs from the trend")
# forest lost and gained in the protection scenario
fl = np.ma.where((t0 == 1) & (last["protection"] != 1), 1, np.ma.where((t0 != 1) & (last["protection"] == 1), 2, 0))
fl = np.ma.masked_where(t0.mask, fl)
FL = {0: ("Not forest, or forest both times", "#e6e8e3"), 1: ("Forest of 1985 lost", "#c0392b"),
      2: ("Forest gained elsewhere", "#3b8a5a")}
figs.raster(None, out / "protection_forest_moves.png", array=fl, palette=FL, counts=True, width=860,
            title="Protection scenario: the forest area is kept, not the forest")
