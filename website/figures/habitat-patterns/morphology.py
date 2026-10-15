"""Figures of 'The idea of morphological pattern analysis' (habitat-patterns/morphology).

A small drawn forest (a toy grid) run through Terraflux's landscape.mspa, drawn cell by cell: its classes, the
steps behind them (erosion, the edge, the strips left), 4 and 8 neighbours, three edge widths; and the core-area
model as a chart.

    python website/figures/habitat-patterns/morphology.py
"""

import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import gtbfig as g  # noqa: E402
from gtbfig import figs, np, plt  # noqa: E402
from matplotlib.patches import Patch, Rectangle  # noqa: E402

NAME = "habitat-patterns/morphology"
work = figs.workdir(NAME)
out = figs.img_dir(NAME)

# ------------------------------------------------------------------------------------------------ the toy forest
toy = np.ones((26, 44), dtype="uint8")       # 1 background
toy[3:14, 3:14] = 2                          # block A
toy[5:21, 23:36] = 2                         # block B
toy[8:10, 14:23] = 2                         # a strip joining A and B
toy[9:16, 27:33] = 1                         # a clearing inside B
toy[12, 29:31] = 2                           # a few trees in the clearing
toy[7:19, 38] = 2                            # a strip leaving B ...
toy[7, 36:38] = 2
toy[18, 36:38] = 2                           # ... and coming back to it
toy[14:22, 8] = 2                            # a dead-end strip from A
toy[21:23, 3:5] = 2                          # a small fragment
toy[2, 18:32] = 2                            # a long thin strip
toy[21, 24] = toy[22, 25] = toy[23, 26] = 2  # a spur from B, cells touching by their corners
toy[22:24, 14:17] = 2                        # a fragment ...
toy[21, 13] = toy[20, 12] = 2                # ... with a diagonal tail
g.write_grid(toy, work / "toy.tif")

nodes = [g.node("toy", "raster.read", path="toy.tif")]
runs = {}
for conn in (8, 4):
    for ew in (1, 2, 3):
        for internal in (True, False):
            key = f"m{conn}_{ew}_{int(internal)}"
            nodes.append(g.node(key, "landscape.mspa", {"map": "toy.map"}, edge_width=ew, connectivity=conn,
                                internal=internal))
            nodes.append(g.node("w_" + key, "raster.write", {"value": f"{key}.map"}, path=f"{key}.tif"))
            runs[key] = f"{key}.tif"
figs.run(g.toml_nodes(*nodes), work)
M = {k: g.read(work / v) for k, v in runs.items()}

FOREST = {1: ("Background", "#e9e4d8"), 2: ("Forest", figs.FOREST)}
g.classmap(toy, out / "toy-forest.png", FOREST, order=[2, 1], grid=True, width=1000, legend_width=0.2)

# the classes, with names on the map
m = M["m8_1_1"]
labels = []
pal = dict(g.MSPA_FOLDED)
fold = g.mspa_fold(m)


def draw_toy(ax, a, title=None, pal=pal):
    ax.imshow(g._rgba(a, pal), interpolation="nearest")
    h, w = a.shape
    ax.set_xticks(np.arange(-0.5, w, 1), minor=True)
    ax.set_yticks(np.arange(-0.5, h, 1), minor=True)
    ax.grid(which="minor", color="#00000026", linewidth=0.4, linestyle="-")
    ax.grid(which="major", visible=False)
    ax.tick_params(which="both", length=0, labelbottom=False, labelleft=False)
    for s in ax.spines.values():
        s.set_visible(True)
        s.set_color("#00000040")
    if title:
        ax.set_title(title, loc="left", fontsize=11)


def arrow(ax, xy, text, xytext):
    ax.annotate(text, xy=(xy[1], xy[0]), xytext=(xytext[1], xytext[0]), ha="center", va="center", fontsize=8.6,
                color=figs.INK, bbox=dict(boxstyle="round,pad=0.25", fc="#ffffffee", ec="#00000040", lw=0.5),
                arrowprops=dict(arrowstyle="-", color=figs.INK, lw=0.7, shrinkA=0, shrinkB=1))


fig = plt.figure(figsize=(1000 / 150, 1000 / 150 * 26 / 44 * 0.80 + 0.1), dpi=150)
ax = fig.add_axes([0.0, 0.0, 0.80, 1.0])
draw_toy(ax, fold)
arrow(ax, (8, 8), "Core", (8, 8))
arrow(ax, (13, 10.5), "Edge", (17, 15.5))
arrow(ax, (8.5, 18), "Bridge", (5, 18.5))
arrow(ax, (2, 25), "Islet (a thin strip)", (0.4, 35))
arrow(ax, (22, 4), "Islet", (24.6, 6))
arrow(ax, (17, 8), "Branch", (17, 4))
arrow(ax, (23, 26), "Branch", (24.6, 21.5))
arrow(ax, (15, 30), "Perforation", (24.6, 31))
arrow(ax, (10, 28), "Core-opening", (3.3, 26.5))
arrow(ax, (12, 29.5), "Islet in the opening", (24.6, 41))
arrow(ax, (15, 38), "Loop", (17, 41.5))
arrow(ax, (12, 36.5), "Border-opening", (3.3, 40))
handles = [Patch(facecolor=pal[k][1], edgecolor="#00000033", label=pal[k][0]) for k in g.MSPA_ORDER[:-1]]
fig.legend(handles=handles, loc="center left", bbox_to_anchor=(0.81, 0.5), fontsize=9, handlelength=1.1,
           handleheight=1.1, labelspacing=0.5)
fig.savefig(out / "toy-mspa.png", dpi=150)
plt.close(fig)
figs._shrink(out / "toy-mspa.png")

# ------------------------------------------------------------------------------------------------ the steps
fg = toy == 2
core = M["m8_1_0"] == 17
# the core grown back by one cell (the structuring element again), within the forest
grown = np.zeros_like(core)
for dr in (-1, 0, 1):
    for dc in (-1, 0, 1):
        grown |= np.roll(np.roll(core, dr, 0), dc, 1)
ring = grown & fg & ~core
rest = fg & ~grown

S = {0: ("", "#e9e4d8"), 1: ("", "#b9d8c3"), 2: ("", _c := figs.FOREST), 3: ("", g.MSPA_FOLDED[17][1]),
     4: ("", "#000000"), 5: ("", "#ff8c00")}
a1 = np.where(fg, 2, 0)
a2 = np.where(core, 3, np.where(fg, 1, 0))
a3 = np.where(core, 3, np.where(ring, 4, np.where(fg, 1, 0)))
a4 = np.where(core, 3, np.where(ring, 4, np.where(rest, 5, 0)))
fig, axs = plt.subplots(2, 2, figsize=(1050 / 150, 1050 / 150 * 26 / 44 * 1.02), dpi=150)
draw_toy(axs[0, 0], a1, "1. The forest, a 3 x 3 element", pal=S)
# the 3 x 3 element at a cell where it fits and one where it does not
for (r, c), ok in (((6, 6), True), ((3, 16), False)):
    axs[0, 0].add_patch(Rectangle((c - 1.5, r - 1.5), 3, 3, fill=False, ec="#1f77b4" if ok else "#d62728", lw=1.6))
axs[0, 0].annotate("fits: core", (6, 9.5), fontsize=8, color="#1f77b4", ha="center", bbox=dict(boxstyle="round,pad=0.2", fc="#ffffffdd", ec="none"))
axs[0, 0].annotate("does not fit", (16, 6.3), fontsize=8, color="#d62728", ha="center", bbox=dict(boxstyle="round,pad=0.2", fc="#ffffffdd", ec="none"))
draw_toy(axs[0, 1], a2, "2. Erosion: the core", pal=S)
draw_toy(axs[1, 0], a3, "3. The core grown back: its edges", pal=S)
draw_toy(axs[1, 1], a4, "4. What is left: strips, fragments", pal=S)
fig.tight_layout(pad=0.4, h_pad=0.8, w_pad=0.6)
fig.savefig(out / "steps.png", dpi=150)
plt.close(fig)
figs._shrink(out / "steps.png")
# Terraflux's core is the erosion by the 3 x 3 square (checked here, so the figure says what MSPA does)
er = fg.copy()
pad = np.pad(fg, 1, constant_values=False)
for dr in (0, 1, 2):
    for dc in (0, 1, 2):
        er &= pad[dr:dr + fg.shape[0], dc:dc + fg.shape[1]]
assert (er == core).all(), "the core is not the erosion by the 3x3 square"

# ------------------------------------------------------------------------------------------------ 4 and 8 neighbours
fig = plt.figure(figsize=(1000 / 150, 2.2), dpi=150)
# the two neighbourhoods
for i, (name, cells) in enumerate((("4 neighbours", [(0, 1), (1, 0), (1, 2), (2, 1)]),
                                   ("8 neighbours", [(r, c) for r in range(3) for c in range(3) if (r, c) != (1, 1)]))):
    ax = fig.add_axes([0.01 + i * 0.16, 0.12, 0.14, 0.64])
    a = np.zeros((3, 3), int)
    for r, c in cells:
        a[r, c] = 1
    a[1, 1] = 2
    draw_toy(ax, a, pal={0: ("", "#f4f1ea"), 1: ("", "#9fc9ad"), 2: ("", figs.FOREST)})
    ax.set_title(name, fontsize=10, loc="center")
# the spur of the toy at both connectivities
r0, r1, c0, c1 = 17, 25, 19, 31
for i, conn in enumerate((8, 4)):
    ax = fig.add_axes([0.36 + i * 0.32, 0.04, 0.30, 0.82])
    draw_toy(ax, g.mspa_fold(M[f"m{conn}_1_1"])[r0:r1, c0:c1])
    ax.set_title(f"connectivity = {conn}", fontsize=10, loc="left")
fig.savefig(out / "connectivity.png", dpi=150)
plt.close(fig)
figs._shrink(out / "connectivity.png")

# ------------------------------------------------------------------------------------------------ edge widths
for ew in (1, 2, 3):
    fig = plt.figure(figsize=(600 / 150, 600 / 150 * 26 / 44), dpi=150)
    ax = fig.add_axes([0, 0, 1, 1])
    draw_toy(ax, g.mspa_fold(M[f"m8_{ew}_1"]))
    fig.savefig(out / f"edge-{ew}.png", dpi=150)
    plt.close(fig)
    figs._shrink(out / f"edge-{ew}.png")

rows = []
for ew in (1, 2, 3):
    s = g.mspa_shares(M[f"m8_{ew}_1"])
    rows.append({"edge_width": ew, **s})
figs.write_csv(figs.data_dir(NAME) / "toy-edge-width.csv", rows)
print(rows)

# ------------------------------------------------------------------------------------------------ the core-area model
side = np.linspace(1, 1000, 400)


def draw(ax):
    for d, col in ((25, "#8cc864"), (50, figs.FOREST), (100, "#1d5c38"), (200, figs.INK)):
        share = np.clip(side - 2 * d, 0, None) ** 2 / side ** 2 * 100
        ax.plot(side, share, color=col, lw=2, label=f"edge effects reach {d} m")
    ax.set_xlabel("Side of a square forest patch (m)")
    ax.set_ylabel("Core, % of the patch")
    ax.set_ylim(0, 100)
    ax.set_xlim(0, 1000)
    ax.legend(loc="lower right")


figs.chart(out / "core-area.png", draw, width=900, height=420)
print("figures in", out)

# ------------------------------------------------------------------------------------------------ the template
work2 = figs.workdir(NAME + "-template")
model = figs.add_writes(figs.template("mspa-of-a-forest"), {"landscape.map": "landscape.tif", "mspa.map": "mspa.tif"})
figs.run(model, work2)
land = g.read(work2 / "landscape.tif")
g.classmap(land, out / "template-landscape.png",
           {1: ("1 farming", "#d9a441"), 2: ("2 water", "#3f7fae"), 3: ("3 forest", figs.FOREST)}, order=[3, 1, 2],
           width=640, legend_width=0.32, counts=True)
g.classmap(g.mspa_fold(g.read(work2 / "mspa.tif")), out / "template-mspa.png", g.MSPA_FOLDED,
           order=g.MSPA_ORDER[:-1], width=640, legend_width=0.32)
print(g.mspa_shares(g.read(work2 / "mspa.tif")))
