"""Figures of "Classes without samples" (tutorials/object-based-image-analysis/classes-without-samples.md).

The Sentinel-2 clip of the series (obia_data.py), the multiresolution objects at the scale 2 and their attributes (the
twelve of part 5), grouped without a single sample by ml.kmeans and by ml.som, and classed by objects.classify; the groups
named by objects.reclassify and compared with part 5's random forest (obia_samples.py is read, not changed: the same
training and reference objects) and with the reference objects of part 5. Then the experiments of the page, each a run
of Terraflux:
  * k = auto (the choice table: inertia, silhouette, Calinski-Harabasz) and a slider over k = 2 to 8,
  * the SOM with Vesanto's grid, and a slider over the grid size,
  * what the `columns` setting is for (id, cells and area as variables),
  * how steady the groups are over seeds.
Run with: python website/figures/object-based-image-analysis/classes-without-samples.py   (at most 2 threads: it sets them)
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
import obia_samples as sm  # noqa: E402
import rasterio  # noqa: E402

for _k in ("RAYON_NUM_THREADS", "OMP_NUM_THREADS", "GDAL_NUM_THREADS", "OPENBLAS_NUM_THREADS", "MKL_NUM_THREADS",
           "TERRAFLUX_THREADS"):
    os.environ[_k] = "2"  # obia_data sets 4: this script runs with 2

figs = od.figs
SLUG = "classes-without-samples"
OUT = figs.img_dir(f"{od.CATEGORY}/{SLUG}")
DATA = figs.data_dir(f"{od.CATEGORY}/{SLUG}")
WORK = None

R0, R1, C0, C1 = 90, 360, 200, 470  # the part of the clip the other figures show
FEATURES = ["mean", "sd", "ndvi", "ndwi", "ndmi", "contrast", "homogeneity", "compactness"]
# the variables of the page: colour, texture and compactness, the twelve of part 5 (no id, cells or area)
COLS = ["mean_blue", "mean_green", "mean_red", "mean_nir", "mean_swir16", "ndvi", "ndwi", "ndmi", "glcm_contrast",
        "glcm_homogeneity", "sd_nir", "compactness"]
# what the clusters of k = 5 are, read from their centres (the page explains how)
# (clusters 4 and 5, the dull and the bright built-up objects, get one name: a name may join clusters)
NAMES = {"cluster 1": "water", "cluster 2": "cerrado", "cluster 3": "forest", "cluster 4": "urban",
         "cluster 5": "urban"}
# and the groups of the 8 x 8 map
SOM_NAMES = {"group 1": "water", "group 2": "cerrado", "group 3": "forest", "group 4": "urban", "group 5": "urban"}
ORDER = ["bare soil", "cerrado", "forest", "urban", "water"]
CL = figs.CLASSES  # colours of clusters and groups


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


def read_map(path):
    with rasterio.open(path) as ds:
        a = ds.read(1, masked=True).astype("float64")
    return np.ma.filled(a, 0).astype("int64")


def head(data_stac=False):
    """The clip (the page's data.stac model, or its cached file for the many small runs), reflectance and the
    objects at the scale 2 with their attributes."""
    if data_stac:
        m = od.sentinel2_model()
    else:
        tif = str(od.sentinel2()).replace("\\", "/")
        m = 'format = "terraflux/1"\n\n[[nodes]]\nid = "s2"\nop = "raster.read"\nparams = { path = "%s" }\n' % tif
        m += node("refl", "raster.calc", '{ s = "s2.raster" }', '{ expression = "out image = s / 10000" }')
    m += node("segments", "segment.multiresolution", '{ image = "refl.image" }', "{ scale = 2 }")
    m += node("feat", "objects.features", '{ objects = "segments.objects", image = "refl.image" }',
              f'{{ features = {tl(FEATURES)}, texture_bands = ["brightness"], swir = "swir16" }}')
    return m


def km_nodes(k, extra="", cols=COLS, i="km", cl="cl"):
    kk = '"auto"' if k == "auto" else (str(k) if isinstance(k, int) else str(list(k)))
    p = f"k = {kk}" + (f", columns = {tl(cols)}" if cols else "") + extra
    m = node(i, "ml.kmeans", '{ table = "feat.table" }', "{ " + p + " }")
    return m + node(cl, "objects.classify", f'{{ objects = "feat.objects", model = "{i}.model" }}')


def som_nodes(clusters=5, extra="", cols=COLS, i="som", cl="clsom"):
    p = f"clusters = {clusters}" + (f", columns = {tl(cols)}" if cols else "") + extra
    m = node(i, "ml.som", '{ table = "feat.table" }', "{ " + p + " }")
    return m + node(cl, "objects.classify", f'{{ objects = "feat.objects", model = "{i}.model" }}')


def toml_map(d):
    return "{ " + ", ".join(f'"{a}" = "{b}"' for a, b in d.items()) + " }"


def named(src, names, i):
    return node(i, "objects.reclassify", f'{{ objects = "{src}.objects" }}',
                f"{{ classes = {toml_map(names)}, colours = {toml_map({c: sm.COLORS[c] for c in names.values()})} }}")


def run(model, name, writes, log=False):
    work = WORK / name
    work.mkdir(parents=True, exist_ok=True)
    r = figs.run(figs.add_writes(model, writes), work, "--threads", "2", "--no-journal")
    if log:
        print(r.stdout[-700:], r.stderr[-1500:])
    return work / "output"


def overall(path):
    return {k: num(v) for k, v in read_csv(path)[0].items()}


def paint_clusters(mp, n=None):
    img = np.ones(mp.shape + (3,)) * 0.88
    for k in range(1, (n or int(mp.max())) + 1):
        img[mp == k] = matplotlib.colors.to_rgb(CL[(k - 1) % len(CL)])
    return img


def paint_named(mp):
    """A map of the cluster/group numbers painted by the names of NAMES (a class has one colour)."""
    img = np.ones(mp.shape + (3,)) * 0.88
    for k, c in NAMES.items():
        img[mp == int(k.split()[1])] = matplotlib.colors.to_rgb(sm.COLORS[c])
    return img


def name_map(mp, names):
    """The map of cluster numbers as class numbers 1..5 of ORDER (the alphabetical order of the names)."""
    out = np.zeros_like(mp)
    for k, c in names.items():
        out[mp == int(k.split()[1])] = ORDER.index(c) + 1
    return out


def paint_classes(mp):
    img = np.ones(mp.shape + (3,)) * 0.88
    for i, c in enumerate(ORDER, 1):
        img[mp == i] = matplotlib.colors.to_rgb(sm.COLORS[c])
    return img


def save(fig, name, width, colors=256):
    fig.savefig(OUT / name, dpi=fig.dpi)
    plt.close(fig)
    od.save_png(np.asarray(plt.imread(OUT / name))[..., :3], OUT / name, width=width, colors=colors)


def legend_boxes(fig, items, ncol, fs=8.5):
    fig.legend([plt.Rectangle((0, 0), 1, 1, fc=c) for _, c in items], [n for n, _ in items], loc="lower center",
               ncol=ncol, fontsize=fs, frameon=False)


def purity(matrix_csv, prefix):
    """From the error matrix of objects.accuracy of unnamed groups against the reference: the share of the reference
    objects that are in the group where their own class is most frequent (the groups' purity), and the number of
    reference classes that have a group (rows `prefix n`) of their own (no other class in it)."""
    rows = [r for r in read_csv(matrix_csv) if r["map"].startswith(prefix)]
    cols = [c for c in rows[0] if c != "map" and not c.startswith(prefix)]
    tot = sum(num(r[c]) for r in rows for c in cols)
    pure = sum(max(num(r[c]) for c in cols) for r in rows) / tot
    alone = {max(cols, key=lambda c: num(r[c])) for r in rows if sum(num(r[c]) > 0 for c in cols) == 1}
    return round(pure, 3), sorted(alone)


def agreement(a, b):
    """The share of the cells where the two maps of groups say the same, after matching each group of `a` to the
    group of `b` it overlaps most (the groups are numbered by the same rule, so the numbers themselves are compared too)."""
    ok = (a > 0) & (b > 0)
    return float((a[ok] == b[ok]).mean())


def main():
    global WORK
    od.sentinel2()
    WORK = figs.workdir(f"{od.CATEGORY}/{SLUG}")
    rgb = od.rgb("sentinel2")

    # the ids of the objects, to place the samples of part 5 (the same points: same boxes and seeds)
    m = head(False) + node("idmap", "objects.to_map", '{ objects = "segments.objects" }', '{ what = "id" }')
    run(m, "ids", {"idmap.map": "ids.tif"})
    ids = read_map(WORK / "ids" / "ids.tif")
    taken = set()
    train = sm.draw(sm.TRAIN, ids, seed=7, taken=taken)
    ref = sm.draw(sm.REFERENCE, ids, seed=11, taken=taken)
    print(len(train), "training objects (for the forest of the comparison),", len(ref), "reference objects")
    figs.write_csv(DATA / "reference_samples.csv",
                   [{"x": sm.xy(r, c)[0], "y": sm.xy(r, c)[1], "class": cls} for cls, r, c in ref])
    figs.write_csv(DATA / "training_samples.csv",
                   [{"x": sm.xy(r, c)[0], "y": sm.xy(r, c)[1], "class": cls} for cls, r, c in train])
    lab = ids[R0:R1, C0:C1]

    def outl(img, white=True):
        return od.outlines(img[R0:R1, C0:C1], lab, color=(1, 1, 1) if white else (1, 0.85, 0.1), scale=1, thick=1)

    # ------------------------------------------------------------ the page's model: k-means, SOM, names, accuracy
    ref_nodes = sm.table_nodes("ref", ref)
    train_nodes = sm.table_nodes("train", train)
    rf = node("samples", "objects.samples", '{ objects = "feat.objects", samples = "train.vector" }')
    rf += node("fit", "ml.fit", '{ table = "samples.table" }',
               f'{{ column = "class", columns = {tl(COLS)}, n_estimators = 300 }}')
    rf += node("rf", "objects.classify", '{ objects = "feat.objects", model = "fit.model", samples = "samples.table" }')
    m = head(True) + km_nodes(5) + named("cl", NAMES, "named")
    m += som_nodes(5, ", rows = 8, cols = 8") + named("clsom", SOM_NAMES, "named_som")
    m += train_nodes + ref_nodes + rf
    for a in ("named", "named_som", "rf"):
        m += node("acc_" + a, "objects.accuracy", f'{{ objects = "{a}.objects", samples = "ref.vector" }}')
    m += node("named_map", "objects.to_map", '{ objects = "named.objects" }', '{ what = "class" }')
    writes = {"cl.map": "km5.tif", "clsom.map": "som8.tif", "rf.map": "rf.tif", "named_map.map": "named_class.tif",
              "km.centres": "centres.csv", "km.choice": "choice1.csv", "som.codebook": "codebook8.csv",
              "som.quality": "quality8.csv", "cl.table": "cl_table.csv", "rf.table": "rf_table.csv",
              "acc_named.overall": "ov_named.csv", "acc_named.matrix": "mat_named.csv",
              "acc_named_som.overall": "ov_som.csv", "acc_named_som.matrix": "mat_som.csv",
              "acc_rf.overall": "ov_rf.csv", "acc_rf.matrix": "mat_rf.csv", "acc_named.table": "acc_named.csv"}
    o = run(m, "main", writes, log=True)
    km5 = read_map(o.parent / "km5.tif")
    som8 = read_map(o.parent / "som8.tif")
    rfm = read_map(o.parent / "rf.tif")
    # the names of reclassify's class numbers: the same partition as the names applied here
    nm = read_map(o.parent / "named_class.tif")
    mine = name_map(km5, NAMES)
    ok = (km5 > 0)
    # the classes may be numbered differently by reclassify: compare as partitions
    pairs = {(int(a), int(b)) for a, b in zip(mine[ok].ravel(), nm[ok].ravel())}
    print("named partition check (mine, reclassify):", sorted(pairs))
    assert len(pairs) == len({a for a, _ in pairs}) == len({b for _, b in pairs})
    for f in ("ov_named", "ov_som", "ov_rf"):
        print(f, (o / f"{f}.csv").read_text()[:400])

    # ---- centres table
    cen = read_csv(o / "centres.csv")
    keep = ["cluster", "size", "mean_red", "mean_nir", "ndvi", "ndwi", "glcm_contrast", "glcm_homogeneity"]
    rows = []
    for r in cen:
        d = {k: (round(num(r[k]), 3) if k not in ("cluster", "size") else int(num(r[k]))) for k in keep}
        d["named"] = NAMES["cluster " + r["cluster"]]
        rows.append(d)
    figs.write_csv(DATA / "centres.csv", rows)
    print(rows)

    # ---- accuracy table
    def acc_row(name, path, vars_):
        r = overall(path)
        return {"map": name, "variables": vars_, "overall": round(r["overall_objects"], 3),
                "adjusted": round(r["overall_adjusted"], 3) if "overall_adjusted" in r else float("nan"),
                "kappa": round(r["kappa"], 3)}

    print(read_csv(o / "ov_rf.csv"))
    rows = [acc_row("Random forest (part 5, 93 samples)", o / "ov_rf.csv", "12 attributes"),
            acc_row("k-means, k = 5, named", o / "ov_named.csv", "12 attributes, no sample"),
            acc_row("SOM 8 x 8, 5 groups, named", o / "ov_som.csv", "12 attributes, no sample")]
    figs.write_csv(DATA / "accuracy.csv", rows)
    print(rows)
    for name in ("mat_named", "mat_som"):
        rr = read_csv(o / f"{name}.csv")
        print(name)
        for r in rr:
            print(" ", {k: v for k, v in r.items() if k == "map" or num(v) > 0})
        figs.write_csv(DATA / f"{name}.csv", [{("Map \\ reference" if k == "map" else k): v for k, v in r.items()}
                                              for r in rr])

    # ---- the named k-means against the random forest, by area (cells of 0.01 ha)
    rf_names = ORDER
    tab = []
    for k, c in NAMES.items():
        sel = km5 == int(k.split()[1])
        row = {"cluster": f"{k} ({c})"}
        for i, cn in enumerate(rf_names, 1):
            row[cn] = round(float((sel & (rfm == i)).sum()) * 0.01)
        row["unclassified by the forest"] = round(float((sel & (rfm == 0)).sum()) * 0.01)
        tab.append(row)
    figs.write_csv(DATA / "vs_forest.csv", tab)
    same = float(((name_map(km5, NAMES) == rfm) & (rfm > 0)).sum()) / float((rfm > 0).sum())
    print("share of the cells with the same class as the forest:", same)
    print(tab)

    # ---- figure: the maps to compare
    fig, axs = plt.subplots(2, 2, figsize=(8.4, 7.9), dpi=100)
    axs[0, 0].imshow(outl(rgb, white=False))
    axs[0, 0].set_title("True colours and objects", loc="left", fontsize=10.5)
    panels = [(axs[0, 1], paint_classes(rfm), "Random forest, 93 samples (part 5)"),
              (axs[1, 0], paint_classes(name_map(km5, NAMES)), "k-means, k = 5, named"),
              (axs[1, 1], paint_classes(name_map(som8, SOM_NAMES)), "SOM 8 x 8, 5 groups, named")]
    for ax, img, t in panels:
        ax.imshow(outl(img))
        ax.set_title(t, loc="left", fontsize=10.5)
    for ax in axs.ravel():
        ax.set_axis_off()
    legend_boxes(fig, [(c, sm.COLORS[c]) for c in ["water", "forest", "cerrado", "urban", "bare soil"]], 5)
    fig.tight_layout(rect=(0, 0.05, 1, 1), h_pad=1.2)
    save(fig, "compare.png", 960)

    # ---- figure: the clusters of k = 5 as the groups are numbered, beside the true colours
    fig, axs = plt.subplots(1, 2, figsize=(8.2, 4.1), dpi=100)
    axs[0].imshow(outl(rgb, white=False))
    axs[0].set_title("True colours and objects", loc="left", fontsize=10.5)
    axs[1].imshow(outl(paint_clusters(km5, 5)))
    axs[1].set_title("The five clusters of k-means (no samples)", loc="left", fontsize=10.5)
    for ax in axs:
        ax.set_axis_off()
    legend_boxes(fig, [(f"cluster {i}", CL[i - 1]) for i in range(1, 6)], 5)
    fig.tight_layout(rect=(0, 0.07, 1, 1))
    save(fig, "clusters5.png", 900)

    # ---------------------------------------------------------------------------------- k = auto and the choice of k
    m = head(False) + km_nodes("auto")
    o = run(m, "auto", {"km.choice": "choice.csv", "cl.map": "auto.tif", "km.centres": "centres.csv"}, log=True)
    ch = read_csv(o / "choice.csv")
    figs.write_csv(DATA / "choice.csv", [{"k": int(r["k"]), "inertia": round(num(r["inertia"])),
                                          "silhouette": round(num(r["silhouette"]), 3),
                                          "calinski_harabasz": round(num(r["calinski_harabasz"]))} for r in ch])
    print("choice", [(r["k"], round(num(r["silhouette"]), 3)) for r in ch])
    auto = read_map(o.parent / "auto.tif")
    print("auto chose", int(auto.max()))
    fig, axs = plt.subplots(1, 2, figsize=(8.2, 4.1), dpi=100)
    axs[0].imshow(outl(rgb, white=False))
    axs[0].set_title("True colours and objects", loc="left", fontsize=10.5)
    axs[1].imshow(outl(paint_clusters(auto)))
    axs[1].set_title(f"k = auto chose k = {int(auto.max())}", loc="left", fontsize=10.5)
    for ax in axs:
        ax.set_axis_off()
    legend_boxes(fig, [(f"cluster {i}", CL[i - 1]) for i in range(1, int(auto.max()) + 1)], 5)
    fig.tight_layout(rect=(0, 0.07, 1, 1))
    save(fig, "auto.png", 900)

    # the slider over k
    def acc(src):
        return node("acc", "objects.accuracy", f'{{ objects = "{src}.objects", samples = "ref.vector" }}')

    frames = []
    for k in range(2, 9):
        o = run(head(False) + km_nodes(k) + ref_nodes + acc("cl"), f"k{k}",
                {"cl.map": "cl.tif", "km.choice": "choice.csv", "acc.matrix": "mat.csv"})
        pu = purity(o / "mat.csv", "cluster")
        mp = read_map(o.parent / "cl.tif")
        od.save_png(od.outlines(paint_clusters(mp, k)[R0:R1, C0:C1], lab, color=(1, 1, 1), scale=2, thick=1),
                    OUT / f"k_{k}.png", width=560, colors=64)
        c = read_csv(o / "choice.csv")[0]
        frames.append((k, num(c["silhouette"]), num(c["inertia"]), pu))
    figs.write_csv(DATA / "k_slider.csv", [{"k": k, "silhouette": round(sil, 3), "inertia": round(ine), "purity": pu[0]}
                                           for k, sil, ine, pu in frames])
    print("slider k", frames)

    # ---------------------------------------------------------------------------- the SOM: default grid, codebook
    o = run(head(False) + som_nodes(5), "somdef",
            {"clsom.map": "som.tif", "som.codebook": "codebook.csv", "som.quality": "quality.csv"}, log=True)
    cbd = read_csv(o / "codebook.csv")
    qd = read_csv(o / "quality.csv")[0]
    print("default SOM quality", qd)
    somdef = read_map(o.parent / "som.tif")

    def grid(cb, col):
        rws = max(int(r["row"]) for r in cb)
        cls = max(int(r["column"]) for r in cb)
        a = np.full((rws, cls), np.nan)
        for r in cb:
            a[int(r["row"]) - 1, int(r["column"]) - 1] = num(r[col])
        return a

    def som_panels(cb, cols_, title_fn, fig_w, name, width):
        n = len(cols_)
        nc = 3
        nr = (n + nc - 1) // nc
        fig, axs = plt.subplots(nr, nc, figsize=(fig_w, 2.9 * nr + 0.4), dpi=100)
        axs = np.atleast_1d(axs).ravel()
        for ax, (col, t, cmap) in zip(axs, cols_):
            a = grid(cb, col)
            if col == "group":
                img = np.ones(a.shape + (3,))
                for g in np.unique(a[~np.isnan(a)]):
                    img[a == g] = matplotlib.colors.to_rgb(CL[int(g) - 1])
                ax.imshow(img, interpolation="nearest")
            else:
                im = ax.imshow(a, cmap=cmap, interpolation="nearest")
                cb_ = fig.colorbar(im, ax=ax, fraction=0.046, pad=0.03)
                cb_.ax.tick_params(labelsize=7)
            ax.set_title(t, loc="left", fontsize=10)
            ax.set_xticks([])
            ax.set_yticks([])
            ax.grid(False)
            for s in ax.spines.values():
                s.set_visible(False)
        for ax in axs[n:]:
            ax.set_axis_off()
        fig.tight_layout()
        save(fig, name, width)

    som_panels(cbd, [("umatrix", "U-matrix (distance to the neighbours)", "viridis"),
                     ("count", "Objects on each node", "viridis"), ("group", "The 5 groups of nodes", None)],
               None, 9.6, "som-default.png", 960)

    # the 8 x 8 map of the page
    o = run(head(False) + som_nodes(5, ", rows = 8, cols = 8"), "som8", {"som.codebook": "codebook.csv",
                                                                         "som.quality": "quality.csv"})
    cb8 = read_csv(o / "codebook.csv")
    print("8x8 quality", read_csv(o / "quality.csv")[0])
    som_panels(cb8, [("umatrix", "U-matrix", "viridis"), ("count", "Objects on each node", "viridis"),
                     ("group", "The 5 groups", None), ("ndvi", "NDVI of the weights", "viridis"),
                     ("ndwi", "NDWI of the weights", "viridis"), ("glcm_contrast", "Texture contrast", "viridis")],
               None, 9.6, "som-codebook.png", 960)
    figs.write_csv(DATA / "codebook8.csv", [{k: (round(num(r[k]), 3) if k not in ("topology",) else r[k])
                                              for k in ("node", "row", "column", "count", "group", "umatrix", "ndvi",
                                                        "ndwi", "glcm_contrast")} for r in cb8])

    # the slider over the grid
    grids = [(3, 3), (4, 4), (6, 6), (8, 8), (12, 10), None]
    rows = []
    for g in grids:
        extra = f", rows = {g[0]}, cols = {g[1]}" if g else ""
        o = run(head(False) + som_nodes(5, extra) + ref_nodes + acc("clsom"), f"g{g[0] if g else 0}",
                {"clsom.map": "som.tif", "som.quality": "quality.csv", "som.codebook": "codebook.csv",
                 "acc.matrix": "mat.csv"})
        pu = purity(o / "mat.csv", "group")
        mp = read_map(o.parent / "som.tif")
        q = read_csv(o / "quality.csv")[0]
        cbg = read_csv(o / "codebook.csv")
        label = f"{int(q['rows'])} x {int(q['columns'])}"
        key = f"{int(q['rows'])}x{int(q['columns'])}"
        od.save_png(od.outlines(paint_clusters(mp, 5)[R0:R1, C0:C1], lab, color=(1, 1, 1), scale=2, thick=1),
                    OUT / f"grid_{key}.png", width=560, colors=64)
        rows.append({"grid": label, "nodes": int(q["rows"]) * int(q["columns"]), "key": key,
                     "quantisation": round(num(q["quantisation_error"]), 3),
                     "topographic": round(num(q["topographic_error"]), 3), "empty": int(num(q["empty_nodes"])),
                     "objects_per_node": round(3035 / (int(q["rows"]) * int(q["columns"])), 1),
                     "purity": pu[0], "classes_with_own_group": ", ".join(pu[1])})
    figs.write_csv(DATA / "grids.csv", rows)
    print("grids", rows)

    # ------------------------------------------------------------------------- what the columns setting is for
    m = head(False) + km_nodes(5, cols=None)
    o = run(m, "nocols", {"cl.map": "cl.tif", "km.centres": "centres.csv", "km.choice": "choice.csv"}, log=True)
    nocols = read_map(o.parent / "cl.tif")
    cen2 = read_csv(o / "centres.csv")
    sil2 = num(read_csv(o / "choice.csv")[0]["silhouette"])
    print("without columns: variables", [k for k in cen2[0].keys()][3:], "silhouette", sil2,
          "same cluster as with columns:", agreement(km5, nocols))
    figs.write_csv(DATA / "columns.csv", [
        {"variables": "columns = the twelve attributes", "count": 12, "silhouette": round(frames[3][1], 3),
         "same_as_first": 1.0},
        {"variables": "no columns: every numeric column", "count": len(cen2[0]) - 3, "silhouette": round(sil2, 3),
         "same_as_first": round(agreement(km5, nocols), 3)}])
    fig, axs = plt.subplots(1, 2, figsize=(8.2, 4.1), dpi=100)
    axs[0].imshow(outl(paint_clusters(km5, 5)))
    axs[0].set_title("columns = the twelve attributes", loc="left", fontsize=10.5)
    axs[1].imshow(outl(paint_clusters(nocols, 5)))
    axs[1].set_title("no columns (id, cells, area enter)", loc="left", fontsize=10.5)
    for ax in axs:
        ax.set_axis_off()
    legend_boxes(fig, [(f"cluster {i}", CL[i - 1]) for i in range(1, 6)], 5)
    fig.tight_layout(rect=(0, 0.07, 1, 1))
    save(fig, "columns.png", 900)
    # how much of the grouping follows the object's size and its number
    big = {}
    cellsz = read_csv(WORK / "nocols" / "output" / "centres.csv")
    figs.write_csv(DATA / "centres_nocolumns.csv",
                   [{k: (round(num(v), 3) if k not in ("cluster",) else int(num(v))) for k, v in r.items()
                     if k in ("cluster", "size", "id", "cells", "area", "mean_nir", "ndvi")} for r in cellsz])

    # ----------------------------------------------------------------------------------- steady over seeds
    rows = []
    base = None
    for seed in range(1, 6):
        o = run(head(False) + km_nodes(5, f", seed = {seed}") + som_nodes(5, f", rows = 8, cols = 8, seed = {seed}"),
                f"seed{seed}", {"cl.map": "km.tif", "clsom.map": "som.tif"})
        a, b = read_map(o.parent / "km.tif"), read_map(o.parent / "som.tif")
        if base is None:
            base = (a, b)
        rows.append({"seed": seed, "kmeans_same": round(agreement(a, base[0]), 3),
                     "som_same": round(agreement(b, base[1]), 3)})
    figs.write_csv(DATA / "seeds.csv", rows)
    print("seeds", rows)


if __name__ == "__main__":
    main()
