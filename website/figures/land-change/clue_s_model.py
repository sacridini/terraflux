"""Figures of "A complete CLUE-S model" (land-change/clue-s-model): the 14 yearly maps of lulcc's vignette model on
Plum Island with the demand against the allocated cells, its validation against the observed map of 1999
(validate.agreement and validate.three_map, against persistence), CLUE's scale setting on the first step, and
replicates with other seeds (real runs)."""

import json
import re

import numpy as np

from clue_common import CLASSES, NAMES, PIE, figs, panels, pie_work

SLUG = "land-change/clue-s-model"
out = figs.img_dir(SLUG)
data = figs.data_dir(SLUG)
work = pie_work(SLUG)
MODEL = (figs.WEB / "figures" / "land-change" / "clue_model.toml").read_text(encoding="utf-8")
(data / "clue.toml").write_text(MODEL, encoding="utf-8")

# --- the run
figs.run(MODEL + '\n[outputs.demand]\nfrom = "demand.demand"\n', work)
steps = figs.read_csv(work / "output" / "steps.csv")
dem = json.loads((work / "output" / "demand.json").read_text())
start = figs.read(work / "pie_landuse.tif", 1)[0]
obs99 = figs.read(work / "pie_landuse.tif", 3)[0]
rows = []
prev = start
frames = []
figs.raster(None, out / "year_1985.png", array=start, palette=dict(CLASSES), legend=False, width=640)
for i, s in enumerate(steps, start=1):
    year = 1985 + i
    m = figs.read(work / "output" / "maps.tif", i)[0]
    changed = int(((m != prev) & ~m.mask).sum())
    prev = m
    r = {"year": year}
    for j, k in enumerate(dem["classes"]):
        n = NAMES[int(k)]
        r[f"{n} demand"] = int(dem["table"][i][j])
        r[f"{n}"] = int(float(s[f"cells_{int(k)}"]))
    r["iterations"] = int(float(s["iterations"]))
    r["changed"] = changed
    rows.append(r)
    figs.raster(None, out / f"year_{year}.png", array=m, palette=dict(CLASSES), legend=False, width=640)
    frames.append((year, r))
row0 = {"year": 1985, **{f"{NAMES[k]} demand": int(dem["table"][0][j]) for j, k in enumerate(CLASSES)},
        **{NAMES[k]: int((start == k).sum()) for k in CLASSES}, "iterations": 0, "changed": 0}
figs.write_csv(data / "years.csv", [row0] + rows)
print(rows[0], rows[-1])

# --- validation against the observed map of 1999, with persistence (the 1985 map) as the null model
sim99 = figs.read(work / "output" / "maps.tif", 14)[0]
VALID = """format = "terraflux/1"

[inputs.t0]
kind = "map"
path = "pie_landuse.tif"
band = "1985"

[inputs.t1]
kind = "map"
path = "pie_landuse.tif"
band = "1999"

[inputs.simulated]
kind = "map"
path = "SIM"
band = "14"

[[nodes]]
id = "agreement"
op = "validate.agreement"
inputs = { start = "$t0", observed = "$t1", simulated = "$simulated" }

[[nodes]]
id = "three"
op = "validate.three_map"
inputs = { t0 = "$t0", t1 = "$t1", simulated = "$simulated" }
params = { factors = [1, 2, 4, 8, 16] }

[outputs.agreement]
from = "agreement.table"

[outputs.fom]
from = "three.figure_of_merit"

[outputs.budget]
from = "three.budget"
"""
v = VALID.replace("SIM", "output/maps.tif")
figs.run(v, work, name="validate.toml")
agree = figs.read_csv(work / "output" / "agreement.csv")
fom = figs.read_csv(work / "output" / "fom.csv")
budget = figs.read_csv(work / "output" / "budget.csv")
print("agreement", agree)
print("fom", fom)
print("budget overall", [b for b in budget if b.get("level") in ("0", "0.0")])
# persistence: the map of 1985 as the "simulation"
vp = VALID.replace('path = "SIM"\nband = "14"', 'path = "pie_landuse.tif"\nband = "1985"')
wp = pie_work(f"{SLUG}/persistence")
figs.run(vp, wp, name="validate.toml")
print("persistence agreement", figs.read_csv(wp / "output" / "agreement.csv"))
fomp = figs.read_csv(wp / "output" / "fom.csv")
print("persistence fom", fomp)

figs.raster(None, out / "observed_1999.png", array=obs99, palette=dict(CLASSES), legend=False, width=760)
figs.raster(None, out / "simulated_1999.png", array=sim99, palette=dict(CLASSES), legend=False, width=760)
# hits, misses, false alarms and wrong hits of the change
t0 = start
H = np.ma.zeros(t0.shape, dtype=int)
obs_ch = obs99 != t0
sim_ch = sim99 != t0
H = np.where(~obs_ch & ~sim_ch, 0, H)
H = np.where(obs_ch & sim_ch & (obs99 == sim99), 1, H)
H = np.where(obs_ch & sim_ch & (obs99 != sim99), 2, H)
H = np.where(obs_ch & ~sim_ch, 3, H)
H = np.where(~obs_ch & sim_ch, 4, H)
H = np.ma.masked_where(t0.mask, H)
HP = {0: ("Persistence, simulated as such", "#e6e8e3"), 1: ("Hit: change simulated right", "#2a9d8f"),
      2: ("Wrong hit: change to the wrong class", "#e9c46a"), 3: ("Miss: change not simulated", "#e76f51"),
      4: ("False alarm: change where none", "#6d597a")}
figs.raster(None, out / "hits_misses.png", array=H, palette=HP, counts=True, width=900,
            title="Simulated against observed change, 1985 to 1999")
fom_rows = []
for a, b in zip(fom, fomp):
    fom_rows.append({"block": int(float(a["factor"])), "CLUE-S": round(float(a["overall"]), 4),
                     "Persistence": round(float(b["overall"]), 4)})
print(fom_rows)
figs.write_csv(data / "fom.csv", fom_rows)

# --- CLUE's scale on the first step: the iteration variable's step per cell of difference
one = re.sub(r'times = "demand.steps"', "times = 1", MODEL)
scales = ["0.000001", "0.00001", "0.0001", "0.0005"]
for sc in scales:
    m = one.replace("scale = 0.000001", f"scale = {sc}")
    w = pie_work(f"{SLUG}/scale_{sc}")
    figs.run(m, w)
    st = figs.read_csv(w / "output" / "steps.csv")[0]
    a = figs.read(w / "output" / "maps.tif", 1)[0]
    figs.raster(None, out / f"scale_{sc}.png", array=a, palette=dict(CLASSES), legend=False, width=640)
    cells = {NAMES[k]: int(float(st[f"cells_{k}"])) for k in CLASSES}
    print(f'  - {{ value: "{sc}", src: /img/tutorials/{SLUG}/scale_{sc}.png, stats: {{ Converged: '
          f'"{"yes" if float(st["converged"]) == 1 else "no"}", Iterations: "{int(float(st["iterations"]))}", '
          f'"Largest difference": "{int(float(st["max_diff"])):,} cells", ' +
          ", ".join(f'{n}: "{c:,}"' for n, c in cells.items()) + " } }")

# --- replicates: other seeds
reps = {}
for seed in (1, 2, 3, 4, 5):
    w = pie_work(f"{SLUG}/seed_{seed}")
    figs.run(MODEL, w, "--set", f"seed={seed}")
    reps[seed] = figs.read(w / "output" / "maps.tif", 14)[0]
diffs = [int(((reps[1] != reps[s]) & ~reps[1].mask).sum()) for s in (2, 3, 4, 5)]
print("cells differing from seed 1 in 1999:", diffs)
stack = np.ma.stack([reps[s] for s in reps])
built_share = (stack == 2).mean(axis=0)
amb = np.ma.masked_where(start.mask, built_share)
print("cells built in some but not all runs:", int(((amb > 0) & (amb < 1)).sum()))

# the slider's frames (pasted into the page)
print('  - { value: 1985, src: /img/tutorials/land-change/clue-s-model/year_1985.png, stats: { Forest: "49,013", '
      'Built: "37,122", Other: "27,428", "Cells changed": "0" } }')
for year, r in frames:
    print(f'  - {{ value: {year}, src: /img/tutorials/{SLUG}/year_{year}.png, stats: {{ Forest: "{r["Forest"]:,} '
          f'(asked {r["Forest demand"]:,})", Built: "{r["Built"]:,} (asked {r["Built demand"]:,})", Other: '
          f'"{r["Other"]:,} (asked {r["Other demand"]:,})", "Cells changed": "{r["changed"]:,}" }} }}')

# --- the elasticities: the same model (terraflux new clue) with other elasticities, the change of 1985-1999
from clue_common import new_model  # noqa: E402

print("elasticities:")
EL = ["0.0,0.0,0.0", "0.2,0.2,0.2", "0.4,0.4,0.4", "0.6,0.6,0.6", "0.8,0.8,0.8", "0.2,0.9,0.2"]
erows = []
for el in EL:
    tag = el.replace(",", "_")
    w = pie_work(f"{SLUG}/elasticity_{tag}")
    text = new_model(w, elasticity=el)
    if el == "0.2,0.2,0.2":
        (data / "clue_new.toml").write_text(text, encoding="utf-8")
    figs.run(text, w)
    st = figs.read_csv(w / "output" / "steps.csv")
    conv = all(float(s["converged"]) == 1 for s in st)
    prev = start
    gross = 0
    for b in range(1, 15):
        m = figs.read(w / "output" / "maps.tif", b)[0]
        gross += int(((m != prev) & ~m.mask).sum())
        prev = m
    v = VALID.replace("SIM", "output/maps.tif")
    figs.run(v, w, name="validate.toml")
    a = figs.read_csv(w / "output" / "agreement.csv")[0]
    net = int(((prev != start) & ~prev.mask).sum())
    code = np.ma.where(prev != start, prev, 0)
    code = np.ma.masked_where(start.mask, code)
    figs.raster(None, out / f"elasticity_{tag}.png", array=code,
                palette={0: ("Unchanged", "#e6e8e3"), 1: ("to forest", CLASSES[1][1]), 2: ("to built", CLASSES[2][1]),
                         3: ("to other", CLASSES[3][1])}, legend=False, width=640)
    erows.append({"elasticity": el, "converged": conv, "gross": gross, "net": net,
                  "fom": round(float(a["figure_of_merit"]), 4), "overall": round(float(a["overall"]), 4)})
    print("  ", erows[-1])
figs.write_csv(data / "elasticity.csv", erows)
figs.raster(None, out / "elasticity_legend.png", array=np.ma.masked_invalid(np.array([[0.0, 1, 2, 3]])),
            palette={0: ("Unchanged", "#e6e8e3"), 1: ("to forest", CLASSES[1][1]), 2: ("to built", CLASSES[2][1]),
                     3: ("to other", CLASSES[3][1])}, width=300)
