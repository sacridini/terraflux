"""Figures of "Forest gain and loss" (change-detection/forest-gain-and-loss).

Terraflux's ports of GuidosToolbox's change analyses, run on:

- the template *Forest gain and loss* as it is (a forest drawn by chance, cleared at its edges where a pressure
  is high);
- Sibuyan's forest (class 1 of sibuyan_landuse.tif) in 1997 and 2011, the data of parts 1 to 3: change.simple,
  change.morphological, change.fad at three windows, and fragmentation.fos + change.fos at three windows (a
  slider); and the 2011 forest compared with itself moved by one cell (a misregistration made here with numpy);
- Plum Island's forest (class 1 of pie_landuse.tif) in 1985 and 1999, where forest was also gained, at cells of
  100 m: simple and morphological change, FAD's change.

change.fos's map holds 100 + the connectivity before - the connectivity after on the forest of both dates: below
100 the forest became more connected, above 100 less. GuidosToolbox's own legend calls (before - after) "ΔFOS", so
its "+21 to +100" is a loss: the legend here says it in words.

    python website/figures/change-detection/forest-gain-and-loss.py
"""

import pathlib
import shutil
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import _common as c  # noqa: E402
from _common import figs, np, plt  # noqa: E402

SLUG = "forest-gain-and-loss"
work, img, data = c.setup(SLUG, files=("sibuyan_landuse.tif", "pie_landuse.tif"))
for p in img.glob("*.png"):
    p.unlink()

import rasterio  # noqa: E402

CHG = figs.gtb_colors("CHANGE")
SIMPLE = {11: ("Stable background", CHG[11]), 12: ("Gain", CHG[12]), 21: ("Loss", CHG[21]),
          22: ("Stable forest", CHG[22])}
MORPH = dict(SIMPLE)
MORPH[176] = ("Spurious change", CHG[176])
FC = figs.gtb_colors("FOS_CHANGE")
# change.fos's codes, grouped as GuidosToolbox groups them, named by what happened to the forest's connectivity
FOS_GROUPS = [(0, 79, "Gained 21-100 points"), (80, 89, "Gained 11-20"), (90, 98, "Gained 2-10"),
              (99, 101, "No change (±1)"), (102, 110, "Lost 2-10"), (111, 120, "Lost 11-20"),
              (121, 200, "Lost 21-100 points"), (250, 250, "Forest gained"), (251, 251, "Forest lost"),
              (252, 252, "Stable background")]
FOSPAL = {lo: (name, FC[lo] if lo != 121 else FC[150]) for lo, hi, name in FOS_GROUPS}


def fos_grouped(a):
    g = np.ma.masked_all(a.shape)
    for lo, hi, _ in FOS_GROUPS:
        g[(a >= lo) & (a <= hi)] = lo
    return g


def fmt(r):
    return {k: (round(v, 3) if isinstance(v, float) else v) for k, v in r.items()}


# ------------------------------------------------------------------------------------------------ the template
tw = figs.workdir(f"{c.CATEGORY}/{SLUG}-template")
tmpl = figs.template("forest-gain-and-loss")
figs.run(figs.add_writes(tmpl, {"before.map": "before.tif", "after.map": "after.tif", "simple.map": "simple.tif",
                                "morphological.map": "morph.tif", "simple.stats": "simple.csv",
                                "morphological.stats": "morph.csv", "fad.stats": "fad.csv"}), tw)
for k in ("simple.csv", "morph.csv", "fad.csv"):
    print("TEMPLATE", k)
    for r in c.table(tw / "output" / k):
        print("   ", fmt(r))
FOREST2 = {1: ("Background", "#e9e4d8"), 2: ("Forest", "#3b8a5a")}
for name in ("before", "after"):
    a = figs.read(tw / f"{name}.tif")[0]
    figs.raster(None, img / f"template_{name}.png", array=a, palette=FOREST2, legend=False, width=620)
for name, pal in (("simple", SIMPLE), ("morph", MORPH)):
    a = figs.read(tw / f"{name}.tif")[0]
    a = np.ma.masked_where(a == 255, a)
    figs.raster(None, img / f"template_{name}.png", array=a, palette=pal, legend=False, width=620)
figs.raster(None, img / "change_legend.png", array=np.ma.masked_invalid(np.array([[11.0, 12, 21, 22, 176]])),
            palette=MORPH, width=330)

# ------------------------------------------------------------------------------------------------ Sibuyan
# the 2011 forest moved by one cell to the east (a misregistration), as a map of its own
with rasterio.open(work / "sibuyan_landuse.tif") as ds:
    prof = ds.profile
    b2 = ds.read(2)
prof.update(count=1)
shifted = np.full_like(b2, 255)
shifted[:, 1:] = b2[:, :-1]
shifted[b2 == 255] = 255          # the island's outline kept: only the classes move
shifted[(shifted == 255) & (b2 != 255)] = 5
with rasterio.open(work / "sibuyan_2011_shifted.tif", "w", **prof) as o:
    o.write(shifted, 1)

MODEL = """format = "terraflux/1"

[[nodes]]
id = "before"
op = "raster.read"
params = { path = "sibuyan_landuse.tif", band = "1997" }

[[nodes]]
id = "after"
op = "raster.read"
params = { path = "sibuyan_landuse.tif", band = "2011" }

[[nodes]]
id = "simple"
op = "change.simple"
inputs = { before = "before.map", after = "after.map" }
params = { foreground = [1] }

[[nodes]]
id = "morphological"
op = "change.morphological"
inputs = { before = "before.map", after = "after.map" }
params = { foreground = [1] }

[[nodes]]
id = "fad"
op = "change.fad"
inputs = { before = "before.map", after = "after.map" }
params = { foreground = [1], windows = [7, 13, 27] }

[[nodes]]
id = "shifted"
op = "raster.read"
params = { path = "sibuyan_2011_shifted.tif" }

[[nodes]]
id = "misregistered"
op = "change.morphological"
inputs = { before = "after.map", after = "shifted.map" }
params = { foreground = [1] }

[[nodes]]
id = "misregistered_simple"
op = "change.simple"
inputs = { before = "after.map", after = "shifted.map" }
params = { foreground = [1] }
"""
writes = {"simple.map": "simple.tif", "morphological.map": "morph.tif", "misregistered.map": "misreg.tif",
          "misregistered_simple.map": "misreg_simple.tif",
          "simple.stats": "simple.csv", "morphological.stats": "morph.csv", "fad.stats": "fad.csv",
          "fad.matrix": "fad_matrix.csv", "misregistered.stats": "misreg.csv",
          "misregistered_simple.stats": "misreg_simple.csv"}
figs.run(figs.add_writes(MODEL, writes), work)
out = work / "output"
for k in ("simple.csv", "morph.csv", "fad.csv", "misreg.csv", "misreg_simple.csv"):
    print("SIBUYAN", k)
    for r in c.table(out / k):
        print("   ", fmt(r))


def crop(a, box):
    r0, r1, c0, c1 = box
    return a[r0:r1, c0:c1]


for name, src, pal in (("simple", "simple.tif", SIMPLE), ("morph", "morph.tif", MORPH),
                       ("misreg_simple", "misreg_simple.tif", SIMPLE), ("misreg", "misreg.tif", MORPH)):
    a = figs.read(work / src)[0]
    a = np.ma.masked_where(a == 255, a)
    figs.raster(None, img / f"{name}.png", array=a, palette=pal, counts=True, width=900)
    figs.raster(None, img / f"{name}_bare.png", array=a, palette=pal, legend=False, width=700)

# a close-up of the north-west, simple and morphological side by side
box = (5, 50, 5, 60)
fig, axes = plt.subplots(1, 2, figsize=(9.6, 4.3), dpi=150)
for ax, src, t in zip(axes, ("simple.tif", "morph.tif"), ("Simple change", "Morphological change")):
    a = crop(figs.read(work / src)[0].filled(255), box)
    c.classes_map(np.where(a == 255, np.nan, a), MORPH, ax)
    ax.set_title(t, loc="left", fontsize=11)
fig.legend(handles=c.legend_handles(MORPH), loc="lower center", ncol=5, fontsize=9.5, bbox_to_anchor=(0.5, -0.02))
fig.subplots_adjust(bottom=0.1, wspace=0.04)
c.save(fig, img / "closeup.png")

# the forest's cores: cells whose 5 x 5 window is all forest, at each date (what MCD needs to call change real)
from scipy import ndimage  # noqa: E402

for band, year in ((1, 1997), (2, 2011)):
    a = figs.read(work / "sibuyan_landuse.tif", band)[0]
    f = (a == 1).filled(False)
    core = ndimage.binary_erosion(f, structure=np.ones((5, 5)))
    print(year, "forest", int(f.sum()), "cores (5 x 5)", int(core.sum()))

# ------------------------------------------------------------------------------------------------ FOS change slider
FOS = """format = "terraflux/1"

[[nodes]]
id = "before"
op = "raster.read"
params = { path = "sibuyan_landuse.tif", band = "1997" }

[[nodes]]
id = "after"
op = "raster.read"
params = { path = "sibuyan_landuse.tif", band = "2011" }

[[nodes]]
id = "fos_before"
op = "fragmentation.fos"
inputs = { map = "before.map" }
params = { foreground = [1], window = WINDOW }

[[nodes]]
id = "fos_after"
op = "fragmentation.fos"
inputs = { map = "after.map" }
params = { foreground = [1], window = WINDOW }

[[nodes]]
id = "fos_change"
op = "change.fos"
inputs = { before = "fos_before.map", after = "fos_after.map" }
"""
frames = []
for wsize in (5, 7, 13, 27):
    w = figs.workdir(f"{c.CATEGORY}/{SLUG}-fos{wsize}")
    shutil.copy2(c.FILES["sibuyan_landuse.tif"], w / "sibuyan_landuse.tif")
    figs.run(figs.add_writes(FOS.replace("WINDOW", str(wsize)),
                             {"fos_change.map": "fos_change.tif", "fos_change.stats": "stats.csv",
                              "fos_change.classes": "classes.csv", "fos_before.map": "fos_a.tif",
                              "fos_after.map": "fos_b.tif"}), w)
    a = figs.read(w / "fos_change.tif")[0].filled(254).astype(int)
    vals, counts = np.unique(a, return_counts=True)
    print("FOS", wsize, dict(zip(vals.tolist(), counts.tolist())))
    fa = figs.read(w / "fos_a.tif")[0]
    fb = figs.read(w / "fos_b.tif")[0]
    common = (fa <= 100) & (fb <= 100)
    d = (fb - fa)[common.filled(False)]
    st = c.table(w / "output" / "stats.csv")[0]
    st["mean_drop"] = float(-d.mean())
    st["dropped_10"] = int((d <= -10).sum())
    st["changed_cells"] = int((a[(a >= 0) & (a <= 200)] != 100).sum())
    st["lost_cells"] = int(((a >= 102) & (a <= 200)).sum())
    print("   stats", fmt(st))
    print("   classes", c.table(w / "output" / "classes.csv"))
    figs.raster(None, img / f"fos_{wsize}.png", array=fos_grouped(a), palette=FOSPAL, width=700, legend=False)
    frames.append((wsize, st))
fig = plt.figure(figsize=(9, 0.95), dpi=150)
order = [lo for lo, _, _ in FOS_GROUPS]
fig.legend(handles=c.legend_handles(FOSPAL, order), loc="center", ncol=5, fontsize=9, handlelength=1.1,
           handleheight=1.1)
c.save(fig, img / "fos_legend.png")
for wsize, st in frames:
    print(f'  - {{ value: {wsize}, src: /img/tutorials/{c.CATEGORY}/{SLUG}/fos_{wsize}.png, stats: {{ '
          f'"Common forest": "{int(st["common"]):,}", "Lost connectivity": "{st["lost_cells"]:,}", '
          f'"Mean drop": "{st["mean_drop"]:.1f} points", "In another class": "{int(st["other_class"]):,}" }} }}')

# ------------------------------------------------------------------------------------------------ Plum Island
PIE = """format = "terraflux/1"

[[nodes]]
id = "before"
op = "raster.read"
params = { path = "pie_landuse.tif", band = "1985" }

[[nodes]]
id = "after"
op = "raster.read"
params = { path = "pie_landuse.tif", band = "1999" }

[[nodes]]
id = "simple"
op = "change.simple"
inputs = { before = "before.map", after = "after.map" }
params = { foreground = [1] }

[[nodes]]
id = "morphological"
op = "change.morphological"
inputs = { before = "before.map", after = "after.map" }
params = { foreground = [1] }

[[nodes]]
id = "fad"
op = "change.fad"
inputs = { before = "before.map", after = "after.map" }
params = { foreground = [1], windows = [7, 13, 27] }
"""
w = figs.workdir(f"{c.CATEGORY}/{SLUG}-pie")
shutil.copy2(c.FILES["pie_landuse.tif"], w / "pie_landuse.tif")
figs.run(figs.add_writes(PIE, {"simple.stats": "simple.csv", "morphological.stats": "morph.csv",
                               "fad.stats": "fad.csv", "morphological.map": "morph.tif", "simple.map": "simple.tif"}), w)
for k in ("simple.csv", "morph.csv", "fad.csv"):
    print("PIE", k)
    for r in c.table(w / "output" / k):
        print("   ", fmt(r))
pf = c.table(w / "output" / "fad.csv")
sf = c.table(out / "fad.csv")
rows = []
for r in sf:
    p = next(x for x in pf if x["scale"] == r["scale"])
    rows.append({"scale": "average" if r["scale"] == 0 else f"{int(r['scale'])} x {int(r['scale'])}",
                 "sibuyan_fad": round(r["delta_fad_av"], 2), "pie_fad": round(p["delta_fad_av"], 2),
                 "sibuyan_avcon": round(r["delta_avcon"], 2), "pie_avcon": round(p["delta_avcon"], 2)})
figs.write_csv(data / "fad_change.csv", rows)
for r in rows:
    print("FAD", r)

# PIE close-up: simple and morphological
pa = figs.read(w / "simple.tif")[0].filled(255)
pm = figs.read(w / "morph.tif")[0].filled(255)
box = (150, 290, 150, 330)
fig, axes = plt.subplots(1, 2, figsize=(9.6, 4.0), dpi=150)
for ax, a, t in zip(axes, (pa, pm), ("Simple change, Plum Island", "Morphological change, Plum Island")):
    a = crop(a, box)
    c.classes_map(np.where(a == 255, np.nan, a), MORPH, ax)
    ax.set_title(t, loc="left", fontsize=11)
fig.legend(handles=c.legend_handles(MORPH), loc="lower center", ncol=5, fontsize=9.5, bbox_to_anchor=(0.5, -0.02))
fig.subplots_adjust(bottom=0.1, wspace=0.04)
c.save(fig, img / "pie_closeup.png")
# the size of PIE's patches of loss: how many could hold a 5 x 5 block
loss = pa == 21
lab, n = ndimage.label(loss, structure=np.ones((3, 3)))
sizes = np.bincount(lab.ravel())[1:]
print("PIE patches of loss", n, "median", float(np.median(sizes)), "largest", int(sizes.max()),
      ">= 25 cells", int((sizes >= 25).sum()))
f85 = (figs.read(w / "pie_landuse.tif", 1)[0] == 1).filled(False)
f99 = (figs.read(w / "pie_landuse.tif", 3)[0] == 1).filled(False)
print("PIE forest cores 1985", int(ndimage.binary_erosion(f85, structure=np.ones((5, 5))).sum()),
      "background cores 1999", int(ndimage.binary_erosion(~f99, structure=np.ones((5, 5))).sum()))
print("images:", round(sum(p.stat().st_size for p in img.glob("*.png")) / 1e6, 2), "MB")

# ------------------------------------------------------------------------------------------------ a toy for the idea
# a forest of 24 x 30 cells; at the second date a clearing of 7 x 7 cells inside it, its eastern edge one cell back,
# and two single cells cleared: real runs of change.simple and change.morphological on the two toy maps
tw = figs.workdir(f"{c.CATEGORY}/{SLUG}-toy")
t1 = np.ones((24, 30), np.uint8)
t1[3:21, 3:27] = 2
t2 = t1.copy()
t2[8:15, 6:13] = 1            # a clearing
t2[3:21, 26] = 1              # the eastern edge moved back by one cell
t2[18, 16] = 1                # two cells cleared alone
t2[5, 20] = 1
t2[3:21, 2] = 2               # the western edge one cell out (gain)
prof = {"driver": "GTiff", "height": 24, "width": 30, "count": 1, "dtype": "uint8"}
for name, a in (("toy_1.tif", t1), ("toy_2.tif", t2)):
    with rasterio.open(tw / name, "w", **prof) as o:
        o.write(a, 1)
TOY = """format = "terraflux/1"

[[nodes]]
id = "before"
op = "raster.read"
params = { path = "toy_1.tif" }

[[nodes]]
id = "after"
op = "raster.read"
params = { path = "toy_2.tif" }

[[nodes]]
id = "simple"
op = "change.simple"
inputs = { before = "before.map", after = "after.map" }

[[nodes]]
id = "morphological"
op = "change.morphological"
inputs = { before = "before.map", after = "after.map" }
"""
figs.run(figs.add_writes(TOY, {"simple.map": "simple.tif", "morphological.map": "morph.tif",
                               "morphological.stats": "morph.csv"}), tw)
print("TOY", c.table(tw / "output" / "morph.csv"))
fig, axes = plt.subplots(1, 4, figsize=(11, 2.9), dpi=150)
pal2 = {1: ("Background", "#e9e4d8"), 2: ("Forest", "#3b8a5a")}
core = ndimage.binary_erosion(t1 == 2, structure=np.ones((5, 5)))
for ax, a, pal, t in ((axes[0], t1, pal2, "First date"), (axes[1], t2, pal2, "Second date"),
                      (axes[2], figs.read(tw / "simple.tif")[0].filled(255), MORPH, "Simple change"),
                      (axes[3], figs.read(tw / "morph.tif")[0].filled(255), MORPH, "Morphological change")):
    c.classes_map(np.asarray(a, float), pal, ax)
    ax.set_title(t, loc="left", fontsize=10.5)
# the cores of the first date, outlined on the first panel
ys, xs = np.where(core)
axes[0].contour(core.astype(float), levels=[0.5], colors="#1c2420", linewidths=0.9)
fig.legend(handles=c.legend_handles({**pal2, **{k: MORPH[k] for k in (12, 21, 176)}}), loc="lower center", ncol=5,
           fontsize=9, bbox_to_anchor=(0.5, -0.03))
fig.subplots_adjust(left=0.01, right=0.99, top=0.9, bottom=0.14, wspace=0.05)
c.save(fig, img / "toy.png")
