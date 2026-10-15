"""Figures of 'Simplified patterns, objects and distances' (habitat-patterns/spa-and-objects).

GuidosToolbox's sample forest map through SPA (2, 3, 5 and 6 classes), Accounting (objects by size), parcellation,
contortion, the Euclidean distance to the forest's edge, and the zones of influence and proximity of its objects.

    python website/figures/habitat-patterns/spa-and-objects.py
"""

import json
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import gtbfig as g  # noqa: E402
from gtbfig import figs, np  # noqa: E402

NAME = "habitat-patterns/spa-and-objects"
work = figs.workdir(NAME)
out = figs.img_dir(NAME)
data = figs.data_dir(NAME)
g.copy_data(g.GTB_INPUT, work)

nodes = [g.node("forest", "raster.read", path="input.tif")]
writes = {}
outputs = {}
for v in (2, 3, 5, 6):
    nodes.append(g.node(f"spa{v}", "landscape.spa", {"map": "forest.map"}, variant=v))
    writes[f"spa{v}.map"] = f"spa{v}.tif"
    outputs[f"spa{v}.stats"] = f"spa{v}_stats"
    outputs[f"spa{v}.porosity"] = f"spa{v}_porosity"
    outputs[f"spa{v}.objects"] = f"spa{v}_objects"
nodes.append(g.node("objects", "landscape.accounting", {"map": "forest.map"}, highlight_largest=True))
writes.update({"objects.map": "acc.tif", "objects.ids": "acc_ids.tif", "objects.area": "acc_area.tif"})
outputs.update({"objects.stats": "acc_stats", "objects.report": "acc_report"})
nodes.append(g.node("objects_auto", "landscape.accounting", {"map": "forest.map"}, thresholds="auto"))
outputs["objects_auto.stats"] = "acc_auto_stats"
nodes.append(g.node("parcels", "landscape.parcellation", {"map": "forest.map"}))
writes["parcels.parcels"] = "parcels.tif"
outputs.update({"parcels.stats": "parc_stats", "parcels.summary": "parc_summary"})
nodes.append(g.node("contortion", "landscape.contortion", {"map": "forest.map"}, min_area=100))
writes.update({"contortion.map": "cont.tif", "contortion.display": "cont_display.tif"})
nodes.append(g.node("distance", "distance.euclidean", {"map": "forest.map"}))
writes.update({"distance.map": "dist.tif", "distance.display": "dist_display.tif"})
outputs["distance.stats"] = "dist_stats"
nodes.append(g.node("zones", "distance.influence_zones", {"map": "forest.map"}, min_area=5000))
writes.update({"zones.map": "zones.tif", "zones.zones": "zones_id.tif", "zones.ids": "zones_obj.tif"})
outputs["zones.objects"] = "zones_objects"
nodes.append(g.node("proximity", "distance.objects_proximity", {"map": "forest.map"}, min_area=5000))
writes.update({"proximity.map": "prox.tif", "proximity.distance": "prox_dist.tif", "proximity.cag": "prox_cag.tif"})
outputs["proximity.lines"] = "prox_lines"
model = g.toml_nodes(*nodes)
model = figs.add_writes(model, writes)
for ref, name in outputs.items():
    model += f'\n[outputs.{name}]\nfrom = "{ref}"\n'
figs.run(model, work)
o = work / "output"

fo = g.read(work / "input.tif")
R, C = slice(650, 950), slice(250, 550)

# ------------------------------------------------------------------------------------------------ SPA
SPA = {17: ("Core", "#00c800"), 9: ("Islet", "#a03c00"), 5: ("Perforation", "#0000ff"), 3: ("Edge", "#000000"),
       1: ("Margin", "#ff8c00"), 100: ("Core-opening", "#888888"), 0: ("Background", "#dcdcdc"),
       129: ("Missing", "#ffffff")}
SPA2 = {17: ("Contiguous", "#00c800"), 1: ("Small linear features", "#ff8c00"), 0: ("Background", "#dcdcdc"),
        129: ("Missing", "#ffffff")}
for v in (2, 3, 5, 6):
    a = g.read(work / f"spa{v}.tif")
    pal = SPA2 if v == 2 else SPA
    g.classmap(a[R, C], out / f"spa{v}.png", pal, legend=False, width=520)
    print(v, {int(k): int(c) for k, c in zip(*np.unique(a, return_counts=True))})
g.classmap(g.read(work / "spa6.tif")[R, C], out / "spa6-legend.png", SPA, order=[17, 9, 5, 3, 1, 100, 0], width=900)
spa = {v: figs.read_csv(o / f"spa{v}_stats.csv") for v in (2, 3, 5, 6)}
por = {v: json.loads((o / f"spa{v}_porosity.json").read_text()) for v in (2, 3, 5, 6)}

# ------------------------------------------------------------------------------------------------ Accounting
ACC = {103: ("1: up to 50 cells", figs.gtb_colors("MSPA_ON")[103]), 33: ("2: 51-250", "#ff0000"),
       65: ("3: 251-1,000", "#ffff00"), 1: ("4: 1,001-5,000", "#ff8c00"), 9: ("5: 5,001-20,000", "#a03c00"),
       17: ("6: over 20,000", "#00c800"), 80: ("The three largest", "#ff00ff"), 0: ("Background", "#dcdcdc"),
       129: ("Missing", "#ffffff")}
acc = g.read(work / "acc.tif")
print("acc codes", {int(k): int(c) for k, c in zip(*np.unique(acc, return_counts=True))})
g.classmap(acc, out / "accounting.png", ACC, order=[103, 33, 65, 1, 9, 17, 80, 0], width=1000)
g.classmap(acc[R, C], out / "accounting-zoom.png", ACC, order=[103, 33, 65, 1, 9, 17, 80, 0], width=900)
rows = figs.read_csv(o / "acc_stats.csv")
chart = [{"class": f"{r['from']}-{r['to']}" if r["to"] else f"over {int(r['from']) - 1}",
          "objects": round(float(r["percent_of_objects"]), 2), "area": round(float(r["percent_of_area"]), 2)}
         for r in rows]
figs.write_csv(data / "accounting.csv", chart)
auto = figs.read_csv(o / "acc_auto_stats.csv")

# ------------------------------------------------------------------------------------------------ parcellation
parc = figs.read_csv(o / "parc_stats.csv")
parc_sum = figs.read_csv(o / "parc_summary.csv")

# ------------------------------------------------------------------------------------------------ contortion
cd = g.read(work / "cont_display.tif").astype(float)
cmap_vals = np.where(cd >= 3, cd, np.nan)
mask_bg = cd < 3
from matplotlib import colors as mcolors  # noqa: E402

cont_cols = figs.gtb_colors("CONTORTION")
cm = mcolors.ListedColormap([cont_cols[i] for i in range(3, 256)])
# background and neglected forest drawn under the contortion
under = np.where(cd == 2, 2, np.where(cd == 1, 1, 0))
g.classmap(under[R, C], work / "tmp.png", {0: ("", "#ffffff"), 1: ("", "#dcdcdc"), 2: ("", "#555555")},
           legend=False)
import matplotlib.pyplot as plt  # noqa: E402

for name, sl in (("contortion.png", (slice(None), slice(None))), ("contortion-zoom.png", (R, C))):
    a = cmap_vals[sl]
    base = under[sl]
    fig = plt.figure(figsize=(900 / 150, 900 / 150 * 0.8 * a.shape[0] / a.shape[1] + 0.05), dpi=150)
    ax = fig.add_axes([0.005, 0.01, 0.79, 0.98])
    ax.set_axis_off()
    ax.imshow(g._rgba(base, {0: ("", "#ffffff"), 1: ("", "#e6e6e6"), 2: ("", "#9a9a9a")}), interpolation="nearest")
    im = ax.imshow(a, cmap=cm, vmin=3, vmax=255, interpolation="nearest")
    cax = fig.add_axes([0.83, 0.15, 0.025, 0.7])
    cb = fig.colorbar(im, cax=cax, ticks=[3, 255])
    cb.ax.set_yticklabels(["least\ncontorted", "most\ncontorted"], fontsize=8.5)
    cb.outline.set_visible(False)
    fig.savefig(out / name, dpi=150)
    plt.close(fig)
    figs._shrink(out / name)

# ------------------------------------------------------------------------------------------------ distance
d = g.read(work / "dist.tif").astype(float)
d[fo == 0] = np.nan
g.ramp(d, out / "distance.png", cmap="PiYG", diverging=True, vmin=-40, vmax=40, width=1000,
       unit="cells to the forest's edge (+ inside, - outside)")
g.ramp(d[R, C], out / "distance-zoom.png", cmap="PiYG", diverging=True, vmin=-40, vmax=40, width=900,
       unit="cells (+ inside, - outside)")
dist_stats = figs.read_csv(o / "dist_stats.csv")
# a profile of the inside distances
inside = d[(fo == 2)]
hist = np.bincount(np.clip(inside.astype(int), 0, 60))[1:]
# the share of the forest further than d cells from its edge: MSPA's core for an edge width of d
# (a distance of at least d + 1 cells; within a few cells of MSPA's core counts, see the page)
deeper = [{"edge_width": dd, "forest": round(float((inside >= dd + 1).sum()) / inside.size * 100, 2)}
          for dd in range(0, 31)]
figs.write_csv(data / "distance-deeper.csv", deeper)

# ------------------------------------------------------------------------------------------------ influence zones
z = g.read(work / "zones.tif")
zid = g.read(work / "zones_id.tif")
zo = g.read(work / "zones_obj.tif")
print("zones codes", {int(k): int(c) for k, c in zip(*np.unique(z, return_counts=True))}[:0] if False else
      sorted({int(k) for k in np.unique(z)})[:30])
rng = np.random.default_rng(3)
n = int(zid.max())
cols = rng.uniform(0.55, 0.95, size=(n + 1, 3))
rgb = np.ones(zid.shape + (3,))
rgb[zid > 0] = cols[zid[zid > 0]]
rgb[zo > 0] = (0.12, 0.45, 0.25)
rgb[zid == 0] = (0.1, 0.1, 0.1)
rgb[fo == 0] = (1, 1, 1)
fig = plt.figure(figsize=(1000 / 150, 1000 / 150 * 0.66), dpi=150)
ax = fig.add_axes([0.005, 0.01, 0.65, 0.98])
ax.set_axis_off()
ax.imshow(rgb, interpolation="nearest")
from matplotlib.patches import Patch  # noqa: E402

fig.legend(handles=[Patch(facecolor=(0.12, 0.45, 0.25), label="Objects (5,000 cells or more)"),
                    Patch(facecolor=(0.8, 0.75, 0.9), label="Their zones of influence"),
                    Patch(facecolor=(0.1, 0.1, 0.1), label="Lines between zones")],
           loc="center left", bbox_to_anchor=(0.665, 0.5), fontsize=9, handlelength=1.1)
fig.savefig(out / "zones.png", dpi=150)
plt.close(fig)
figs._shrink(out / "zones.png")
zobj = figs.read_csv(o / "zones_objects.csv")

lines = figs.read_csv(o / "prox_lines.csv")
print("prox lines cols", list(lines[0].keys()) if lines else None, len(lines))

summary = {"spa": spa, "porosity": por, "acc": rows, "acc_auto": auto, "parc": parc, "parc_summary": parc_sum,
           "dist": dist_stats, "zones_objects": len(zobj), "prox_lines": len(lines)}
(work / "summary.json").write_text(json.dumps(summary, indent=1))
print(json.dumps(summary, indent=1)[:6000])
