"""Figures of "Levels" (tutorials/object-based-image-analysis/levels.md).

One model of Terraflux, run on the Sentinel-2 clip of the series (obia_data.py): the fine objects (multiresolution, scale
2) classified by the class tree of "Rules and class trees"; a level above made by merging (objects.merge by class, with
and without `level`); a level above made by segmenting from the fine objects (segment.multiresolution with `from`) at
five scales, classified by what is inside (objects.rules with `below` and children.area); the fine objects refined by
an attribute of their parent (parent.x); and the tidying (objects.remove_small at several sizes, objects.grow,
objects.reclassify with merge). The figures draw the levels, the landscape units and the refinement.
At most 2 threads (the module obia_data sets 4: this script sets 2 after importing it).
Run with: python website/figures/object-based-image-analysis/levels.py
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
from matplotlib.patches import Patch  # noqa: E402

for _k in ("TERRAFLUX_THREADS", "RAYON_NUM_THREADS", "OMP_NUM_THREADS", "GDAL_NUM_THREADS", "OPENBLAS_NUM_THREADS",
           "MKL_NUM_THREADS"):
    os.environ[_k] = "2"

figs = od.figs
SLUG = "levels"
OUT = figs.img_dir(f"{od.CATEGORY}/{SLUG}")
DATA = figs.data_dir(f"{od.CATEGORY}/{SLUG}")
CELL_HA = 0.01

# the detail the level figures show (rows, columns of the clip): the lake's shore, the town, the gallery forest
D = (130, 250, 270, 410)
SCALES = [3, 4, 6, 10, 16]
MAIN = 6
MIN_CELLS = [10, 25, 50, 100, 200]
COL = {"water": "#2b6cb0", "forest": "#2f7d3b", "built_up": "#c0504d", "cerrado": "#d9b86c", "urban_open": "#a7d676",
       "urban": "#c0504d", "vegetation": "#5a9a4a"}
FINE = ["water", "land", "forest", "open", "built_up", "cerrado"]  # the tree's list: the map's numbers follow it
UNITS = ["urban", "forest", "water", "cerrado"]  # the landscape units' list
REFINED = ["water", "forest", "built_up", "cerrado", "urban_open"]

TREE = """[
  { name = "water", rule = 'above(ndwi, -0.1, 0.2)', colour = "#2b6cb0" },
  { name = "land", rule = 'below(ndwi, -0.1, 0.2)' },
  { name = "forest", rule = 'above(ndvi, 0.45, 0.6)', parent = "land", colour = "#2f7d3b" },
  { name = "open", rule = 'below(ndvi, 0.45, 0.6)', parent = "land" },
  { name = "built_up", rule = 'above(vis, 0.06, 0.09)', parent = "open", colour = "#c0504d" },
  { name = "cerrado", rule = 'below(vis, 0.06, 0.09)', parent = "open", colour = "#d9b86c" }]"""
UNITS_RULES = """[
  { name = "urban", rule = 'above(children.area(cls == "built_up"), 0.25, 0.5)', colour = "#c0504d" },
  { name = "forest", rule = 'above(children.area(cls == "forest"), 0.4, 0.7)', colour = "#2f7d3b" },
  { name = "water", rule = 'above(children.area(cls == "water"), 0.4, 0.7)', colour = "#2b6cb0" },
  { name = "cerrado", rule = 'above(children.area(cls == "cerrado"), 0.4, 0.7)', colour = "#d9b86c" }]"""
REFINE = """[
  { name = "water", rule = 'cls == "water"', colour = "#2b6cb0" },
  { name = "forest", rule = 'cls == "forest"', colour = "#2f7d3b" },
  { name = "built_up", rule = 'cls == "built_up"', colour = "#c0504d" },
  { name = "cerrado", rule = 'cls == "cerrado" and (parent.mean_red + parent.mean_green + parent.mean_blue) / 3 < 0.07', colour = "#d9b86c" },
  { name = "urban_open", rule = 'cls == "cerrado" and (parent.mean_red + parent.mean_green + parent.mean_blue) / 3 >= 0.07', colour = "#a7d676" }]"""


def rgb255(h):
    return np.array([int(h[i:i + 2], 16) for i in (1, 3, 5)]) / 255


def node(i, op, inputs, params):
    return f'\n[[nodes]]\nid = "{i}"\nop = "{op}"\ninputs = {inputs}\nparams = {params}\n'


def build():
    m = od.sentinel2_model()
    m += node("seg", "segment.multiresolution", '{ image = "refl.image" }', "{ scale = 2 }")
    m += node("feat", "objects.features", '{ objects = "seg.objects", image = "refl.image" }', '{ features = ["spectral"] }')
    m += node("calc", "objects.calc", '{ objects = "feat.objects" }',
              "{ expression = 'out vis = (mean_red + mean_green + mean_blue) / 3' }")
    m += node("rules", "objects.rules", '{ objects = "calc.objects" }', "{ classes = " + TREE + " }")
    w = {"rules.map": "rules.tif", "rules.table": "rules.csv", "feat.table": "feat.csv"}
    m += node("rules_id", "objects.to_map", '{ objects = "rules.objects" }', '{ what = "id" }')
    w["rules_id.map"] = "rules_id.tif"
    # a level above by merging
    m += node("patches", "objects.merge", '{ objects = "rules.objects" }', '{ by = "class" }')
    m += node("patches_id", "objects.to_map", '{ objects = "patches.objects" }', '{ what = "id" }')
    m += node("patches_t", "objects.features", '{ objects = "patches.objects", image = "refl.image" }', '{ features = ["spectral", "shape"] }')
    m += node("lvl", "objects.merge", '{ objects = "rules.objects" }', '{ by = "class", level = true }')
    m += node("kids", "objects.calc", '{ objects = "lvl.below" }',
              '{ expression = "out patch_cells = parent.cells\\nout patch_ha = parent.area / 10000" }')
    w |= {"patches_id.map": "patches_id.tif", "patches_t.table": "patches.csv", "kids.table": "kids.csv"}
    # a level above by segmenting from the fine objects, classified by what is inside
    for s in SCALES:
        m += node(f"b{s}", "segment.multiresolution", '{ image = "refl.image", from = "rules.objects" }', f"{{ scale = {s} }}")
        m += node(f"u{s}", "objects.rules", f'{{ objects = "b{s}.objects", below = "b{s}.below" }}', "{ classes = " + UNITS_RULES + " }")
        m += node(f"b{s}_id", "objects.to_map", f'{{ objects = "b{s}.objects" }}', '{ what = "id" }')
        w |= {f"u{s}.map": f"u{s}.tif", f"b{s}_id.map": f"b{s}_id.tif", f"u{s}.table": f"u{s}.csv", f"b{s}.table": f"b{s}.csv"}
    # a level above by merging neighbours that look alike (whatever their class)
    m += node("cond", "objects.merge", '{ objects = "rules.objects" }',
              "{ by = \"condition\", condition = 'abs(a.vis - b.vis) < 0.01 and abs(a.ndvi - b.ndvi) < 0.05' }")
    m += '\n[outputs.cond_count]\nfrom = "cond.count"\n\n[outputs.patches_count]\nfrom = "patches.count"\n'
    # what a parent made by segmenting knows: its class is not there (the level was made before the classes)
    m += node("pc", "objects.calc", f'{{ objects = "b{MAIN}.below" }}', "{ expression = 'out pcls = parent.cls' }")
    w["pc.table"] = "pc.csv"
    # the fine objects refined by their parent (the block of scale MAIN)
    m += node("refine", "objects.rules", f'{{ objects = "b{MAIN}.below" }}', "{ classes = " + REFINE + " }")
    w |= {"refine.map": "refine.tif", "refine.table": "refine.csv"}
    # the same, with the block's class given to the fine objects as a thematic layer
    m += node("bm", "objects.to_map", f'{{ objects = "u{MAIN}.objects" }}', '{ what = "class" }')
    m += node("feat2", "objects.features", f'{{ objects = "b{MAIN}.below", image = "refl.image", thematic = "bm.map" }}', '{ features = ["spectral"] }')
    m += node("refine2", "objects.rules", '{ objects = "feat2.objects" }', "{ classes = [" + ", ".join([
        "{ name = \"water\", rule = 'cls == \"water\"' }", "{ name = \"forest\", rule = 'cls == \"forest\"' }",
        "{ name = \"built_up\", rule = 'cls == \"built_up\"' }",
        "{ name = \"cerrado\", rule = 'cls == \"cerrado\" and th_mode != 1' }",
        "{ name = \"urban_open\", rule = 'cls == \"cerrado\" and th_mode == 1' }"]) + "] }")
    w |= {"refine2.map": "refine2.tif"}
    # tidying: remove small objects at several sizes
    for n in MIN_CELLS:
        m += node(f"s{n}", "objects.remove_small", '{ objects = "rules.objects" }', f"{{ min_cells = {n} }}")
        m += node(f"s{n}_c", "objects.to_map", f'{{ objects = "s{n}.objects" }}', '{ what = "class" }')
        w[f"s{n}_c.map"] = f"s{n}.tif"
        m += f'\n[outputs.s{n}_count]\nfrom = "s{n}.count"\n'
    m += node("sb", "objects.remove_small", '{ objects = "rules.objects" }', '{ min_cells = 50, into = "border" }')
    m += node("sb_c", "objects.to_map", '{ objects = "sb.objects" }', '{ what = "class" }')
    w["sb_c.map"] = "sb.tif"
    m += '\n[outputs.sb_count]\nfrom = "sb.count"\n'
    # grow: the objects the strict rules left unclassified
    m += node("strict", "objects.rules", '{ objects = "calc.objects" }', "{ classes = " + TREE + ", min = 0.6 }")
    m += node("strict_c", "objects.to_map", '{ objects = "strict.objects" }', '{ what = "class" }')
    m += node("grow", "objects.grow", '{ objects = "strict.objects" }',
              "{ class = \"forest\", condition = 'cls == \"unclassified\" and ndvi > 0.4 and cells < 200' }")
    m += node("grow_c", "objects.to_map", '{ objects = "grow.objects" }', '{ what = "class" }')
    m += node("grow_free", "objects.grow", '{ objects = "strict.objects" }',
              "{ class = \"forest\", condition = 'cls == \"unclassified\" and ndvi > 0.4' }")
    m += node("grow_free_c", "objects.to_map", '{ objects = "grow_free.objects" }', '{ what = "class" }')
    w |= {"strict_c.map": "strict.tif", "grow_c.map": "grow.tif", "grow_free_c.map": "grow_free.tif"}
    m += '\n[outputs.grow_count]\nfrom = "grow.count"\n\n[outputs.grow_done]\nfrom = "grow.done"\n'
    # reclassify: forest and cerrado are one vegetation, neighbours merge
    m += node("veg", "objects.reclassify", '{ objects = "refine.objects" }',
              '{ classes = { forest = "vegetation", cerrado = "vegetation" }, merge = true }')
    m += node("veg_c", "objects.to_map", '{ objects = "veg.objects" }', '{ what = "class" }')
    w |= {"veg_c.map": "veg.tif"}
    m += '\n[outputs.veg_count]\nfrom = "veg.count"\n'
    return figs.add_writes(m, w)


def read(work, name):
    with rasterio.open(work / name) as ds:
        a = ds.read(1, masked=True).astype("float64")
    return np.ma.filled(a, np.nan)


def table(path):
    with open(path, newline="", encoding="utf-8") as f:
        return list(csv.DictReader(f))


def paint(cls, names, colours, window=None, blank=(1.0, 1.0, 1.0)):
    pal = np.ones((len(names) + 1, 3)) * np.array(blank)
    for k, n in enumerate(names, 1):
        if n in colours:
            pal[k] = rgb255(colours[n])
    img = pal[np.nan_to_num(cls, nan=0).astype(int)]
    if window:
        r0, r1, c0, c1 = window
        img = img[r0:r1, c0:c1]
    return img


def areas(cls, names):
    return {n: int((cls == k).sum()) for k, n in enumerate(names, 1)}


def count(path):
    return int(json.loads(pathlib.Path(path).read_text()))


def save(img, name, width, colors=96):
    od.save_png(img, OUT / name, width=width, colors=colors)


def ids_of(work, name):
    return np.nan_to_num(read(work, name), nan=0).astype("int64")


def main():
    od.sentinel2()
    work = figs.workdir(f"{od.CATEGORY}/{SLUG}")
    r = figs.run(build(), work, "--threads", "2", "--no-journal", "--profile")
    print(r.stdout[-500:])
    out = work / "output"
    rgb = od.rgb("sentinel2")
    stats = {}
    fine_t = table(out / "feat.csv")
    n_fine = len(fine_t)
    cls = read(work, "rules.tif")
    ar = areas(cls, FINE)
    stats["fine_objects"] = n_fine
    stats["fine_areas"] = ar
    print("fine", n_fine, ar)

    # --- levels by merging
    pat = table(out / "patches.csv")
    n_pat = len(pat)
    stats["patches"] = n_pat
    cl = [x["class"] for x in table(out / "rules.csv")]  # (a row per fine object)
    # patch table: the class of a patch is that of its largest part: the map of patches_id with the class map
    pid = ids_of(work, "patches_id.tif")
    rows = []
    for k, name in ((1, "water"), (3, "forest"), (5, "built_up"), (6, "cerrado")):
        ids_k = np.unique(pid[cls == k])
        cells = []
        for i in ids_k:
            cells.append(int((pid == i).sum()))
        cells = np.array(cells)
        rows.append({"class": name.replace("_", "-"), "patches": len(cells), "hectares": round(cells.sum() * CELL_HA, 1),
                     "mean_ha": round(cells.mean() * CELL_HA, 2), "median_ha": round(float(np.median(cells)) * CELL_HA, 2),
                     "largest_ha": round(cells.max() * CELL_HA, 1),
                     "fine_objects": int(sum(1 for c in cl if c == name))})
    figs.write_csv(DATA / "patches.csv", rows)
    print("patches", n_pat, rows)
    kids = table(out / "kids.csv")
    pc = np.array([float(x["patch_cells"]) for x in kids])
    stats["kids"] = len(kids)
    stats["kids_in_patch_of_one"] = int((pc == np.array([float(x["cells"]) for x in kids])).sum())
    stats["kids_in_patch_over_10ha"] = int((pc * CELL_HA > 10).sum())
    stats["kids_in_patch_under_1ha"] = int((pc * CELL_HA < 1).sum())
    print({k: v for k, v in stats.items() if k.startswith("kids")})

    stats["cond_objects"] = count(out / "cond_count.json")
    pcl = table(out / "pc.csv")
    stats["parent_cls_values"] = sorted(set(x["pcls"] for x in pcl))
    print("merge by condition", stats["cond_objects"], "parent.cls values", stats["parent_cls_values"], list(pcl[0].keys())[-3:])

    # --- levels by segmenting, and the landscape units
    srows = []
    frames = []
    uni_main = None
    for s in SCALES:
        bt = table(out / f"b{s}.csv")
        ut = table(out / f"u{s}.csv")
        u = read(work, f"u{s}.tif")
        a = areas(u, UNITS)
        unc = int(np.isnan(u).sum())
        bid = ids_of(work, f"b{s}_id.tif")
        srows.append({"scale": s, "blocks": len(bt), "urban_ha": round(a["urban"] * CELL_HA, 1),
                      "forest_ha": round(a["forest"] * CELL_HA, 1), "water_ha": round(a["water"] * CELL_HA, 1),
                      "cerrado_ha": round(a["cerrado"] * CELL_HA, 1), "unclassified_ha": round(unc * CELL_HA, 1)})
        img = od.outlines(paint(u, UNITS, COL, blank=(0.85, 0.2, 0.9)), bid, color=(0.1, 0.1, 0.1), scale=1, thick=1)
        save(img, f"units_{s}.png", 560, 64)
        if s == MAIN:
            uni_main = u
            stats["blocks_main"] = len(bt)
            stats["units_main"] = a
            stats["units_main_unclassified_cells"] = unc
    print(srows)
    figs.write_csv(DATA / "scales.csv", srows)

    # the main level: the landscape units, with a legend
    img = paint(uni_main, UNITS, COL, blank=(0.85, 0.2, 0.9))
    fig, ax = plt.subplots(figsize=(5.4, 5.9), dpi=150)
    ax.imshow(img)
    ax.set_axis_off()
    hs = [Patch(fc=COL[n], label=n) for n in UNITS] + [Patch(fc=(0.85, 0.2, 0.9), label="unclassified")]
    ax.legend(handles=hs, loc="upper center", bbox_to_anchor=(0.5, -0.01), ncol=5, frameon=False, fontsize=8)
    fig.tight_layout(pad=0.3)
    fig.savefig(OUT / "units.png", dpi=150)
    plt.close(fig)
    od.save_png(np.asarray(plt.imread(OUT / "units.png"))[..., :3], OUT / "units.png", width=700, colors=128)

    # --- the four levels over a detail: fine objects, patches, blocks, landscape units
    r0, r1, c0, c1 = D
    k = 4
    fid = ids_of(work, "rules_id.tif")
    bid = ids_of(work, f"b{MAIN}_id.tif")

    def outl(labels):
        base = np.kron(rgb[r0:r1, c0:c1], np.ones((k, k, 1)))
        lab = np.kron(labels[r0:r1, c0:c1], np.ones((k, k), dtype=labels.dtype))
        b = od.boundary(lab)
        base[b] = (1.0, 0.85, 0.1)
        return base

    def cls_img(a, names):
        return np.kron(paint(a, names, COL, D, blank=(0.85, 0.2, 0.9)), np.ones((k, k, 1)))

    unit_ids = bid  # (the units are blocks classed: their outlines are the blocks')
    pan = [outl(fid), outl(pid), outl(bid), cls_img(uni_main, UNITS)]
    titles = ["Fine objects (scale 2)", "Patches: fine objects merged by class", f"Blocks: segmented from the fine ones (scale {MAIN})",
              "Landscape units: blocks classed by what they hold"]
    n_lab = [n_fine, n_pat, stats["blocks_main"], None]
    fig, axes = plt.subplots(2, 2, figsize=(8.8, 8.8 * (r1 - r0) / (c1 - c0) + 0.9), dpi=150)
    for ax, im, t, n in zip(axes.flat, pan, titles, n_lab):
        ax.imshow(im)
        ax.set_axis_off()
        ax.set_title(t + (f": {n:,}" if n else ""), fontsize=8, loc="left")
    fig.tight_layout(pad=0.6)
    fig.savefig(OUT / "levels.png", dpi=150)
    plt.close(fig)
    od.save_png(np.asarray(plt.imread(OUT / "levels.png"))[..., :3], OUT / "levels.png", width=1000, colors=96)
    # where the detail is
    fig = plt.figure(figsize=(3.2, 3.2), dpi=150)
    ax = fig.add_axes([0, 0, 1, 1])
    ax.imshow(rgb)
    ax.add_patch(plt.Rectangle((c0, r0), c1 - c0, r1 - r0, fill=False, ec="#ffd91a", lw=1.6))
    ax.set_axis_off()
    fig.savefig(OUT / "detail_box.png", dpi=150)
    plt.close(fig)
    od.save_png(np.asarray(plt.imread(OUT / "detail_box.png"))[..., :3], OUT / "detail_box.png", width=420, colors=128)

    # --- the refinement by the parent
    ref = read(work, "refine.tif")
    ref2 = read(work, "refine2.tif")
    ra = areas(ref, REFINED)
    ra2 = areas(ref2, REFINED)
    stats["refine"] = ra
    stats["refine_thematic"] = ra2
    rt = table(out / "refine.csv")
    stats["urban_open_objects"] = sum(1 for x in rt if x["class"] == "urban_open")
    stats["refine_vs_thematic_differ"] = int(((ref != ref2) & ~(np.isnan(ref) & np.isnan(ref2))).sum())
    print("refine", ra, "thematic", ra2, "objects", stats["urban_open_objects"], "differ", stats["refine_vs_thematic_differ"])
    crops = [np.kron(rgb[r0:r1, c0:c1], np.ones((k, k, 1))), cls_img(cls, FINE), cls_img(ref, REFINED)]
    gap = np.ones((crops[0].shape[0], 10, 3))
    save(np.concatenate([crops[0], gap, crops[1], gap, crops[2]], axis=1), "refine.png", 1100, 96)

    # --- tidying: remove_small at several sizes
    trows = []
    base_cells = int(np.isfinite(cls).sum())
    for n in MIN_CELLS:
        c = read(work, f"s{n}.tif")
        # (the classes keep their numbers: compare cell by cell with the first level's)
        changed = int(((c != cls) & ~(np.isnan(c) & np.isnan(cls))).sum())
        trows.append({"min_cells": n, "min_ha": n * CELL_HA, "objects": count(out / f"s{n}_count.json"),
                      "cells_changed": changed, "percent_changed": round(100 * changed / base_cells, 2)})
    trows.insert(0, {"min_cells": 0, "min_ha": 0, "objects": n_fine, "cells_changed": 0, "percent_changed": 0.0})
    figs.write_csv(DATA / "remove_small.csv", trows)
    print("remove_small", trows)
    sb = read(work, "sb.tif")
    stats["small_border_objects"] = count(out / "sb_count.json")
    stats["small_border_changed"] = int(((sb != cls) & ~(np.isnan(sb) & np.isnan(cls))).sum())
    s50 = read(work, "s50.tif")
    stats["small_50_vs_border_differ"] = int(((sb != s50) & ~(np.isnan(sb) & np.isnan(s50))).sum())
    print({k: v for k, v in stats.items() if k.startswith("small")})

    # --- grow
    st = read(work, "strict.tif")
    gr = read(work, "grow.tif")
    stats["strict_forest"] = int((st == 3).sum())
    stats["strict_unclassified"] = int(np.isnan(st).sum())
    stats["grow_forest"] = int((gr == 3).sum())
    stats["grow_unclassified"] = int(np.isnan(gr).sum())
    stats["grow_count"] = count(out / "grow_count.json")
    gf = read(work, "grow_free.tif")
    stats["grow_free_forest"] = int((gf == 3).sum())
    stats["grow_free_unclassified"] = int(np.isnan(gf).sum())
    print("grow", {k: v for k, v in stats.items() if k.startswith(("strict", "grow"))})
    crops = [np.kron(paint(st, FINE, COL, D, blank=(0.85, 0.2, 0.9)), np.ones((k, k, 1))),
             np.kron(paint(gr, FINE, COL, D, blank=(0.85, 0.2, 0.9)), np.ones((k, k, 1)))]
    save(np.concatenate([crops[0], np.ones((crops[0].shape[0], 10, 3)), crops[1]], axis=1), "grow.png", 900, 32)

    # --- reclassify: one vegetation
    veg = read(work, "veg.tif")
    stats["veg_objects"] = count(out / "veg_count.json")
    print("veg objects", stats["veg_objects"],)
    (work / "summary.json").write_text(json.dumps(stats, indent=1), encoding="utf-8")
    print(json.dumps(stats, indent=1))


if __name__ == "__main__":
    main()
