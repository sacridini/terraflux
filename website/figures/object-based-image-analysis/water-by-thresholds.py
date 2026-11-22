"""Figures of "Water by thresholds" (tutorials/object-based-image-analysis/water-by-thresholds.md).

One model of Terraflux, run on the Sentinel-2 clip of the series (obia_data.py): the NDWI, segment.threshold (two
classes by Otsu, a sweep of given thresholds, three classes, the MNDWI), objects.rules for water and land,
objects.to_vector for the polygons (written with vector.write), segment.slic + segment.spectral_difference at several
limits, and the same Otsu on a 100 x 100 window of dry cerrado with no water. The figures draw the water over the RGB.
Run with: python website/figures/object-based-image-analysis/water-by-thresholds.py
"""

import csv
import json
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import matplotlib  # noqa: E402

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402
import numpy as np  # noqa: E402
import obia_data as od  # noqa: E402
import rasterio  # noqa: E402

figs = od.figs
SLUG = "water-by-thresholds"
OUT = figs.img_dir(f"{od.CATEGORY}/{SLUG}")
DATA = figs.data_dir(f"{od.CATEGORY}/{SLUG}")
CELL_HA = 0.01

# the part of the clip the figures show (rows, columns): the lake, its shore, the town and the cerrado below
R0, R1, C0, C1 = 0, 290, 120, 410
# a detail of the shore (rows, columns of the clip) for the polygons
D = (150, 260, 150, 290)
# the window of dry cerrado with no water (as in choosing-the-scale)
CERRADO = (440, 540, 250, 350)
THRESHOLDS = [-0.3, -0.2, -0.1, 0, 0.061, 0.2, 0.3, 0.4, 0.5, 0.6]
LIMITS = [0.02, 0.05, 0.1, 0.2, 0.4]
BLUE = np.array([0.10, 0.50, 1.0])


def tid(t):
    return "t" + str(t).replace("-", "n").replace(".", "_")


def lid(d):
    return "sd" + str(d).replace(".", "_")


def node(i, op, inputs, params):
    return f'\n[[nodes]]\nid = "{i}"\nop = "{op}"\ninputs = {inputs}\nparams = {params}\n'


def extent(r0, r1, c0, c1):
    x0, y1 = od.S2_UTM_EXTENT[0], od.S2_UTM_EXTENT[3]
    return [x0 + c0 * 10, y1 - r1 * 10, x0 + c1 * 10, y1 - r0 * 10]


def build():
    m = od.sentinel2_model()
    m += node("ndwi", "raster.calc", '{ s = "refl.image" }', '{ expression = "(s.green - s.nir) / (s.green + s.nir)" }')
    m += node("mndwi", "raster.calc", '{ s = "refl.image" }', '{ expression = "(s.green - s.swir16) / (s.green + s.swir16)" }')
    w = {"ndwi.map": "ndwi.tif", "mndwi.map": "mndwi.tif"}
    # Otsu, two classes, and the water / land classes, the polygons
    m += node("th", "segment.threshold", '{ image = "ndwi.map" }', "{ classes = 2 }")
    m += node("water", "objects.rules", '{ objects = "th.objects" }',
              '{ classes = [{ name = "water", rule = "above(class, 1, 2)", colour = "#2b6cb0" }, '
              '{ name = "land", rule = "below(class, 1, 2)", colour = "#c8b88a" }] }')
    m += node("poly0", "objects.to_vector", '{ objects = "water.objects" }', "{ }")
    m += node("poly1", "objects.to_vector", '{ objects = "water.objects" }', "{ smooth = 1 }")
    m += node("w0", "vector.write", '{ vector = "poly0.vector" }', '{ path = "water_cells.gpkg" }')
    m += node("w1", "vector.write", '{ vector = "poly1.vector" }', '{ path = "water_smooth.gpkg" }')
    w |= {"th.table": "th.csv", "th.thresholds": "thresholds.csv", "water.map": "water.tif", "water.table": "water_classes.csv"}
    # Otsu with three classes, and on the MNDWI
    m += node("th3", "segment.threshold", '{ image = "ndwi.map" }', "{ classes = 3 }")
    m += node("th3_map", "objects.to_map", '{ objects = "th3.objects" }', '{ what = "id" }')
    m += node("thm", "segment.threshold", '{ image = "mndwi.map" }', "{ classes = 2 }")
    w |= {"th3.table": "th3.csv", "th3_map.map": "th3_map.tif", "th3.thresholds": "thresholds3.csv", "thm.table": "thm.csv", "thm.thresholds": "thresholdsm.csv"}
    # a sweep of given thresholds
    for t in THRESHOLDS:
        i = tid(t)
        m += node(i, "segment.threshold", '{ image = "ndwi.map" }', f"{{ thresholds = [{t}] }}")
        w[f"{i}.table"] = f"{i}.csv"
    # the thresholds' classes as maps: the class of every cell (1 below, 2 above), made by objects.rules on a copy
    for t in THRESHOLDS + ["otsu"]:
        i = "tw_" + (tid(t) if t != "otsu" else "otsu")
        src = tid(t) if t != "otsu" else "th"
        m += node(i, "objects.rules", f'{{ objects = "{src}.objects" }}',
                  '{ classes = [{ name = "water", rule = "above(class, 1, 2)" }, { name = "land", rule = "below(class, 1, 2)" }] }')
        w[f"{i}.map"] = f"{i}.tif"
    # objects first: SLIC on the NDWI, merged by spectral difference at several limits
    m += node("sl", "segment.slic", '{ image = "ndwi.map" }', "{ size = 6 }")
    w["sl.table"] = "sl.csv"
    for d in LIMITS:
        i = lid(d)
        m += node(i, "segment.spectral_difference", '{ objects = "sl.objects", image = "ndwi.map" }', f"{{ max_difference = {d} }}")
        m += node(i + "_id", "objects.to_map", f'{{ objects = "{i}.objects" }}', '{ what = "id" }')
        w[i + "_id.map"] = i + ".tif"
        w[i + ".table"] = i + ".csv"
    # the objects of the middle limit, classed by the mean NDWI with a ramp around the Otsu cut
    m += node("owater", "objects.rules", '{ objects = "sd0_1.objects" }',
              '{ classes = [{ name = "water", rule = "above(mean_image, 0.0, 0.13)" }, '
              '{ name = "land", rule = "below(mean_image, 0.0, 0.13)" }] }')
    w |= {"owater.map": "owater.tif", "owater.table": "owater.csv"}
    # a window of dry cerrado: Otsu finds a cut even when there is no water
    m += node("cer", "raster.clip", '{ raster = "refl.image" }', f"{{ extent = {extent(*CERRADO)} }}")
    m += node("cer_ndwi", "raster.calc", '{ s = "cer.raster" }', '{ expression = "(s.green - s.nir) / (s.green + s.nir)" }')
    m += node("cer_th", "segment.threshold", '{ image = "cer_ndwi.map" }', "{ classes = 2 }")
    m += node("cer_w", "objects.rules", '{ objects = "cer_th.objects" }',
              '{ classes = [{ name = "water", rule = "above(class, 1, 2)" }, { name = "land", rule = "below(class, 1, 2)" }] }')
    w |= {"cer_th.thresholds": "cer_thresholds.csv", "cer_th.table": "cer_th.csv", "cer_w.map": "cer_water.tif",
          "cer_ndwi.map": "cer_ndwi.tif"}
    return figs.add_writes(m, w)


def read(work, name):
    with rasterio.open(work / name) as ds:
        a = ds.read(1, masked=True).astype("float64")
    return np.ma.filled(a, np.nan)


def ids(work, name):
    return np.nan_to_num(read(work, name), nan=-1).astype("int64")


def table(path):
    with open(path, newline="", encoding="utf-8") as f:
        return list(csv.DictReader(f))


def cut(path):
    return [float(r["threshold"]) for r in table(path)]


def crop(a):
    return a[R0:R1, C0:C1]


def tint(rgb, water):
    """The RGB with the water cells tinted blue and the water's edge drawn (2 px at 2x)."""
    img = rgb.copy()
    img[water] = 0.45 * img[water] + 0.55 * BLUE
    return od.outlines(img, water.astype("int64"), color=(0.05, 0.95, 1.0), scale=2, thick=2)


def main():
    od.sentinel2()
    work = figs.workdir(f"{od.CATEGORY}/{SLUG}")
    r = od.run(build(), work, "--profile")
    print(r.stdout[:600])
    out = work / "output"
    rgb = od.rgb("sentinel2")
    nd = read(work, "ndwi.tif")
    cuts = cut(out / "thresholds.csv")
    otsu = cuts[0]
    print("otsu", otsu, "three classes", cut(out / "thresholds3.csv"), "mndwi", cut(out / "thresholdsm.csv"),
          "cerrado", cut(out / "cer_thresholds.csv"))
    th = table(out / "th.csv")
    print("objects", len(th), sorted(((int(x["cells"]), x["class"]) for x in th), reverse=True)[:8])

    # --- the NDWI and its histogram with the cut
    figs.raster(work / "ndwi.tif", OUT / "ndwi.png", cmap="RdYlBu", vmin=-0.7, vmax=0.7, legend=True,
                unit="NDWI", title=None)
    values = nd[np.isfinite(nd)]

    def hist(ax):
        ax.hist(values, bins=140, color=figs.WATER, alpha=0.9)
        ax.set_yscale("log")
        ax.axvline(otsu, color=figs.EMBER, lw=2)
        ax.text(otsu + 0.02, ax.get_ylim()[1] * 0.35, f"Otsu: {otsu:.3f}", color=figs.EMBER, fontsize=9.5, fontweight="bold")
        ax.axvline(0, color=figs.INK2, lw=1, ls="--")
        ax.text(-0.02, ax.get_ylim()[1] * 0.12, "NDWI = 0", color=figs.INK2, fontsize=9, ha="right")
        ax.set_xlabel("NDWI of a cell")
        ax.set_ylabel("Cells (log scale)")

    figs.chart(OUT / "histogram.png", hist, width=900, height=380)

    # --- water by Otsu over the clip, and its classes
    water = ids(work, "tw_otsu.tif") == 1  # objects.rules numbers its classes in the list's order: water is 1
    fig = plt.figure(figsize=(5.4, 5.4), dpi=150)
    ax = fig.add_axes([0, 0, 1, 1])
    img = rgb.copy()
    img[water] = 0.4 * img[water] + 0.6 * BLUE
    ax.imshow(img)
    ax.set_axis_off()
    ax.add_patch(plt.Rectangle((C0, R0), C1 - C0, R1 - R0, fill=False, ec="#ffd91a", lw=1.4))
    ax.add_patch(plt.Rectangle((D[2], D[0]), D[3] - D[2], D[1] - D[0], fill=False, ec="#ffffff", lw=1.4))
    fig.savefig(OUT / "water_clip.png", dpi=150)
    plt.close(fig)
    od.save_png(np.asarray(plt.imread(OUT / "water_clip.png"))[..., :3], OUT / "water_clip.png", width=700, colors=160)
    wc = int(water.sum())
    print("water cells", wc, "ha", wc * CELL_HA)

    # --- the threshold sweep: frames and numbers
    rows = []
    for t in THRESHOLDS:
        i = tid(t)
        w = ids(work, f"tw_{i}.tif") == 1
        img = tint(crop(rgb), crop(w))
        od.save_png(img, OUT / f"thr_{i}.png", width=480, colors=64)
        tb = table(out / f"{i}.csv")
        objs = sum(1 for x in tb if x["class"] == "2")
        cells = sum(int(x["cells"]) for x in tb if x["class"] == "2")
        rows.append({"threshold": t, "water_ha": round(cells * CELL_HA, 1), "water_objects": objs})
    print(rows)
    figs.write_csv(DATA / "sweep.csv", rows)
    # Otsu's own frame, for the slider's text
    img = tint(crop(rgb), crop(water))
    od.save_png(img, OUT / "thr_otsu.png", width=480, colors=64)

    # --- classes of objects: SLIC + spectral difference
    srows = []
    for d in LIMITS:
        i = lid(d)
        lab = ids(work, f"{i}.tif")
        n = len(table(out / f"{i}.csv"))
        od.save_png(od.outlines(crop(rgb), crop(lab), scale=2, thick=2), OUT / f"sd_{str(d).replace('.', '_')}.png", width=480, colors=80)
        srows.append({"max_difference": d, "objects": n})
    sl = len(table(out / "sl.csv"))
    print("slic", sl, srows)
    figs.write_csv(DATA / "spectral_difference.csv", srows)
    ow = ids(work, "owater.tif") == 1  # class 1 is water here (the first of the list)
    od.save_png(tint(crop(rgb), crop(ow)), OUT / "objects_water.png", width=480, colors=64)
    print("object water cells", int(ow.sum()), "pixel otsu cells", wc, "disagree", int((ow != water).sum()))
    # where the two differ
    sd = np.zeros(water.shape + (3,))
    sd[:] = rgb * 0.55 + 0.45
    sd[water & ow] = BLUE
    sd[water & ~ow] = (0.9, 0.2, 0.1)
    sd[~water & ow] = (0.95, 0.75, 0.0)

    # --- the three classes of Otsu
    c3 = table(out / "th3.csv")
    t3 = cut(out / "thresholds3.csv")
    lab3 = ids(work, "th3_map.tif")
    cls = {int(x["id"]): int(x["class"]) for x in c3}
    cmap = np.zeros(lab3.shape, int)
    for k, c in cls.items():
        cmap[lab3 == k] = c
    pal = np.array([[0.2, 0.2, 0.2], [0.23, 0.54, 0.35], [0.85, 0.78, 0.60], [0.17, 0.42, 0.69]])
    img3 = pal[cmap]
    img3 = np.kron(img3[R0:R1, C0:C1], np.ones((2, 2, 1)))
    od.save_png(img3, OUT / "three_classes.png", width=560, colors=16)
    n3 = {c: int((cmap == c).sum()) for c in (1, 2, 3)}
    print("three classes", t3, n3)

    # --- the MNDWI and the cerrado window
    thm = cut(out / "thresholdsm.csv")[0]
    thm_t = table(out / "thm.csv")
    mcells = sum(int(x["cells"]) for x in thm_t if x["class"] == "2")
    cer_cut = cut(out / "cer_thresholds.csv")[0]
    cer_w = ids(work, "cer_water.tif")
    cer_ndwi = read(work, "cer_ndwi.tif")
    print("mndwi cut", thm, "cells", mcells, "cerrado cut", cer_cut, "class water share", float((cer_w == 1).mean()),
          "ndwi range", float(np.nanmin(cer_ndwi)), float(np.nanmax(cer_ndwi)))
    a, b, c, d = CERRADO
    cimg = np.kron(rgb[a:b, c:d], np.ones((4, 4, 1)))
    cw = np.kron((cer_w == 1), np.ones((4, 4), bool))
    cimg2 = cimg.copy()
    cimg2[cw] = 0.45 * cimg2[cw] + 0.55 * BLUE
    od.save_png(np.concatenate([cimg, np.ones((cimg.shape[0], 10, 3)), cimg2], axis=1), OUT / "cerrado.png", width=900, colors=128)

    # --- the polygons: the cell edges and the smoothed corners, over the RGB of the detail
    import geopandas as gpd

    r0, r1, c0, c1 = D
    ext = extent(r0, r1, c0, c1)
    fig, axes = plt.subplots(1, 2, figsize=(7.2, 7.2 * (r1 - r0) / (c1 - c0) / 2 + 0.55), dpi=150)
    polys = {}
    for ax, (name, title) in zip(axes, (("water_cells.gpkg", "smooth = 0 (cell edges)"), ("water_smooth.gpkg", "smooth = 1"))):
        g = gpd.read_file(work / name)
        g = g[g["class"] == "water"]
        polys[name] = (len(g), float(g.geometry.area.sum()) / 10000)
        ax.imshow(rgb[r0:r1, c0:c1], extent=(ext[0], ext[2], ext[1], ext[3]), interpolation="nearest")
        g.boundary.plot(ax=ax, color="#00e5ff", linewidth=1.3)
        ax.set_xlim(ext[0], ext[2])
        ax.set_ylim(ext[1], ext[3])
        ax.set_axis_off()
        ax.set_title(title, fontsize=10, loc="left")
    fig.tight_layout(pad=0.8)
    fig.savefig(OUT / "polygons.png", dpi=150)
    plt.close(fig)
    od.save_png(np.asarray(plt.imread(OUT / "polygons.png"))[..., :3], OUT / "polygons.png", width=900, colors=128)
    print("polygons", polys)

    # numbers for the table
    zero = [x for x in rows if x["threshold"] == 0][0]
    summary = {
        "otsu": otsu, "water_cells": wc, "water_ha": wc * CELL_HA, "objects_otsu": len(th),
        "water_objects": sum(1 for x in th if x["class"] == "2"), "zero_ha": zero["water_ha"],
        "mndwi_cut": thm, "mndwi_ha": mcells * CELL_HA, "three": t3, "three_cells": n3,
        "object_water_ha": float(ow.sum()) * CELL_HA, "slic_objects": sl,
        "polygons": polys, "cerrado_cut": cer_cut, "cerrado_water_cells": int((cer_w == 1).sum()),
    }
    (work / "summary.json").write_text(json.dumps(summary, indent=1), encoding="utf-8")
    print(summary)


if __name__ == "__main__":
    main()
