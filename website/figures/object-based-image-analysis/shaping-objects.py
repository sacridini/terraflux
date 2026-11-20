"""Figures of "Shaping objects: grow, shrink, open, close, fill, smooth and refine" (tutorials/object-based-image-analysis/shaping-objects.md).

Runs of objects.resize on the Sentinel-2 clip of the series (obia_data.py), in two sets of objects:
  * water: the NDWI of the clip kept only where it is above Otsu's cut (0.065; the other cells are None, so they are
    no object), segment.threshold on it (the lake and ten small waters), and then grow and shrink by a distance (exact
    Euclidean, one pass), grow ring by ring under a mask (a map of ones: only the metric changes, 8 or 4 neighbours;
    the NDWI above -0.3: a condition), and the report of each;
  * patches: multiresolution scale 2 -> spectral features -> rules (water, forest, built_up, cerrado) -> objects.merge by
    class, as in "Rules and class trees"; the built_up patches are shrunk, opened, closed (into cerrado and forest),
    filled, smoothed, and the four last chained; the numbers of cells, patches and edge length of each;
  * border optimisation (refine) on the 3,035 multiresolution objects of scale 2, for 1 to 10 passes: the within-object
    sum of squares of the z-scored bands, the number of objects, and refine followed by remove_small.
Then the sliders of the opening's and the smoothing's distance. Every figure is drawn from the maps these runs write.
Run with: python website/figures/object-based-image-analysis/shaping-objects.py   (at most 2 threads: it sets them)
"""

import csv
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

for _k in ("RAYON_NUM_THREADS", "OMP_NUM_THREADS", "GDAL_NUM_THREADS", "OPENBLAS_NUM_THREADS", "MKL_NUM_THREADS",
           "TERRAFLUX_THREADS"):
    os.environ[_k] = "2"  # obia_data sets 4: this script runs with 2

figs = od.figs
SLUG = "shaping-objects"
OUT = figs.img_dir(f"{od.CATEGORY}/{SLUG}")
DATA = figs.data_dir(f"{od.CATEGORY}/{SLUG}")
WORK = None
CELL = 10  # m

WIN_L = (90, 260, 200, 360)  # the lake's south shore (rows, columns of the clip)
WIN_B = (235, 335, 290, 410)  # a block of the town with its gaps and fingers
GAIN, LOSS = (1.0, 0.85, 0.0), (0.0, 0.95, 1.0)
RED, BLUE = np.array([0.8, 0.2, 0.2]), np.array([0.1, 0.4, 0.9])

RULES = """[
  { name = "water", rule = "above(ndwi, -0.1, 0.2)", colour = "#2b6cb0" },
  { name = "land", rule = "below(ndwi, -0.1, 0.2)" },
  { name = "forest", rule = "above(ndvi, 0.45, 0.6)", parent = "land", colour = "#2f7d3b" },
  { name = "open", rule = "below(ndvi, 0.45, 0.6)", parent = "land" },
  { name = "built_up", rule = "above(vis, 0.06, 0.09)", parent = "open", colour = "#c0504d" },
  { name = "cerrado", rule = "below(vis, 0.06, 0.09)", parent = "open", colour = "#d9b86c" },
]"""
BUILT = 5  # the class number of built_up in the list above (1 water, 2 land, 3 forest, 4 open, 5 built_up, 6 cerrado)
INTO = '["cerrado", "forest"]'


def node(i, op, inputs, params="{ }"):
    return f'\n[[nodes]]\nid = "{i}"\nop = "{op}"\ninputs = {inputs}\nparams = {params}\n'


def read_map(path):
    with rasterio.open(path) as ds:
        a = ds.read(1, masked=True).astype("float64")
    return np.ma.filled(a, 0).astype("int64")


def read_csv(path):
    with open(path, newline="", encoding="utf-8") as f:
        return list(csv.DictReader(f))


def head(data_stac=False):
    """The clip (the page's data.stac model, or its cached file for the runs) and the reflectance."""
    if data_stac:
        return od.sentinel2_model()
    tif = str(od.sentinel2()).replace("\\", "/")
    m = 'format = "terraflux/1"\n\n[[nodes]]\nid = "s2"\nop = "raster.read"\nparams = { path = "%s" }\n' % tif
    return m + node("refl", "raster.calc", '{ s = "s2.raster" }', '{ expression = "out image = s / 10000" }')


WATER = (
    node("ndwi", "raster.calc", '{ s = "refl.image" }', '{ expression = "(s.green - s.nir) / (s.green + s.nir)" }')
    + node("wet", "raster.calc", '{ n = "ndwi.map" }', '{ expression = "n if n > 0.065 else None" }')
    + node("waters", "segment.threshold", '{ image = "wet.map" }', "{ thresholds = [0.065] }")
)
PATCHES = (
    node("seg", "segment.multiresolution", '{ image = "refl.image" }', "{ scale = 2 }")
    + node("feat", "objects.features", '{ objects = "seg.objects", image = "refl.image" }', '{ features = ["spectral"] }')
    + node("calc", "objects.calc", '{ objects = "feat.objects" }',
           "{ expression = 'out vis = (mean_red + mean_green + mean_blue) / 3' }")
    + node("rules", "objects.rules", '{ objects = "calc.objects" }', "{ classes = " + RULES + " }")
    + node("patches", "objects.merge", '{ objects = "rules.objects" }', "{ }")
)


def run(model, name, writes):
    work = WORK / name
    work.mkdir(parents=True, exist_ok=True)
    figs.run(figs.add_writes(model, writes), work, "--threads", "2", "--no-journal")
    return work


def maps(m, prefix, src, writes):
    """Adds the id and class maps of an objects output."""
    m += node(f"{prefix}_id", "objects.to_map", f'{{ objects = "{src}" }}', '{ what = "id" }')
    m += node(f"{prefix}_cl", "objects.to_map", f'{{ objects = "{src}" }}', '{ what = "class" }')
    writes[f"{prefix}_id.map"] = f"{prefix}_id.tif"
    writes[f"{prefix}_cl.map"] = f"{prefix}_cl.tif"
    return m


def edge_km(mask):
    """The length (km) of the boundary of a mask: the cell edges between a cell in it and one out of it."""
    return float((np.sum(mask[:, 1:] != mask[:, :-1]) + np.sum(mask[1:, :] != mask[:-1, :])) * CELL / 1000)


def zoom(rgb, win, s):
    r0, r1, c0, c1 = win
    return np.kron(rgb[r0:r1, c0:c1], np.ones((s, s, 1)))


def up(a, win, s):
    r0, r1, c0, c1 = win
    return np.kron(a[r0:r1, c0:c1], np.ones((s, s), dtype=a.dtype))


def draw_change(rgb, before, after, win, s, color, name, width=640, line=(1, 1, 1), mark=None):
    """The window of the image with the cells of `after` tinted, its outline in white, the cells it gained in yellow and
    the cells it lost in cyan (against `before`)."""
    img = zoom(rgb, win, s)
    a, b = up(after, win, s), up(before, win, s)
    img[a] = img[a] * 0.55 + np.array(color) * 0.45
    img[a & ~b] = GAIN
    img[b & ~a] = LOSS
    edge = od.boundary(a.astype(np.uint8))
    img[edge & ~(a & ~b) & ~(b & ~a)] = line
    if mark:
        r, c = mark
        r0, _, c0, _ = win
        y, x = (r - r0) * s + s // 2, (c - c0) * s + s // 2
        img[max(y - 1, 0):y + 2, max(x - s * 3, 0):max(x - s * 2, 0)] = (1, 0, 1)
    od.save_png(img, OUT / name, width=width, colors=200)


def main():
    global WORK
    od.sentinel2()
    WORK = figs.workdir(f"{od.CATEGORY}/{SLUG}")
    rgb = od.rgb("sentinel2")
    R0, R1, C0, C1 = 90, 360, 200, 470

    # =========================================================================================== water: grow and shrink
    wr = {}
    m = head() + WATER
    m += node("ones", "raster.calc", '{ n = "ndwi.map" }', '{ expression = "n * 0 + 1" }')
    m += node("damp", "raster.calc", '{ n = "ndwi.map" }', '{ expression = "1 if n > -0.3 else 0" }')
    m = maps(m, "w0", "waters.objects", wr)
    water_modes = {
        "grow": '{ mode = "grow", distance = 40 }',
        "grow_ring8": '{ mode = "grow", steps = 4 }',
        "grow_ring4": '{ mode = "grow", steps = 4, neighbours = 4 }',
        "grow_damp": '{ mode = "grow", steps = 8 }',
        "shrink": '{ mode = "shrink", distance = 20 }',
        "open": '{ mode = "open", distance = 20 }',
    }
    masks = {"grow_ring8": "ones.map", "grow_ring4": "ones.map", "grow_damp": "damp.map"}
    for k, p in water_modes.items():
        ins = '{ objects = "waters.objects"' + (f', mask = "{masks[k]}"' if k in masks else "") + " }"
        m += node(k, "objects.resize", ins, p)
        m = maps(m, k, f"{k}.objects", wr)
        wr[f"{k}.report"] = f"rep_{k}.csv"
    # the same distance without a mask is exact: 40 m by steps = 4 too
    m += node("grow_steps", "objects.resize", '{ objects = "waters.objects" }', '{ mode = "grow", steps = 4 }')
    wr["grow_steps.report"] = "rep_grow_steps.csv"
    wr |= {"waters.table": "waters.csv", "waters.thresholds": "thresholds.csv"}
    w = run(m, "water", wr)
    w0 = read_map(w / "w0_id.tif") > 0
    tab = read_csv(w / "output" / "waters.csv")
    cells = {int(r["id"]): int(float(r["cells"])) for r in tab}
    print("water objects:", len(tab), "lake cells", max(cells.values()))
    sets = {k: read_map(w / f"{k}_id.tif") > 0 for k in water_modes}
    rep = {k: {int(float(r["object"])): r for r in read_csv(w / "output" / f"rep_{k}.csv")}
           for k in list(water_modes) + ["grow_steps"]}
    lake = max(cells, key=cells.get)
    speck = min((i for i in cells if cells[i] == 1), default=None)
    print("lake", lake, "speck", speck, {k: (rep[k][lake]["before"], rep[k][lake]["after"]) for k in rep})
    ids_w = read_map(w / "w0_id.tif")
    sr, sc = [int(x.mean()) for x in np.nonzero(ids_w == speck)]
    print("the speck is at", sr, sc, {k: rep[k][speck]["after"] for k in rep})

    # the lake's shore: before and three ways of growing
    for k, nm in (("grow", "lake_grow"), ("grow_damp", "lake_damp"), ("shrink", "lake_shrink")):
        draw_change(rgb, w0, sets[k], WIN_L, 4, BLUE, nm + ".png", width=620)
    # one cell grown 4 cells: a disc, a square and a diamond
    swin = (sr - 12, sr + 13, sc - 12, sc + 13)
    for k, nm in (("grow", "speck_exact"), ("grow_ring8", "speck_ring8"), ("grow_ring4", "speck_ring4")):
        draw_change(rgb, w0, sets[k], swin, 18, BLUE, nm + ".png", width=420)
    draw_change(rgb, w0, w0, swin, 18, BLUE, "speck_before.png", width=420)
    figs.write_csv(DATA / "grow_ways.csv", [
        {"way": "exact, distance 40 m", "setting": "grow, distance = 40", "speck_cells": int(rep["grow"][speck]["after"]),
         "lake_cells": int(float(rep["grow"][lake]["after"]))},
        {"way": "ring by ring, 8 neighbours", "setting": "grow, steps = 4, mask of ones",
         "speck_cells": int(rep["grow_ring8"][speck]["after"]), "lake_cells": int(float(rep["grow_ring8"][lake]["after"]))},
        {"way": "ring by ring, 4 neighbours", "setting": "grow, steps = 4, neighbours = 4, mask of ones",
         "speck_cells": int(rep["grow_ring4"][speck]["after"]), "lake_cells": int(float(rep["grow_ring4"][lake]["after"]))},
        {"way": "ring by ring under a condition", "setting": "grow, steps = 8, mask NDWI > -0.3",
         "speck_cells": int(rep["grow_damp"][speck]["after"]), "lake_cells": int(float(rep["grow_damp"][lake]["after"]))},
        {"way": "exact, steps = 4", "setting": "grow, steps = 4", "speck_cells": int(rep["grow_steps"][speck]["after"]),
         "lake_cells": int(float(rep["grow_steps"][lake]["after"]))},
    ])
    figs.write_csv(DATA / "water_before.csv", [
        {"object": i, "cells": cells[i], "area_ha": cells[i] * 0.01} for i in sorted(cells, key=cells.get, reverse=True)])
    # the report of the shrink and the open for every water (the specks go)
    rows = []
    for i in sorted(cells, key=cells.get, reverse=True):
        rows.append({"object": i, "cells": cells[i],
                     "grow_40m": int(float(rep["grow"][i]["after"])), "shrink_20m": int(float(rep["shrink"][i]["after"])),
                     "open_20m": int(float(rep["open"][i]["after"])), "open_pieces": int(float(rep["open"][i]["pieces"]))})
    figs.write_csv(DATA / "water_report.csv", rows)
    print(rows)
    print("lake shore zone (cells lost by shrink 20 m):", int(w0.sum() - sets["shrink"].sum()))

    # ======================================================================================= patches: shape the outlines
    pr = {}
    m = head() + PATCHES
    m = maps(m, "p0", "patches.objects", pr)
    pmodes = {
        "shrink": f'{{ mode = "shrink", classes = ["built_up"], distance = 20 }}',
        "open": f'{{ mode = "open", classes = ["built_up"], distance = 30 }}',
        "close": f'{{ mode = "close", classes = ["built_up"], distance = 30, into = {INTO} }}',
        "fill": f'{{ mode = "fill", classes = ["built_up"], into = {INTO}, max_hole = 40 }}',
        "smooth": '{ mode = "smooth", classes = ["built_up"], distance = 30 }',
    }
    for k, p in pmodes.items():
        m += node(k, "objects.resize", '{ objects = "patches.objects" }', p)
        m = maps(m, k, f"{k}.objects", pr)
        pr[f"{k}.report"] = f"rep_{k}.csv"
    # the chain of the page: open, close, fill, smooth
    chain = [("c_open", "patches.objects", pmodes["open"]), ("c_close", "c_open.objects", pmodes["close"]),
             ("c_fill", "c_close.objects", pmodes["fill"]), ("c_smooth", "c_fill.objects", pmodes["smooth"])]
    for i, src, p in chain:
        m += node(i, "objects.resize", f'{{ objects = "{src}", image = "refl.image" }}', p)
        pr[f"{i}.report"] = f"rep_{i}.csv"
    m = maps(m, "chain", "c_smooth.objects", pr)
    m += node("fa", "objects.features", '{ objects = "c_smooth.objects", image = "refl.image" }',
              '{ features = ["compactness", "area"] }')
    m += node("fb", "objects.features", '{ objects = "patches.objects", image = "refl.image" }',
              '{ features = ["compactness", "area"] }')
    pr |= {"fa.table": "fa.csv", "fb.table": "fb.csv"}
    w = run(m, "patches", pr)

    base_cl = read_map(w / "p0_cl.tif")
    base_id = read_map(w / "p0_id.tif")
    built0 = base_cl == BUILT
    r0, r1, c0, c1 = WIN_B
    draw_change(rgb, built0, built0, WIN_B, 6, RED, "block_before.png", width=620)
    res = {"before": (built0, base_id)}
    for k in list(pmodes) + ["chain"]:
        cl = read_map(w / f"{k}_cl.tif")
        ids = read_map(w / f"{k}_id.tif")
        res[k] = (cl == BUILT, ids)
        draw_change(rgb, built0, cl == BUILT, WIN_B, 6, RED, f"block_{k}.png", width=620)

    def stats(mask, ids):
        n = len(np.unique(ids[mask]))
        return int(mask.sum()), n, round(edge_km(mask), 1)

    names = {"before": "Before", "shrink": "shrink 20 m", "open": "open 30 m", "close": "close 30 m (into cerrado, forest)",
             "fill": "fill, holes up to 40 cells", "smooth": "smooth, radius 30 m", "chain": "open, close, fill, smooth"}
    rows = []
    for k in ["before", "shrink", "open", "close", "fill", "smooth", "chain"]:
        c, n, e = stats(*res[k])
        rows.append({"step": names[k], "cells": c, "area_ha": round(c * 0.01, 1), "patches": n, "edge_km": e,
                     "change_cells": c - int(built0.sum())})
        print(rows[-1])
    figs.write_csv(DATA / "modes.csv", rows)
    # the reports: the five largest built-up patches
    for k in ("open", "close", "fill", "smooth"):
        rp = read_csv(w / "output" / f"rep_{k}.csv")
        rp.sort(key=lambda r: -float(r["before"]))
        figs.write_csv(DATA / f"report_{k}.csv", [{"object": int(float(r["object"])), "before": int(float(r["before"])),
                                                   "after": int(float(r["after"])), "pieces": int(float(r["pieces"]))}
                                                  for r in rp[:5]])
    # the compactness of the patches, before and after the chain
    def mean_compact(table, ids, mask):
        t = {int(float(r["id"])): (float(r["compactness"]), float(r["area"])) for r in table}
        ids_in = [i for i in np.unique(ids[mask]) if i in t]
        a = np.array([t[i][1] for i in ids_in])
        c = np.array([t[i][0] for i in ids_in])
        return float((a * c).sum() / a.sum())

    cb = mean_compact(read_csv(w / "output" / "fb.csv"), base_id, built0)
    ca = mean_compact(read_csv(w / "output" / "fa.csv"), res["chain"][1], res["chain"][0])
    print("area-weighted compactness of the built-up patches", round(cb, 3), "->", round(ca, 3))
    figs.write_csv(DATA / "compactness.csv", [{"patches": "before", "compactness": round(cb, 3)},
                                              {"patches": "after the chain", "compactness": round(ca, 3)}])
    # the whole picture, with the chain's outline
    fig = plt.figure(figsize=(5.4, 5.4), dpi=150)
    ax = fig.add_axes([0, 0, 1, 1])
    ax.imshow(rgb[R0:R1, C0:C1])
    for (a0, a1, b0, b1), col, lab in ((WIN_L, "#ffffff", "water"), (WIN_B, "#ffd91a", "built-up")):
        ax.add_patch(plt.Rectangle((b0 - C0, a0 - R0), b1 - b0, a1 - a0, fill=False, ec=col, lw=1.8))
        ax.text(b0 - C0 + 3, a0 - R0 + 10, lab, color=col, fontsize=9, fontweight="bold",
                bbox=dict(fc="#000000aa", ec="none", pad=1.5))
    ax.set_axis_off()
    fig.savefig(OUT / "where.png", dpi=150)
    plt.close(fig)
    od.save_png(np.asarray(plt.imread(OUT / "where.png"))[..., :3], OUT / "where.png", width=700, colors=200)

    # ============================================================================ the opening's and the smoothing's size
    sweep = {}
    ws = {}
    m = head() + PATCHES
    m = maps(m, "p0", "patches.objects", ws)
    open_d = [10, 20, 30, 40, 50, 60]
    smooth_d = [10, 20, 30, 40, 60]
    for d in open_d:
        m += node(f"o{d}", "objects.resize", '{ objects = "patches.objects" }',
                  f'{{ mode = "open", classes = ["built_up"], distance = {d} }}')
        m = maps(m, f"o{d}", f"o{d}.objects", ws)
    for d in smooth_d:
        m += node(f"s{d}", "objects.resize", '{ objects = "patches.objects" }',
                  f'{{ mode = "smooth", classes = ["built_up"], distance = {d} }}')
        m = maps(m, f"s{d}", f"s{d}.objects", ws)
    w = run(m, "sweep", ws)
    for prefix, vals, label, fname in (("o", open_d, "Opening distance (m)", "open"), ("s", smooth_d, "Smoothing radius (m)", "smooth")):
        rows = []
        for d in vals:
            cl = read_map(w / f"{prefix}{d}_cl.tif") == BUILT
            ids = read_map(w / f"{prefix}{d}_id.tif")
            c, n, e = stats(cl, ids)
            rows.append({fname + "_m": d, "cells": c, "patches": n, "edge_km": e})
            draw_change(rgb, built0, cl, WIN_B, 5, RED, f"{fname}_{d}.png", width=520)
        rows.insert(0, {fname + "_m": 0, "cells": int(built0.sum()), "patches": len(np.unique(base_id[built0])),
                        "edge_km": round(edge_km(built0), 1)})
        draw_change(rgb, built0, built0, WIN_B, 5, RED, f"{fname}_0.png", width=520)
        figs.write_csv(DATA / f"{fname}_sweep.csv", rows)
        print(fname, rows)

    # ===================================================================================================== refine
    rf = {}
    m = head() + node("seg", "segment.multiresolution", '{ image = "refl.image" }', "{ scale = 2 }")
    m = maps(m, "r0", "seg.objects", rf)
    rf["refl.image"] = "img.tif"
    passes = [1, 2, 3, 5, 10]
    for s in passes:
        m += node(f"r{s}", "objects.resize", '{ objects = "seg.objects", image = "refl.image" }',
                  f'{{ mode = "refine", steps = {s} }}')
        m = maps(m, f"r{s}", f"r{s}.objects", rf)
        rf[f"r{s}.report"] = f"rep{s}.csv"
    m += node("clean", "objects.remove_small", '{ objects = "r3.objects" }', '{ min_cells = 4, into = "border" }')
    m = maps(m, "clean", "clean.objects", rf)
    w = run(m, "refine", rf)
    with rasterio.open(w / "img.tif") as ds:
        img = ds.read().astype("float64")
    z = (img - img.mean(axis=(1, 2), keepdims=True)) / img.std(axis=(1, 2), keepdims=True)

    def sse(ids):
        n = int(ids.max()) + 1
        tot = 0.0
        flat = ids.ravel()
        cnt = np.bincount(flat, minlength=n)
        for b in range(z.shape[0]):
            s1 = np.bincount(flat, z[b].ravel(), n)
            s2 = np.bincount(flat, z[b].ravel() ** 2, n)
            tot += float((s2 - s1 ** 2 / np.maximum(cnt, 1)).sum())
        return tot

    ids0 = read_map(w / "r0_id.tif")
    base_sse = sse(ids0)
    rows = [{"passes": 0, "objects": int(ids0.max()), "within_ss": round(base_sse, 1), "share_of_start": 100.0, "cells_moved": 0}]
    outs = {0: ids0}
    for s in passes:
        ids = read_map(w / f"r{s}_id.tif")
        outs[s] = ids
        rp = read_csv(w / "output" / f"rep{s}.csv")
        moved = int(sum(abs(float(r["after"]) - float(r["before"])) for r in rp) / 2)
        rows.append({"passes": s, "objects": int(ids.max()), "within_ss": round(sse(ids), 1),
                     "share_of_start": round(100 * sse(ids) / base_sse, 1), "cells_moved": moved})
    cl = read_map(w / "clean_id.tif")
    outs["clean"] = cl
    clean = {"passes": "3, then remove_small (4 cells)", "objects": int(cl.max()), "within_ss": round(sse(cl), 1),
             "share_of_start": round(100 * sse(cl) / base_sse, 1)}
    print(rows, clean)
    figs.write_csv(DATA / "refine.csv", rows)
    figs.write_csv(DATA / "refine_clean.csv", [clean])
    # the objects of the 5 cell wide lake border and a built-up block, before and after 3 passes
    rwin = (265, 325, 330, 400)
    s = 10
    for key, nm in ((0, "refine_before"), (3, "refine_after"), ("clean", "refine_clean")):
        img_o = od.outlines(rgb[rwin[0]:rwin[1], rwin[2]:rwin[3]], outs[key][rwin[0]:rwin[1], rwin[2]:rwin[3]], scale=s,
                            color=(1.0, 0.85, 0.1), thick=2)
        od.save_png(img_o, OUT / f"{nm}.png", width=640, colors=200)
    # the outlines before (cyan), after 3 passes (yellow) and on both (white)
    big = np.kron(rgb[rwin[0]:rwin[1], rwin[2]:rwin[3]], np.ones((s, s, 1)))
    b0 = od.boundary(np.kron(outs[0][rwin[0]:rwin[1], rwin[2]:rwin[3]], np.ones((s, s), dtype=np.int64)))
    b3 = od.boundary(np.kron(outs[3][rwin[0]:rwin[1], rwin[2]:rwin[3]], np.ones((s, s), dtype=np.int64)))
    big[b0 & ~b3] = LOSS
    big[b3 & ~b0] = GAIN
    big[b0 & b3] = (1, 1, 1)
    od.save_png(big, OUT / "refine_diff.png", width=640, colors=200)
    print("done")


if __name__ == "__main__":
    main()
