"""Figures of "Structural networks from MSPA" (habitat networks, part 6): GuidosToolbox's network of cores and
bridges of the forests of the CORINE landscape, its components and the importance of its links and nodes."""

import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import _clc  # noqa: E402
from _clc import figs, np  # noqa: E402
figs.plt = __import__("matplotlib.pyplot").pyplot

SLUG = "habitat-networks/mspa-network"
work = figs.workdir(SLUG)
out = figs.img_dir(SLUG)
data = figs.data_dir(SLUG)
_clc.setup(work)

MODEL = """format = "terraflux/1"

[[nodes]]
id = "land"
op = "raster.read"
params = { path = "clc_100m.tif" }

[[nodes]]
id = "mspa"
op = "landscape.mspa"
inputs = { map = "land.map" }
params = { foreground = [23, 24, 25], edge_width = 1 }

[[nodes]]
id = "components"
op = "networks.gtn_components"
inputs = { map = "mspa.map" }

[[nodes]]
id = "importance"
op = "networks.gtn_importance"
inputs = { map = "mspa.map" }

[[nodes]]
id = "conefor"
op = "networks.conefor"
inputs = { map = "mspa.map" }
"""
figs.run(figs.add_writes(MODEL, {"mspa.map": "mspa.tif", "components.map": "components.tif",
                                 "importance.map": "importance.tif", "importance.importance": "dpc.tif",
                                 "components.components": "comp_ids.tif", "components.summary": "summary.csv",
                                 "components.stats": "stats.csv", "importance.links": "links.csv",
                                 "importance.nodes": "nodes.csv", "conefor.nodes": "conefor_nodes.csv",
                                 "conefor.links": "conefor_links.csv"}), work)
o = work / "output"

# a window on the north-west, where the network is dense, and the whole map
CROP = (0, 260, 0, 300)
mspa = figs.read(work / "mspa.tif")[0]
figs.raster(None, out / "mspa.png", array=mspa, palette=figs.mspa_palette(), width=980)

# the networks: every component with a bridge in a colour of its own, isolated cores grey, bridges dark
cid = np.ma.filled(figs.read(work / "comp_ids.tif")[0], 0).astype(int)
mf = np.ma.filled(mspa, 0).astype(int)
bridge = np.isin(mf, [33, 35, 37, 133, 135, 137])
core = np.isin(mf, [17, 117])
has_bridge = set(np.unique(cid[bridge]).tolist()) - {0}
from matplotlib import colors as mc  # noqa: E402
rgba = np.ones(cid.shape + (4,))
rgba[(mf > 0) & (mf != 129) & (mf != 220) & (mf != 100)] = mc.to_rgba("#e3ece5")
rank = {c: k for k, c in enumerate(sorted(has_bridge, key=lambda c: -int((cid == c).sum())))}
for c in np.unique(cid[core]):
    if c == 0:
        continue
    col = figs.CLASSES[rank[c] % len(figs.CLASSES)] if c in rank else "#a7aca9"
    rgba[(cid == c) & core] = mc.to_rgba(col)
rgba[bridge] = mc.to_rgba(figs.INK)
fig = figs.plt.figure(figsize=(6.2, 6.2), dpi=150)
ax = fig.add_axes([0, 0, 1, 1])
ax.set_axis_off()
ax.imshow(rgba, interpolation="nearest")
fig.savefig(out / "components.png", dpi=150)
figs.plt.close(fig)
figs._shrink(out / "components.png")
print("components with a bridge:", len(has_bridge))

imp = figs.read(work / "importance.tif")[0]
ipal = {176: ("Core", "#9cc5a6"), 103: ("Bridge", "#3c3c3c"), 192: ("Link, top 1%", "#e8000b"),
        194: ("Link, top 5%", "#ff7f0e"), 195: ("Link, top 10%", "#ffbe3d"), 208: ("Node, top 1%", "#3f1d8f"),
        210: ("Node, top 5%", "#6a51a3"), 211: ("Node, top 10%", "#9e9ac8")}
figs.raster(None, out / "importance.png", array=np.ma.masked_equal(imp, 0), palette=ipal, width=980)

summary = figs.read_csv(o / "summary.csv")[0]
print("summary:", summary)
links = sorted(figs.read_csv(o / "links.csv"), key=lambda r: -float(r["dpc"]))
nodes = sorted(figs.read_csv(o / "nodes.csv"), key=lambda r: -float(r["dpc"]))
print("links:", len(links), "top:", [(r["link"], round(float(r["dpc"]), 2), r["pixels"]) for r in links[:5]])
print("stepping-stone nodes:", len(nodes), "top:", [(r["node"], round(float(r["dpc"]), 2), r["pixels"], r["degree"]) for r in nodes[:5]])
print("conefor nodes:", sum(1 for _ in open(o / "conefor_nodes.csv")) - 1, "links:", sum(1 for _ in open(o / "conefor_links.csv")) - 1)
v, c = np.unique(mspa.compressed(), return_counts=True)
fg = c[v > 0].sum() - c[v == 220].sum() if 220 in v else c[v > 0].sum()
print({int(a): round(100 * b / mspa.count(), 2) for a, b in zip(v, c)})

# a closer look at the most important links and nodes: their dPC (links on the bridges, nodes on the cores)
dpc = np.abs(np.ma.filled(figs.read(work / "dpc.tif")[0], 0))
r0, r1, c0, c1 = 20, 200, 330, 500
sub = dpc[r0:r1, c0:c1]
msub = mf[r0:r1, c0:c1]
rgba = np.ones(sub.shape + (4,))
rgba[(msub > 0) & (msub != 129) & (msub != 220) & (msub != 100)] = mc.to_rgba("#dfe9e1")
rgba[np.isin(msub, [17, 117])] = mc.to_rgba("#b9d3bf")
rgba[np.isin(msub, [33, 35, 37, 133, 135, 137])] = mc.to_rgba("#6b716d")
fig = figs.plt.figure(figsize=(6.4, 6.4 * sub.shape[0] / sub.shape[1] + 0.8), dpi=150)
ax = fig.add_axes([0, 0.13, 1, 0.87])
ax.set_axis_off()
ax.imshow(rgba, interpolation="nearest")
norm = mc.LogNorm(vmin=0.01, vmax=10)
im = ax.imshow(np.ma.masked_less(sub, 0.01), cmap="plasma", norm=norm, interpolation="nearest")
cax = fig.add_axes([0.2, 0.075, 0.6, 0.022])
cb = fig.colorbar(im, cax=cax, orientation="horizontal")
cb.outline.set_visible(False)
cb.ax.grid(False)
cb.ax.tick_params(labelsize=8)
cb.set_label("dPC of the link or stepping-stone core (%)", fontsize=8.5, color=figs.INK2)
fig.savefig(out / "importance-zoom.png", dpi=150)
figs.plt.close(fig)
figs._shrink(out / "importance-zoom.png")
