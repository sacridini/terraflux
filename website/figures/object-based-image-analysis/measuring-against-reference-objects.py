"""Figures of "Measuring a segmentation against reference objects"
(tutorials/object-based-image-analysis/measuring-against-reference-objects.md).

One model of Terraflux, run on the Sentinel-2 clip of the series (obia_data.py, read from its cache with raster.read, so
nothing is downloaded): twelve reference polygons drawn over the lake edge of Brasilia (a GeoJSON written here, the
coordinates read by eye off the clip's RGB: what a user draws with Draw on the map), the multiresolution segmentation at
the ten scales of part 3, each measured by segment.evaluate, segment.optimize with the reference (the objective ed2) and
without it (USPO). The figures draw the reference over the clip, and the objects at three scales coloured by os less us.
Run with: TERRAFLUX_BIN=<release terraflux> python website/figures/object-based-image-analysis/measuring-against-reference-objects.py
"""

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
SLUG = "measuring-against-reference-objects"
OUT = figs.img_dir(f"{od.CATEGORY}/{SLUG}")
DATA = figs.data_dir(f"{od.CATEGORY}/{SLUG}")

# the part of the clip the figures show (as part 3's slider)
R0, R1, C0, C1 = 90, 360, 200, 470
SL = [1, 1.5, 2, 3, 4, 6, 8, 12, 16, 24]
# the references: (name, the corners as (column, row) of the clip's cells), drawn by eye over the RGB
REFS = [
    ("lake", [(215, 100), (255, 95), (285, 120), (270, 150), (235, 165), (212, 140)]),
    ("inlet", [(206, 205), (222, 205), (224, 225), (207, 228)]),
    ("cerrado", [(420, 270), (468, 262), (468, 350), (405, 350), (395, 310)]),
    ("bare shore", [(322, 125), (355, 120), (340, 148), (315, 145)]),
    ("building site", [(362, 290), (380, 290), (382, 313), (364, 314)]),
    ("forest strip", [(216, 265), (230, 265), (232, 310), (226, 358), (214, 358), (212, 300)]),
    ("forest creek", [(342, 262), (352, 262), (366, 300), (358, 312), (348, 290)]),
    ("block west", [(243, 210), (282, 203), (290, 222), (250, 240)]),
    ("block north", [(352, 110), (392, 108), (392, 150), (354, 152)]),
    ("block south", [(305, 275), (345, 272), (345, 305), (308, 310)]),
    ("avenue", [(205, 255), (330, 212), (332, 218), (207, 262)]),
    ("white roof", [(243, 296), (254, 296), (254, 308), (243, 308)]),
]


def sid(s):
    s = int(s) if float(s) == int(float(s)) else s
    return f"mr_{str(s).replace('.', '_')}"


def node(i, op, inputs, params):
    return f'\n[[nodes]]\nid = "{i}"\nop = "{op}"\ninputs = {inputs}\nparams = {params}\n'


def utm(c, r):
    x0, y1 = od.S2_UTM_EXTENT[0], od.S2_UTM_EXTENT[3]
    return [x0 + c * 10, y1 - r * 10]


def write_reference(work):
    feats = [
        {"type": "Feature", "properties": {"name": n},
         "geometry": {"type": "Polygon", "coordinates": [[utm(c, r) for c, r in pts + [pts[0]]]]}}
        for n, pts in REFS
    ]
    gj = {"type": "FeatureCollection", "crs": {"type": "name", "properties": {"name": "urn:ogc:def:crs:EPSG::31983"}},
          "features": feats}
    path = work / "reference.geojson"
    path.write_text(json.dumps(gj), encoding="utf-8")
    return path


def build(tif, ref):
    m = 'format = "terraflux/1"\n'
    m += node("s2", "raster.read", "{ }", f'{{ path = "{tif.as_posix()}" }}')
    m += "\n" + od.reflectance_node("refl", "s2.raster")
    m += node("ref", "vector.read", "{ }", f'{{ path = "{ref.as_posix()}" }}')
    values = ", ".join(str(v) for v in SL)
    m += node("opt_ref", "segment.optimize", '{ image = "refl.image", reference = "ref.vector" }',
              f'{{ values = [{values}], objective = "ed2" }}')
    m += node("opt", "segment.optimize", '{ image = "refl.image" }', f"{{ values = [{values}] }}")
    writes = {"opt_ref.scores": "scores_ref.csv", "opt.scores": "scores.csv"}
    for s in SL:
        i = sid(s)
        m += node(i, "segment.multiresolution", '{ image = "refl.image" }', f"{{ scale = {s} }}")
        m += node(f"{i}_map", "objects.to_map", f'{{ objects = "{i}.objects" }}', '{ what = "id" }')
        m += node(f"ev_{i}", "segment.evaluate", f'{{ objects = "{i}.objects", reference = "ref.vector" }}', "{ }")
        writes |= {f"{i}_map.map": f"{i}.tif", f"ev_{i}.table": f"pairs_{i}.csv", f"ev_{i}.summary": f"summary_{i}.csv"}
    return figs.add_writes(m, writes)


def labels(work, name):
    with rasterio.open(work / f"{name}.tif") as ds:
        a = ds.read(1, masked=True).astype("float64")
    return np.ma.filled(a, -1).astype("int64")


def table(path):
    import csv

    with open(path, newline="", encoding="utf-8") as f:
        return [{k: float(v) if v not in ("", "NaN", "nan") else float("nan") for k, v in r.items()} for r in csv.DictReader(f)]


def scaled(x):
    """segment.optimize's scaling over the runs: 1 for the lowest, 0 for the highest."""
    x = np.asarray(x, float)
    lo, hi = np.nanmin(x), np.nanmax(x)
    return (hi - x) / (hi - lo) if hi > lo else np.zeros_like(x)


def cell_polygon(ax, pts, **kw):
    ax.add_patch(plt.Polygon([(c - C0, r - R0) for c, r in pts], closed=True, fill=False, **kw))


def errors_panel(rgb, lab, pairs, path, title):
    """The objects coloured by os less us (red: the reference was cut; blue: the object holds what is outside it)."""
    v = np.full(lab.shape, np.nan)
    per = {}
    for p in pairs:
        if p["best_for_object"] == 1:
            per.setdefault(int(p["object"]), p["os"] - p["us"])
    for k, x in per.items():
        v[lab == k] = x
    fig = plt.figure(figsize=(4.6, 4.6), dpi=130)
    ax = fig.add_axes([0, 0, 1, 1])
    ax.imshow(rgb[R0:R1, C0:C1])
    show = np.ma.masked_invalid(v[R0:R1, C0:C1])
    ax.imshow(show, cmap="RdBu_r", vmin=-1, vmax=1, alpha=0.62, interpolation="nearest")
    b = od.boundary(lab[R0:R1, C0:C1])
    ax.imshow(np.ma.masked_where(~b, np.ones(b.shape)), cmap="gray_r", vmin=0, vmax=1.6, alpha=0.55, interpolation="nearest")
    for _, pts in REFS:
        cell_polygon(ax, pts, ec="#111111", lw=1.2)
    ax.text(5, 14, title, color="white", fontsize=10, fontweight="bold",
            bbox={"fc": "#000000", "alpha": 0.55, "ec": "none", "pad": 2})
    ax.set_axis_off()
    fig.savefig(path)
    plt.close(fig)
    od.save_png(np.asarray(plt.imread(path))[..., :3], path, width=560, colors=128)


def main():
    tif = od.sentinel2()
    work = figs.workdir(f"{od.CATEGORY}/{SLUG}")
    ref = write_reference(work)
    r = od.run(build(tif, ref), work, "--profile")
    print(r.stdout[:300])
    out = work / "output"
    rgb = od.rgb("sentinel2", tif)

    # --- the references over the clip
    fig = plt.figure(figsize=(5.6, 5.6), dpi=140)
    ax = fig.add_axes([0, 0, 1, 1])
    ax.imshow(rgb[R0:R1, C0:C1])
    for k, (n, pts) in enumerate(REFS):
        cell_polygon(ax, pts, ec="#ffd91a", lw=1.6)
        cx = np.mean([c for c, _ in pts]) - C0
        cy = np.mean([r_ for _, r_ in pts]) - R0
        ax.text(cx, cy, str(k + 1), color="white", fontsize=9, fontweight="bold", ha="center", va="center",
                bbox={"fc": "#000000", "alpha": 0.5, "ec": "none", "pad": 1.5})
    ax.set_axis_off()
    fig.savefig(OUT / "references.png")
    plt.close(fig)
    od.save_png(np.asarray(plt.imread(OUT / "references.png"))[..., :3], OUT / "references.png", width=640, colors=128)
    cells = sum(0.5 * abs(sum(a[0] * b[1] - b[0] * a[1] for a, b in zip(p, p[1:] + p[:1]))) for _, p in REFS)
    print("reference area (cells)", cells)

    # --- the scores: the reference's against USPO's
    sc = table(out / "scores_ref.csv")
    uspo = table(out / "scores.csv")
    best_ed2 = min(sc, key=lambda x: x["ed2"])
    best_d = min(sc, key=lambda x: x["d"])
    best_f = max(sc, key=lambda x: x["f_overlap"])
    best_uspo = max(uspo, key=lambda x: x["objective"])
    print("best ed2", best_ed2["value"], "d", best_d["value"], "f", best_f["value"], "uspo", best_uspo["value"])
    summ = {s: table(out / f"summary_{sid(s)}.csv")[0] for s in SL}
    rows = []
    for k, x in enumerate(sc):
        s = x["value"]
        m = summ[s]
        rows.append({"scale": s, "objects": int(x["objects"]), "os": round(x["os"], 3), "us": round(x["us"], 3),
                     "d": round(x["d"], 3), "ed2": round(x["ed2"], 3), "f_overlap": round(x["f_overlap"], 3),
                     "afi": round(m["afi"], 3), "pse": round(m["pse"], 3), "nsr": round(m["nsr"], 3),
                     "uspo_objective": round(uspo[k]["objective"], 3),
                     "uspo_half": round(uspo[k]["objective"] / 2, 3),
                     "ed2_scaled": round(float(scaled([y["ed2"] for y in sc])[k]), 3)})
    figs.write_csv(DATA / "scores.csv", rows)
    for row in rows:
        print(row)
    # the objects of each scale that overlap a reference, and the pairs
    for s in SL:
        p = table(out / f"pairs_{sid(s)}.csv")
        print(s, "pairs", len(p), "matched", int(sum(1 for q in p if q["matched"] == 1)), "of", len(REFS),
              "summary", {k: round(v, 3) for k, v in summ[s].items() if k in ("os", "us", "d", "ed2", "f_overlap", "matched")})

    # --- the errors on the map, at a fine, the best and a coarse scale
    pick = {"fine": 1, "best": best_ed2["value"], "coarse": 16}
    for tag, s in pick.items():
        lab = labels(work, sid(s))
        pairs = table(out / f"pairs_{sid(s)}.csv")
        errors_panel(rgb, lab, pairs, OUT / f"errors_{tag}.png", f"scale {s}")
        print(tag, s, len(np.unique(lab[lab >= 0])), "objects")

    # --- a table of the pairs of the best scale (the references and their objects)
    p = table(out / f"pairs_{sid(best_ed2['value'])}.csv")
    rows = []
    for q in p:
        if q["matched"] == 1:
            rows.append({"reference": int(q["reference"]), "name": REFS[int(q["reference"]) - 1][0],
                         "object": int(q["object"]), "overlap": int(q["cells"]), "reference_cells": int(q["reference_cells"]),
                         "object_cells": int(q["object_cells"]), "os": round(q["os"], 3), "us": round(q["us"], 3),
                         "d": round(q["d"], 3)})
    figs.write_csv(DATA / "pairs_best.csv", rows)
    print("pairs of the best", len(rows))
    (work / "numbers.json").write_text(json.dumps({"best_ed2": best_ed2["value"], "best_uspo": best_uspo["value"]}), encoding="utf-8")


if __name__ == "__main__":
    main()
