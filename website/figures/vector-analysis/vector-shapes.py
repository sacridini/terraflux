"""Figures of 'Vector analysis' for tf.vector.split and tf.vector.shape: two images.

- select-join-overlay/split-road.png: the forest patches the planned road cuts, each piece coloured, the largest piece
  of a patch (part 1) in green and the others in other colours.
- shapes-lines-clusters/shape-circles.png: the largest forest patches with their maximum inscribed circles (the depth
  of the patch) and their minimum rotated rectangles.

    TERRAFLUX_PY=C:/path/to/terraflux-py python website/figures/vector-analysis/vector-shapes.py

The layers are those made by select-join-overlay.py (read from each part's download folder).
"""

from _common import *  # noqa: F403

v = tf.vector

# ---- the road cutting the forest (part 1) ----
out = figs.img_dir(f"{CATEGORY}/select-join-overlay")
d = figs.data_dir(f"{CATEGORY}/select-join-overlay")
forest = tf.load_vector(d / "forest.gpkg")
road = tf.load_vector(d / "road.gpkg")
cut = v.split(forest, road)
hit = cut[cut.parts > 1]
print("patches cut:", hit.patch.nunique(), "pieces:", len(hit))

fig, ax = base_axes()
forest.plot(ax=ax, color=FOREST, alpha=0.22, linewidth=0)
for _, g in hit.groupby("patch"):
    g = g.sort_values("part")
    g[g.part == 1].plot(ax=ax, color=FOREST, edgecolor="white", linewidth=0.5)
    rest = g[g.part > 1]
    if len(rest):
        rest.plot(ax=ax, color=EMBER, edgecolor="white", linewidth=0.5)
road.plot(ax=ax, color=RED, linewidth=1.3, zorder=5)
ax.set_title("Forest patches cut by the planned road", fontsize=9, fontweight="normal", color=INK2)
ax.legend(handles=[Patch(facecolor=FOREST, label="largest piece (part 1)"),
                   Patch(facecolor=EMBER, label="other pieces of a cut patch"),
                   Patch(facecolor=FOREST, alpha=0.22, label="patch the road does not cut"),
                   Line2D([0], [0], color=RED, lw=1.3, label="planned road")],
          loc="lower right", fontsize=7.5, frameon=True, facecolor="white", edgecolor=RULE, framealpha=0.92)
save(fig, out / "split-road.png")
print("made", out / "split-road.png")

# ---- inscribed circles and rectangles (part 2) ----
out = figs.img_dir(f"{CATEGORY}/shapes-lines-clusters")
d = figs.data_dir(f"{CATEGORY}/shapes-lines-clusters")
forest = tf.load_vector(d / "forest.gpkg")
big = forest[forest.area_ha >= 300].reset_index(drop=True)
circles = v.shape(big, geometry="inscribed")
boxes = v.shape(big, geometry="rectangle")
print("patches drawn:", len(big), big.patch.tolist())

fig, ax = base_axes()
forest.plot(ax=ax, color=FOREST, alpha=0.22, linewidth=0)
big.plot(ax=ax, color=FOREST, alpha=0.55, edgecolor="white", linewidth=0.5)
boxes.boundary.plot(ax=ax, color=VIOLET, linewidth=1.2, linestyle=(0, (4, 2)))
circles.plot(ax=ax, color=EMBER, alpha=0.55, edgecolor=INK, linewidth=0.8, zorder=4)
for _, r in circles.iterrows():
    c = r.geometry.centroid
    ax.annotate(f"{r.patch}", (c.x, c.y), ha="center", va="center", fontsize=7.5, fontweight="bold", color=INK,
                zorder=6)
ax.set_title("Largest forest patches: deepest circle and minimum rotated rectangle", fontsize=9,
             fontweight="normal", color=INK2)
ax.legend(handles=[Patch(facecolor=FOREST, alpha=0.55, label="patch of 300 ha or more (its number)"),
                   Patch(facecolor=EMBER, alpha=0.55, edgecolor=INK, label="maximum inscribed circle"),
                   Line2D([0], [0], color=VIOLET, lw=1.2, linestyle=(0, (4, 2)), label="minimum rotated rectangle")],
          loc="lower right", fontsize=7.5, frameon=True, facecolor="white", edgecolor=RULE, framealpha=0.92)
save(fig, out / "shape-circles.png")
print("made", out / "shape-circles.png")
