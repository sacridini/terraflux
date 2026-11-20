"""Figures of "Classifying by samples" (tutorials/object-based-image-analysis/classifying-by-samples.md).

The Sentinel-2 clip of the series (obia_data.py), the multiresolution objects at the scale 2 and their attributes,
trained with samples drawn from labelled places of the clip (obia_samples.py: a figure run cannot click), classified with
a random forest (ml.fit + objects.classify) and with the nearest neighbour (objects.nearest) and assessed with
objects.accuracy on reference objects in other places. Then four small experiments, each a run of Terraflux:
  * the number of training objects per class (slider),
  * the attributes the classifiers read,
  * spatial autocorrelation: the same classifier assessed on neighbours of its training objects and on other places,
  * the settings of the nearest neighbour (slope, min).
Run with: python website/figures/object-based-image-analysis/classifying-by-samples.py   (at most 2 threads: it sets them)
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
import obia_samples as sm  # noqa: E402
import rasterio  # noqa: E402
from matplotlib.lines import Line2D  # noqa: E402

for _k in ("RAYON_NUM_THREADS", "OMP_NUM_THREADS", "GDAL_NUM_THREADS", "OPENBLAS_NUM_THREADS", "MKL_NUM_THREADS",
           "TERRAFLUX_THREADS"):
    os.environ[_k] = "2"  # obia_data sets 4: this script runs with 2

figs = od.figs
SLUG = "classifying-by-samples"
OUT = figs.img_dir(f"{od.CATEGORY}/{SLUG}")
DATA = figs.data_dir(f"{od.CATEGORY}/{SLUG}")
WORK = None

R0, R1, C0, C1 = 90, 360, 200, 470  # the part of the clip the other figures show
# the attributes of the random forest of the page (colour, texture, one shape attribute)
FEATURES = ["mean", "sd", "ndvi", "ndwi", "ndmi", "contrast", "homogeneity", "compactness"]
RF_COLS = ["mean_blue", "mean_green", "mean_red", "mean_nir", "mean_swir16", "ndvi", "ndwi", "ndmi", "glcm_contrast",
           "glcm_homogeneity", "sd_nir", "compactness"]
BANDS = ["mean_blue", "mean_green", "mean_red", "mean_nir", "mean_swir16"]
ORDER = ["bare soil", "cerrado", "forest", "urban", "water"]  # alphabetical: the class numbers 1..5


def tl(v):
    return "[" + ", ".join(f'"{x}"' for x in v) + "]"


def node(i, op, inputs, params="{ }"):
    return f'\n[[nodes]]\nid = "{i}"\nop = "{op}"\ninputs = {inputs}\nparams = {params}\n'


def read_csv(path):
    with open(path, newline="", encoding="utf-8") as f:
        return list(csv.DictReader(f))


def num(x):
    try:
        return float(x)
    except (TypeError, ValueError):
        return float("nan")


def read_ids(path):
    with rasterio.open(path) as ds:
        a = ds.read(1, masked=True).astype("float64")
    return np.ma.filled(a, 0).astype("int64")


def read_map(path):
    with rasterio.open(path) as ds:
        a = ds.read(1, masked=True).astype("float64")
    return np.ma.filled(a, 0).astype("int64")


def head(data_stac=False):
    """The first nodes: the clip (the page's data.stac model, or its cached file for the many small runs), reflectance and the
    objects at the scale 2, with their attributes."""
    if data_stac:
        m = od.sentinel2_model()
    else:
        tif = str(od.sentinel2()).replace("\\", "/")
        m = ('format = "terraflux/1"\n\n[[nodes]]\nid = "s2"\nop = "raster.read"\nparams = { path = "%s" }\n' % tif)
        m += node("refl", "raster.calc", '{ s = "s2.raster" }', '{ expression = "out image = s / 10000" }')
    m += node("segments", "segment.multiresolution", '{ image = "refl.image" }', "{ scale = 2 }")
    m += node("feat", "objects.features", '{ objects = "segments.objects", image = "refl.image" }',
              f'{{ features = {tl(FEATURES)}, texture_bands = ["brightness"], swir = "swir16" }}')
    return m


def rf_nodes(train="train", rf_cols=None, fit_extra=""):
    cols = rf_cols or RF_COLS
    m = node("samples", "objects.samples", f'{{ objects = "feat.objects", samples = "{train}.vector" }}')
    m += node("fit", "ml.fit", '{ table = "samples.table" }',
              f'{{ column = "class", columns = {tl(cols)}, n_estimators = 300{fit_extra} }}')
    m += node("rf", "objects.classify", '{ objects = "feat.objects", model = "fit.model", samples = "samples.table" }')
    return m


def nn_node(i="nn", columns=None, extra=""):
    p = (f"columns = {tl(columns)}" if columns else "") + extra
    p = p.lstrip(", ")
    return node(i, "objects.nearest", '{ objects = "feat.objects", samples = "samples.table" }', "{ " + p + " }")


def acc_node(i, objs, ref="ref"):
    return node(i, "objects.accuracy", f'{{ objects = "{objs}.objects", samples = "{ref}.vector" }}')


def run(model, name, writes, log=False):
    work = WORK / name
    work.mkdir(parents=True, exist_ok=True)
    r = figs.run(figs.add_writes(model, writes), work, "--threads", "2", "--no-journal")
    if log:
        print(r.stdout[-700:], r.stderr[-1200:])
    return work / "output"


def overall(path):
    r = read_csv(path)[0]
    return {k: num(v) for k, v in r.items()}


def pts_of(places, ids, seed, taken):
    return sm.draw(places, ids, seed=seed, taken=taken)


def main():
    global WORK
    od.sentinel2()
    WORK = figs.workdir(f"{od.CATEGORY}/{SLUG}")
    # the ids of the objects, to place the points one per object
    m = head(True) + node("idmap", "objects.to_map", '{ objects = "segments.objects" }', '{ what = "id" }')
    out = run(m, "ids", {"idmap.map": "ids.tif"})
    ids = read_ids(WORK / "ids" / "ids.tif")
    taken = set()
    train = pts_of(sm.TRAIN, ids, 7, taken)
    ref = pts_of(sm.REFERENCE, ids, 11, taken)
    print(len(train), "training objects,", len(ref), "reference objects")
    ids_of = lambda pts: [int(ids[r, c]) - 1 for _, r, c in pts]  # noqa: E731

    # the points as the downloads of the page (x, y, class in the clip's CRS) and a check that the page's model reads them
    for name, pts in (("training_samples.csv", train), ("reference_samples.csv", ref)):
        figs.write_csv(DATA / name, [{"x": sm.xy(r, c)[0], "y": sm.xy(r, c)[1], "class": cls} for cls, r, c in pts])
    page = head(True)
    for pre, f in (("train", "training_samples.csv"), ("ref", "reference_samples.csv")):
        page += node(f"{pre}_table", "table.read", "{ }", f'{{ path = "{f}" }}')
        page += node(pre, "vector.from_table", f'{{ table = "{pre}_table.table" }}', f'{{ crs = "{sm.CRS}" }}')
    page += rf_nodes() + acc_node("acc_rf", "rf")
    wdir = WORK / "page"
    wdir.mkdir(parents=True, exist_ok=True)
    for f in ("training_samples.csv", "reference_samples.csv"):
        (wdir / f).write_text((DATA / f).read_text(encoding="utf-8"), encoding="utf-8")
    run(page, "page", {"acc_rf.overall": "overall.csv"})
    chk = overall(WORK / "page" / "output" / "overall.csv")
    print("page model from the CSVs:", chk["overall_objects"], chk["kappa"])
    assert abs(chk["overall_objects"] - 0.8) < 1e-9

    # ---------------------------------------------------------------- the model of the page (data.stac)
    m = head(True) + sm.table_nodes("train", train) + sm.table_nodes("ref", ref)
    m += rf_nodes() + nn_node("nn", None)
    m += acc_node("acc_rf", "rf") + acc_node("acc_nn", "nn")
    m += node("draw", "objects.sample", '{ objects = "rf.objects" }', "{ per_class = 20, seed = 1 }")
    m += node("idmap", "objects.to_map", '{ objects = "segments.objects" }', '{ what = "id" }')
    writes = {"idmap.map": "ids.tif", "rf.map": "rf.tif", "nn.map": "nn.tif", "rf.table": "rf_table.csv",
              "nn.table": "nn_table.csv", "samples.counts": "counts.csv", "fit.importance": "importance.csv",
              "acc_rf.table": "acc_rf.csv", "acc_rf.overall": "overall_rf.csv", "acc_rf.matrix": "matrix_rf.csv",
              "acc_rf.errors": "errors_rf.csv", "acc_nn.table": "acc_nn.csv", "acc_nn.overall": "overall_nn.csv",
              "acc_nn.matrix": "matrix_nn.csv", "draw.table": "draw.csv", "feat.table": "feat.csv"}
    o = run(m, "main", writes, log=True)
    for f in ("counts", "overall_rf", "overall_nn", "matrix_rf", "matrix_nn", "acc_rf", "draw"):
        print("==", f)
        print((o / f"{f}.csv").read_text()[:1100])
    imp = read_csv(o / "importance.csv")
    print("importance", imp[:3])
    rf_map, nn_map = read_map(o.parent / "rf.tif"), read_map(o.parent / "nn.tif")
    rgb = od.rgb("sentinel2")

    # ---------------------------------------------------------------- figure: the samples
    fig = plt.figure(figsize=(5.8, 5.8), dpi=150)
    ax = fig.add_axes([0, 0, 1, 1])
    ax.imshow(rgb)
    for pts, mk, s in ((train, "o", 30), (ref, "s", 30)):
        for cls, r, c in pts:
            ax.scatter(c, r, s=s, marker=mk, c=sm.COLORS[cls], ec="white", lw=0.8, zorder=3)
    ax.set_axis_off()
    handles = [Line2D([0], [0], marker="o", ls="", mfc="#777", mec="white", ms=7, label="training (circles)"),
               Line2D([0], [0], marker="s", ls="", mfc="#777", mec="white", ms=7, label="reference (squares)")]
    handles += [Line2D([0], [0], marker="o", ls="", mfc=sm.COLORS[c], mec="white", ms=7, label=c) for c in
                ["water", "forest", "cerrado", "urban", "bare soil"]]
    ax.legend(handles=handles, loc="lower left", fontsize=7, framealpha=0.85, frameon=True, facecolor="white",
              edgecolor="none", ncol=2, borderpad=0.5)
    fig.savefig(OUT / "samples.png", dpi=150)
    plt.close(fig)
    od.save_png(np.asarray(plt.imread(OUT / "samples.png"))[..., :3], OUT / "samples.png", width=760, colors=256)

    # ---------------------------------------------------------------- figure: the two maps
    pal = {1: sm.COLORS["bare soil"], 2: sm.COLORS["cerrado"], 3: sm.COLORS["forest"], 4: sm.COLORS["urban"],
           5: sm.COLORS["water"]}

    def paint(mp):
        img = np.ones(mp.shape + (3,)) * 0.88
        for k, col in pal.items():
            img[mp == k] = matplotlib.colors.to_rgb(col)
        return img

    lab = ids[R0:R1, C0:C1]
    fig, axs = plt.subplots(1, 3, figsize=(11, 3.9), dpi=100)
    axs[0].imshow(od.outlines(rgb[R0:R1, C0:C1], lab, color=(1, 0.85, 0.1), scale=1, thick=1))
    axs[0].set_title("True colours and objects", loc="left", fontsize=10.5)
    for ax, mp, title in ((axs[1], rf_map, "Random forest"), (axs[2], nn_map, "Nearest neighbour")):
        ax.imshow(od.outlines(paint(mp)[R0:R1, C0:C1], lab, color=(1, 1, 1), scale=1, thick=1))
        ax.set_title(title, loc="left", fontsize=10.5)
    for ax in axs:
        ax.set_axis_off()
    handles = [plt.Rectangle((0, 0), 1, 1, fc=sm.COLORS[c]) for c in ["water", "forest", "cerrado", "urban", "bare soil"]]
    handles.append(plt.Rectangle((0, 0), 1, 1, fc="#e0e0e0"))
    fig.legend(handles, ["water", "forest", "cerrado", "urban", "bare soil", "unclassified"], loc="lower center",
               ncol=6, fontsize=8.5, frameon=False)
    fig.tight_layout(rect=(0, 0.06, 1, 0.96))
    fig.savefig(OUT / "maps.png", dpi=100)
    plt.close(fig)
    od.save_png(np.asarray(plt.imread(OUT / "maps.png"))[..., :3], OUT / "maps.png", width=1100, colors=256)

    # ---------------------------------------------------------------- figure: the margin of the forest
    rt = read_csv(o / "rf_table.csv")
    margin = np.array([num(r["margin"]) for r in rt])
    fig, axs = plt.subplots(1, 2, figsize=(8.2, 3.9), dpi=100)
    axs[0].imshow(od.outlines(paint(rf_map)[R0:R1, C0:C1], lab, color=(1, 1, 1), scale=1, thick=1))
    axs[0].set_title("Random forest", loc="left", fontsize=10.5)
    t = np.clip(margin, 0, 1)
    img = np.where((lab > 0)[..., None], plt.get_cmap("viridis")(t)[..., :3][np.clip(lab, 1, None) - 1], 0.88)
    axs[1].imshow(od.outlines(img, lab, color=(1, 1, 1), scale=1, thick=1))
    axs[1].set_title("Margin of each object", loc="left", fontsize=10.5)
    sm_ = plt.cm.ScalarMappable(cmap="viridis", norm=plt.Normalize(0, 1))
    cb = fig.colorbar(sm_, ax=axs[1], fraction=0.046, pad=0.02)
    cb.set_label("largest minus second probability", fontsize=8.5)
    cb.ax.tick_params(labelsize=8)
    for ax in axs:
        ax.set_axis_off()
    fig.tight_layout()
    fig.savefig(OUT / "margin.png", dpi=100)
    plt.close(fig)
    od.save_png(np.asarray(plt.imread(OUT / "margin.png"))[..., :3], OUT / "margin.png", width=860, colors=256)
    print("margin: share of objects under 0.3:", float((margin < 0.3).mean()), "median", float(np.median(margin)))

    # ---------------------------------------------------------------- tables of the page
    def write_matrix(src, dst):
        rows = read_csv(src)
        figs.write_csv(DATA / dst, [{("Map \\ reference" if k == "map" else k): v for k, v in r.items()} for r in rows])

    write_matrix(o / "matrix_rf.csv", "matrix_rf.csv")
    write_matrix(o / "matrix_nn.csv", "matrix_nn.csv")
    for tag in ("rf", "nn"):
        rows = read_csv(o / f"acc_{tag}.csv")
        keep = []
        nz = lambda x, d=3: "" if np.isnan(num(x)) else round(num(x), d)  # noqa: E731
        for r in rows:
            keep.append({"class": r["class"], "mapped_objects": r["objects"], "reference_objects": r["reference_objects"],
                         "users": nz(r["user_adjusted"]), "users_ci": nz(r["user_ci"]),
                         "producers": nz(r["producer_adjusted"]),
                         "producers_ci": nz(r["producer_ci"]), "area_ha": nz(r["area_ha"], 1), "area_ha_ci": nz(r["area_ha_ci"], 1),
                         "map_ha": round(num(r["map_cells"]) * 0.01, 1)})
        figs.write_csv(DATA / f"accuracy_{tag}.csv", keep)
    figs.write_csv(DATA / "counts.csv", read_csv(o / "counts.csv"))
    (DATA / "draw.csv").write_text((o / "draw.csv").read_text(), encoding="utf-8")
    print("tables written")

    # ---------------------------------------------------------------- experiment 1: samples per class (slider)
    base = head(False) + sm.table_nodes("ref", ref)
    by_class = {}
    for cls, r, c in train:
        by_class.setdefault(cls, []).append((cls, r, c))
    sizes = [3, 5, 8, 12, 20]
    rows = []
    for n in sizes:
        rng = np.random.default_rng(100 + n)
        sub = []
        for cls in ORDER:
            pool = by_class[cls]
            idx = rng.permutation(len(pool))[: min(n, len(pool))]
            sub += [pool[i] for i in idx]
        mm = base + sm.table_nodes("train", sub) + rf_nodes() + acc_node("acc", "rf")
        oo = run(mm, f"n{n}", {"rf.map": "rf.tif", "acc.overall": "overall.csv"})
        ov = overall(oo / "overall.csv")
        mp = read_map(oo.parent / "rf.tif")
        img = od.outlines(paint(mp)[R0:R1, C0:C1], lab, color=(1, 1, 1), scale=2, thick=1)
        od.save_png(img, OUT / f"n_{n}.png", width=560, colors=96)
        rows.append({"per_class": n, "training_objects": len(sub), "overall": round(ov["overall_objects"], 3),
                     "kappa": round(ov["kappa"], 3)})
    figs.write_csv(DATA / "samples_per_class.csv", rows)
    print("samples per class", rows)

    # ---------------------------------------------------------------- experiment 2: the attributes
    sets = [
        ("Band means", BANDS, BANDS),
        ("Band means and the three indices", BANDS + ["ndvi", "ndwi", "ndmi"], BANDS + ["ndvi", "ndwi", "ndmi"]),
        ("+ texture (contrast, homogeneity) and sd of the NIR",
         BANDS + ["ndvi", "ndwi", "ndmi", "glcm_contrast", "glcm_homogeneity", "sd_nir"], None),
        ("+ the compactness (the forest of the page)", RF_COLS, None),
    ]
    rows = []
    for name, cols, nncols in sets:
        mm = head(False) + sm.table_nodes("train", train) + sm.table_nodes("ref", ref)
        mm += rf_nodes(rf_cols=cols)
        mm += nn_node("nn", nncols) if nncols is not None else nn_node("nn", cols)
        mm += acc_node("acc_rf", "rf") + acc_node("acc_nn", "nn")
        oo = run(mm, f"set{len(rows)}", {"acc_rf.overall": "overall_rf.csv", "acc_nn.overall": "overall_nn.csv"})
        a, b = overall(oo / "overall_rf.csv"), overall(oo / "overall_nn.csv")
        rows.append({"attributes": name, "predictors": len(cols), "rf_overall": round(a["overall_objects"], 3),
                     "rf_kappa": round(a["kappa"], 3), "nn_overall": round(b["overall_objects"], 3),
                     "nn_kappa": round(b["kappa"], 3)})
    figs.write_csv(DATA / "attribute_sets.csv", rows)
    print("attribute sets", rows)

    # ---------------------------------------------------------------- experiment 3: nearest neighbour settings
    rows = []
    for slope, mn in [(0.2, 0.1), (0.2, 0.0), (0.5, 0.1), (0.5, 0.3), (0.05, 0.1)]:
        mm = head(False) + sm.table_nodes("train", train) + sm.table_nodes("ref", ref) + rf_nodes()
        mm += nn_node("nn", BANDS + ["ndvi", "ndwi", "ndmi"], f", slope = {slope}, min = {mn}")
        mm += acc_node("acc_nn", "nn")
        oo = run(mm, f"nn_{slope}_{mn}", {"acc_nn.overall": "overall.csv", "nn.table": "nn_table.csv", "nn.map": "nn.tif"})
        b = overall(oo / "overall.csv")
        mp = read_map(oo.parent / "nn.tif")
        rows.append({"slope": slope, "min": mn, "unclassified_share": round(float((mp == 0).sum() / mp.size), 3),
                     "overall": round(b["overall_objects"], 3), "kappa": round(b["kappa"], 3)})
    figs.write_csv(DATA / "nn_settings.csv", rows)
    print("nn settings", rows)

    # ---------------------------------------------------------------- experiment 4: autocorrelated samples
    four = [p for p in sm.REFERENCE if p[0] != "bare soil"]
    ref4 = [p for p in ref if p[0] != "bare soil"]
    one_place = {}
    for cls, box, n in sm.TRAIN:
        if cls != "bare soil" and cls not in one_place:
            one_place[cls] = (cls, box, 60)
    taken2 = set(ids_of(ref))
    near_pts = pts_of(list(one_place.values()), ids, 21, taken2)
    print("one place per class:", {c: sum(1 for p in near_pts if p[0] == c) for c in one_place})
    rows = []
    for seed in range(6):
        rng = np.random.default_rng(300 + seed)
        tr, te = [], []
        for cls in one_place:
            pool = [p for p in near_pts if p[0] == cls]
            idx = rng.permutation(len(pool))
            half = len(pool) // 2
            tr += [pool[i] for i in idx[:half]]
            te += [pool[i] for i in idx[half:]]
        mm = head(False) + sm.table_nodes("train", tr) + sm.table_nodes("near", te) + sm.table_nodes("ref", ref4)
        mm += rf_nodes()
        mm += acc_node("acc_near", "rf", "near") + acc_node("acc_far", "rf", "ref")
        oo = run(mm, f"auto{seed}", {"acc_near.overall": "near.csv", "acc_far.overall": "far.csv"})
        a, b = overall(oo / "near.csv"), overall(oo / "far.csv")
        rows.append({"split": f"split {seed + 1}", "training_objects": len(tr), "neighbours": round(a["overall_objects"], 3),
                     "elsewhere": round(b["overall_objects"], 3)})
    figs.write_csv(DATA / "autocorrelation.csv", rows)
    print("autocorrelation", rows, "mean near", np.mean([r["neighbours"] for r in rows]), "mean far",
          np.mean([r["elsewhere"] for r in rows]))

    (WORK / "numbers.json").write_text(json.dumps({"train": len(train), "ref": len(ref)}), encoding="utf-8")


if __name__ == "__main__":
    main()
