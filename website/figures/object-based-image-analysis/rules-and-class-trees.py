"""Figures of "Rules and class trees" (tutorials/object-based-image-analysis/rules-and-class-trees.md).

One model of Terraflux, run on the Sentinel-2 clip of the series (obia_data.py): the multiresolution objects of scale 2,
objects.features (spectral), objects.calc (the visible brightness), objects.rules with a class tree (water, forest,
built-up, cerrado), the same classes as a flat list (to check that inheritance is what the "and"s say), a sweep of the
forest's ramp, crisp ramps, a minimum membership, and a second objects.rules that reads the neighbours' classes. The
figures draw the classes, the memberships and the margins; drawings explain the fuzzy functions and the tree.
At most 2 threads (the module obia_data sets 4: this script sets 2 after importing it).
Run with: python website/figures/object-based-image-analysis/rules-and-class-trees.py
"""

import csv
import json
import os
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import matplotlib  # noqa: E402

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402
import numpy as np  # noqa: E402
import obia_data as od  # noqa: E402
import rasterio  # noqa: E402
from matplotlib.patches import FancyBboxPatch, Patch  # noqa: E402

for _k in ("TERRAFLUX_THREADS", "RAYON_NUM_THREADS", "OMP_NUM_THREADS", "GDAL_NUM_THREADS", "OPENBLAS_NUM_THREADS",
           "MKL_NUM_THREADS"):
    os.environ[_k] = "2"

figs = od.figs
SLUG = "rules-and-class-trees"
OUT = figs.img_dir(f"{od.CATEGORY}/{SLUG}")
DATA = figs.data_dir(f"{od.CATEGORY}/{SLUG}")
CELL_HA = 0.01

# the town's detail (rows, columns of the clip): roofs, streets and the dark gaps between them
D = (130, 250, 270, 410)
NAMES = ["water", "land", "forest", "open", "built_up", "cerrado"]
COLOURS = {"water": "#2b6cb0", "forest": "#2f7d3b", "built_up": "#c0504d", "cerrado": "#d9b86c", "urban_open": "#a7d676"}


def rgb255(h):
    return np.array([int(h[i:i + 2], 16) for i in (1, 3, 5)]) / 255


def node(i, op, inputs, params):
    return f'\n[[nodes]]\nid = "{i}"\nop = "{op}"\ninputs = {inputs}\nparams = {params}\n'


def toml_class(name, rule=None, parent=None, colour=None):
    t = f'name = "{name}"'
    if rule:
        t += f", rule = '{rule}'"
    if parent:
        t += f', parent = "{parent}"'
    if colour:
        t += f', colour = "{colour}"'
    return "{ " + t + " }"


def tree(forest=(0.45, 0.6), water=(-0.1, 0.2), vis=(0.06, 0.09)):
    """The class tree of the page as the setting `classes` (TOML)."""
    f0, f1 = forest
    w0, w1 = water
    v0, v1 = vis
    cl = [
        toml_class("water", f"above(ndwi, {w0}, {w1})", None, COLOURS["water"]),
        toml_class("land", f"below(ndwi, {w0}, {w1})"),
        toml_class("forest", f"above(ndvi, {f0}, {f1})", "land", COLOURS["forest"]),
        toml_class("open", f"below(ndvi, {f0}, {f1})", "land"),
        toml_class("built_up", f"above(vis, {v0}, {v1})", "open", COLOURS["built_up"]),
        toml_class("cerrado", f"below(vis, {v0}, {v1})", "open", COLOURS["cerrado"]),
    ]
    return "[" + ", ".join(cl) + "]"


def flat():
    """The same classes with no parents: each rule says everything itself."""
    cl = [
        toml_class("water", "above(ndwi, -0.1, 0.2)"),
        toml_class("forest", "below(ndwi, -0.1, 0.2) and above(ndvi, 0.45, 0.6)"),
        toml_class("built_up", "below(ndwi, -0.1, 0.2) and below(ndvi, 0.45, 0.6) and above(vis, 0.06, 0.09)"),
        toml_class("cerrado", "below(ndwi, -0.1, 0.2) and below(ndvi, 0.45, 0.6) and below(vis, 0.06, 0.09)"),
    ]
    return "[" + ", ".join(cl) + "]"


SECOND = (
    "[" + ", ".join([
        toml_class("water", 'cls == "water"', None, COLOURS["water"]),
        toml_class("forest", 'cls == "forest"', None, COLOURS["forest"]),
        toml_class("built_up", 'cls == "built_up"', None, COLOURS["built_up"]),
        toml_class("cerrado", 'cls == "cerrado" and border_to("built_up") < 0.5', None, COLOURS["cerrado"]),
        toml_class("urban_open", 'cls == "cerrado" and border_to("built_up") >= 0.5', None, COLOURS["urban_open"]),
    ]) + "]"
)

FOREST_CUTS = [0.30, 0.375, 0.45, 0.525, 0.60]


def fid(a):
    return "f" + f"{a:.3f}".replace(".", "_")


def build():
    m = od.sentinel2_model()
    m += node("seg", "segment.multiresolution", '{ image = "refl.image" }', "{ scale = 2 }")
    m += node("feat", "objects.features", '{ objects = "seg.objects", image = "refl.image" }', '{ features = ["spectral"] }')
    m += node("calc", "objects.calc", '{ objects = "feat.objects" }',
              "{ expression = 'out vis = (mean_red + mean_green + mean_blue) / 3' }")
    m += node("rules", "objects.rules", '{ objects = "calc.objects" }', "{ classes = " + tree() + " }")
    w = {"seg.table": "seg.csv", "calc.table": "calc.csv", "rules.table": "rules.csv", "rules.map": "rules.tif"}
    m += node("idm", "objects.to_map", '{ objects = "calc.objects" }', '{ what = "id" }')
    w["idm.map"] = "id.tif"
    # the flat list
    m += node("flat", "objects.rules", '{ objects = "calc.objects" }', "{ classes = " + flat() + " }")
    w |= {"flat.map": "flat.tif"}
    # the forest's ramp, moved
    for a in FOREST_CUTS:
        i = fid(a)
        m += node(i, "objects.rules", '{ objects = "calc.objects" }', "{ classes = " + tree(forest=(a, round(a + 0.15, 3))) + " }")
        w[f"{i}.map"] = f"{i}.tif"
    # crisp ramps (almost steps)
    m += node("crisp", "objects.rules", '{ objects = "calc.objects" }',
              "{ classes = " + tree((0.5249, 0.5251), (0.0499, 0.0501), (0.0749, 0.0751)) + " }")
    w |= {"crisp.table": "crisp.csv", "crisp.map": "crisp.tif"}
    # a minimum membership
    m += node("strict", "objects.rules", '{ objects = "calc.objects" }', "{ classes = " + tree() + ", min = 0.6 }")
    w |= {"strict.map": "strict.tif", "strict.table": "strict.csv"}
    # a second pass that reads the neighbours' classes
    m += node("second", "objects.rules", '{ objects = "rules.objects" }', "{ classes = " + SECOND + " }")
    w |= {"second.map": "second.tif", "second.table": "second.csv"}
    return figs.add_writes(m, w)


def read(work, name):
    with rasterio.open(work / name) as ds:
        a = ds.read(1, masked=True).astype("float64")
    return np.ma.filled(a, np.nan)


def table(path):
    with open(path, newline="", encoding="utf-8") as f:
        return list(csv.DictReader(f))


def paint(cls, names, colours, window=None):
    """RGB image of a map of class numbers (1.. in the list's order); no value is white."""
    pal = np.ones((len(names) + 1, 3))
    for k, n in enumerate(names, 1):
        if n in colours:
            pal[k] = rgb255(colours[n])
    a = np.nan_to_num(cls, nan=0).astype(int)
    img = pal[a]
    if window:
        r0, r1, c0, c1 = window
        img = img[r0:r1, c0:c1]
    return img


def areas(cls, names):
    return {n: int((cls == k).sum()) for k, n in enumerate(names, 1)}


def draw_functions(path):
    fig, axes = plt.subplots(1, 4, figsize=(8.8, 2.3), dpi=150, sharey=True)
    x = np.linspace(0, 1, 401)
    ramp = lambda lo, hi: np.clip((x - lo) / (hi - lo), 0, 1)  # noqa: E731
    cases = [
        ("above(x, 0.3, 0.7)", ramp(0.3, 0.7), [0.3, 0.7]),
        ("below(x, 0.3, 0.7)", 1 - ramp(0.3, 0.7), [0.3, 0.7]),
        ("between(x, 0.2, 0.4, 0.6, 0.8)", np.minimum(ramp(0.2, 0.4), 1 - ramp(0.6, 0.8)), [0.2, 0.4, 0.6, 0.8]),
        ("about(x, 0.5, 0.3)", np.clip(1 - np.abs(x - 0.5) / 0.3, 0, 1), [0.2, 0.5, 0.8]),
    ]
    for ax, (title, y, marks) in zip(axes, cases):
        ax.plot(x, y, color=figs.FOREST, lw=2.2)
        ax.fill_between(x, 0, y, color=figs.FOREST, alpha=0.12)
        for mk in marks:
            ax.axvline(mk, color=figs.RULE, lw=1, zorder=0)
        ax.set_title(title, fontsize=7.6, loc="left", color=figs.INK)
        ax.set_xlim(0, 1)
        ax.set_ylim(-0.03, 1.08)
        ax.set_xlabel("value of x", fontsize=7.5)
        ax.tick_params(labelsize=7)
        for s in ("top", "right"):
            ax.spines[s].set_visible(False)
    axes[0].set_ylabel("membership", fontsize=7.5)
    fig.tight_layout(pad=0.6)
    fig.savefig(path, dpi=150)
    plt.close(fig)
    od.save_png(np.asarray(plt.imread(path))[..., :3], path, width=1000, colors=64)


def draw_tree(path):
    fig, ax = plt.subplots(figsize=(8.6, 4.5), dpi=150)
    ax.set_xlim(0, 100)
    ax.set_ylim(0, 54)
    ax.axis("off")

    def box(x, y, w, h, title, rule, colour, leaf, dashed=False):
        p = FancyBboxPatch((x, y), w, h, boxstyle="round,pad=0.3,rounding_size=1.2", fc=colour if leaf else "#f4f6f4",
                           ec=colour if leaf else figs.INK2, lw=1.6, ls="--" if dashed else "-", alpha=1 if leaf else 1)
        ax.add_patch(p)
        tc = "white" if leaf else figs.INK
        ax.text(x + w / 2, y + h - 2.6, title, ha="center", va="center", fontsize=8.6, fontweight="bold", color=tc)
        ax.text(x + w / 2, y + 3.0, rule, ha="center", va="center", fontsize=6.6, color=tc, family="monospace")

    def link(x0, y0, x1, y1):
        ax.plot([x0, x0, x1, x1], [y0, (y0 + y1) / 2, (y0 + y1) / 2, y1], color=figs.INK2, lw=1.2, zorder=0)

    # level 0 (no parent): water, land
    box(10, 42, 26, 9, "water", "above(ndwi, -0.1, 0.2)", COLOURS["water"], True)
    box(56, 42, 26, 9, "land", "below(ndwi, -0.1, 0.2)", None, False)
    # level 1: forest, open (parent land)
    box(32, 24, 26, 9, "forest", "above(ndvi, 0.45, 0.6)", COLOURS["forest"], True)
    box(70, 24, 26, 9, "open", "below(ndvi, 0.45, 0.6)", None, False)
    link(69, 42, 45, 33.3)
    link(69, 42, 83, 33.3)
    # level 2: built_up, cerrado (parent open)
    box(54, 6, 26, 9, "built_up", "above(vis, 0.06, 0.09)", COLOURS["built_up"], True)
    box(84, 6, 26, 9, "cerrado", "below(vis, 0.06, 0.09)", COLOURS["cerrado"], True)
    ax.set_xlim(0, 112)
    link(83, 24, 67, 15.3)
    link(83, 24, 97, 15.3)
    ax.text(1, 31, "Boxes in colour are the leaves:\nan object becomes one of them.\nGrey boxes are parents: they\nonly narrow what their children\nmay take.", fontsize=7.4,
            color=figs.INK2, va="center")
    fig.tight_layout(pad=0.4)
    fig.savefig(path, dpi=150)
    plt.close(fig)
    od.save_png(np.asarray(plt.imread(path))[..., :3], path, width=1000, colors=64)


def main():
    od.sentinel2()
    work = figs.workdir(f"{od.CATEGORY}/{SLUG}")
    r = od.figs.run(build(), work, "--threads", "2", "--no-journal", "--profile")
    print(r.stdout[-700:])
    out = work / "output"
    rgb = od.rgb("sentinel2")
    ids = np.nan_to_num(read(work, "id.tif"), nan=0).astype(int)
    seg = table(out / "seg.csv")
    rt = table(out / "rules.csv")
    print("objects", len(seg), "table columns", list(table(out / "calc.csv")[0].keys())[:6])
    cls = read(work, "rules.tif")
    ar = areas(cls, NAMES)
    print("areas (cells)", ar)
    total = int(np.isfinite(cls).sum())
    pal_names = NAMES
    pal = {n: COLOURS[n] for n in COLOURS if n in NAMES}

    # --- the picture and the map (same size, for the compare)
    od.save_png(rgb, OUT / "clip.png", width=700, colors=200)
    img = paint(cls, pal_names, pal)
    od.save_png(img, OUT / "classes_clean.png", width=700, colors=24)
    # the classes with a legend
    fig, ax = plt.subplots(figsize=(5.4, 5.9), dpi=150)
    ax.imshow(img)
    ax.set_axis_off()
    ax.legend(handles=[Patch(fc=COLOURS[n], label=n.replace("_", "-")) for n in ("water", "forest", "built_up", "cerrado")],
              loc="upper center", bbox_to_anchor=(0.5, -0.01), ncol=4, frameon=False, fontsize=8.5)
    fig.tight_layout(pad=0.3)
    fig.savefig(OUT / "classes.png", dpi=150)
    plt.close(fig)
    od.save_png(np.asarray(plt.imread(OUT / "classes.png"))[..., :3], OUT / "classes.png", width=700, colors=32)

    # the table of areas
    rows = []
    obj_count = {}
    for k, n in enumerate(NAMES, 1):
        if n in ("land", "open"):
            continue
        objs = sum(1 for x in rt if x["class"] == n) if rt and "class" in rt[0] else 0
        rows.append({"class": n.replace("_", "-"), "cells": ar[n], "hectares": round(ar[n] * CELL_HA, 1),
                     "share": round(100 * ar[n] / total, 1), "objects": objs})
    figs.write_csv(DATA / "areas.csv", rows)
    print(rows)

    # --- the fuzzy functions and the tree
    draw_functions(OUT / "functions.png")
    draw_tree(OUT / "tree.png")

    # --- memberships and margin: a column of the rules' table as a map of the objects
    def column_map(col):
        v = np.array([float(x[col]) for x in rt])
        img = np.full(ids.shape, np.nan)
        m = ids > 0
        img[m] = v[ids[m] - 1]
        return img

    panels = [("m_water", "Membership in water"), ("m_forest", "Membership in forest"),
              ("m_built_up", "Membership in built-up"), ("m_cerrado", "Membership in cerrado"),
              ("m_land", "Membership in land (parent)"), ("margin", "Margin (best minus second)")]
    fig, axes = plt.subplots(2, 3, figsize=(9.0, 5.4), dpi=150)
    for ax, (col, title) in zip(axes.flat, panels):
        a = column_map(col)
        im = ax.imshow(a, vmin=0, vmax=1, cmap="viridis", interpolation="nearest")
        ax.set_axis_off()
        ax.set_title(title, fontsize=8, loc="left")
    fig.subplots_adjust(hspace=0.12, wspace=0.04, left=0.01, right=0.9, top=0.95, bottom=0.01)
    cb = fig.colorbar(im, ax=axes, shrink=0.55, pad=0.01, aspect=30)
    cb.ax.tick_params(labelsize=7)
    fig.savefig(OUT / "memberships.png", dpi=150, bbox_inches="tight")
    plt.close(fig)
    od.save_png(np.asarray(plt.imread(OUT / "memberships.png"))[..., :3], OUT / "memberships.png", width=1000, colors=128)
    margin = np.array([float(x["margin"]) for x in rt])
    cells = np.array([int(x["cells"]) for x in seg])
    low = margin < 0.2
    print("margin<0.2: objects", int(low.sum()), "cells", int(cells[low].sum()), "share", cells[low].sum() / cells.sum())
    stats = {"objects": len(seg), "low_margin_objects": int(low.sum()), "low_margin_share": float(cells[low].sum() / cells.sum())}

    # --- the flat list is the same
    flat_map = read(work, "flat.tif")
    # (the flat list has 4 classes in the order water, forest, built_up, cerrado: the tree's numbers are 1, 3, 5, 6)
    remap = {1: 1, 2: 3, 3: 5, 4: 6}
    fm = np.full(flat_map.shape, np.nan)
    for k, v in remap.items():
        fm[flat_map == k] = v
    differ = int(((fm != cls) & ~(np.isnan(fm) & np.isnan(cls))).sum())
    print("flat vs tree: cells that differ", differ)
    stats["flat_differ"] = differ

    # --- crisp against fuzzy
    crisp = read(work, "crisp.tif")
    crt = table(out / "crisp.csv")
    cm = np.array([float(x["margin"]) for x in crt])
    stats["crisp_low_margin_objects"] = int((cm < 0.2).sum())
    stats["crisp_cells_differ"] = int(((crisp != cls) & ~(np.isnan(crisp) & np.isnan(cls))).sum())
    print("crisp: low margin", stats["crisp_low_margin_objects"], "cells that differ", stats["crisp_cells_differ"])

    # --- the strict minimum
    strict = read(work, "strict.tif")
    st = table(out / "strict.csv")
    un = int(np.isnan(strict).sum())
    stats["strict_unclassified_cells"] = un
    stats["strict_unclassified_objects"] = sum(1 for x in st if x["class"] == "")
    print("min 0.6: unclassified cells", un, "objects", stats["strict_unclassified_objects"], st[0].keys())
    simg = paint(strict, pal_names, pal)
    simg[np.isnan(strict)] = (1.0, 0.2, 0.9)
    od.save_png(simg, OUT / "strict.png", width=700, colors=24)

    # --- the sweep of the forest's lower limit
    srows = []
    frames = []
    for a in FOREST_CUTS:
        i = fid(a)
        c = read(work, f"{i}.tif")
        ar_i = areas(c, NAMES)
        od.save_png(paint(c, pal_names, pal), OUT / f"forest_{str(a).replace('.', '_')}.png", width=560, colors=24)
        srows.append({"lower_limit": a, "forest_ha": round(ar_i["forest"] * CELL_HA, 1),
                      "built_up_ha": round(ar_i["built_up"] * CELL_HA, 1), "cerrado_ha": round(ar_i["cerrado"] * CELL_HA, 1)})
    print(srows)
    figs.write_csv(DATA / "forest_sweep.csv", srows)

    # --- the second pass, over a detail of the town
    sec = read(work, "second.tif")
    sec_names = ["water", "forest", "built_up", "cerrado", "urban_open"]
    sar = areas(sec, sec_names)
    stats["second"] = sar
    sec_t = table(out / "second.csv")
    stats["urban_open_objects"] = sum(1 for x in sec_t if x["class"] == "urban_open")
    print("second pass", sar, "objects urban_open", stats["urban_open_objects"])
    r0, r1, c0, c1 = D
    k = 4
    crops = [np.kron(rgb[r0:r1, c0:c1], np.ones((k, k, 1))),
             np.kron(paint(cls, pal_names, pal, D), np.ones((k, k, 1))),
             np.kron(paint(sec, sec_names, COLOURS, D), np.ones((k, k, 1)))]
    gap = np.ones((crops[0].shape[0], 10, 3))
    od.save_png(np.concatenate([crops[0], gap, crops[1], gap, crops[2]], axis=1), OUT / "second_pass.png", width=1100, colors=96)
    # where the detail is in the clip
    fig = plt.figure(figsize=(3.2, 3.2), dpi=150)
    ax = fig.add_axes([0, 0, 1, 1])
    ax.imshow(rgb)
    ax.add_patch(plt.Rectangle((c0, r0), c1 - c0, r1 - r0, fill=False, ec="#ffd91a", lw=1.6))
    ax.set_axis_off()
    fig.savefig(OUT / "detail_box.png", dpi=150)
    plt.close(fig)
    od.save_png(np.asarray(plt.imread(OUT / "detail_box.png"))[..., :3], OUT / "detail_box.png", width=420, colors=128)

    (work / "summary.json").write_text(json.dumps(stats, indent=1), encoding="utf-8")
    print(json.dumps(stats, indent=1))


if __name__ == "__main__":
    main()
