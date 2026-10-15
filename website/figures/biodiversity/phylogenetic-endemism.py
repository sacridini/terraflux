"""Figures of "Phylogenetic diversity, endemism and CANAPE" (biodiversity, part 2): the example tree, PD, PE and
relative PE in neighbourhoods of 200 km, their p-ranks against 999 randomisations (rand_structured) and CANAPE's
classes, from real runs of terraflux (bio.table, bio.tree, bio.spatial, bio.randomise)."""

import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import _bio  # noqa: E402
from _bio import figs, np, plt  # noqa: E402

SLUG = "biodiversity/phylogenetic-endemism"
work = figs.workdir(SLUG)
out = figs.img_dir(SLUG)
data = figs.data_dir(SLUG)
_bio.setup(work)

MODEL = """format = "terraflux/1"

[[nodes]]
id = "sites"
op = "bio.table"
params = { path = "sites.csv", group_columns = ["x", "y"], label_columns = ["genus", "species"], cell_sizes = [100000, 100000] }

[[nodes]]
id = "tree"
op = "bio.tree"
params = { path = "tree.nex" }

[[nodes]]
id = "indices"
op = "bio.spatial"
inputs = { basedata = "sites.basedata", tree = "tree.tree" }
params = { conditions = ["sp_circle (radius => 200000)"], calculations = ["richness", "endemism_whole", "pd", "pe", "phylo_rpe2"] }

[[nodes]]
id = "random"
op = "bio.randomise"
inputs = { basedata = "sites.basedata", tree = "tree.tree" }
params = { conditions = ["sp_circle (radius => 200000)"], calculations = ["richness", "pd", "pe", "phylo_rpe2"], function = "rand_structured", iterations = 999, rand_seed = 2024 }
"""
figs.run(figs.add_writes(MODEL, {"indices.maps": "indices.tif", "indices.table": "indices.csv",
                                 "random.maps": "random.tif", "random.canape": "canape.csv",
                                 "random.p_rank": "p_rank.csv"}), work)
o = work / "output"


def band(tif, name):
    import rasterio

    with rasterio.open(tif) as ds:
        i = list(ds.descriptions).index(name) + 1
        a = ds.read(i, masked=True).astype(float)
        b = ds.bounds
    return a, (b.left, b.right, b.bottom, b.top)


ext = band(work / "indices.tif", "PD")[1]
for name, label, cmap in (("RICHNESS_ALL", "Species", "viridis"), ("PD", "Phylogenetic diversity", "viridis"),
                          ("PE_WE", "Phylogenetic endemism", "magma_r"), ("PHYLO_RPE2", "Relative PE (RPE)", "PuOr_r")):
    a = band(work / "indices.tif", name)[0]
    lim = dict(vmin=0, vmax=2) if name == "PHYLO_RPE2" else {}
    _bio.cell_map(a, out / f"{name.lower()}.png", cmap=cmap, unit=label, extent=ext, **lim)

# p-rank of PE: below 0.025 low, above 0.975 high
pr = band(work / "random.tif", "P_RANK_PE_WE")[0]
cls = np.ma.masked_array(np.full(pr.shape, 2.0), mask=np.ma.getmaskarray(pr))
cls[pr <= 0.05] = 1
cls[pr <= 0.01] = 0
cls[pr >= 0.95] = 3
cls[pr >= 0.99] = 4
pal = {0: ("Lower than chance (p ≤ 0.01)", "#2166ac"), 1: ("Lower (p ≤ 0.05)", "#92c5de"),
       2: ("Not significant", "#e7e7e7"), 3: ("Higher (p ≥ 0.95)", "#f4a582"), 4: ("Higher than chance (p ≥ 0.99)", "#b2182b")}
_bio.cell_map(cls, out / "prank_pe.png", palette=pal, extent=ext, width=900)
print("p-rank PE classes", {pal[k][0]: int((cls == k).sum()) for k in pal})

can = band(work / "random.tif", "CANAPE_CODE")[0]
cpal = {0: ("Not significant", "#e7e7e7"), 1: ("Neo-endemism", "#d7191c"), 2: ("Palaeo-endemism", "#2b83ba"),
        3: ("Mixed endemism", "#9e5ac8"), 4: ("Super-endemism", "#5e3c99")}
_bio.cell_map(can, out / "canape.png", palette=cpal, extent=ext, width=900)
rows = figs.read_csv(o / "canape.csv")
counts = {cpal[k][0]: sum(int(float(r["CANAPE_CODE"])) == k for r in rows) for k in cpal}
print("CANAPE", counts)
figs.write_csv(data / "canape.csv", [{"class": k, "cells": v} for k, v in counts.items()])

ind = figs.read_csv(o / "indices.csv")
rk = lambda v: np.argsort(np.argsort(v))  # noqa: E731
R = np.array([float(r["RICHNESS_ALL"]) for r in ind])
PD = np.array([float(r["PD"]) for r in ind])
PE = np.array([float(r["PE_WE"]) for r in ind])
WE = np.array([float(r["ENDW_WE"]) for r in ind])
print("Spearman R-PD", round(float(np.corrcoef(rk(R), rk(PD))[0, 1]), 2), "WE-PE",
      round(float(np.corrcoef(rk(WE), rk(PE))[0, 1]), 2), "max PD", PD.max().round(2), "max PE", PE.max().round(3),
      "max R", R.max())
figs.write_csv(data / "pd-vs-richness.csv", [{"richness": float(r), "pd": round(float(p), 3)} for r, p in zip(R, PD)])

# ---------------------------------------------------------------- the tree
from Bio import Phylo  # noqa: E402

tree = Phylo.read(str(work / "tree.nex"), "nexus")
tips = tree.get_terminals()
ypos = {t: i for i, t in enumerate(tips)}
depths = tree.depths()


def ycoord(cl):
    if cl.is_terminal():
        return ypos[cl]
    ys = [ycoord(c) for c in cl.clades]
    ypos[cl] = (min(ys) + max(ys)) / 2
    return ypos[cl]


ycoord(tree.root)
fig, ax = plt.subplots(figsize=(6.2, 6.6), dpi=150)
for cl in tree.find_clades():
    x1 = depths[cl]
    y1 = ypos[cl]
    for c in cl.clades:
        x2 = depths[c]
        ax.plot([x1, x1], [y1, ypos[c]], color=figs.INK2, lw=1)
        ax.plot([x1, x2], [ypos[c], ypos[c]], color=figs.INK2, lw=1)
for t in tips:
    ax.text(depths[t] + 0.02, ypos[t], t.name.replace("Genus:", "").strip("'"), va="center", fontsize=7.5, color=figs.INK)
ax.set_ylim(len(tips), -1)
ax.set_yticks([])
ax.grid(False)
ax.spines["left"].set_visible(False)
ax.set_xlabel("Branch length (from the root)")
fig.tight_layout()
fig.savefig(out / "tree.png", dpi=150)
plt.close(fig)
figs._shrink(out / "tree.png")
print("tips", len(tips), "root to tip", round(max(depths[t] for t in tips), 3), "total length",
      round(tree.total_branch_length(), 3))
