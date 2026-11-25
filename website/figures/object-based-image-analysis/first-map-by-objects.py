"""Figures of "Your first map by objects in 10 minutes" (tutorials/object-based-image-analysis/first-map-by-objects.md).

The template of the page, "Classify an image by clicking samples" (its nodes as templates.rs writes them: the synthetic
scene nlm.mosaic_gibbs -> nlm.image, multiresolution at scale 2 (about what ESP suggests), features, the 59 example points, folds by
place, a random forest, the classes, the polygons), run as it is, plus the page's check (objects.sample, labelled here from the scene's
truth, objects.accuracy). The figures draw what the page walks through: the image, its objects, the samples, the classes,
the margin, the accuracy and the scale slider.
Run with: python website/figures/object-based-image-analysis/first-map-by-objects.py   (at most 2 threads: it sets them)
"""

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

for _k in ("RAYON_NUM_THREADS", "OMP_NUM_THREADS", "GDAL_NUM_THREADS", "OPENBLAS_NUM_THREADS", "MKL_NUM_THREADS",
           "TERRAFLUX_THREADS"):
    os.environ[_k] = "2"  # obia_data sets 4: this script runs with 2

figs = od.figs
SLUG = "first-map-by-objects"
OUT = figs.img_dir(f"{od.CATEGORY}/{SLUG}")
DATA = figs.data_dir(f"{od.CATEGORY}/{SLUG}")

NAMES = ["cropland", "forest", "grassland", "urban", "water"]  # alphabetical: the order objects.classify numbers them
TRUTH = {1: "water", 2: "forest", 3: "grassland", 4: "cropland", 5: "urban"}  # the scene's codes
COLORS = {"water": "#2b6cb0", "forest": "#1a6b3a", "grassland": "#9bc53d", "cropland": "#e3b341", "urban": "#c2338a"}

# the points of the template's table (x, y, class): 59 of them, twelve per class (eleven of grassland)
SAMPLE_ROWS = """[[8.5, 105.5, "water"], [84.5, 44.5, "water"], [9.5, 17.5, "water"], [68.5, 113.5, "water"], [85.5, 77.5, "water"], [4.5, 38.5, "water"], [9.5, 86.5, "water"], [82.5, 61.5, "water"], [77.5, 100.5, "water"], [22.5, 13.5, "water"], [20.5, 92.5, "water"], [66.5, 102.5, "water"], [99.5, 23.5, "forest"], [23.5, 111.5, "forest"], [10.5, 3.5, "forest"], [42.5, 79.5, "forest"], [67.5, 26.5, "forest"], [111.5, 46.5, "forest"], [38.5, 102.5, "forest"], [25.5, 75.5, "forest"], [82.5, 26.5, "forest"], [35.5, 114.5, "forest"], [39.5, 91.5, "forest"], [29.5, 96.5, "forest"], [107.5, 109.5, "grassland"], [37.5, 5.5, "grassland"], [51.5, 96.5, "grassland"], [94.5, 6.5, "grassland"], [87.5, 115.5, "grassland"], [38.5, 24.5, "grassland"], [90.5, 100.5, "grassland"], [64.5, 91.5, "grassland"], [44.5, 15.5, "grassland"], [60.5, 101.5, "grassland"], [29.5, 20.5, "grassland"], [50.5, 111.5, "cropland"], [115.5, 10.5, "cropland"], [115.5, 85.5, "cropland"], [74.5, 6.5, "cropland"], [97.5, 55.5, "cropland"], [96.5, 83.5, "cropland"], [116.5, 68.5, "cropland"], [83.5, 19.5, "cropland"], [114.5, 25.5, "cropland"], [105.5, 74.5, "cropland"], [104.5, 91.5, "cropland"], [96.5, 66.5, "cropland"], [56.5, 71.5, "urban"], [4.5, 71.5, "urban"], [51.5, 30.5, "urban"], [21.5, 35.5, "urban"], [36.5, 52.5, "urban"], [68.5, 57.5, "urban"], [70.5, 79.5, "urban"], [59.5, 44.5, "urban"], [10.5, 58.5, "urban"], [55.5, 58.5, "urban"], [24.5, 55.5, "urban"], [17.5, 68.5, "urban"]]"""

SCENE = """format = "terraflux/1"

[[nodes]]
id = "land"
op = "nlm.mosaic_gibbs"
params = { rows = 120, columns = 120, germs = 30, r = 12, patch_classes = 5, nlmr_compat = false, rescale = false, seed = 4, crs = "EPSG:32633" }

[[nodes]]
id = "image"
op = "nlm.image"
inputs = { classes = "land.map" }
params = { seed = 1 }

[[nodes]]
id = "segments"
op = "segment.multiresolution"
inputs = { image = "image.image" }
params = { scale = 2 }
"""

CLASSIFY = """
[[nodes]]
id = "features"
op = "objects.features"
inputs = { objects = "segments.objects", image = "image.image" }

[[nodes]]
id = "sample_table"
op = "table.of"

[nodes.params]
columns = ["x", "y", "class"]
rows = SAMPLE_ROWS

[[nodes]]
id = "samples"
op = "vector.from_table"
inputs = { table = "sample_table.table" }

[[nodes]]
id = "training"
op = "objects.samples"
inputs = { objects = "features.objects", samples = "samples.vector" }

[[nodes]]
id = "folds"
op = "objects.folds"
inputs = { objects = "training.objects", samples = "training.table" }
params = { method = "blocks", size = 30, k = 4, seed = 1 }

[[nodes]]
id = "model"
op = "ml.fit"
inputs = { table = "folds.table" }
params = { column = "class", columns = ["mean_blue", "mean_red", "mean_nir", "mean_swir1", "mean_swir2", "sd_blue", "sd_green", "sd_red", "sd_nir", "sd_swir1", "sd_swir2", "ndvi", "area", "compactness", "glcm_homogeneity"], fold = "fold", method = "random_forest", seed = 1 }

[[nodes]]
id = "random_model"
op = "ml.fit"
inputs = { table = "training.table" }
params = { column = "class", columns = ["mean_blue", "mean_red", "mean_nir", "mean_swir1", "mean_swir2", "sd_blue", "sd_green", "sd_red", "sd_nir", "sd_swir1", "sd_swir2", "ndvi", "area", "compactness", "glcm_homogeneity"], cv = 4, method = "random_forest", seed = 1 }

[[nodes]]
id = "classify"
op = "objects.classify"
inputs = { objects = "features.objects", model = "model.model", samples = "training.table" }

[[nodes]]
id = "polygons"
op = "objects.to_vector"
inputs = { objects = "classify.objects" }
params = { smooth = 1 }

[[nodes]]
id = "write"
op = "vector.write"
inputs = { vector = "polygons.vector" }
params = { path = "objects_classified.gpkg" }
""".replace("SAMPLE_ROWS", SAMPLE_ROWS)

CHECK = """
[[nodes]]
id = "check"
op = "objects.sample"
inputs = { objects = "classify.objects" }
params = { per_class = 50, seed = 7 }

[[nodes]]
id = "check_file"
op = "vector.write"
inputs = { vector = "check.points" }
params = { path = "check_points.gpkg" }
"""

ACCURACY = """
[[nodes]]
id = "labelled"
op = "vector.read"
params = { path = "check_points_labelled.gpkg" }

[[nodes]]
id = "accuracy"
op = "objects.accuracy"
inputs = { objects = "classify.objects", samples = "labelled.vector" }
params = { class = "reference", design = "objects" }
"""


def node(i, op, inputs, params="{ }"):
    return f'\n[[nodes]]\nid = "{i}"\nop = "{op}"\ninputs = {inputs}\nparams = {params}\n'


def read_map(path, fill=0):
    with rasterio.open(path) as ds:
        a = ds.read(1, masked=True).astype("float64")
    return np.ma.filled(a, fill)


def read_bands(path):
    with rasterio.open(path) as ds:
        return ds.read().astype("float64"), list(ds.descriptions)


def paint(cls, names=NAMES):
    """Class numbers (1..5, the alphabetical order) as colours."""
    img = np.ones(cls.shape + (3,)) * 0.9
    for k, n in enumerate(names, 1):
        img[cls == k] = matplotlib.colors.to_rgb(COLORS[n])
    return img


def save(fig, name, width=720, colors=200):
    p = OUT / name
    fig.savefig(p, dpi=150, bbox_inches="tight", pad_inches=0.04)
    plt.close(fig)
    img = np.asarray(plt.imread(p))[..., :3]
    od.save_png(img, p, width=width, colors=colors)


def legend(ax, names=NAMES, ncol=5, loc="lower center", y=-0.02):
    from matplotlib.patches import Patch

    ax.legend(handles=[Patch(fc=COLORS[n], ec="none", label=n) for n in names], loc=loc, ncol=ncol,
              bbox_to_anchor=(0.5, y), frameon=False, fontsize=8.5, handlelength=1, columnspacing=1.2)


def panel(ax, img, title):
    ax.imshow(img, interpolation="nearest")
    ax.set_axis_off()
    ax.set_title(title, fontsize=10, loc="left")


def main():
    work = figs.workdir(f"{od.CATEGORY}/{SLUG}")
    base = SCENE + CLASSIFY
    # image, truth, objects, classes, margin, probabilities
    m = base + node("ids", "objects.to_map", '{ objects = "segments.objects" }', '{ what = "id" }')
    m += node("cls", "objects.to_map", '{ objects = "classify.objects" }', '{ what = "class" }')
    m += CHECK
    m += node("auto", "segment.multiresolution", '{ image = "image.image" }')  # no scale: the one ESP suggests
    w = {"image.image": "image.tif", "image.truth": "truth.tif", "ids.map": "ids.tif", "cls.map": "cls.tif",
         "auto.scale": "scale.json", "classify.table": "classify.csv",
         "segments.table": "objects.csv", "model.importance": "importance.csv", "model.cv": "cv_place.csv", "random_model.cv": "cv_random.csv"}
    r = figs.run(figs.add_writes(m, w), work, "--threads", "2", "--no-journal")
    print(r.stdout[-600:], r.stderr[-1500:])
    out = work / "output"

    # label the check points from the scene's truth (what a reader does in QGIS), then the accuracy
    import geopandas as gpd

    truth = read_map(work / "truth.tif")
    pts = gpd.read_file(work / "check_points.gpkg")
    with rasterio.open(work / "truth.tif") as ds:
        ref = [TRUTH[int(truth[ds.index(g.x, g.y)])] for g in pts.geometry]
    pts["reference"] = ref
    pts.to_file(work / "check_points_labelled.gpkg", driver="GPKG")
    wa = {"accuracy.matrix": "matrix.csv", "accuracy.table": "classes.csv", "accuracy.overall": "overall.csv"}
    r = figs.run(figs.add_writes(m + ACCURACY, w | wa), work, "--threads", "2", "--no-journal", name="model_acc.toml")
    print(r.stdout[-400:], r.stderr[-1500:])
    for f in ("matrix", "classes", "overall"):
        print(f, open(out / f"{f}.csv").read()[:900])

    bands, names = read_bands(work / "image.tif")
    print(names, bands.shape)
    ix = {n: i for i, n in enumerate(names)}
    rgb = np.stack([bands[ix["red"]], bands[ix["green"]], bands[ix["blue"]]], axis=-1)
    rgb = np.clip(rgb / np.percentile(rgb, 99), 0, 1) ** 0.7
    fc = np.stack([bands[ix["nir"]], bands[ix["red"]], bands[ix["green"]]], axis=-1)
    fc = np.clip(fc / np.percentile(fc, 99), 0, 1) ** 0.8
    ids = read_map(work / "ids.tif", -1).astype("int64")
    cls = read_map(work / "cls.tif", 0).astype("int64")
    tru = np.vectorize(lambda v: NAMES.index(TRUTH[v]) + 1 if v in TRUTH else 0)(truth.astype("int64"))
    n_obj = len(np.unique(ids[ids > 0]))
    sc = float(json.load(open(out / "scale.json")))  # the scale segment.scale suggested
    print("objects", n_obj, "scale", sc)

    # 1. the image (true and false colour) and the truth the scene was drawn from
    fig, ax = plt.subplots(1, 3, figsize=(8.4, 3.0))
    panel(ax[0], rgb, "The image: red, green, blue")
    panel(ax[1], fc, "Near infrared, red, green")
    panel(ax[2], paint(tru), "The classes it was drawn from")
    legend(fig, ncol=5, y=-0.02)
    fig.subplots_adjust(bottom=0.1, wspace=0.05)
    save(fig, "image.png", width=900)

    # 2. the objects over the image
    img = od.outlines(rgb, ids, color=(1.0, 0.85, 0.1), scale=4, thick=1)
    od.save_png(img, OUT / "objects.png", width=720, colors=200)

    # 3. one object selected, with its neighbours, as the Viewer shows them
    import scipy.ndimage as ndi

    k0 = int(ids[60, 60])
    sel = ids == k0
    near = ndi.binary_dilation(sel, structure=np.ones((3, 3))) & ~sel
    nb = set(np.unique(ids[near]).tolist()) - {0, -1, k0}
    base_img = od.outlines(rgb, ids, color=(0.95, 0.95, 0.95), scale=4, thick=1)
    big = lambda a: np.kron(a, np.ones((4, 4), dtype=a.dtype))  # noqa: E731
    bsel, bnb = big(ids) == k0, np.isin(big(ids), list(nb))
    base_img[bnb] = base_img[bnb] * 0.55 + np.array([0.15, 0.55, 0.95]) * 0.45
    base_img[bsel] = base_img[bsel] * 0.4 + np.array([1.0, 0.55, 0.0]) * 0.6
    od.save_png(base_img, OUT / "selected.png", width=720, colors=200)

    # 4. the samples on the objects
    with rasterio.open(work / "truth.tif") as ds:
        inv = ~ds.transform
    xs = eval(SAMPLE_ROWS)
    fig, ax = plt.subplots(figsize=(5.4, 5.6))
    ax.imshow(od.outlines(rgb, ids, color=(0.95, 0.95, 0.95), scale=4, thick=1), interpolation="nearest")
    for x, y, c in xs:
        col, row = inv * (x, y)
        ax.scatter(col * 4, row * 4, s=120, c="black", zorder=3)
        ax.scatter(col * 4, row * 4, s=84, c="white", zorder=4)
        ax.scatter(col * 4, row * 4, s=44, c=COLORS[c], zorder=5)
    ax.set_axis_off()
    legend(ax, ncol=5, y=-0.06)
    save(fig, "samples.png", width=720)

    # 5. the classes of the objects, beside the truth
    fig, ax = plt.subplots(1, 2, figsize=(7.2, 3.5))
    panel(ax[0], paint(cls), "The map of the objects")
    panel(ax[1], paint(tru), "What the scene was drawn from")
    legend(fig, ncol=5, y=-0.02)
    fig.subplots_adjust(bottom=0.1, wspace=0.05)
    save(fig, "classes.png", width=820)
    right = float(((cls == tru) & (cls > 0)).sum() / (cls > 0).sum())
    print("cells right", right)

    # 6. the margin
    tab = {int(r["id"]): float(r["margin"]) for r in figs.read_csv(out / "classify.csv")}
    mg = np.full(ids.shape, np.nan)
    for k, v in tab.items():
        mg[ids == k] = v
    fig, ax = plt.subplots(figsize=(4.6, 4.0))
    im = ax.imshow(np.where(cls > 0, mg, np.nan), cmap="viridis", vmin=0, vmax=1, interpolation="nearest")
    ax.set_axis_off()
    cb = fig.colorbar(im, ax=ax, fraction=0.045, pad=0.02)
    cb.outline.set_visible(False)
    cb.set_label("margin (largest probability minus the next)", fontsize=8.5)
    save(fig, "margin.png", width=620)

    # 7. the accuracy tables (CSV for tftable)
    import shutil

    for f in ("matrix", "classes", "overall"):
        shutil.copy(out / f"{f}.csv", DATA / f"{f}.csv")

    # the honest check: the template has few objects, so objects.sample (per_class = 50) takes them all, those the forest
    # learned from included; the objects it did not learn from, against the scene's truth (the majority class of the cells)
    train = {int(ids[int((inv * (x, y))[1]), int((inv * (x, y))[0])]) for x, y, _ in xs}
    cmap = {int(r["id"]): r["class"] for r in figs.read_csv(out / "classify.csv")}
    ok = tot = 0
    for k, c in cmap.items():
        if k in train:
            continue
        t = np.bincount(tru[ids == k]).argmax()
        tot += 1
        ok += NAMES[t - 1] == c
    print("training objects", len(train), "of", len(cmap), "others right", ok, "of", tot)

    # 8. the scale: the same segmentation at half to four times the template's scale 2 (ESP suggests `sc`)
    print("suggested", sc)
    sc = 2.0
    factors = [0.5, 1, 2, 4]
    ms = SCENE.rsplit("[[nodes]]", 1)[0]  # no segments node
    wr = {}
    mm = ms
    for f in factors:
        s = round(sc * f, 4)
        i = f"s{str(f).replace('.', '_')}"
        mm += node(f"seg_{i}", "segment.multiresolution", '{ image = "image.image" }', f"{{ scale = {s} }}")
        mm += node(f"ids_{i}", "objects.to_map", f'{{ objects = "seg_{i}.objects" }}', '{ what = "id" }')
        wr[f"ids_{i}.map"] = f"ids_{i}.tif"
    figs.run(figs.add_writes(mm, wr), work, "--threads", "2", "--no-journal", name="model_scale.toml")
    rows = []
    for f in factors:
        i = f"s{str(f).replace('.', '_')}"
        lab = read_map(work / f"ids_{i}.tif", -1).astype("int64")
        n = len(np.unique(lab[lab > 0]))
        od.save_png(od.outlines(rgb, lab, scale=4, thick=1), OUT / f"scale_{f}.png", width=640, colors=160)
        rows.append({"factor": f, "scale": round(sc * f, 3), "objects": n})
        print(f, n)
    figs.write_csv(DATA / "scales.csv", rows)


if __name__ == "__main__":
    main()
