"""Figures of "CLUE-S: the ideas" (land-change/clue-s-ideas): lulcc's Plum Island Ecosystems maps, the change
between them, the suitability of six real cells, and CLUE-S's iterations on the first step (real runs with
max_iter = 1, 2, 3...)."""

import json
import re

import numpy as np

from clue_common import CLASSES, NAMES, PIE, SUITABILITY, figs, new_model, panels, pie_work

SLUG = "land-change/clue-s-ideas"
out = figs.img_dir(SLUG)
data = figs.data_dir(SLUG)
work = pie_work(SLUG)

# the data, for readers to download
for f in ("pie_landuse.tif", "pie_drivers.tif"):
    (data / f).write_bytes((PIE / f).read_bytes())

# --- the observed maps
obs_maps = [figs.read(work / "pie_landuse.tif", b)[0] for b in (1, 2, 3)]
panels(obs_maps, ["1985", "1991", "1999"], out / "observed.png", width=1100)

# --- the change between 1985 and 1999, by map algebra in Terraflux
change = """format = "terraflux/1"

[inputs.t0]
kind = "map"
path = "pie_landuse.tif"
band = "1985"

[inputs.t1]
kind = "map"
path = "pie_landuse.tif"
band = "1999"

[[nodes]]
id = "change"
op = "map.calc"
inputs = { a = "$t0", b = "$t1" }
params = { expression = "if a == b then 0 else a * 10 + b" }
"""
figs.run(figs.add_writes(change, {"change.map": "change.tif"}), work, name="change.toml")
ch = figs.read(work / "change.tif")[0]
pal = {0: ("No change", "#e6e8e3"), 12: ("Forest to built", "#b4508a"), 32: ("Other to built", "#2f6f9f"),
       13: ("Forest to other", "#d9a441"), 31: ("Other to forest", "#3b8a5a"), 21: ("Built to forest", "#1c2420"),
       23: ("Built to other", "#7f7f7f")}
figs.raster(work / "change.tif", out / "change.png", palette=pal, counts=True, width=860,
            title="Change between 1985 and 1999")
changed = int((ch.filled(0) != 0).sum())
total = int(ch.count())
print(f"changed cells: {changed} of {total} ({changed / total * 100:.1f}%)")
for v in sorted(pal):
    print(v, pal[v][0], int((ch == v).sum()))

# --- the suitability of every class in every cell (the regressions of the vignette), for the six-cell figure
model = new_model(work)
figs.run(figs.add_writes(SUITABILITY, {"suitability.suitability": "suitability.tif"}), work, name="suit.toml")
suit = [figs.read(work / "suitability.tif", b)[0] for b in (1, 2, 3)]
land = figs.read(work / "pie_landuse.tif", 1)[0]
rng = np.random.default_rng(4)
valid = np.argwhere(~land.mask)
picked = []
# two cells where each class leads, near the margin between classes
for k in (1, 2, 3):
    s = np.stack([x.filled(np.nan) for x in suit])
    lead = np.nanargmax(np.where(np.isnan(s), -1, s), axis=0) + 1
    gap = np.sort(s, axis=0)[-1] - np.sort(s, axis=0)[-2]
    cand = [tuple(c) for c in valid if lead[tuple(c)] == k and gap[tuple(c)] < 0.12 and land[tuple(c)] == k]
    for i in rng.choice(len(cand), 2, replace=False):
        picked.append(cand[i])
rows = []
for n, (r, c) in enumerate(picked, start=1):
    rows.append({"cell": f"Cell {n}", **{NAMES[k]: round(float(suit[k - 1][r, c]), 3) for k in CLASSES},
                 "now": NAMES[int(land[r, c])]})
print(rows)


def six(ax, boost=0.0):
    x = np.arange(len(rows))
    w = 0.26
    for j, k in enumerate(CLASSES):
        name = NAMES[k]
        vals = [rw[name] + (boost if k == 2 else 0) for rw in rows]
        ax.bar(x + (j - 1) * w, vals, w, color=CLASSES[k][1], label=name + (f" + {boost}" if k == 2 and boost else ""))
    for i, rw in enumerate(rows):
        vals = {k: rw[NAMES[k]] + (boost if k == 2 else 0) for k in CLASSES}
        best = max(vals, key=vals.get)
        ax.text(i, max(vals.values()) + 0.04, NAMES[best], ha="center", fontsize=9.5, color=CLASSES[best][1],
                fontweight="bold")
    ax.set_xticks(x, [rw["cell"] for rw in rows])
    ax.set_ylim(0, 1.05)
    ax.set_ylabel("Suitability")
    ax.legend(ncol=3, loc="upper left", fontsize=9)


figs.chart(out / "six_cells.png", lambda ax: six(ax), width=900, height=380,
           title="Six cells of Plum Island: the class each would take")
figs.chart(out / "six_cells_boost.png", lambda ax: six(ax, 0.1), width=900, height=380,
           title="The same cells with 0.1 added to Built's suitability")

# the cells a step changed, by their new class
DIFF = {0: ("Unchanged", "#e6e8e3"), 1: ("Now forest", "#3b8a5a"), 2: ("Now built", "#b4508a"),
        3: ("Now other", "#d9a441")}
figs.raster(None, out / "diff_legend.png", array=np.ma.array([[0, 1, 2, 3]]), palette=DIFF, width=300)

# --- CLUE-S's iterations: the first step (1985 to 1986) allocated with at most k iterations
one = re.sub(r'times = "demand.steps"', "times = 1", model)
ks = [1, 2, 3, 5, 10, 20, 30, 40, 60]
demand = None
curve = []
frames = []
for k in ks:
    m = one.replace("max_iter = 1000", f"max_iter = {k}") + '\n[outputs.demand]\nfrom = "demand.demand"\n'
    w = figs.workdir(f"{SLUG}/k{k}")
    for f in ("pie_landuse.tif", "pie_drivers.tif"):
        (w / f).write_bytes((PIE / f).read_bytes())
    r = figs.run(m, w)
    steps = figs.read_csv(w / "output" / "steps.csv")[0]
    cells = {NAMES[c]: int(float(steps[f"cells_{c}"])) for c in CLASSES}
    curve.append({"iterations": k, **cells, "max_diff": float(steps["max_diff"]),
                  "converged": int(float(steps["converged"]))})
    sim = figs.read(w / "output" / "maps.tif", 1)[0]
    diff = np.ma.where(sim != land, sim, 0)
    figs.raster(None, out / f"iter_{k}.png", array=diff, palette=DIFF, legend=False, width=640)
    frames.append((k, cells, int((diff.filled(0) != 0).sum())))
    print(k, cells, steps["max_diff"], steps["converged"], steps["iterations"])

# the demand of 1986, as demand.from_maps computed it in the runs
dj = json.loads((w / "output" / "demand.json").read_text())
dem = {NAMES[int(c)]: dj["table"][1][j] for j, c in enumerate(dj["classes"])}
print("demand 1986 ~", dem)
rows_csv = [{"iterations": r["iterations"], "Forest": r["Forest"], "Built": r["Built"], "Other": r["Other"],
             "Forest demand": round(dem["Forest"]), "Built demand": round(dem["Built"]),
             "Other demand": round(dem["Other"])} for r in curve]
figs.write_csv(data / "iterations.csv", rows_csv)

# the slider's frames (pasted into the page)
for k, cells, changed in frames:
    st = ", ".join(f'"{n}": "{cells[n]:,} (asked {round(dem[n]):,})"' for n in ("Forest", "Built", "Other"))
    print(f'  - {{ value: {k}, src: /img/tutorials/{SLUG}/iter_{k}.png, stats: {{ {st}, "Cells changed": "{changed:,}" }} }}')
