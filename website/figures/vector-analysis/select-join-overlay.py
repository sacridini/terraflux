"""Figures of 'Vector analysis, part 1': makes the layers of both parts (their download folders) and draws the map the
part opens with: the forest, the water, the reserves, the planned road with its 500 m buffer, and the records, those
inside a reserve apart from the others.

    TERRAFLUX_PY=C:/path/to/terraflux-py python website/figures/vector-analysis/select-join-overlay.py

The records are simulated (a random generator with a fixed seed); everything else is cut from the CORINE excerpt that
ships with the library, or drawn for the tutorial (see _common.py). The Python is the library's `tf.vector`.
"""

from _common import *  # noqa: F403

v = tf.vector
SLUG = f"{CATEGORY}/select-join-overlay"
out = figs.img_dir(SLUG)

layers = make_layers()
raw, noded = make_cover()
write_layers(layers, raw, noded)

forest, water, towns, reserves = layers["forest"], layers["water"], layers["towns"], layers["reserves"]
road, records, window = layers["road"], layers["records"], layers["window"]

inside = v.relate(records, reserves, "within")
buf = v.buffer(road, 500)

fig, ax = base_axes((6.8, 6.4))
draw(ax, forest, color=FOREST, alpha=0.55, linewidth=0)
draw(ax, water, color=WATER, linewidth=0)
draw(ax, towns, color=TOWN, alpha=0.55, linewidth=0)
draw(ax, buf, facecolor="none", edgecolor=RED, linewidth=0.9, linestyle=(0, (4, 2)))
draw(ax, road, color=RED, linewidth=1.8)
draw(ax, reserves.boundary, color=EMBER, linewidth=1.8)
draw(ax, records[~inside], color="#4a4f4c", markersize=7, linewidth=0)
draw(ax, records[inside], color="white", edgecolor=INK, markersize=11, linewidth=0.6)
handles = [
    Patch(facecolor=FOREST, alpha=0.55, label="Forest patches"),
    Patch(facecolor=WATER, label="Lakes and a river"),
    Patch(facecolor=TOWN, alpha=0.55, label="Towns"),
    Line2D([0], [0], color=EMBER, lw=2, label="Reserves"),
    Line2D([0], [0], color=RED, lw=2, label="Planned road, with its 500 m buffer"),
    Line2D([0], [0], marker="o", color="none", markerfacecolor="white", markeredgecolor=INK, markersize=6,
           label=f"Records inside a reserve ({int(inside.sum())})"),
    Line2D([0], [0], marker="o", color="none", markerfacecolor="#4a4f4c", markeredgecolor="#4a4f4c", markersize=5,
           label=f"Records outside ({int((~inside).sum())})"),
]
ax.legend(handles=handles, loc="upper center", bbox_to_anchor=(0.5, -0.02), ncol=2, fontsize=8.5, handlelength=1.4)
save(fig, out / "map.png")
print("made", out / "map.png")
