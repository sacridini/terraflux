"""Figures of "Classes by parcel" (tutorials/object-based-image-analysis/classes-by-parcel.md).

The Sentinel-2 clip of the series (obia_data.py) and a layer of 33 city blocks made from OpenStreetMap streets
(obia_blocks.py: blocks.geojson in the page's data folder, with the attribute `faces`, the kind of street the block faces).
Runs of Terraflux:
  * segment.multiresolution with and without `bounds` (the blocks as a vector, and as a map made by vector.rasterize),
    at scales 1 to 8, and felzenszwalb, slic and watershed with and without `bounds`: how many objects cross a zone,
    how many are slivers, whether a map of the zones gives the same objects as the vector;
  * objects.features with `thematic` (the blocks as a vector, no field and field = "faces"): th_mode and th_share;
  * objects.rules in two passes (the second reads th_mode: trees and bare land inside a block or outside), the area of
    each class by block and by kind of street (table.join and table.aggregate).
Then the figures are drawn from the maps and tables these runs write.
Run with: python website/figures/object-based-image-analysis/classes-by-parcel.py   (at most 2 threads: it sets them;
the first run asks the Overpass API for the streets, later ones read website/.cache/obia/osm/)
"""

import csv
import json
import os
import pathlib
import shutil
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import matplotlib  # noqa: E402

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402
import numpy as np  # noqa: E402
import obia_blocks as ob  # noqa: E402
import obia_data as od  # noqa: E402
import rasterio  # noqa: E402

for _k in ("RAYON_NUM_THREADS", "OMP_NUM_THREADS", "GDAL_NUM_THREADS", "OPENBLAS_NUM_THREADS", "MKL_NUM_THREADS",
           "TERRAFLUX_THREADS"):
    os.environ[_k] = "2"  # obia_data sets 4: this script runs with 2

figs = od.figs
SLUG = "classes-by-parcel"
OUT = figs.img_dir(f"{od.CATEGORY}/{SLUG}")
DATA = figs.data_dir(f"{od.CATEGORY}/{SLUG}")
WORK = None
R0, R1, C0, C1 = 0, 580, 0, 580
ZOOM = (185, 305, 245, 370)  # rows, columns of the clip: four blocks of Lago Sul by the lake
ZOOM2 = (60, 150, 0, 120)  # the north-west blocks
SCALES = [1, 2, 3, 4, 6, 8]
MAG = (0.9, 0.1, 0.8)  # the blocks' outline

RULES1 = """[
  { name = "water", rule = "above(ndwi, -0.1, 0.2)", colour = "#2b6cb0" },
  { name = "land", rule = "below(ndwi, -0.1, 0.2)" },
  { name = "forest", rule = "above(ndvi, 0.45, 0.6)", parent = "land", colour = "#2f7d3b" },
  { name = "open", rule = "below(ndvi, 0.45, 0.6)", parent = "land" },
  { name = "built_up", rule = "above(vis, 0.06, 0.09)", parent = "open", colour = "#c0504d" },
  { name = "cerrado", rule = "below(vis, 0.06, 0.09)", parent = "open", colour = "#d9b86c" },
]"""
RULES2 = """[
  { name = "water", rule = 'cls == "water"', colour = "#2b6cb0" },
  { name = "built_up", rule = 'cls == "built_up"', colour = "#c0504d" },
  { name = "forest", rule = 'cls == "forest" and coalesce(th_mode, 0) == 0', colour = "#2f7d3b" },
  { name = "street_trees", rule = 'cls == "forest" and coalesce(th_mode, 0) > 0', colour = "#8fcf6f" },
  { name = "cerrado", rule = 'cls == "cerrado" and coalesce(th_mode, 0) == 0', colour = "#d9b86c" },
  { name = "vacant", rule = 'cls == "cerrado" and coalesce(th_mode, 0) > 0', colour = "#e8a0bf" },
]"""
CLASS2 = ["water", "built_up", "forest", "street_trees", "cerrado", "vacant"]
COL2 = {"water": "#2b6cb0", "built_up": "#c0504d", "forest": "#2f7d3b", "street_trees": "#8fcf6f", "cerrado": "#d9b86c",
        "vacant": "#e8a0bf"}


def node(i, op, inputs, params="{ }"):
    return f'\n[[nodes]]\nid = "{i}"\nop = "{op}"\ninputs = {inputs}\nparams = {params}\n'


def read_map(path):
    with rasterio.open(path) as ds:
        a = ds.read(1, masked=True).astype("float64")
    return np.ma.filled(a, 0).astype("int64")


def read_csv(path):
    with open(path, newline="", encoding="utf-8") as f:
        return list(csv.DictReader(f))


def num(x):
    try:
        return float(x)
    except (TypeError, ValueError):
        return float("nan")


def head(data_stac=False):
    """The clip (the page's data.stac model, or its cached file for the runs), its reflectance and the blocks."""
    if data_stac:
        m = od.sentinel2_model()
    else:
        tif = str(od.sentinel2()).replace("\\", "/")
        m = 'format = "terraflux/1"\n\n[[nodes]]\nid = "s2"\nop = "raster.read"\nparams = { path = "%s" }\n' % tif
        m += node("refl", "raster.calc", '{ s = "s2.raster" }', '{ expression = "out image = s / 10000" }')
    return m + node("blocks", "vector.read", "{ }", '{ path = "blocks.geojson" }')


def run(model, name, writes, files=("blocks.geojson",)):
    work = WORK / name
    work.mkdir(parents=True, exist_ok=True)
    for f in files:
        shutil.copy(DATA / f, work / f)
    r = figs.run(figs.add_writes(model, writes), work, "--threads", "2", "--no-journal")
    return work


def crossing(ids, zone):
    """How many objects have cells in more than one zone, and how many cells those objects hold."""
    n = int(ids.max()) + 1
    flat, z = ids.ravel(), zone.ravel()
    first = np.full(n, np.iinfo(np.int64).max)
    last = np.full(n, np.iinfo(np.int64).min)
    np.minimum.at(first, flat, z)
    np.maximum.at(last, flat, z)
    cnt = np.bincount(flat, minlength=n)
    cross = (first != last) & (cnt > 0)
    cross[0] = False
    return int(cross.sum()), int(cnt[cross].sum()), cross


def slivers(ids, small=3):
    cnt = np.bincount(ids.ravel())[1:]
    cnt = cnt[cnt > 0]
    return int((cnt <= small).sum())


def draw(rgb, ids, zone, cross, win, s, name, width=640):
    """The window of the image, the objects' outlines in yellow, the zones' in magenta, the objects that cross a zone
    tinted cyan."""
    r0, r1, c0, c1 = win
    img = np.kron(rgb[r0:r1, c0:c1], np.ones((s, s, 1)))
    lab = np.kron(ids[r0:r1, c0:c1], np.ones((s, s), dtype=np.int64))
    zo = np.kron(zone[r0:r1, c0:c1], np.ones((s, s), dtype=np.int64))
    if cross is not None:
        m = cross[lab]
        img[m] = img[m] * 0.45 + np.array([0.0, 0.9, 1.0]) * 0.55
    b = od.boundary(lab)
    b = b | np.roll(b, 1, 0) if s >= 5 else b
    img[b] = (1.0, 0.9, 0.1)
    zb = od.boundary(zo)
    zb = zb | np.roll(zb, 1, 0) | np.roll(zb, 1, 1)
    img[zb] = MAG
    od.save_png(img, OUT / name, width=width, colors=200)


def main():
    global WORK
    od.sentinel2()
    WORK = figs.workdir(f"{od.CATEGORY}/{SLUG}")
    info = ob.write(DATA / "blocks.geojson")
    print("blocks:", info)
    (DATA / "ATTRIBUTION.txt").write_text(
        "Blocks of the tutorial 'Classes by parcel' (Terraflux, https://terraflux.org)\n\n"
        "blocks.geojson: 33 city blocks of Lago Sul, Brasilia, Brazil, made by closing the streets of OpenStreetMap "
        "(highway = residential, tertiary, secondary, primary and their links) inside the Sentinel-2 clip of the series, "
        f"with the data of {info['timestamp'][:10]} (Overpass API). Attributes: block (1 to {info['count']}), faces (the kind "
        "of street along most of the block's boundary) and area_ha. EPSG:31983.\n"
        "(c) OpenStreetMap contributors, Open Database License (ODbL) 1.0, https://www.openstreetmap.org/copyright. "
        "This derived database is shared under the same licence.\n", encoding="utf-8")
    gj = json.loads((DATA / "blocks.geojson").read_text(encoding="utf-8"))
    faces_order = []
    for f in gj["features"]:
        k = f["properties"]["faces"]
        if k not in faces_order:
            faces_order.append(k)  # the codes th_mode takes with field = "faces": 1, 2, ... in this order
    print("faces codes:", faces_order)
    n_blocks = len(gj["features"])
    rgb = od.rgb("sentinel2")

    # ======================================================================== segmentations, with and without bounds
    m = head() + node("zmap", "vector.rasterize", '{ vector = "blocks.vector", like = "refl.image" }',
                      '{ field = "block", fill = 0 }')
    wr = {"zmap.map": "zones.tif"}
    segs = {}
    for s in SCALES:
        segs[f"mr{s}"] = ("segment.multiresolution", f"{{ scale = {s} }}", "{ image = \"refl.image\"%s }")
    segs["fz"] = ("segment.felzenszwalb", "{ k = 2 }", "{ image = \"refl.image\"%s }")
    segs["slic"] = ("segment.slic", "{ size = 12 }", "{ image = \"refl.image\"%s }")
    segs["ws"] = ("segment.watershed", "{ }", "{ image = \"refl.image\"%s }")
    for k, (op, p, ins) in segs.items():
        for tag, extra in (("free", ""), ("bound", ', bounds = "blocks.vector"')):
            m += node(f"{k}_{tag}", op, ins % extra, p)
            m += node(f"{k}_{tag}_id", "objects.to_map", f'{{ objects = "{k}_{tag}.objects" }}', '{ what = "id" }')
            wr[f"{k}_{tag}_id.map"] = f"{k}_{tag}.tif"
    # a map of the zones as the bounds (multiresolution, scale 2)
    m += node("mr2_map", "segment.multiresolution", '{ image = "refl.image", bounds = "zmap.map" }', "{ scale = 2 }")
    m += node("mr2_map_id", "objects.to_map", '{ objects = "mr2_map.objects" }', '{ what = "id" }')
    wr["mr2_map_id.map"] = "mr2_map.tif"
    w = run(m, "segs", wr)
    zone = read_map(w / "zones.tif")  # 0: no block
    print("zones:", len(np.unique(zone)), "cells in a block:", int((zone > 0).sum()))
    res = {}
    for k in segs:
        for tag in ("free", "bound"):
            res[(k, tag)] = read_map(w / f"{k}_{tag}.tif")
    same = bool(np.array_equal(res[("mr2", "bound")], read_map(w / "mr2_map.tif")))
    print("a map of the zones gives the same objects as the vector:", same)

    # the scales: objects that cross a block, without and with the bounds
    rows = []
    for s in SCALES:
        a, b = res[(f"mr{s}", "free")], res[(f"mr{s}", "bound")]
        na, ca, _ = crossing(a, zone)
        nb, cb, _ = crossing(b, zone)
        rows.append({"scale": s, "objects_free": int(a.max()), "objects_bound": int(b.max()),
                     "cross_free": na, "cross_free_pct": round(100 * na / a.max(), 1),
                     "cross_cells_pct": round(100 * ca / a.size, 1), "cross_bound": nb,
                     "extra_objects_pct": round(100 * (b.max() - a.max()) / a.max(), 1)})
        print(rows[-1])
    figs.write_csv(DATA / "scales.csv", rows)
    # the other segmenters
    rows = []
    label = {"mr2": ("segment.multiresolution", "scale 2", "inside the segmenter"),
             "fz": ("segment.felzenszwalb", "k = 2", "inside the segmenter"),
             "slic": ("segment.slic", "size 12", "cut afterwards"),
             "ws": ("segment.watershed", "defaults", "cut afterwards")}
    for k, (op, setting, how) in label.items():
        a, b = res[(k, "free")], res[(k, "bound")]
        na, ca, _ = crossing(a, zone)
        nb, cb, _ = crossing(b, zone)
        rows.append({"segmenter": op, "setting": setting, "bounds_work": how, "objects_free": int(a.max()),
                     "cross_free": na, "objects_bound": int(b.max()), "cross_bound": nb,
                     "tiny_free": slivers(a), "tiny_bound": slivers(b)})
        print(rows[-1])
    figs.write_csv(DATA / "segmenters.csv", rows)

    # ---------------------------------------------------------------- the figures of the zones and the objects
    r0, r1, c0, c1 = ZOOM
    fa, fb = res[("mr2", "free")], res[("mr2", "bound")]
    _, _, cross_a = crossing(fa, zone)
    _, _, cross_b = crossing(fb, zone)
    draw(rgb, fa, zone, cross_a, ZOOM, 6, "zoom_free.png", width=700)
    draw(rgb, fb, zone, cross_b, ZOOM, 6, "zoom_bound.png", width=700)
    draw(rgb, fa, zone, cross_a, ZOOM2, 7, "zoom2_free.png", width=700)
    draw(rgb, fb, zone, cross_b, ZOOM2, 7, "zoom2_bound.png", width=700)
    for s in SCALES:
        _, _, cr = crossing(res[(f"mr{s}", "free")], zone)
        draw(rgb, res[(f"mr{s}", "free")], zone, cr, ZOOM, 5, f"scale_{s}.png", width=560)
    for k in ("slic",):
        for tag in ("free", "bound"):
            _, _, cr = crossing(res[(k, tag)], zone)
            draw(rgb, res[(k, tag)], zone, cr, ZOOM, 5, f"{k}_{tag}.png", width=560)
    # the blocks over the clip
    gjx = od.S2_UTM_EXTENT
    fig = plt.figure(figsize=(6, 6), dpi=150)
    ax = fig.add_axes([0, 0, 1, 1])
    ax.imshow(rgb, extent=[gjx[0], gjx[2], gjx[1], gjx[3]])
    cm = {"primary": "#e8901f", "secondary": "#f2c14e", "tertiary": "#ffffff", "residential": "#4cc9f0"}
    for f in gj["features"]:
        for ring in f["geometry"]["coordinates"][:1]:
            xs, ys = zip(*ring)
            ax.plot(xs, ys, color=cm[f["properties"]["faces"]], lw=1.3)
    for win, name in ((ZOOM, "zoom"), (ZOOM2, "zoom 2")):
        a0, a1, b0, b1 = win
        ax.add_patch(plt.Rectangle((gjx[0] + b0 * 10, gjx[3] - a1 * 10), (b1 - b0) * 10, (a1 - a0) * 10, fill=False,
                                   ec="#ff2a6d", lw=1.6))
    ax.set_xlim(gjx[0], gjx[2])
    ax.set_ylim(gjx[1], gjx[3])
    ax.set_axis_off()
    from matplotlib.lines import Line2D
    leg = ax.legend(handles=[Line2D([0], [0], color=c, lw=2, label=k) for k, c in cm.items()], loc="lower left", fontsize=7,
              title="street the block faces", title_fontsize=7, framealpha=0.9, facecolor="#1c2420",
              labelcolor="white")
    leg.get_title().set_color("white")
    fig.savefig(OUT / "blocks.png", dpi=150)
    plt.close(fig)
    od.save_png(np.asarray(plt.imread(OUT / "blocks.png"))[..., :3], OUT / "blocks.png", width=760, colors=200)

    # ================================================================================ features and rules, the page's model
    pm = head()
    pm += node("seg", "segment.multiresolution", '{ image = "refl.image", bounds = "blocks.vector" }', "{ scale = 2 }")
    pm += node("feat", "objects.features", '{ objects = "seg.objects", image = "refl.image", thematic = "blocks.vector" }',
               '{ features = ["spectral", "area"] }')
    pm += node("calc", "objects.calc", '{ objects = "feat.objects" }',
               "{ expression = 'out vis = (mean_red + mean_green + mean_blue) / 3' }")
    pm += node("rules1", "objects.rules", '{ objects = "calc.objects" }', "{ classes = " + RULES1 + " }")
    pm += node("rules2", "objects.rules", '{ objects = "rules1.objects" }', "{ classes = " + RULES2 + " }")
    pm += node("joined", "table.join", '{ left = "calc.table", right = "rules2.table" }', '{ on = "id" }')
    pm += node("by_block", "table.aggregate", '{ table = "joined.table" }',
               '{ by = ["th_mode", "class"], columns = ["area"], stats = ["sum"] }')
    # the kind of street: the same objects, field = "faces"
    pm += node("feat_faces", "objects.features",
               '{ objects = "seg.objects", image = "refl.image", thematic = "blocks.vector" }',
               '{ features = ["area"], field = "faces" }')
    pm += node("joined_faces", "table.join", '{ left = "feat_faces.table", right = "rules2.table" }', '{ on = "id" }')
    pm += node("by_faces", "table.aggregate", '{ table = "joined_faces.table" }',
               '{ by = ["th_mode", "class"], columns = ["area"], stats = ["sum"] }')
    pm += node("idm", "objects.to_map", '{ objects = "seg.objects" }', '{ what = "id" }')
    pm += node("clm", "objects.to_map", '{ objects = "rules2.objects" }', '{ what = "class" }')
    pm += node("clm1", "objects.to_map", '{ objects = "rules1.objects" }', '{ what = "class" }')
    pw = {"feat.table": "feat.csv", "rules2.table": "rules2.csv", "by_block.table": "by_block.csv",
          "by_faces.table": "by_faces.csv", "idm.map": "ids.tif", "clm.map": "cls.tif", "clm1.map": "cls1.tif",
          "seg.table": "seg.csv", "feat_faces.table": "feat_faces.csv"}
    w = run(pm, "page", pw)
    ids = read_map(w / "ids.tif")
    feat = read_csv(w / "output" / "feat.csv")
    th_mode = {int(num(r["id"])): num(r["th_mode"]) for r in feat}
    th_share = {int(num(r["id"])): num(r["th_share"]) for r in feat}
    inb = [i for i, v in th_mode.items() if v == v]
    print("objects", len(feat), "in a block", len(inb), "th_share == 1:", sum(1 for i in inb if th_share[i] >= 0.9999),
          "outside (no value):", len(feat) - len(inb))
    print("faces table head:", read_csv(w / "output" / "feat_faces.csv")[0].keys())
    figs.write_csv(DATA / "features_head.csv", [
        {"id": int(num(r["id"])), "cells": int(num(r["cells"])), "th_mode": r["th_mode"], "th_share": r["th_share"]}
        for r in feat if r["th_mode"] not in ("", "nan", "NaN")][:8])
    for f in ("by_block.csv", "by_faces.csv"):
        shutil.copy(w / "output" / f, DATA / f)

    # the free segmentation's th_share: the objects that cross from one block to another, and that cross a block's edge
    free_run = head() + node("seg", "segment.multiresolution", '{ image = "refl.image" }', "{ scale = 2 }")
    free_run += node("feat", "objects.features", '{ objects = "seg.objects", image = "refl.image", thematic = "blocks.vector" }',
                     '{ features = ["area"] }')
    wf = run(free_run, "free_feat", {"feat.table": "feat.csv"})
    ff = read_csv(wf / "output" / "feat.csv")
    sh = np.array([num(r["th_share"]) for r in ff])
    nan = int(np.isnan(sh).sum())
    n_lt1 = int((sh < 0.9999).sum())
    n_one = int((sh >= 0.9999).sum())
    print("without bounds: objects", len(ff), "no block", nan, "th_share < 1:", n_lt1, "th_share = 1:", n_one)
    na, ca, cross_free = crossing(fa, zone)
    print("without bounds: objects that cross a zone (blocks and outside) =", na, "; th_share < 1 finds", n_lt1)
    figs.write_csv(DATA / "th_share.csv", [
        {"objects": "multiresolution, scale 2, no bounds", "total": len(ff), "no_block": nan, "th_share_below_1": n_lt1,
         "th_share_1": n_one, "cross_a_zone": na,
         "cross_no_value": int(sum(1 for r in ff if np.isnan(num(r["th_mode"])) and cross_free[int(num(r["id"]))]))},
        {"objects": "multiresolution, scale 2, bounds = blocks", "total": len(feat), "no_block": len(feat) - len(inb),
         "th_share_below_1": sum(1 for i in inb if th_share[i] < 0.9999),
         "th_share_1": sum(1 for i in inb if th_share[i] >= 0.9999), "cross_a_zone": crossing(fb, zone)[0],
         "cross_no_value": 0}])
    # the share of cells of a crossing object that lie in each zone: one example
    ids_f = fa
    flat_cross = np.nonzero(cross_free)[0]
    print("crossing objects without bounds (ids), first 5:", flat_cross[:5])

    # ---------------------------------------------------------------------------- the classes inside and outside the blocks
    cls = read_map(w / "cls.tif")
    cls1 = read_map(w / "cls1.tif")
    print("classes pass 1 cells:", {k: int((cls1 == k).sum()) for k in range(1, 7)})
    print("classes pass 2 cells:", {CLASS2[k - 1]: int((cls == k).sum()) for k in range(1, 7)})
    img = np.ones(cls.shape + (3,)) * 0.92
    for k, n in enumerate(CLASS2, 1):
        img[cls == k] = matplotlib.colors.to_rgb(COL2[n])
    zb = od.boundary(zone)
    big = np.kron(img, np.ones((2, 2, 1)))
    zb2 = od.boundary(np.kron(zone, np.ones((2, 2), dtype=np.int64)))
    big[zb2] = (0.1, 0.1, 0.1)
    od.save_png(big, OUT / "classes.png", width=760, colors=64)
    # the zoom, classes
    r0, r1, c0, c1 = ZOOM
    zimg = np.kron(img[r0:r1, c0:c1], np.ones((6, 6, 1)))
    zz = od.boundary(np.kron(zone[r0:r1, c0:c1], np.ones((6, 6), dtype=np.int64)))
    zz = zz | np.roll(zz, 1, 0) | np.roll(zz, 1, 1)
    zimg[zz] = (0.1, 0.1, 0.1)
    od.save_png(zimg, OUT / "classes_zoom.png", width=700, colors=64)

    # area of each class by block, and by the kind of street
    bb = read_csv(w / "output" / "by_block.csv")
    print(bb[:3], len(bb))
    area_by = {}
    for r in bb:
        b = num(r["th_mode"])
        if b != b:
            continue
        area_by.setdefault(int(b), {})[r["class"]] = num(r["area_sum"]) / 1e4  # m2 -> ha
    rows = []
    for b in range(1, n_blocks + 1):
        a = area_by.get(b, {})
        tot = sum(a.values())
        rows.append({"block": b, "faces": gj["features"][b - 1]["properties"]["faces"], "area_ha": round(tot, 2),
                     **{c: round(a.get(c, 0.0), 2) for c in CLASS2},
                     "green_pct": round(100 * (a.get("street_trees", 0.0)) / tot, 1) if tot else 0.0,
                     "built_pct": round(100 * a.get("built_up", 0.0) / tot, 1) if tot else 0.0})
    figs.write_csv(DATA / "blocks_classes.csv", rows)
    print(rows[:3])
    ff2 = read_csv(w / "output" / "by_faces.csv")
    by_faces = {}
    for r in ff2:
        if r["th_mode"] in ("", "nan", "NaN"):
            continue
        by_faces.setdefault(r["th_mode"], {})[r["class"]] = num(r["area_sum"]) / 1e4  # the table carries the names
    out = []
    for kind in ("primary", "secondary", "tertiary", "residential"):
        a = by_faces.get(kind)
        if not a:
            continue
        tot = sum(a.values())
        out.append({"faces": kind, "blocks": sum(1 for f in gj["features"] if f["properties"]["faces"] == kind),
                    "area_ha": round(tot, 1), **{c: round(100 * a.get(c, 0.0) / tot, 1) for c in CLASS2}})
    figs.write_csv(DATA / "faces_classes.csv", out)
    print(out)
    # a map of the blocks by the share of street trees
    fig = plt.figure(figsize=(6, 6), dpi=150)
    ax = fig.add_axes([0, 0, 1, 1])
    ax.imshow(rgb * 0.45 + 0.55, extent=[gjx[0], gjx[2], gjx[1], gjx[3]])
    from matplotlib.cm import ScalarMappable
    from matplotlib.colors import Normalize
    from matplotlib.patches import Polygon as MPoly
    gp = {r["block"]: r["green_pct"] for r in rows}
    vmax = max(gp.values())
    norm = Normalize(0, vmax)
    for f in gj["features"]:
        ring = f["geometry"]["coordinates"][0]
        ax.add_patch(MPoly(ring, closed=True, fc=plt.cm.viridis(norm(gp[f["properties"]["block"]])), ec="white", lw=0.7, alpha=0.88))
    ax.set_xlim(gjx[0], gjx[2])
    ax.set_ylim(gjx[1], gjx[3])
    ax.set_axis_off()
    cax = fig.add_axes([0.62, 0.08, 0.3, 0.02])
    cb = fig.colorbar(ScalarMappable(norm=norm, cmap="viridis"), cax=cax, orientation="horizontal")
    cb.set_label("street trees, % of the block", fontsize=7)
    cb.ax.tick_params(labelsize=7)
    fig.savefig(OUT / "green_by_block.png", dpi=150)
    plt.close(fig)
    od.save_png(np.asarray(plt.imread(OUT / "green_by_block.png"))[..., :3], OUT / "green_by_block.png", width=760, colors=200)
    print("done")


if __name__ == "__main__":
    main()
