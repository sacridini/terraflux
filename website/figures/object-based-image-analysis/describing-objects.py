"""Figures of "Describing objects" (tutorials/object-based-image-analysis/describing-objects.md).

One model of Terraflux, run on the Sentinel-2 clip of the series (obia_data.py): the multiresolution objects (scale 2),
objects.features with every group, a first classification (the forest of part 5, only to give the objects classes for the
context features) and objects.features again with `levels` of 4 to 128 for the texture slider. The figures paint the
attributes of the table over the map of object ids, draw the feature space of the labelled objects (obia_samples.py) and
the Jeffries-Matusita separability of the classes with the formula of the Viewer's *Feature space* window.
Run with: python website/figures/object-based-image-analysis/describing-objects.py   (at most 2 threads: it sets them)
"""

import csv
import itertools
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
import obia_samples as sm  # noqa: E402
import rasterio  # noqa: E402

for _k in ("RAYON_NUM_THREADS", "OMP_NUM_THREADS", "GDAL_NUM_THREADS", "OPENBLAS_NUM_THREADS", "MKL_NUM_THREADS",
           "TERRAFLUX_THREADS"):
    os.environ[_k] = "2"  # obia_data sets 4: this script runs with 2

figs = od.figs
SLUG = "describing-objects"
OUT = figs.img_dir(f"{od.CATEGORY}/{SLUG}")
DATA = figs.data_dir(f"{od.CATEGORY}/{SLUG}")

R0, R1, C0, C1 = 90, 360, 200, 470  # the part of the clip the figures show (as in part 2)
LEVELS = [4, 8, 16, 32, 64, 128]
CELL_HA = 0.01
# the attributes of the classes' table and of the separability table
MEANS = ["area_ha", "ndvi", "ndwi", "ndmi", "brightness", "mean_nir", "sd_nir", "glcm_contrast", "compactness",
         "shape_index"]
JM_ATTR = ["ndvi", "ndwi", "ndmi", "brightness", "mean_nir", "mean_swir16", "sd_nir", "glcm_contrast",
           "glcm_homogeneity", "compactness", "area", "nb_diff_nir"]
JM_PAIRS = [("urban", "forest"), ("urban", "cerrado"), ("forest", "cerrado"), ("bare soil", "cerrado"),
            ("forest", "water")]
RF_COLS = ["mean_blue", "mean_green", "mean_red", "mean_nir", "mean_swir16", "ndvi", "ndwi", "ndmi", "glcm_contrast",
           "glcm_homogeneity", "sd_nir", "compactness"]


def node(i, op, inputs, params):
    return f'\n[[nodes]]\nid = "{i}"\nop = "{op}"\ninputs = {inputs}\nparams = {params}\n'


def toml_list(v):
    return "[" + ", ".join(f'"{x}"' for x in v) + "]"


def read_table(path):
    with open(path, newline="", encoding="utf-8") as f:
        rows = list(csv.DictReader(f))
    out = {}
    for k in rows[0]:
        try:
            out[k] = np.array([float(r[k]) if r[k] != "" else np.nan for r in rows])
        except ValueError:
            out[k] = np.array([r[k] for r in rows])
    return out


def read_ids(path):
    with rasterio.open(path) as ds:
        a = ds.read(1, masked=True).astype("float64")
    return np.ma.filled(a, 0).astype("int64")


def jm(m1, v1, m2, v2):
    """The Jeffries-Matusita distance of two classes in one attribute (the formula of the Viewer's window)."""
    v1, v2 = max(v1, 1e-12), max(v2, 1e-12)
    b = (m1 - m2) ** 2 / (4 * (v1 + v2)) + 0.5 * np.log((v1 + v2) / (2 * np.sqrt(v1 * v2)))
    return float(2 * (1 - np.exp(-max(b, 0))))


def pair_jm(vals, a, b):
    x, y = vals[a], vals[b]
    if len(x) < 2 or len(y) < 2 or np.isnan(x).any() or np.isnan(y).any():
        return np.nan
    return jm(x.mean(), x.var(ddof=1), y.mean(), y.var(ddof=1))


def save_fig(fig, name, colors=256, width=None):
    p = OUT / name
    fig.savefig(p, dpi=fig.dpi)
    plt.close(fig)
    img = np.asarray(plt.imread(p))[..., :3]
    od.save_png(img, p, width=width, colors=colors)


def painted(ids, values, cmap, vmin, vmax, log=False):
    """RGB (rows, cols, 3) of the objects painted by a value of their table (grey where no value)."""
    v = np.asarray(values, float)
    if log:
        v = np.log10(v)
    t = np.clip((v - vmin) / (vmax - vmin), 0, 1)
    rgb = plt.get_cmap(cmap)(t)[..., :3]
    rgb[np.isnan(v)] = (0.85, 0.85, 0.85)
    lab = ids[R0:R1, C0:C1]
    img = np.where((lab > 0)[..., None], rgb[np.clip(lab, 1, None) - 1], 0.85)
    return od.outlines(img, lab, color=(1, 1, 1), scale=2, thick=1)


def panels(name, items, ids, width=1100, height=3.9):
    """items: [(title, values, cmap, vmin, vmax, label, log)] drawn side by side with a colour bar each."""
    n = len(items)
    fig, axs = plt.subplots(1, n, figsize=(width / 100, height), dpi=100)
    for ax, (title, vals, cmap, lo, hi, label, log) in zip(np.atleast_1d(axs), items):
        img = painted(ids, vals, cmap, lo, hi, log)
        ax.imshow(img)
        ax.set_axis_off()
        ax.set_title(title, loc="left", fontsize=10.5)
        sm_ = plt.cm.ScalarMappable(cmap=cmap, norm=plt.Normalize(lo, hi))
        cb = fig.colorbar(sm_, ax=ax, fraction=0.046, pad=0.02)
        cb.set_label(label, fontsize=8.5)
        cb.ax.tick_params(labelsize=8)
        if log:
            cb.set_ticks([lo, (lo + hi) / 2, hi])
            cb.set_ticklabels([f"{10 ** t:.3g}" for t in (lo, (lo + hi) / 2, hi)])
    fig.tight_layout()
    save_fig(fig, name, width=width)


def main():
    od.sentinel2()
    work = figs.workdir(f"{od.CATEGORY}/{SLUG}")
    # 1. segmentation to learn the ids and draw the samples
    m = od.sentinel2_model()
    m += node("seg", "segment.multiresolution", '{ image = "refl.image" }', "{ scale = 2 }")
    m += node("idmap", "objects.to_map", '{ objects = "seg.objects" }', '{ what = "id" }')
    figs.run(figs.add_writes(m, {"idmap.map": "ids.tif"}), work, "--threads", "2", "--no-journal")
    ids = read_ids(work / "ids.tif")
    taken = set()
    train = sm.draw(sm.TRAIN, ids, seed=7, taken=taken)
    print(len(train), "training objects", len(set(taken)))

    # 2. the model of the page, with a first classification (for the context features) and the texture levels
    m = od.sentinel2_model()
    m += node("seg", "segment.multiresolution", '{ image = "refl.image" }', "{ scale = 2 }")
    m += node("idmap", "objects.to_map", '{ objects = "seg.objects" }', '{ what = "id" }')
    m += node("feat", "objects.features", '{ objects = "seg.objects", image = "refl.image" }',
              '{ features = "all", swir = "swir16" }')
    m += sm.table_nodes("train", train)
    m += node("samples", "objects.samples", '{ objects = "feat.objects", samples = "train.vector" }', "{ }")
    m += node("fit", "ml.fit", '{ table = "samples.table" }',
              f'{{ column = "class", columns = {toml_list(RF_COLS)}, n_estimators = 300 }}')
    m += node("rf", "objects.classify", '{ objects = "feat.objects", model = "fit.model", samples = "samples.table" }',
              "{ }")
    m += node("ctx", "objects.features", '{ objects = "rf.objects", image = "refl.image" }',
              '{ features = ["context"], swir = "swir16" }')
    writes = {"feat.table": "feat.csv", "ctx.table": "ctx.csv"}
    for lv in LEVELS:
        m += node(f"lv{lv}", "objects.features", '{ objects = "seg.objects", image = "refl.image" }',
                  f'{{ features = ["contrast", "homogeneity"], levels = {lv}, texture_bands = ["brightness"] }}')
        writes[f"lv{lv}.table"] = f"lv{lv}.csv"
    r = figs.run(figs.add_writes(m, writes), work, "--threads", "2", "--no-journal")
    print(r.stdout[-300:], r.stderr[-600:])
    t = read_table(work / "output" / "feat.csv")
    ctx = read_table(work / "output" / "ctx.csv")
    n = len(t["cells"])
    t["area_ha"] = t["area"] / 10000
    print(n, "objects;", "columns:", len(t), "; context columns:", [k for k in ctx if k.startswith(("rel_", "dist_", "nb_"))])

    # 3. the maps
    panels("spectral.png", [
        ("NDVI of each object", t["ndvi"], "viridis", -0.1, 0.7, "NDVI", False),
        ("NDWI of each object", t["ndwi"], "viridis", -0.45, 0.55, "NDWI", False),
        ("Brightness of each object", t["brightness"], "viridis", 0, 0.2, "mean of the band means (reflectance)",
         False),
    ], ids)
    lg = np.log10(np.clip(t["area_ha"], 0.01, None))
    panels("shape.png", [
        ("Area (ha, log scale)", t["area_ha"], "viridis", np.log10(0.1), np.log10(8), "ha", True),
        ("Compactness", t["compactness"], "viridis", 1.0, 2.0, "length x width / area (1 = a rectangle)", False),
        ("Shape index", t["shape_index"], "viridis", 1.0, 1.8, "perimeter / (4 sqrt(area))", False),
    ], ids)
    panels("texture.png", [
        ("GLCM homogeneity", t["glcm_homogeneity"], "viridis", 0.2, 1.0, "brightness", False),
        ("GLCM contrast", t["glcm_contrast"], "viridis", 0, 10, "brightness", False),
        ("GLCM entropy", t["glcm_entropy"], "viridis", 1.0, 4.5, "brightness", False),
    ], ids)
    panels("context.png", [
        ("Difference to the neighbours (NIR)", t["nb_diff_nir"], "coolwarm", -0.08, 0.08, "own mean minus the neighbours' (NIR)", False),
        ("Distance to the nearest water object (m)", ctx["dist_water"], "viridis", 0, 3000, "m", False),
        ("Share of the border with urban objects", ctx["rel_border_urban"], "viridis", 0, 1, "of the perimeter", False),
    ], ids)
    print("ranges:", {k: (float(np.nanmin(t[k])), float(np.nanpercentile(t[k], 50)), float(np.nanmax(t[k]))) for k in
                      ("glcm_contrast", "glcm_homogeneity", "glcm_entropy", "compactness", "shape_index", "nb_diff_nir",
                       "area_ha", "brightness")})
    print("dist_water", float(np.nanmax(ctx["dist_water"])), "rel_border_water", float(np.nanmax(ctx["rel_border_water"])))

    # 4. the labelled objects: their rows of the table
    cls = {}
    for c, r_, k in train:
        cls.setdefault(c, []).append(int(ids[r_, k]) - 1)
    order = ["water", "forest", "cerrado", "urban", "bare soil"]

    # class means
    rows = []
    for c in order:
        row = {"class": c, "objects": len(cls[c])}
        for a in MEANS:
            row[a] = round(float(np.nanmean(t[a][cls[c]])), 3)
        rows.append(row)
    figs.write_csv(DATA / "class_means.csv", rows)
    print(rows)

    # separability: JM by pair for the attributes
    jrows, best = [], {}
    for a in JM_ATTR:
        vals = {c: t[a][cls[c]] for c in order}
        row = {"attribute": a}
        for p, q in JM_PAIRS:
            row[f"{p} / {q}"] = round(pair_jm(vals, p, q), 2)
        allp = [pair_jm(vals, p, q) for p, q in itertools.combinations(order, 2)]
        row["least of all pairs"] = round(float(np.nanmin(allp)), 2)
        jrows.append(row)
    figs.write_csv(DATA / "separability.csv", jrows)
    for r_ in jrows:
        print(r_)

    # feature space: two scatters and the histograms
    def scatter(ax, x, y, xl, yl):
        ax.scatter(t[x], t[y], s=6, c="#c9cdca", lw=0, alpha=0.7, label="other objects")
        for c in order:
            ax.scatter(t[x][cls[c]], t[y][cls[c]], s=26, c=sm.COLORS[c], ec="white", lw=0.6, label=c)
        ax.set_xlabel(xl)
        ax.set_ylabel(yl)

    fig, axs = plt.subplots(1, 2, figsize=(11, 4.4), dpi=100)
    scatter(axs[0], "mean_red", "mean_nir", "Mean red (reflectance)", "Mean near infrared (reflectance)")
    axs[0].set_xlim(0, 0.25)
    axs[0].set_ylim(0, 0.35)
    scatter(axs[1], "ndvi", "glcm_contrast", "NDVI", "GLCM contrast of the brightness")
    axs[1].set_ylim(0, 12)
    axs[1].set_xlim(-0.5, 1.0)
    axs[1].legend(loc="upper right", fontsize=8, ncol=1, markerscale=1.1, bbox_to_anchor=(1.0, 1.0))
    fig.tight_layout()
    save_fig(fig, "space.png", width=1100)

    fig, axs = plt.subplots(1, 3, figsize=(11, 3.4), dpi=100)
    for ax, (a, label, rng) in zip(axs, [("ndvi", "NDVI", (0.0, 0.8)), ("brightness", "Brightness", (0.05, 0.25)),
                                          ("glcm_contrast", "GLCM contrast", (0, 12))]):
        bins = np.linspace(*rng, 17)
        for c in order[1:]:
            ax.hist(np.clip(t[a][cls[c]], *rng), bins=bins, histtype="stepfilled", alpha=0.45, color=sm.COLORS[c],
                    label=c, density=True)
        ax.set_xlabel(label)
        ax.set_yticks([])
    axs[0].legend(fontsize=8)
    fig.tight_layout()
    save_fig(fig, "hist.png", width=1100)

    # the texture levels slider
    srows = []
    for lv in LEVELS:
        tl = read_table(work / "output" / f"lv{lv}.csv")
        ids_ = {c: tl["glcm_contrast"][cls[c]] for c in order}
        j_uc = pair_jm(ids_, "urban", "cerrado")
        j_uf = pair_jm(ids_, "urban", "forest")
        srows.append({"levels": lv, "urban_cerrado": round(j_uc, 2), "urban_forest": round(j_uf, 2),
                      "median_contrast": round(float(np.nanmedian(tl["glcm_contrast"])), 2)})
        img = painted(ids, tl["glcm_contrast"], "viridis", 0, float(np.nanpercentile(tl["glcm_contrast"], 92)))
        od.save_png(img, OUT / f"levels_{lv}.png", width=560, colors=200)
    figs.write_csv(DATA / "levels.csv", srows)
    print(srows)

    (work / "numbers.json").write_text(json.dumps({"objects": n}), encoding="utf-8")


if __name__ == "__main__":
    main()
