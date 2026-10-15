"""Figures of "Rules: elasticities, conversions, neighbourhoods and restrictions" (land-change/clue-s-rules).

Real runs of the CLUE-S model that `terraflux new clue` writes for lulcc's Plum Island Ecosystems (the vignette's
settings, as in part 4), with each of lulcc's decision rules added in turn:

- a restricted block (tests/data/inputs/pie_restrictions.tif, lulcc's reference case): with and without;
- conversion matrices: built land that cannot change, and lulcc's reference matrix with its time and demand codes,
  with the history of 1 everywhere and with the map of histories of lulcc's reference case (pie_history.tif);
- a neighbourhood rule on built land at several thresholds (built land fixed by the matrix).

An explanatory drawing of the time codes 100 + t and 1000 + t comes first.

    python website/figures/land-change/clue-s-rules.py
"""

import json
import shutil

import matplotlib.pyplot as plt
import numpy as np
from scipy import ndimage

from clue_common import CLASSES, PIE, figs, new_model, pie_work

SLUG = "land-change/clue-s-rules"
out = figs.img_dir(SLUG)
data = figs.data_dir(SLUG)
for f in ("pie_restrictions.tif", "pie_history.tif"):
    shutil.copy(PIE / f, data / f)

GREY = "#e6e8e3"
CHANGE = {0: ("Unchanged", GREY), 1: ("to forest", CLASSES[1][1]), 2: ("to built", CLASSES[2][1]),
          3: ("to other", CLASSES[3][1]), 9: ("Restricted block", "#1c2420")}
BUILT_ONLY = "[[1,1,1],[0,1,0],[1,1,1]]"
LULCC_MATRIX = "[[1,1008,105],[0,1,0],[-1,-2,1]]"

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

[outputs.agreement]
from = "agreement.table"
"""

R = figs.read(PIE / "pie_restrictions.tif")[0].filled(0) == 1
START = figs.read(PIE / "pie_landuse.tif", 1)[0]
OBS99 = figs.read(PIE / "pie_landuse.tif", 3)[0]


def run(tag, extra=()):
    """The model of `terraflux new clue` with the options `extra`, run; its maps, steps and agreement."""
    w = pie_work(f"{SLUG}/{tag}")
    for f in ("pie_restrictions.tif", "pie_history.tif"):
        shutil.copy(PIE / f, w / f)
    text = new_model(w, extra=tuple(extra))
    r = figs.run(text, w)
    warnings = sum("did not meet the demand" in line for line in (r.stdout + r.stderr).splitlines())
    st = figs.read_csv(w / "output" / "steps.csv")
    maps = [figs.read(w / "output" / "maps.tif", b)[0] for b in range(1, 15)]
    figs.run(VALID, w, name="validate.toml")
    a = figs.read_csv(w / "output" / "agreement.csv")[0]
    prev, yearly = START, []
    for m in maps:
        yearly.append(int(((m != prev) & ~m.mask).sum()))
        prev = m
    last = maps[-1]
    res = {
        "tag": tag, "text": text, "maps": maps, "steps": st, "warnings": warnings,
        "converged": sum(float(s["converged"]) == 1 for s in st),
        "first_year": yearly[0], "yearly": yearly,
        "changed": int(((last != START) & ~last.mask).sum()),
        "inside": int(((last != START) & ~last.mask & R).sum()),
        "fom": float(a["figure_of_merit"]), "overall": float(a["overall"]),
        "trans": {f"{i}{j}": int(((START == i) & (last == j)).sum()) for i in (1, 2, 3) for j in (1, 2, 3) if i != j},
    }
    print(tag, {k: v for k, v in res.items() if k not in ("text", "maps", "steps", "yearly")})
    return res


def change_code(last, block=False):
    code = np.ma.where(last != START, last, 0)
    code = np.ma.masked_where(np.ma.getmaskarray(START), code).astype(float)
    if block:
        edge = R & ~ndimage.binary_erosion(R, structure=np.ones((3, 3)))
        edge = ndimage.binary_dilation(edge, structure=np.ones((2, 2)))
        code = np.ma.where(edge, 9, code)
    return code


# ------------------------------------------------------------------------------------------------ the time codes
def timeline():
    fig, axes = plt.subplots(3, 1, figsize=(7.4, 2.9), dpi=150, sharex=True)
    hist = np.arange(0, 15)
    rows = [("1 (always)", np.ones_like(hist, bool)),
            ("105: only while history < 5", hist < 5),
            ("1008: only once history >= 8", hist >= 8)]
    for ax, (label, ok) in zip(axes, rows):
        for h, o in zip(hist, ok):
            ax.add_patch(plt.Rectangle((h - 0.45, 0), 0.9, 1, color="#3b8a5a" if o else "#e9e4d8", lw=0))
        ax.set_xlim(-0.6, 14.6)
        ax.set_ylim(0, 1)
        ax.set_yticks([])
        ax.tick_params(axis="x", length=0)
        ax.grid(False)
        for s in ("left", "bottom"):
            ax.spines[s].set_visible(False)
        ax.set_title(label, loc="left", fontsize=10.5, fontweight="normal", pad=3)
    axes[-1].set_xticks(hist)
    axes[-1].set_xlabel("History of the cell: years it has had its class")
    fig.tight_layout(h_pad=0.6)
    fig.savefig(out / "time_codes.png", dpi=150)
    plt.close(fig)
    figs._shrink(out / "time_codes.png")


timeline()

# ------------------------------------------------------------------------------------------------ the runs
base = run("vignette")
restricted = run("restricted", ["--restrictions", "pie_restrictions.tif"])
(data / "clue_restricted.toml").write_text(restricted["text"], encoding="utf-8")
built = run("built_fixed", ["--allowed", BUILT_ONLY])
lulcc_h1 = run("lulcc_matrix", ["--allowed", LULCC_MATRIX])
lulcc_hist = run("lulcc_matrix_history", ["--allowed", LULCC_MATRIX, "--history", "pie_history.tif"])
everything = run("every_rule", ["--allowed", LULCC_MATRIX, "--history", "pie_history.tif", "--restrictions",
                                "pie_restrictions.tif", "--neighbourhood", json.dumps({"rules": {"2": 0.3}})])
(data / "clue_rules.toml").write_text(everything["text"], encoding="utf-8")

# the restricted block over the map of 1985
lu = np.ma.masked_where(np.ma.getmaskarray(START), START.astype(float))
blk = np.ma.where(R, 9, lu)
pal = {k: v for k, v in CLASSES.items()}
pal[9] = ("Restricted block", "#1c2420")
figs.raster(None, out / "restrictions.png", array=blk, palette=pal, counts=True, width=820,
            title="The restricted block over the map of 1985")
inside_cells = int((R & ~np.ma.getmaskarray(START)).sum())
print("cells of the block with a class:", inside_cells, {CLASSES[k][0]: int(((START == k) & R).sum()) for k in CLASSES})

# with and without the block: the change 1985 -> 1999 by new class, the block outlined in both
for name, res in (("free", base), ("restricted", restricted)):
    figs.raster(None, out / f"change_{name}.png", array=change_code(res["maps"][-1], block=True), palette=CHANGE,
                legend=False, width=760)
figs.raster(None, out / "change_legend.png", array=np.ma.masked_invalid(np.array([[0.0, 1, 2, 3, 9]])),
            palette=CHANGE, width=320)
# where the built land of 1985-1999 went, in and out of the block
for name, res in (("free", base), ("restricted", restricted)):
    last = res["maps"][-1]
    nb = ((last == 2) & (START != 2)).filled(False)
    print(name, "new built inside", int((nb & R).sum()), "outside", int((nb & ~R).sum()),
          "changed outside", int(((last != START).filled(False) & ~R).sum()))

# the table of the runs
rows = []
for label, res in (("Vignette (elasticities only)", base), ("Restricted block", restricted),
                   ("Built land cannot change", built), ("lulcc's matrix, history 1", lulcc_h1),
                   ("lulcc's matrix, history map", lulcc_hist), ("Every rule", everything)):
    rows.append({"run": label, "first_year": res["first_year"], "changed": res["changed"],
                 "built_lost": res["trans"]["21"] + res["trans"]["23"],
                 "converged": f"{res['converged']} of 14", "agreement": round(res["overall"] * 100, 1),
                 "fom": round(res["fom"], 3)})
figs.write_csv(data / "runs.csv", rows)

# forest -> built year by year, with the code 1008
fb = []
for y in range(14):
    r = {"year": 1986 + y}
    for label, res in (("Vignette", base), ("lulcc's matrix, history 1", lulcc_h1),
                       ("lulcc's matrix, history map", lulcc_hist)):
        prev = START if y == 0 else res["maps"][y - 1]
        m = res["maps"][y]
        r[label] = int(((prev == 1) & (m == 2)).sum())
    fb.append(r)
figs.write_csv(data / "forest_to_built.csv", fb)
print("forest->built", fb)
for label, res in (("h1", lulcc_h1), ("hist", lulcc_hist)):
    print(label, "iterations", [int(float(s["iterations"])) for s in res["steps"]],
          "converged", [int(float(s["converged"])) for s in res["steps"]])

# ------------------------------------------------------------------------------------------------ neighbourhood
NEW = {0: ("Not built", GREY), 4: ("Built in 1985", "#d8b9cc"), 2: ("New built land", CLASSES[2][1])}
BOX = (230, 390, 50, 250)         # rows, columns of the close-up (the south-west)
nb_rows = []
frames = []
for th in (0.0, 0.2, 0.3, 0.4, 0.5, 0.6):
    extra = ["--allowed", BUILT_ONLY]
    if th:
        extra += ["--neighbourhood", json.dumps({"rules": {"2": th}})]
    res = built if th == 0 else run(f"nb_{th}", extra)
    last = res["maps"][-1]
    newb = ((last == 2) & (START != 2)).filled(False)
    lab, n = ndimage.label(newb, structure=np.ones((3, 3)))
    sizes = np.bincount(lab.ravel())[1:]
    code = np.where(newb, 2, np.where((START == 2).filled(False), 4, 0)).astype(float)
    code = np.ma.masked_where(np.ma.getmaskarray(START), code)
    r0, r1, c0, c1 = BOX
    figs.raster(None, out / f"nb_{th}.png", array=code[r0:r1, c0:c1], palette=NEW, legend=False, width=760)
    nb_rows.append({"threshold": th, "new_built": int(newb.sum()), "patches": int(n),
                    "mean_patch": round(float(sizes.mean()), 2) if n else 0,
                    "converged": res["converged"], "fom": round(res["fom"], 3)})
    print("nb", nb_rows[-1])
figs.write_csv(data / "neighbourhood.csv", nb_rows)
figs.raster(None, out / "nb_legend.png", array=np.ma.masked_invalid(np.array([[0.0, 4, 2]])), palette=NEW, width=320)
# where the close-up is
loc = np.ma.masked_where(np.ma.getmaskarray(START), START.astype(float))
for k in range(BOX[0], BOX[1]):
    for c in (BOX[2], BOX[3] - 1):
        loc[k, c] = 9
for c in range(BOX[2], BOX[3]):
    for k in (BOX[0], BOX[1] - 1):
        loc[k, c] = 9
pal2 = dict(CLASSES)
pal2[9] = ("Close-up", "#1c2420")
figs.raster(None, out / "nb_where.png", array=loc, palette=pal2, legend=False, width=300)

# the observed new built land in the same close-up, for comparison
obs_new = ((OBS99 == 2) & (START != 2)).filled(False)
lab, n = ndimage.label(obs_new, structure=np.ones((3, 3)))
print("observed new built", int(obs_new.sum()), "patches", n, "mean", round(float(np.bincount(lab.ravel())[1:].mean()), 2))
code = np.where(obs_new, 2, np.where((START == 2).filled(False), 4, 0)).astype(float)
code = np.ma.masked_where(np.ma.getmaskarray(START), code)
r0, r1, c0, c1 = BOX
figs.raster(None, out / "nb_observed.png", array=code[r0:r1, c0:c1], palette=NEW, legend=False, width=760)
print("images:", round(sum(p.stat().st_size for p in out.glob("*.png")) / 1e6, 2), "MB")

# a neighbourhood rule on built land without the matrix: it also applies to the built cells themselves, and those
# that fail it are converted automatically
for th in (0.3, 0.5):
    res = run(f"nb_own_{th}", ["--neighbourhood", json.dumps({"rules": {"2": th}})])
    print("own class rule", th, "built lost", res["trans"]["21"] + res["trans"]["23"], "first year", res["first_year"])
# a restricted block and a neighbourhood rule on forest that most of the block's forest fails: restricted cells
# whose own class is no longer allowed are still converted
res = run("restricted_nb_forest", ["--restrictions", "pie_restrictions.tif", "--neighbourhood",
                                   json.dumps({"rules": {"1": 0.9}})])
print("restricted block, forest rule 0.9: cells of the block changed", res["inside"])
