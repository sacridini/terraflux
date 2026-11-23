"""Figures of 'Vector analysis, part 2': one image of three panels. At the left, the Voronoi catchments of six
stations cut to the study area, with the records coloured by the catchment that holds them. Beside it, a patch of a
polygon coverage simplified polygon by polygon (`simplify`) and all together (`coverage_simplify`) with the same
tolerance: the first leaves gaps and overlaps, the second none.

    TERRAFLUX_PY=C:/path/to/terraflux-py python website/figures/vector-analysis/shapes-lines-clusters.py

The layers are those made by select-join-overlay.py (read from the part's download folder).
"""

from _common import *  # noqa: F403

v = tf.vector
SLUG = f"{CATEGORY}/shapes-lines-clusters"
out = figs.img_dir(SLUG)
d = figs.data_dir(SLUG)

stations = tf.load_vector(d / "stations.gpkg")
records = tf.load_vector(d / "records.gpkg")
window = tf.load_vector(d / "window.gpkg")
forest = tf.load_vector(d / "forest.gpkg")
cover = tf.load_vector(d / "cover.gpkg")

TOL = 60
cells = v.voronoi(stations, clip=window)
homes = v.join(records, cells, "within", how="first")
simple = v.simplify(cover, TOL)
joint = v.coverage_simplify(cover, TOL)

# where the polygons simplified alone leave the largest gap: the box the two right panels zoom to
union = shapely.union_all(simple.geometry.values)
gaps = shapely.difference(shapely.box(*cover.total_bounds), union)
gap_parts = sorted(shapely.get_parts(gaps), key=lambda g: -g.area)
cx, cy = shapely.get_coordinates(gap_parts[0].representative_point())[0]
half = 450
zoom = (cx - half, cy - half, cx + half, cy + half)
print("gaps:", len(gap_parts), "largest", round(gap_parts[0].area), "m2 at", round(cx), round(cy))

COVER_COLORS = {2: TOWN, 7: "#a89f91", 12: "#eadfb4", 18: "#d9e08a", 21: "#e6cf86", 24: FOREST, 36: "#8fa7cf"}


def coverage_panel(ax, layer, title):
    for cls, g in layer.groupby("class"):
        g.plot(ax=ax, color=COVER_COLORS.get(cls, "#dddddd"), edgecolor="white", linewidth=0.5)
    pol = layer.geometry.values
    tree = shapely.STRtree(pol)
    ia, ib = tree.query(pol, predicate="intersects")
    m = ia < ib
    over = shapely.intersection(pol[ia[m]], pol[ib[m]])
    over = [g for g in shapely.get_parts(over) if g.geom_type in ("Polygon", "MultiPolygon") and g.area > 0.5]
    hole = [g for g in shapely.get_parts(shapely.difference(shapely.box(*cover.total_bounds), shapely.union_all(pol)))
            if g.area > 0.5]
    for g in hole:
        gpd.GeoSeries([g]).plot(ax=ax, color=RED, linewidth=0)
    for g in over:
        gpd.GeoSeries([g]).plot(ax=ax, color=INK, linewidth=0)
    ax.set_xlim(zoom[0], zoom[2])
    ax.set_ylim(zoom[1], zoom[3])
    ax.set_aspect("equal")
    ax.set_xticks([])
    ax.set_yticks([])
    ax.set_title(title, fontsize=8.5, fontweight="normal", color=INK2)
    ax.set_anchor("N")
    for s in ax.spines.values():
        s.set_color(RULE)
    return len(hole), len(over), sum(g.area for g in hole), sum(g.area for g in over)


fig = plt.figure(figsize=(7.4, 2.95))
gs = fig.add_gridspec(1, 3, width_ratios=[1, 1, 1], wspace=0.06, left=0.01, right=0.99, top=0.9, bottom=0.1)
ax = fig.add_subplot(gs[0])
palette = ["#2f6f9f", "#e8901f", "#3b8a5a", "#b4508a", "#6f5aa8", "#8c564b"]
for k, row in cells.reset_index(drop=True).iterrows():
    gpd.GeoSeries([row.geometry]).plot(ax=ax, color=palette[k], alpha=0.16, edgecolor=palette[k], linewidth=1.1)
    sel = homes[homes.station == row.station]
    sel.plot(ax=ax, color=palette[k], markersize=5, linewidth=0)
stations.plot(ax=ax, color="white", edgecolor=INK, marker="^", markersize=46, linewidth=1.0, zorder=5)
for _, r in stations.iterrows():
    ax.annotate(r.station, (r.geometry.x, r.geometry.y), xytext=(5, 5), textcoords="offset points", fontsize=7.5,
                color=INK, fontweight="bold")
ax.set_xlim(WINDOW[0], WINDOW[2])
ax.set_ylim(WINDOW[1], WINDOW[3])
ax.set_aspect("equal")
ax.set_xticks([])
ax.set_yticks([])
ax.set_title("Voronoi catchments of the stations", fontsize=8.5, fontweight="normal", color=INK2)
ax.set_anchor("N")
for s in ax.spines.values():
    s.set_color(RULE)
r1 = coverage_panel(fig.add_subplot(gs[1]), simple, f"simplify({TOL}) polygon by polygon")
r2 = coverage_panel(fig.add_subplot(gs[2]), joint, f"coverage_simplify({TOL}) together")
fig.legend(handles=[Patch(facecolor=RED, label="gap"), Patch(facecolor=INK, label="overlap")], loc="lower right",
           ncol=2, fontsize=8, bbox_to_anchor=(0.99, -0.01), handlelength=1.0)
print("per polygon: holes, overlaps, areas", r1, "| together", r2)
save(fig, out / "voronoi-coverage.png")
print("made", out / "voronoi-coverage.png")
