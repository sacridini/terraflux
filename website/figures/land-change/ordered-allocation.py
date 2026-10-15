"""Figures of "The ordered allocation" (land-change/ordered-allocation).

The model `terraflux new ordered` writes for lulcc's Plum Island Ecosystems (the regressions and the demand of
the series), run with every order of the three classes, stochastic and deterministic, with five seeds, and
compared with the CLUE-S model of part 4 (lulcc's vignette settings) against the observed map of 1999
(validate.agreement, validate.three_map at blocks of 1 to 16 cells). A toy drawing of the two ways of choosing
cells comes first.

    python website/figures/land-change/ordered-allocation.py
"""

import itertools
import time

import matplotlib.pyplot as plt
import numpy as np

from clue_common import CLASSES, NAMES, PIE, figs, new_model, pie_work

SLUG = "land-change/ordered-allocation"
out = figs.img_dir(SLUG)
data = figs.data_dir(SLUG)
GREY = "#e6e8e3"
CHANGE = {0: ("Unchanged", GREY), 1: ("to forest", CLASSES[1][1]), 2: ("to built", CLASSES[2][1]),
          3: ("to other", CLASSES[3][1])}
START = figs.read(PIE / "pie_landuse.tif", 1)[0]
OBS99 = figs.read(PIE / "pie_landuse.tif", 3)[0]

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
path = "output/maps.tif"
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
"""


def run(tag, kind="ordered", extra=(), seed=None):
    w = pie_work(f"{SLUG}/{tag}")
    text = new_model(w, kind, extra=tuple(extra))
    t = time.time()
    r = figs.run(text, w, *(("--set", f"seed={seed}") if seed else ()))
    secs = time.time() - t
    short = [line for line in (r.stdout + r.stderr).splitlines() if "not met" in line or "did not meet" in line]
    st = figs.read_csv(w / "output" / "steps.csv")
    maps = [figs.read(w / "output" / "maps.tif", b)[0] for b in range(1, 15)]
    figs.run(VALID, w, name="validate.toml")
    a = figs.read_csv(w / "output" / "agreement.csv")[0]
    fom = [float(x["overall"]) for x in figs.read_csv(w / "output" / "fom.csv")]
    prev, yearly = START, []
    for m in maps:
        yearly.append(int(((m != prev) & ~m.mask).sum()))
        prev = m
    last = maps[-1]
    res = {"tag": tag, "text": text, "maps": maps, "steps": st, "short": short, "seconds": round(secs, 1),
           "converged": sum(float(s["converged"]) == 1 for s in st), "yearly": yearly,
           "changed": int(((last != START) & ~last.mask).sum()), "fom": float(a["figure_of_merit"]),
           "overall": float(a["overall"]), "fom_blocks": fom, "agreement": a,
           "cells": {NAMES[k]: int((last == k).sum()) for k in CLASSES},
           "trans": {f"{NAMES[i]}->{NAMES[j]}": int(((START == i) & (last == j)).sum())
                     for i in (1, 2, 3) for j in (1, 2, 3) if i != j}}
    print(tag, {k: v for k, v in res.items() if k not in ("text", "maps", "steps", "yearly", "agreement")},
          "yearly", yearly[:4])
    print("   ", a)
    return res


def change_code(last):
    code = np.ma.where(last != START, last, 0)
    return np.ma.masked_where(np.ma.getmaskarray(START), code).astype(float)


# ------------------------------------------------------------------------------------------------ the toy
def toy():
    rng = np.random.default_rng(4)
    p = np.sort(rng.uniform(0.05, 0.95, 14))[::-1]
    u = rng.uniform(0, 1, 14)
    n = 5
    fig, axes = plt.subplots(1, 2, figsize=(9.6, 3.3), dpi=150, sharey=True)
    x = np.arange(1, 15)
    # deterministic: the n first
    cols = ["#b4508a" if i < n else "#d8d8d8" for i in range(14)]
    axes[0].bar(x, p, color=cols, width=0.75)
    axes[0].set_title("Deterministic: the 5 most suitable", loc="left", fontsize=11)
    # stochastic: those that pass the draw, in rank order, the first 5
    passed = p >= u
    sel = np.zeros(14, bool)
    k = 0
    for i in range(14):
        if passed[i] and k < n:
            sel[i] = True
            k += 1
    cols = ["#b4508a" if s else ("#e3b3cf" if ps else "#d8d8d8") for s, ps in zip(sel, passed)]
    axes[1].bar(x, p, color=cols, width=0.75)
    axes[1].scatter(x, u, marker="_", s=160, color="#1c2420", lw=2, zorder=3, label="random number u")
    axes[1].set_title("Stochastic: the first 5 whose suitability ≥ u", loc="left", fontsize=11)
    axes[1].legend(loc="upper right", fontsize=9, frameon=False, bbox_to_anchor=(1.0, 1.02))
    for ax in axes:
        ax.set_xticks(x)
        ax.set_xlabel("Candidate cells, ranked by suitability")
        ax.set_ylim(0, 1.2)
        ax.set_yticks([0, 0.2, 0.4, 0.6, 0.8, 1.0])
        ax.grid(axis="x", visible=False)
    axes[0].set_ylabel("Suitability for built land")
    fig.tight_layout()
    fig.savefig(out / "toy.png", dpi=150)
    plt.close(fig)
    figs._shrink(out / "toy.png")


toy()

# ------------------------------------------------------------------------------------------------ the runs
main = run("order_2_1_3", extra=["--order", "2,1,3"])
(data / "ordered.toml").write_text(main["text"], encoding="utf-8")
det = run("deterministic", extra=["--order", "2,1,3", "--deterministic"])
clue = run("clue", kind="clue")
persistence_fom = None

# every order
orders = {}
for o in itertools.permutations((1, 2, 3)):
    tag = "_".join(map(str, o))
    orders[o] = main if o == (2, 1, 3) else run(f"order_{tag}", extra=["--order", ",".join(map(str, o))])
order_rows = []
for o, res in orders.items():
    label = " → ".join(NAMES[k] for k in o)
    figs.raster(None, out / f"order_{'_'.join(map(str, o))}.png", array=change_code(res["maps"][-1]),
                palette=CHANGE, legend=False, width=640)
    order_rows.append({"order": label, "changed": res["changed"], "fom": round(res["fom"], 3),
                       "agreement": round(res["overall"] * 100, 1), **res["trans"]})
    print("  - { value: \"%s\", src: /img/tutorials/%s/order_%s.png, stats: { \"Cells changed by 1999\": \"%s\", "
          "\"Forest to built\": \"%s\", \"Other to built\": \"%s\", \"Other to forest\": \"%s\", "
          "\"Figure of merit\": \"%.3f\" } }" % (label, SLUG, "_".join(map(str, o)), f"{res['changed']:,}",
                                                f"{res['trans']['Forest->Built']:,}",
                                                f"{res['trans']['Other->Built']:,}",
                                                f"{res['trans']['Other->Forest']:,}", res["fom"]))
figs.write_csv(data / "orders.csv", order_rows)

# CLUE-S against the ordered allocation, the change of 1985-1999
for name, res in (("clue", clue), ("ordered", main), ("deterministic", det)):
    figs.raster(None, out / f"change_{name}.png", array=change_code(res["maps"][-1]), palette=CHANGE,
                legend=False, width=760)
obs = change_code(OBS99)
figs.raster(None, out / "change_observed.png", array=obs, palette=CHANGE, legend=False, width=760)
figs.raster(None, out / "change_legend.png", array=np.ma.masked_invalid(np.array([[0.0, 1, 2, 3]])), palette=CHANGE,
            width=300)
obs_changed = int(((OBS99 != START) & ~OBS99.mask).sum())
print("observed changed", obs_changed, {f"{NAMES[i]}->{NAMES[j]}": int(((START == i) & (OBS99 == j)).sum())
                                        for i in (1, 2, 3) for j in (1, 2, 3) if i != j})

# yearly change
yr = [{"year": 1986 + y, "CLUE-S": clue["yearly"][y], "Ordered": main["yearly"][y],
       "Ordered, deterministic": det["yearly"][y]} for y in range(1, 14)]   # 1986, CLUE-S's reshuffle, left out
figs.write_csv(data / "yearly.csv", yr)

# the comparison table
rows = []
for label, res in (("CLUE-S (part 4)", clue), ("Ordered, stochastic", main), ("Ordered, deterministic", det)):
    a = res["agreement"]
    rows.append({"model": label, "first_year": res["yearly"][0], "changed": res["changed"],
                 "hits": int(float(a["hits"])), "misses": int(float(a["misses"])),
                 "false_alarms": int(float(a["false_alarms"])), "fom": round(res["fom"], 3),
                 "agreement": round(res["overall"] * 100, 1), "seconds": res["seconds"]})
figs.write_csv(data / "compare.csv", rows)

# figure of merit at blocks
fom_rows = []
for i, f in enumerate((1, 2, 4, 8, 16)):
    fom_rows.append({"block": f, "CLUE-S": round(clue["fom_blocks"][i], 4),
                     "Ordered": round(main["fom_blocks"][i], 4),
                     "Ordered, deterministic": round(det["fom_blocks"][i], 4)})
figs.write_csv(data / "fom.csv", fom_rows)
print(fom_rows)

# seeds
reps = {1: main["maps"][-1]}
for s in (2, 3, 4, 5):
    reps[s] = run(f"seed_{s}", extra=["--order", "2,1,3"], seed=s)["maps"][-1]
diffs = [int(((reps[1] != reps[s]) & ~reps[1].mask).sum()) for s in (2, 3, 4, 5)]
stack = np.ma.stack([reps[s] for s in reps])
share = (stack == 2).mean(axis=0)
amb = int(((share > 0) & (share < 1) & ~np.ma.getmaskarray(START)).sum())
print("seeds: cells differing from seed 1 in 1999", diffs, "built in some runs, not all:", amb)
runs = np.ma.masked_where(np.ma.getmaskarray(START) | (START == 2).filled(False), np.round(share * 5))
RUNS = {0: ("Built in no run", GREY), 1: ("1 run of 5", "#f3d3e4"), 2: ("2 runs", "#e3a6c8"), 3: ("3 runs", "#cf73a8"),
        4: ("4 runs", "#a8407f"), 5: ("All 5 runs", "#5e1546")}
figs.raster(None, out / "seeds.png", array=runs, palette=RUNS, width=860)
print("images:", round(sum(p.stat().st_size for p in out.glob("*.png")) / 1e6, 2), "MB")
