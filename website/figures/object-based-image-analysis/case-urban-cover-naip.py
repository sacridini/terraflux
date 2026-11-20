"""Figures of "Case study: urban cover of a neighbourhood from NAIP" (tutorials/object-based-image-analysis/case-urban-cover-naip.md).

One model of Terraflux, run on a NAIP clip of Plano, Texas (0.6 m, red green blue nir, 14 October 2020) that the model
itself reads with data.stac (Planetary Computer, sign = planetary_computer, crs = EPSG:26914 and a grid in metres): the
multiresolution objects of scale 3, their spectral and texture attributes, a first pass of fuzzy rules (water, tree,
lawn, pavement), the objects merged into patches by class, the pavement split into pieces and the trees into canopy
pieces (objects.split), the skeleton and spectral attributes of the pieces, a second pass of rules (roof or street), the
outlines smoothed (objects.resize), the polygons (objects.to_vector) and an accuracy assessment against a stratified
reference sample read by eye from the image (see obia_case.py).

The reference is made in two runs of this script: the first run draws the sample (objects.sample, 50 per class) and
writes the image chips that are read to label it (`labels/urban-cover-reference.csv` is the reading); the next runs read
the labels and make every figure. `labels/urban-cover-tuning.csv` is the reading of 100 random pixels, a different sample
that was used to choose the rules' limits. At most 2 threads.
Run with: python website/figures/object-based-image-analysis/case-urban-cover-naip.py [--sheets]
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
import obia_case as oc  # noqa: E402
import obia_data as od  # noqa: E402
import rasterio  # noqa: E402
from matplotlib.patches import Patch  # noqa: E402

for _k in ("TERRAFLUX_THREADS", "RAYON_NUM_THREADS", "OMP_NUM_THREADS", "GDAL_NUM_THREADS", "OPENBLAS_NUM_THREADS",
           "MKL_NUM_THREADS"):
    os.environ[_k] = "2"

figs = od.figs
HERE = pathlib.Path(__file__).resolve().parent
SLUG = "case-urban-cover-naip"
OUT = figs.img_dir(f"{od.CATEGORY}/{SLUG}")
DATA = figs.data_dir(f"{od.CATEGORY}/{SLUG}")
LABELS = HERE / "labels"
CELL_M2 = 0.36
CRS = "EPSG:26914"
EXTENT = [705030.6, 3658458.0, 705630.6, 3659058.0]
DATE = "2020-10-14"
CLASSES = ["water", "tree", "lawn", "street", "roof"]  # the list's order: the map's numbers
COL = {"water": "#2b6cb0", "tree": "#2f7d3b", "lawn": "#a7d676", "street": "#8c8c8c", "roof": "#c0504d",
       "pavement": "#b0a090"}
# the details the figures show: (row0, row1, col0, col1) of the clip
D1 = (230, 390, 560, 720)  # park, street, pond, houses
D2 = (60, 220, 60, 220)  # houses and streets
DETAIL = D1


DATES = {"2020-10-14": "14 October 2020", "2018-11-26": "26 November 2018", "2022-07-04": "4 July 2022"}


def stac_node(node_id="naip", date=DATE):
    return (
        f'[[nodes]]\nid = "{node_id}"\nop = "data.stac"\n'
        'params = { url = "https://planetarycomputer.microsoft.com/api/stac/v1", collection = "naip", assets = ["image"], '
        f'datetime = "{date}", sign = "planetary_computer", crs = "{CRS}", extent = {EXTENT}, cellsize = 0.6 }}\n'
        f'note = "NAIP of {DATES[date]}: red, green, blue and nir layers, 0.6 m."\n'
    )


RULES1 = """[
  { name = "water", rule = "max(above(ndwi, 0.35, 0.45), min(above(ndwi, 0.15, 0.25), above(glcm_homogeneity, 0.55, 0.75)))" },
  { name = "land", rule = "1 - max(above(ndwi, 0.35, 0.45), min(above(ndwi, 0.15, 0.25), above(glcm_homogeneity, 0.55, 0.75)))" },
  { name = "veg", rule = "above(ndvi, 0.12, 0.2)", parent = "land" },
  { name = "tree", rule = "max(below(glcm_homogeneity, 0.45, 0.6), below(vis, 0.15, 0.2))", parent = "veg" },
  { name = "lawn", rule = "min(above(glcm_homogeneity, 0.45, 0.6), above(vis, 0.15, 0.2))", parent = "veg" },
  { name = "pavement", rule = "below(ndvi, 0.12, 0.2)", parent = "land" },
]"""

STREET = "min(below(skel_width, 11, 14), below(bmr, 0.02, 0.05))"
RULES2 = f"""[
  {{ name = "water", rule = 'cls == "water" and above(area, 100, 300)', colour = "#2b6cb0" }},
  {{ name = "tree", rule = 'cls == "tree"', colour = "#2f7d3b" }},
  {{ name = "lawn", rule = 'cls == "lawn"', colour = "#a7d676" }},
  {{ name = "street", rule = 'cls == "pavement" and {STREET}', colour = "#8c8c8c" }},
  {{ name = "roof", rule = '(cls == "pavement" and (1 - {STREET})) or (cls == "water" and below(area, 100, 300))', colour = "#c0504d" }},
]"""


def node(i, op, inputs, params, note=None):
    t = f'\n[[nodes]]\nid = "{i}"\nop = "{op}"\n'
    if inputs:
        t += f"inputs = {inputs}\n"
    t += f"params = {params}\n"
    if note:
        t += f'note = "{note}"\n'
    return t


def build(scale=3, tail="", rules2=RULES2, smooth=1.2, dates=None):
    """The model of the page, without the accuracy nodes (`tail` adds nodes and outputs after `clean`)."""
    m = 'format = "terraflux/1"\n\n' + stac_node()
    m += node("refl", "raster.calc", '{ s = "naip.raster" }', '{ expression = "out image = s / 255\\nout nir = s.nir / 255" }',
              "Reflectance 0 to 1: the four layers, and the near infrared alone.")
    m += node("seg", "segment.multiresolution", '{ image = "refl.image" }', f"{{ scale = {scale} }}",
              "The fine objects.")
    m += node("feat", "objects.features", '{ objects = "seg.objects", image = "refl.image" }', "{ features = [\"spectral\", \"texture\"], texture_bands = [\"brightness\"] }",
              "Colour, indices and texture.")
    m += node("calc", "objects.calc", '{ objects = "feat.objects" }',
              '{ expression = "out vis = (mean_red + mean_green + mean_blue) / 3" }', "Visible brightness.")
    m += node("rules", "objects.rules", '{ objects = "calc.objects" }', "{ classes = " + RULES1 + " }",
              "First pass: water, tree, lawn, pavement.")
    m += node("patch", "objects.merge", '{ objects = "rules.objects" }', '{ by = "class" }',
              "Neighbours of a class become one patch.")
    m += node("pieces", "objects.split", '{ objects = "patch.objects" }',
              '{ classes = ["pavement"], h = 1.5, min_size = 30 }', "Cut the pavement at its necks.")
    m += node("crowns", "objects.split", '{ objects = "pieces.objects", surface = "refl.nir" }',
              '{ classes = ["tree"], by = "surface", sigma = 3, h = 0.03, min_size = 30 }', "Canopy pieces from the near infrared.")
    m += node("pfeat", "objects.features", '{ objects = "crowns.objects", image = "refl.image" }',
              '{ features = ["spectral", "shape", "skeleton"] }', "Shape and skeleton of the pieces.")
    m += node("calc2", "objects.calc", '{ objects = "pfeat.objects" }', '{ expression = "out bmr = mean_blue - mean_red" }',
              "Blue minus red: roofs are cooler than streets.")
    m += node("final", "objects.rules", '{ objects = "calc2.objects" }', "{ classes = " + rules2 + " }",
              "Second pass: roof or street, small water to roof.")
    m += node("clean", "objects.resize", '{ objects = "final.objects" }', f"{{ mode = \"smooth\", distance = {smooth} }}",
              "Smooth the outlines.")
    return m + tail


def read(path):
    with rasterio.open(path) as ds:
        return ds.read(1, masked=True).astype("float64").filled(np.nan)


def table(path):
    with open(path, newline="", encoding="utf-8") as f:
        return list(csv.DictReader(f))


def paint(cls, names=CLASSES, window=None, blank=(1.0, 1.0, 1.0)):
    pal = np.ones((len(names) + 1, 3)) * np.array(blank)
    for k, n in enumerate(names, 1):
        if n in COL:
            h = COL[n]
            pal[k] = [int(h[i:i + 2], 16) / 255 for i in (1, 3, 5)]
    img = pal[np.nan_to_num(cls, nan=0).astype(int)]
    if window:
        r0, r1, c0, c1 = window
        img = img[r0:r1, c0:c1]
    return img


def save(img, name, width, colors=96):
    od.save_png(img, OUT / name, width=width, colors=colors)


def mpl_png(fig, name, width=1000, colors=96):
    p = OUT / name
    fig.savefig(p, dpi=150)
    plt.close(fig)
    od.save_png(np.asarray(plt.imread(p))[..., :3], p, width=width, colors=colors)


def xy_to_rc(x, y):
    """(row, col) of the clip's cell that holds the map point."""
    return int((EXTENT[3] - y) / 0.6), int((x - EXTENT[0]) / 0.6)


def stage_one(work):
    """Run the model up to `clean`, write the clip and the map, draw the reference sample and its chips."""
    tail = node("clean_map", "objects.to_map", '{ objects = "clean.objects" }', '{ what = "class" }')
    tail += node("sample", "objects.sample", '{ objects = "clean.objects" }', "{ per_class = 50, seed = 1 }")
    tail += node("w_pts", "vector.write", '{ vector = "sample.points" }', '{ path = "sample_points.gpkg" }')
    m = figs.add_writes(build(tail=tail), {"naip.raster": "naip_f32.tif", "clean_map.map": "clean.tif", "final.map": "final.tif"})
    r = figs.run(m, work, "--threads", "2", "--no-journal", "--profile")
    print(r.stdout[-900:])
    return work


def sample_order(work):
    """The sample's points in a shuffled order (the number must not tell the class): sample_order.csv and the cells."""
    import geopandas as gpd

    g = gpd.read_file(work / "sample_points.gpkg")
    rng = np.random.default_rng(3)
    order = rng.permutation(len(g))
    pts, rows = [], []
    for n, k in enumerate(order, 1):
        x, y = g.geometry.iloc[k].x, g.geometry.iloc[k].y
        pts.append(xy_to_rc(x, y))
        rows.append({"n": n, "id": int(g["id"].iloc[k]), "x": round(x, 1), "y": round(y, 1), "mapped": g["mapped"].iloc[k]})
    figs.write_csv(work / "sample_order.csv", rows)
    return rows, pts


def make_sheets(work, pts):
    rgb = od.rgb("naip", work / "naip_u8.tif", gamma=0.85)
    sheets = oc.chip_sheets(rgb, pts, work / "sheets", "ref", half=28)
    print(len(pts), "points,", len(sheets), "sheets in", work / "sheets")


def main():
    work = figs.workdir(f"{od.CATEGORY}/{SLUG}")
    stage_one(work)
    u8(work)
    rows, pts = sample_order(work)
    if "--sheets" in sys.argv or not (LABELS / "urban-cover-reference.csv").exists():
        make_sheets(work, pts)
        return
    figures(work, rows, pts)


def u8(work):
    """The clip as 8 bit (what NAIP is) for drawing and for the download."""
    with rasterio.open(work / "naip_f32.tif") as src:
        data = np.rint(src.read()).astype("uint8")
        prof = src.profile | {"dtype": "uint8", "compress": "deflate", "predictor": 2, "tiled": False, "nodata": None}
        prof.pop("blockxsize", None)
        prof.pop("blockysize", None)
        desc = src.descriptions
    with rasterio.open(work / "naip_u8.tif", "w", **prof) as out:
        out.write(data)
        out.descriptions = desc


def reference_points(work):
    """reference_points.csv (x, y, class): the sample's points with the labels read from the chips."""
    order = figs.read_csv(work / "sample_order.csv")
    labels = oc.read_labels(LABELS / "urban-cover-reference.csv")
    rows = [{"x": r["x"], "y": r["y"], "class": labels[int(r["n"])]} for r in order]
    figs.write_csv(work / "reference_points.csv", rows)
    figs.write_csv(DATA / "reference_points.csv", rows)
    return order, labels


ACC = """
[[nodes]]
id = "ref_table"
op = "table.read"
params = { path = "reference_points.csv" }
note = "The reference: x, y and the class read from the image."

[[nodes]]
id = "ref"
op = "vector.from_table"
inputs = { table = "ref_table.table" }
params = { crs = "EPSG:26914" }

[[nodes]]
id = "acc"
op = "objects.accuracy"
inputs = { objects = "clean.objects", samples = "ref.vector" }
params = { design = "objects" }
note = "Error matrix, accuracies and adjusted areas."
"""

REPORT = """
[[nodes]]
id = "map"
op = "objects.to_map"
inputs = { objects = "clean.objects" }
params = { what = "class" }

[[nodes]]
id = "map_file"
op = "raster.write"
inputs = { value = "map.map" }
params = { path = "urban_cover.tif" }
note = "The map, a GeoTIFF."

[[nodes]]
id = "polygons"
op = "objects.to_vector"
inputs = { objects = "clean.objects" }
params = { smooth = 1 }
note = "One polygon per object: its number, class, cells and area."

[[nodes]]
id = "polygons_file"
op = "vector.write"
inputs = { vector = "polygons.vector" }
params = { path = "urban_cover.gpkg" }
"""


def run_main(work):
    """The page's model with the accuracy, and the extra nodes that only the figures need."""
    tail = REPORT + ACC
    # figure-only nodes: object numbers of each level and the attributes the figures draw
    extra = ""
    ids = {"seg_id": "calc", "patch_id": "patch", "pieces_id": "pieces", "crowns_id": "crowns", "final_id": "clean"}
    for k, src in ids.items():
        extra += node(k, "objects.to_map", f'{{ objects = "{src}.objects" }}', '{ what = "id" }')
    extra += node("crowns_d", "objects.split", '{ objects = "pieces.objects" }',
                  '{ classes = ["tree"], h = 1.2, min_size = 30 }')
    extra += node("crowns_d_id", "objects.to_map", '{ objects = "crowns_d.objects" }', '{ what = "id" }')
    writes = {f"{k}.map": f"{k}.tif" for k in ids} | {"crowns_d_id.map": "crowns_d_id.tif", "rules.map": "pass1.tif",
                                                      "final.map": "final.tif", "clean.map": "x"}
    writes.pop("clean.map")
    writes |= {"calc.table": "seg_table.csv", "calc2.table": "pieces_table.csv", "final.table": "final_table.csv",
               "acc.table": "acc_table.csv", "acc.overall": "acc_overall.csv", "acc.matrix": "acc_matrix.csv",
               "acc.errors": "acc_errors.csv", "clean.report": "clean_report.csv", "pieces.report": "split_report.csv",
               "crowns.report": "crowns_report.csv", "patch.count": "patch_count.json", "seg.table": "seg_all.csv"}
    # the same map with trees and lawns as one class, against the reference with its labels joined
    join = {"tree": "vegetation", "lawn": "vegetation"}
    rows = figs.read_csv(work / "reference_points.csv")
    figs.write_csv(work / "reference_points_veg.csv", [{**r, "class": join.get(r["class"], r["class"])} for r in rows])
    extra += node("veg", "objects.reclassify", '{ objects = "clean.objects" }', '{ classes = { tree = "vegetation", lawn = "vegetation" } }')
    extra += node("ref_veg_table", "table.read", None, '{ path = "reference_points_veg.csv" }')
    extra += node("ref_veg", "vector.from_table", '{ table = "ref_veg_table.table" }', '{ crs = "EPSG:26914" }')
    extra += node("acc_veg", "objects.accuracy", '{ objects = "veg.objects", samples = "ref_veg.vector" }', '{ design = "objects" }')
    writes |= {"acc_veg.table": "accveg_table.csv", "acc_veg.overall": "accveg_overall.csv", "acc_veg.matrix": "accveg_matrix.csv"}
    m = figs.add_writes(build(tail=tail + extra), writes)
    r = figs.run(m, work, "--threads", "2", "--no-journal", "--profile")
    print(r.stdout[-1200:])
    (work / "page_model.toml").write_text(build(tail=tail), encoding="utf-8")


def crop(a, box):
    r0, r1, c0, c1 = box
    return a[r0:r1, c0:c1]


def table_by_id(path):
    """The rows of a table of objects as {column: array}, in the order of the ids (1..n)."""
    rows = table(path)
    return {k: np.array([float(r[k]) if r[k] not in ("", "nan") else np.nan for r in rows]) for k in rows[0]
            if k != "class"}


def column_map(ids, tab, col):
    """A map of one column of the objects' table (the objects' number map `ids`)."""
    v = np.full(ids.shape, np.nan)
    m = ids > 0
    v[m] = tab[col][ids[m] - 1]
    return v


def legend(ax, names=CLASSES, ncol=5):
    hs = [Patch(fc=COL[n], label=n) for n in names]
    ax.legend(handles=hs, loc="upper center", bbox_to_anchor=(0.5, -0.01), ncol=ncol, frameon=False, fontsize=8)


def sweep_scale(work, s):
    """The page's model at another scale of the fine objects, assessed against the same reference points."""
    w = work / f"scale_{str(s).replace('.', '_')}"
    w.mkdir(exist_ok=True)
    (w / "reference_points.csv").write_text((work / "reference_points.csv").read_text(encoding="utf-8"), encoding="utf-8")
    tail = ACC + node("seg_id", "objects.to_map", '{ objects = "calc.objects" }', '{ what = "id" }')
    m = figs.add_writes(build(scale=s, tail=tail), {"seg_id.map": "seg_id.tif", "final.map": "final.tif",
                                                    "acc.overall": "acc_overall.csv", "acc.table": "acc_table.csv",
                                                    "final.table": "final_table.csv"})
    figs.run(m, w, "--threads", "2", "--no-journal")
    ov = table(w / "output" / "acc_overall.csv")[0]
    return {"scale": s, "fine_objects": int(np.nanmax(read(w / "seg_id.tif"))),
            "final_objects": len(table(w / "output" / "final_table.csv")),
            "overall_adjusted": float(ov["overall_adjusted"]), "overall_ci": float(ov["overall_ci"]),
            "overall_objects": float(ov["overall_objects"]), "kappa": float(ov["kappa"]), "dir": w}


def publish(work):
    """The small downloads of the page."""
    import shutil

    shutil.copy(work / "naip_u8.tif", DATA / "naip_plano_2020.tif")
    shutil.copy(work / "urban_cover.tif", DATA / "urban_cover.tif")
    shutil.copy(work / "urban_cover.gpkg", DATA / "urban_cover.gpkg")
    shutil.copy(work / "page_model.toml", DATA / "model.toml")
    tp = {int(r["n"]): r for r in figs.read_csv(LABELS / "urban-cover-tuning-points.csv")}
    tl = oc.read_labels(LABELS / "urban-cover-tuning.csv")
    figs.write_csv(DATA / "tuning_points.csv", [
        {"x": round(EXTENT[0] + (int(tp[n]["col"]) + 0.5) * 0.6, 1), "y": round(EXTENT[3] - (int(tp[n]["row"]) + 0.5) * 0.6, 1),
         "class": tl[n]} for n in sorted(tl)])
    (DATA / "ATTRIBUTION.txt").write_text(
        "naip_plano_2020.tif: NAIP (National Agriculture Imagery Program), Texas, acquired 14 October 2020, from the "
        "Microsoft Planetary Computer (collection naip). Bands red, green, blue, nir, 8 bit, EPSG:26914, 0.6 m, a window of "
        "600 x 600 m in Plano, Texas. Imagery by USDA Farm Production and Conservation, Business Center, Geospatial "
        "Enterprise Operations; US Government work, in the public domain.\n\n"
        "urban_cover.tif, urban_cover.gpkg: the map and polygons of the tutorial (classes 1 water, 2 tree, 3 lawn, "
        "4 street, 5 roof), made with Terraflux from that image.\n\n"
        "reference_points.csv, tuning_points.csv: points labelled by eye from the image (see the tutorial); not field data.\n",
        encoding="utf-8")


def figures(work, rows, pts):
    reference_points(work)
    run_main(work)
    out = work / "output"
    rgb = od.rgb("naip", work / "naip_u8.tif", gamma=0.85)
    stats = {}

    # ---------------------------------------------------------------- the clip
    fig = plt.figure(figsize=(5.6, 5.6), dpi=150)
    ax = fig.add_axes([0, 0, 1, 1])
    ax.imshow(rgb)
    r0, r1, c0, c1 = DETAIL
    ax.add_patch(plt.Rectangle((c0, r0), c1 - c0, r1 - r0, fill=False, ec="#ffd91a", lw=1.5))
    ax.plot([30, 30 + 100 / 0.6], [965, 965], color="white", lw=3)
    ax.text(30 + 50 / 0.6, 945, "100 m", color="white", ha="center", fontsize=9, fontweight="bold")
    ax.set_axis_off()
    mpl_png(fig, "clip.png", 800, 200)
    save(rgb, "clip_plain.png", 800, 200)

    # ---------------------------------------------------------------- three dates of the same place
    m3 = 'format = "terraflux/1"\n\n'
    w3 = {}
    for d in DATES:
        m3 += stac_node(f"n{d[:4]}", d) + "\n"
        w3[f"n{d[:4]}.raster"] = f"n{d[:4]}.tif"
    wd = work / "dates"
    wd.mkdir(exist_ok=True)
    figs.run(figs.add_writes(m3, w3), wd, "--threads", "2", "--no-journal")
    box = (230, 410, 560, 740)
    fig, axes = plt.subplots(1, 3, figsize=(9, 3.3), dpi=150)
    for ax, d in zip(axes, sorted(DATES)):
        a = od.read_stack(wd / f"n{d[:4]}.tif")
        v = np.clip(np.stack([a[0], a[1], a[2]], -1) / 255, 0, 1) ** 0.85
        ax.imshow(crop(v, box))
        ax.set_title(DATES[d], fontsize=9, loc="left")
        ax.set_axis_off()
    fig.tight_layout(pad=0.4)
    mpl_png(fig, "dates.png", 1000, 128)

    # ---------------------------------------------------------------- the bands
    a = od.read_stack(work / "naip_u8.tif")
    cir = od.false_color("naip", work / "naip_u8.tif")
    fig, axes = plt.subplots(1, 5, figsize=(11, 2.5), dpi=150)
    for ax, (t, im) in zip(axes, [("red", a[0]), ("green", a[1]), ("blue", a[2]), ("nir", a[3]), ("nir, red, green", None)]):
        if im is None:
            ax.imshow(crop(cir, DETAIL))
        else:
            ax.imshow(crop(im, DETAIL), cmap="gray", vmin=0, vmax=255)
        ax.set_title(t, fontsize=9, loc="left")
        ax.set_axis_off()
    fig.tight_layout(pad=0.4)
    mpl_png(fig, "bands.png", 1100, 128)

    # ---------------------------------------------------------------- scale of the fine objects (and the sweep)
    scales = [1.5, 2, 3, 4, 6]
    sweep = []
    for s in scales:
        r = sweep_scale(work, s)
        seg = np.nan_to_num(read(r["dir"] / "seg_id.tif"), nan=0).astype(int)
        fm = read(r["dir"] / "final.tif")
        img = od.outlines(crop(rgb, DETAIL), crop(seg, DETAIL), color=(1.0, 0.85, 0.1), scale=3)
        mp = np.kron(crop(paint(fm), DETAIL), np.ones((3, 3, 1)))
        gap = np.ones((img.shape[0], 8, 3))
        save(np.concatenate([img, gap, mp], 1), f"scale_{str(s).replace('.', '_')}.png", 760, 96)
        sweep.append({"scale": s, "fine_objects": r["fine_objects"], "final_objects": r["final_objects"],
                      "overall_objects": round(100 * r["overall_objects"], 1),
                      "overall_adjusted": round(100 * r["overall_adjusted"], 1), "ci": round(100 * r["overall_ci"], 1),
                      "kappa": round(r["kappa"], 2)})
    figs.write_csv(DATA / "scales.csv", sweep)
    stats["sweep"] = sweep
    print(sweep)

    # ---------------------------------------------------------------- detail of the fine objects and their attributes
    seg_id = np.nan_to_num(read(work / "seg_id.tif"), nan=0).astype(int)
    st = table_by_id(out / "seg_table.csv")
    stats["fine_objects"] = int(seg_id.max())
    stats["fine_cells_mean"] = float(np.mean(st["cells"]))
    stats["fine_area_median_m2"] = float(np.median(st["area"]))
    save(od.outlines(crop(rgb, DETAIL), crop(seg_id, DETAIL), scale=4), "objects.png", 760, 128)
    fig, axes = plt.subplots(2, 3, figsize=(9.0, 6.0), dpi=150)
    for ax, (col, lo, hi, t) in zip(axes.flat, [("ndvi", -0.2, 0.6, "NDVI"), ("ndwi", -0.5, 0.4, "NDWI"),
                                                ("vis", 0.1, 0.6, "Visible brightness"),
                                                ("glcm_homogeneity", 0.2, 1.0, "GLCM homogeneity"),
                                                ("mean_nir", 0.1, 0.8, "Mean near infrared"),
                                                ("sd_nir", 0.0, 0.15, "Standard deviation of the nir")]):
        ax.imshow(crop(column_map(seg_id, st, col), DETAIL), vmin=lo, vmax=hi, cmap="viridis", interpolation="nearest")
        ax.set_title(t, fontsize=8.5, loc="left")
        ax.set_axis_off()
    fig.tight_layout(pad=0.5)
    mpl_png(fig, "attributes.png", 1000, 128)

    # ---------------------------------------------------------------- first pass
    p1 = read(work / "pass1.tif")
    n1 = ["water", "land", "veg", "tree", "lawn", "pavement"]
    stats["pass1_cells"] = {n: int((p1 == k).sum()) for k, n in enumerate(n1, 1) if n in ("water", "tree", "lawn", "pavement")}
    img1 = paint(p1, n1)
    fig, axes = plt.subplots(1, 2, figsize=(8.2, 4.4), dpi=150)
    axes[0].imshow(crop(rgb, DETAIL))
    axes[1].imshow(crop(img1, DETAIL), interpolation="nearest")
    for ax, t in zip(axes, ["NAIP, the detail", "First pass: water, tree, lawn, pavement"]):
        ax.set_title(t, fontsize=8.5, loc="left")
        ax.set_axis_off()
    axes[1].legend(handles=[Patch(fc=COL[n], label=n) for n in ("water", "tree", "lawn", "pavement")], loc="upper center",
                   bbox_to_anchor=(0.5, -0.01), ncol=4, frameon=False, fontsize=8)
    fig.tight_layout(pad=0.5)
    mpl_png(fig, "pass1.png", 1000, 64)

    # ---------------------------------------------------------------- patches and pieces
    patch = np.nan_to_num(read(work / "patch_id.tif"), nan=0).astype(int)
    pcs = np.nan_to_num(read(work / "pieces_id.tif"), nan=0).astype(int)
    crowns = np.nan_to_num(read(work / "crowns_id.tif"), nan=0).astype(int)
    crowns_d = np.nan_to_num(read(work / "crowns_d_id.tif"), nan=0).astype(int)
    pav = p1 == 6
    stats["patches"] = int(patch.max())
    stats["objects_after_split"] = int(crowns.max())
    cells_patch = np.bincount(patch.ravel())
    pav_patches = np.unique(patch[pav])
    big = pav_patches[cells_patch[pav_patches] * CELL_M2 > 10000]
    stats["pavement_patches"] = int(len(pav_patches))
    stats["pavement_patch_largest_ha"] = float(cells_patch[pav_patches].max() * CELL_M2 / 10000)
    stats["pavement_patches_over_1ha"] = int(len(big))
    stats["pavement_cells_in_patches_over_1ha"] = int(sum(cells_patch[big]))
    stats["pavement_cells"] = int(pav.sum())
    stats["pavement_pieces"] = int(len(np.unique(pcs[pav])))
    sp = table(out / "split_report.csv")
    stats["split_patches"] = len(sp)
    stats["split_largest"] = max(int(r["pieces"]) for r in sp)
    DP = (40, 300, 40, 300)
    rng = np.random.default_rng(4)
    base = crop(rgb, DP) * 0.5
    ar = cells_patch[patch] * CELL_M2
    im = base.copy()
    sel = crop(pav, DP)
    ar_c = crop(ar, DP)
    im[sel & (ar_c > 10000)] = (0.9, 0.2, 0.2)
    im[sel & (ar_c <= 10000) & (ar_c > 1000)] = (1.0, 0.65, 0.1)
    im[sel & (ar_c <= 1000)] = (0.25, 0.55, 1.0)
    save(np.kron(crop(rgb, DP), np.ones((2, 2, 1))), "patches_image.png", 520, 200)
    save(np.kron(im, np.ones((2, 2, 1))), "patches.png", 520, 32)
    pal = rng.random((pcs.max() + 2, 3)) * 0.65 + 0.35
    im2 = base.copy()
    pc_c = crop(pcs, DP)
    im2[sel] = pal[pc_c[sel]]
    save(np.kron(im2, np.ones((2, 2, 1))), "pieces.png", 520, 128)

    # ---------------------------------------------------------------- canopy pieces, by distance and by the near infrared
    DC = (600, 860, 20, 280)
    tr = p1 == 4
    stats["crowns_surface"] = int(len(np.unique(crowns[tr])))
    stats["crowns_distance"] = int(len(np.unique(crowns_d[tr])))
    stats["tree_patches"] = int(len(np.unique(patch[tr])))
    outs = []
    for idm in (crowns_d, crowns):
        pal = rng.random((idm.max() + 2, 3)) * 0.65 + 0.35
        im3 = crop(rgb, DC) * 0.45
        t = crop(tr, DC)
        im3[t] = pal[crop(idm, DC)[t]]
        outs.append(np.kron(im3, np.ones((2, 2, 1))))
    save(np.kron(crop(rgb, DC), np.ones((2, 2, 1))), "crowns_image.png", 520, 200)
    save(outs[0], "crowns_distance.png", 520, 128)
    save(outs[1], "crowns_surface.png", 520, 128)

    # ---------------------------------------------------------------- skeleton and colour of the pieces
    pt = table_by_id(out / "pieces_table.csv")
    ft = table(out / "final_table.csv")
    fcls = np.array([r["class"] for r in ft])
    flat = crowns.ravel()
    uniq, idx = np.unique(flat, return_index=True)
    p1f = p1.ravel()
    k1 = {int(u): p1f[i] for u, i in zip(uniq, idx)}
    ispav = np.array([k1.get(i + 1, np.nan) == 6 for i in range(len(fcls))])
    stats["pavement_pieces_final"] = {"street": int((ispav & (fcls == "street")).sum()), "roof": int((ispav & (fcls == "roof")).sum())}
    fig, axes = plt.subplots(1, 3, figsize=(10.5, 3.7), dpi=150, gridspec_kw={"width_ratios": [1, 1, 1.15]})
    for ax, (col, lo, hi, t) in zip(axes[:2], [("skel_width", 0, 24, "Skeleton width (m)"), ("bmr", -0.08, 0.12, "Blue minus red")]):
        v = column_map(crowns, pt, col)
        v[~pav] = np.nan
        ax.imshow(crop(rgb, DP) * 0.35)
        ax.imshow(crop(v, DP), vmin=lo, vmax=hi, cmap="viridis", interpolation="nearest")
        ax.set_title(t, fontsize=8.5, loc="left")
        ax.set_axis_off()
    ax = axes[2]
    for n_ in ("roof", "street"):
        m_ = ispav & (fcls == n_) & (pt["area"] > 36)
        ax.scatter(pt["skel_width"][m_], pt["bmr"][m_], s=8, color={"roof": "#c0504d", "street": "#555555"}[n_], alpha=0.8, label=f"{n_} ({int(m_.sum())})", linewidths=0)
    ax.axvspan(11, 14, color="#cccccc", alpha=0.4, lw=0)
    ax.axhspan(0.02, 0.05, color="#cccccc", alpha=0.4, lw=0)
    ax.set_xlim(0, 35)
    ax.set_ylim(-0.12, 0.14)
    ax.set_xlabel("Skeleton width (m)", fontsize=8.5)
    ax.set_ylabel("Blue minus red", fontsize=8.5)
    ax.tick_params(labelsize=7.5)
    ax.legend(fontsize=7.5, frameon=False, loc="lower right")
    for sp_ in ("top", "right"):
        ax.spines[sp_].set_visible(False)
    fig.tight_layout(pad=0.5)
    mpl_png(fig, "skeleton.png", 1100, 128)


    # ---------------------------------------------------------------- what the 100 tuning points say (not the reference)
    tp = {int(r["n"]): r for r in figs.read_csv(LABELS / "urban-cover-tuning-points.csv")}
    tl = oc.read_labels(LABELS / "urban-cover-tuning.csv")
    crowns_id = np.nan_to_num(read(work / "crowns_id.tif"), nan=0).astype(int)
    pt_tab = table_by_id(out / "pieces_table.csv")
    rows_t, rows_p = [], []
    for n_ in ["water", "tree", "lawn", "street", "roof"]:
        ks = [n for n in tl if tl[n] == n_]
        objs = [seg_id[int(tp[n]["row"]), int(tp[n]["col"])] for n in ks]
        med = lambda col: float(np.median([st[col][o - 1] for o in objs]))  # noqa: E731
        rows_t.append({"class": n_, "points": len(ks), "ndvi": round(med("ndvi"), 2), "ndwi": round(med("ndwi"), 2),
                       "vis": round(med("vis"), 2), "homogeneity": round(med("glcm_homogeneity"), 2),
                       "nir": round(med("mean_nir"), 2)})
        if n_ in ("street", "roof"):
            pcs_ = [crowns_id[int(tp[n]["row"]), int(tp[n]["col"])] for n in ks]
            pcs_ = [o for o in pcs_ if o > 0 and k1.get(o, np.nan) == 6]
            if pcs_:
                m2 = lambda col: float(np.median([pt_tab[col][o - 1] for o in pcs_]))  # noqa: E731
                rows_p.append({"class": n_, "pieces": len(pcs_), "skel_width": round(m2("skel_width"), 1),
                               "bmr": round(m2("bmr"), 3), "area": round(m2("area"))})
    cmap_ = read(work / "urban_cover.tif")
    hit = sum(1 for n in tl if np.isfinite(cmap_[int(tp[n]["row"]), int(tp[n]["col"])])
              and CLASSES[int(cmap_[int(tp[n]["row"]), int(tp[n]["col"])]) - 1] == tl[n])
    stats["tuning_agreement"] = hit / len(tl)
    figs.write_csv(DATA / "tuning.csv", rows_t)
    figs.write_csv(DATA / "tuning_pieces.csv", rows_p)
    stats["tuning"] = rows_t
    stats["tuning_pieces"] = rows_p

    # ---------------------------------------------------------------- smoothing distances
    rows_s = []
    for d_ in (0.6, 1.2, 2.4):
        w_ = work / f"smooth_{str(d_).replace('.', '_')}"
        w_.mkdir(exist_ok=True)
        tail_ = node("map", "objects.to_map", '{ objects = "clean.objects" }', '{ what = "class" }')
        m_ = figs.add_writes(build(smooth=d_, tail=tail_), {"map.map": "map.tif", "final.map": "final.tif"})
        figs.run(m_, w_, "--threads", "2", "--no-journal")
        a_, b_ = read(w_ / "map.tif"), read(w_ / "final.tif")
        ch = int(((a_ != b_) & ~(np.isnan(a_) & np.isnan(b_))).sum())
        rows_s.append({"distance": d_, "cells": ch, "percent": round(100 * ch / np.isfinite(b_).sum(), 2)})
    figs.write_csv(DATA / "smooth.csv", rows_s)
    stats["smooth"] = rows_s

    # ---------------------------------------------------------------- the map
    cm = read(work / "urban_cover.tif")
    fm = read(work / "final.tif")
    cells = {n: int((cm == k).sum()) for k, n in enumerate(CLASSES, 1)}
    tot = sum(cells.values())
    stats["map_cells"] = cells
    stats["smoothed_changed_cells"] = int(((cm != fm) & ~(np.isnan(cm) & np.isnan(fm))).sum())
    stats["final_cells"] = {n: int((fm == k).sum()) for k, n in enumerate(CLASSES, 1)}
    stats["objects_by_class"] = {n: int((fcls == n).sum()) for n in CLASSES}
    stats["final_objects"] = len(ft)
    stats["tree_cover_pct"] = 100 * cells["tree"] / tot
    img = paint(cm)
    fig = plt.figure(figsize=(5.6, 6.0), dpi=150)
    ax = fig.add_axes([0, 0.06, 1, 0.94])
    ax.imshow(img, interpolation="nearest")
    ax.set_axis_off()
    legend(ax)
    mpl_png(fig, "map.png", 800, 64)
    save(img, "map_plain.png", 800, 24)
    rows_ar = [{"class": n, "hectares": round(c * CELL_M2 / 1e4, 2), "share": round(100 * c / tot, 1),
                "objects": stats["objects_by_class"][n]} for n, c in cells.items()]
    figs.write_csv(DATA / "map_areas.csv", rows_ar)

    # ---------------------------------------------------------------- smoothing
    save(np.kron(crop(paint(fm), DETAIL), np.ones((4, 4, 1))), "smooth_before.png", 640, 24)
    save(np.kron(crop(paint(cm), DETAIL), np.ones((4, 4, 1))), "smooth_after.png", 640, 24)

    # ---------------------------------------------------------------- polygons
    import geopandas as gpd
    from shapely.geometry import box as sbox

    g = gpd.read_file(work / "urban_cover.gpkg")
    stats["polygons"] = len(g)
    stats["polygon_columns"] = [c for c in g.columns if c != "geometry"]
    stats["polygon_area_ha"] = float(g.geometry.area.sum() / 1e4)
    with rasterio.open(work / "naip_u8.tif") as ds:
        tr_ = ds.transform
    r0, r1, c0, c1 = DETAIL
    x0, y1 = tr_ * (c0, r0)
    x1, y0 = tr_ * (c1, r1)
    gg = g[g.intersects(sbox(x0, y0, x1, y1))].copy()
    fig = plt.figure(figsize=(5.2, 5.2), dpi=150)
    ax = fig.add_axes([0, 0, 1, 1])
    ax.imshow(crop(rgb, DETAIL), extent=(x0, x1, y0, y1))
    gg["colour"] = gg["class"].map(COL).fillna("#ff00ff")
    gg.plot(ax=ax, color=gg["colour"], alpha=0.28, linewidth=0)
    gg.boundary.plot(ax=ax, color="#ffd91a", linewidth=0.7)
    ax.set_xlim(x0, x1)
    ax.set_ylim(y0, y1)
    ax.set_axis_off()
    mpl_png(fig, "polygons.png", 760, 200)
    stats["polygon_detail_n"] = len(gg)

    # ---------------------------------------------------------------- the reference
    lab = oc.read_labels(LABELS / "urban-cover-reference.csv")
    fig = plt.figure(figsize=(5.6, 5.6), dpi=150)
    ax = fig.add_axes([0, 0, 1, 1])
    ax.imshow(rgb * 0.75 + 0.25)
    for (r, c), row in zip(pts, rows):
        lbl = lab[int(row["n"])]
        ok = lbl == row["mapped"]
        ax.scatter([c], [r], s=34 if ok else 50, marker="o" if ok else "X", c=COL[lbl], edgecolors="white" if ok else "black",
                   linewidths=0.7)
    ax.set_axis_off()
    mpl_png(fig, "reference.png", 800, 200)
    from collections import Counter

    stats["reference_by_label"] = dict(Counter(lab.values()))
    stats["sample_by_mapped"] = dict(Counter(r["mapped"] for r in rows))

    # error chips: reference points the map got wrong
    wrong = [(int(r["n"]), pt_, r["mapped"], lab[int(r["n"])]) for r, pt_ in zip(rows, pts) if lab[int(r["n"])] != r["mapped"]]
    stats["wrong"] = len(wrong)
    pick = wrong[:: max(1, len(wrong) // 20)][:20]
    fig, axes = plt.subplots(4, 5, figsize=(10.5, 9.0), dpi=150)
    for ax in axes.flat:
        ax.set_axis_off()
    for ax, (n_, (r, c), mp, lb) in zip(axes.flat, pick):
        ra, rb, ca, cb = max(0, r - 28), min(1000, r + 28), max(0, c - 28), min(1000, c + 28)
        ax.imshow(rgb[ra:rb, ca:cb], extent=(ca, cb, rb, ra), interpolation="bicubic")
        ax.plot([c], [r], marker="+", color="#ffff00", ms=9, mew=1.2)
        ax.set_title(f"map: {mp}, read: {lb}", fontsize=8, color="#c0392b", pad=2)
    fig.tight_layout(pad=0.6)
    mpl_png(fig, "errors.png", 1100, 220)

    # ---------------------------------------------------------------- accuracy tables
    ov = table(out / "acc_overall.csv")[0]
    stats["overall"] = {k: float(v) for k, v in ov.items()}
    mat = {r["map"]: r for r in table(out / "acc_matrix.csv")}
    order_cls = ["water", "tree", "lawn", "street", "roof"]
    rows_m = []
    for n_ in order_cls:
        rr = {"map": n_}
        for c_ in order_cls:
            rr[c_] = int(float(mat[n_][c_]))
        rr["total"] = sum(rr[c_] for c_ in order_cls)
        rows_m.append(rr)
    figs.write_csv(DATA / "matrix.csv", rows_m)
    at = {r["class"]: r for r in table(out / "acc_table.csv")}
    rows_acc = []
    for n_ in order_cls:
        r = at[n_]
        rows_acc.append({"class": n_, "mapped_objects": int(float(r["objects"])), "reference_objects": int(float(r["reference_objects"])),
                         "users": round(float(r["user_adjusted"]), 3), "users_ci": round(float(r["user_ci"]), 3),
                         "producers": round(float(r["producer_adjusted"]), 3), "producers_ci": round(float(r["producer_ci"]), 3),
                         "map_ha": round(float(r["map_cells"]) * CELL_M2 / 1e4, 1), "area_ha": round(float(r["area_ha"]), 1),
                         "area_ha_ci": round(float(r["area_ha_ci"]), 1)})
    figs.write_csv(DATA / "accuracy.csv", rows_acc)
    stats["acc_rows"] = rows_acc
    stats["errors_rows"] = len(table(out / "acc_errors.csv"))
    ovv = table(out / "accveg_overall.csv")[0]
    stats["overall_veg"] = {k: float(v) for k, v in ovv.items()}
    stats["acc_veg_rows"] = table(out / "accveg_table.csv")
    stats["matrix_veg"] = table(out / "accveg_matrix.csv")
    publish(work)

    (work / "summary.json").write_text(json.dumps(stats, indent=1, default=str), encoding="utf-8")
    print(json.dumps(stats, indent=1, default=str)[:7000])


if __name__ == "__main__":
    main()
