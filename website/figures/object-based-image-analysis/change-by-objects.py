"""Figures of "Change by objects" (tutorials/object-based-image-analysis/change-by-objects.md).

Two dry-season Sentinel-2 scenes of the clip of the series (obia_data.py: 2024-08-09; obia_dates.py: 2019-08-13, the
same box and grid), each cut into multiresolution objects at the scale 2 and described by the twelve attributes of
part 5, classed by one random forest trained on the samples of part 5 (obia_samples.py is read, not changed), and
bound by objects.temporal with a fuzzy transition matrix. Experiments, each a run of Terraflux:
  * the comparison of the two classifications as they are (a matrix of ones), with a matrix written by hand, with
    `both`, and with matrices estimated by objects.transition_matrix (from the observed transitions and from samples),
  * a slider over `min`,
  * the 24 largest patches that became urban, looked at by eye (a contact sheet).
Run with: python website/figures/object-based-image-analysis/change-by-objects.py   (at most 2 threads: it sets them)
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
import obia_dates as dt  # noqa: E402
import obia_samples as sm  # noqa: E402
import rasterio  # noqa: E402
from scipy import ndimage as ndi  # noqa: E402

for _k in ("RAYON_NUM_THREADS", "OMP_NUM_THREADS", "GDAL_NUM_THREADS", "OPENBLAS_NUM_THREADS", "MKL_NUM_THREADS",
           "TERRAFLUX_THREADS"):
    os.environ[_k] = "2"  # obia_data sets 4: this script runs with 2

figs = od.figs
SLUG = "change-by-objects"
OUT = figs.img_dir(f"{od.CATEGORY}/{SLUG}")
DATA = figs.data_dir(f"{od.CATEGORY}/{SLUG}")
WORK = None

FEATURES = ["mean", "sd", "ndvi", "ndwi", "ndmi", "contrast", "homogeneity", "compactness"]
COLS = ["mean_blue", "mean_green", "mean_red", "mean_nir", "mean_swir16", "ndvi", "ndwi", "ndmi", "glcm_contrast",
        "glcm_homogeneity", "sd_nir", "compactness"]
ORDER = ["bare soil", "cerrado", "forest", "urban", "water"]  # alphabetical: the class numbers 1..5
# what can become what in five years, written by hand (rows: the class at the first date, columns: at the second)
HAND = {
    "bare soil": [1, 1, 0.2, 1, 0],
    "cerrado": [1, 1, 0.3, 1, 0],
    "forest": [1, 0.5, 1, 1, 0],
    "urban": [0.2, 0, 0, 1, 0],
    "water": [0, 0, 0, 0, 1],
}
# the patches of the contact sheet (by rank, from 0) that the writer judged, by eye at 10 m, to be new roofs or cleared
# ground with confidence, and those that look probable; the others look unchanged or doubtful
REAL = [2, 18, 22]  # #3, #19, #23 (judged again on the run of 2026-11-22, with the grid made by data.stac)
UNCLEAR = [0, 1, 5]  # #1, #2, #6
FIRST_SIZES = [865, 643, 580, 451, 355]  # a guard: the ranking is that of the run these were judged on


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


def date_nodes(tif, suffix):
    """Read the cached clip, reflectance, the objects at the scale 2 and their attributes (the fast form of the page's
    data.stac nodes)."""
    t = str(tif).replace("\\", "/")
    m = f'\n[[nodes]]\nid = "s2{suffix}"\nop = "raster.read"\nparams = {{ path = "{t}" }}\n'
    m += node("refl" + suffix, "raster.calc", f'{{ s = "s2{suffix}.raster" }}', '{ expression = "out image = s / 10000" }')
    m += node("seg" + suffix, "segment.multiresolution", f'{{ image = "refl{suffix}.image" }}', "{ scale = 2 }")
    m += node("feat" + suffix, "objects.features", f'{{ objects = "seg{suffix}.objects", image = "refl{suffix}.image" }}',
              f'{{ features = {tl(FEATURES)}, texture_bands = ["brightness"], swir = "swir16" }}')
    return m


def matrix_node(i, rows):
    cols = ", ".join(f'"{c}"' for c in ORDER)
    body = ", ".join('["' + r + '", ' + ", ".join(str(x) for x in v) + "]" for r, v in rows.items())
    return node(i, "table.of", "{ }", f'{{ columns = ["class", {cols}], rows = [{body}] }}')


def run(model, name, writes, log=False):
    work = WORK / name
    work.mkdir(parents=True, exist_ok=True)
    r = figs.run(figs.add_writes(model, writes), work, "--threads", "2", "--no-journal")
    if log:
        print(r.stdout[-600:], r.stderr[-1500:])
    return work / "output"


def save(fig, name, width, colors=256):
    fig.savefig(OUT / name, dpi=fig.dpi)
    plt.close(fig)
    od.save_png(np.asarray(plt.imread(OUT / name))[..., :3], OUT / name, width=width, colors=colors)


def paint_classes(mp):
    img = np.ones(mp.shape + (3,)) * 0.88
    for i, c in enumerate(ORDER, 1):
        img[mp == i] = matplotlib.colors.to_rgb(sm.COLORS[c])
    return img


URB = ORDER.index("urban") + 1
GAIN, LOSS, OTHER = "#c2338a", "#1f6fd0", "#f0a030"


def change_image(before, after, grey):
    """The 2024 image in grey, the cells that became urban in magenta, those that stopped being urban in blue, the other
    changes in amber, the cells with no class at the second date in white."""
    img = np.repeat(grey[..., None], 3, axis=2) * 0.8 + 0.1
    ch = (before > 0) & (after > 0) & (before != after)
    for sel, col in ((ch & (after == URB), GAIN), (ch & (before == URB), LOSS),
                     (ch & (after != URB) & (before != URB), OTHER)):
        img[sel] = matplotlib.colors.to_rgb(col)
    img[(after == 0)] = 1.0
    return img


def stats(before, after):
    cell_ha = 0.01
    ok = (before > 0) & (after > 0)
    ch = ok & (before != after)
    return {
        "changed_ha": round(float(ch.sum()) * cell_ha),
        "changed_share": round(float(ch.sum()) / float(before.size), 3),
        "none_ha": round(float((after == 0).sum()) * cell_ha),
        "became_urban_ha": round(float((ch & (after == URB)).sum()) * cell_ha),
        "left_urban_ha": round(float((ch & (before == URB)).sum()) * cell_ha),
    }


def main():
    global WORK
    s24, s19 = od.sentinel2(), dt.sentinel2_early()
    WORK = figs.workdir(f"{od.CATEGORY}/{SLUG}")

    # ------------------------------------------------------------------------------------------- the two dates
    rgb24, rgb19 = od.rgb("sentinel2", s24), od.rgb("sentinel2", s19)
    od.save_png(rgb19, OUT / "rgb_2019.png", width=760, colors=256)
    od.save_png(rgb24, OUT / "rgb_2024.png", width=760, colors=256)
    a19, a24 = od.read_stack(s19), od.read_stack(s24)
    nd = lambda a: (a[3] - a[2]) / (a[3] + a[2])  # noqa: E731
    d = nd(a24) - nd(a19)
    print("NDVI difference: mean %.3f, share below -0.2: %.4f, above 0.2: %.4f" % (
        np.nanmean(d), np.nanmean(d < -0.2), np.nanmean(d > 0.2)))
    for n, i in (("blue", 0), ("red", 2), ("nir", 3)):
        print(n, "median 2019 %.0f 2024 %.0f" % (np.nanmedian(a19[i]), np.nanmedian(a24[i])))

    # the downloads: the 2019 clip as Int16, as the series' 2024 clip
    dst = od.DOWNLOADS / SLUG
    dst.mkdir(parents=True, exist_ok=True)
    with rasterio.open(s19) as ds:
        prof = ds.profile | {"dtype": "int16", "nodata": -9999, "compress": "deflate", "predictor": 2, "tiled": False}
        prof.pop("blockxsize", None)
        prof.pop("blockysize", None)
    with rasterio.open(dst / "sentinel2_brasilia_2019.tif", "w", **prof) as o:
        o.write(np.where(np.isnan(a19), -9999, np.rint(a19)).astype("int16"))
        o.descriptions = tuple(od.S2_BANDS)
    (dst / "ATTRIBUTION.txt").write_text(
        "sentinel2_brasilia_2019.tif\n  Sentinel-2 L2A, scene S2A_22LHH_20190813_0_L2A (13 August 2019), from Earth Search\n"
        "  (https://earth-search.aws.element84.com/v1). Bands blue, green, red, nir (10 m) and swir16 (20 m, resampled to\n"
        "  the grid), reflectance x 10000, Int16, EPSG:31983 (SIRGAS 2000 / UTM 23S), square 10 m cells, 580 x 580, the\n"
        "  same grid as sentinel2_brasilia.tif (9 August 2024) of the series. The area is the edge of Lago Paranoa,\n"
        "  Brasilia, Brazil.\n  Contains modified Copernicus Sentinel data 2019.\n", encoding="utf-8")

    # ------------------------------------------------------------------------------ ids and samples (part 5's)
    m0 = 'format = "terraflux/1"\n' + date_nodes(s24, "")
    m0 += node("idmap", "objects.to_map", '{ objects = "seg.objects" }', '{ what = "id" }')
    run(m0, "ids", {"idmap.map": "ids.tif"})
    ids = read_map(WORK / "ids" / "ids.tif")
    taken = set()
    train = sm.draw(sm.TRAIN, ids, seed=7, taken=taken)
    figs.write_csv(DATA / "training_samples.csv",
                   [{"x": sm.xy(r, c)[0], "y": sm.xy(r, c)[1], "class": cls} for cls, r, c in train])

    # --------------------------------------------------------------------------------- the model, several matrices
    base = 'format = "terraflux/1"\n' + date_nodes(s24, "") + date_nodes(s19, "_e") + sm.table_nodes("train", train)
    base += node("samples", "objects.samples", '{ objects = "feat.objects", samples = "train.vector" }')
    base += node("fit", "ml.fit", '{ table = "samples.table" }',
                 f'{{ column = "class", columns = {tl(COLS)}, n_estimators = 300 }}')
    base += node("c_late", "objects.classify",
                 '{ objects = "feat.objects", model = "fit.model", samples = "samples.table" }')
    base += node("c_early", "objects.classify",
                 '{ objects = "feat_e.objects", model = "fit.model", samples = "samples.table" }')
    base += matrix_node("hand", HAND) + matrix_node("ones", {c: [1] * 5 for c in ORDER})

    def temporal(i, matrix, extra=""):
        return (node(i, "objects.temporal", f'{{ before = "c_early.objects", after = "c_late.objects", matrix = "{matrix}" }}',
                     "{ " + extra + " }") +
                node(i + "_b", "objects.to_map", f'{{ objects = "{i}.before" }}', '{ what = "class" }'))

    m = base + temporal("naive", "ones.table") + temporal("by_hand", "hand.table")
    m += temporal("both", "hand.table", "both = true")
    m += node("est_t", "objects.transition_matrix", '{ transitions = "naive.change" }')
    m += temporal("from_t", "est_t.matrix")
    writes = {"c_early.map": "early.tif", "c_late.map": "late.tif", "c_early.table": "early_table.csv",
              "c_late.table": "late_table.csv", "samples.counts": "counts.csv", "fit.importance": "importance.csv",
              "est_t.matrix": "est_t.csv", "by_hand.change": "change_hand.csv", "naive.change": "change_naive.csv",
              "both.change": "change_both.csv", "from_t.change": "change_from_t.csv"}
    for i in ("naive", "by_hand", "both", "from_t"):
        writes[f"{i}.map"] = f"{i}.tif"
        writes[f"{i}_b.map"] = f"{i}_before.tif"
    o = run(m, "main", writes, log=True)
    w = o.parent
    early, late = read_map(w / "early.tif"), read_map(w / "late.tif")
    maps = {i: (read_map(w / f"{i}_before.tif"), read_map(w / f"{i}.tif")) for i in ("naive", "by_hand", "both", "from_t")}

    # areas of the classes at each date
    rows = []
    for i, c in enumerate(ORDER, 1):
        rows.append({"class": c, "ha_2019": round(float((early == i).sum()) * 0.01),
                     "ha_2024": round(float((late == i).sum()) * 0.01)})
    figs.write_csv(DATA / "class_areas.csv", rows)
    print("areas", rows)

    # ------------------------------------------------------------------------------------------ matrices and tables
    est_t = read_csv(o / "est_t.csv")
    figs.write_csv(DATA / "matrix_hand.csv", [{"From \\ to": c, **{ORDER[j]: HAND[c][j] for j in range(5)}} for c in ORDER])
    figs.write_csv(DATA / "matrix_from_transitions.csv",
                   [{"From \\ to": r["class"], **{c: round(num(r[c]), 3) for c in ORDER}} for r in est_t])
    summary = []
    for name, i in (("Compared as they are (all transitions possible)", "naive"),
                    ("Matrix written by hand, min 0.1", "by_hand"),
                    ("The same, with both = true", "both"),
                    ("Matrix estimated from the transitions found", "from_t")):
        b, a = maps[i]
        s = stats(b, a)
        s["variant"] = name
        summary.append(s)
    figs.write_csv(DATA / "variants.csv", [{k: s[k] for k in ("variant", "changed_ha", "changed_share", "became_urban_ha",
                                                              "left_urban_ha", "none_ha")} for s in summary])
    print("variants", summary)

    # transitions by area (by hand), as a matrix of hectares
    def tmatrix(path):
        rr = read_csv(path)
        tab = {}
        for r in rr:
            tab[(r["from"], r["to"])] = tab.get((r["from"], r["to"]), 0) + num(r["cells"]) * 0.01
        cols = ORDER + ["(none)"]
        return [{"From \\ to": f, **{t: round(tab.get((f, t), 0)) for t in cols}} for f in ORDER]

    figs.write_csv(DATA / "transitions_hand.csv", tmatrix(o / "change_hand.csv"))
    figs.write_csv(DATA / "transitions_naive.csv", tmatrix(o / "change_naive.csv"))
    print("hand", read_csv(DATA / "transitions_hand.csv"))

    # ------------------------------------------------------------------------------------------------ figures
    fig, axs = plt.subplots(1, 2, figsize=(9.4, 4.9), dpi=100)
    for ax, mp, t in ((axs[0], early, "2019-08-13"), (axs[1], late, "2024-08-09")):
        ax.imshow(paint_classes(mp))
        ax.set_title(t, loc="left", fontsize=10.5)
        ax.set_axis_off()
    fig.legend([plt.Rectangle((0, 0), 1, 1, fc=sm.COLORS[c]) for c in ["water", "forest", "cerrado", "urban", "bare soil"]],
               ["water", "forest", "cerrado", "urban", "bare soil"], loc="lower center", ncol=5, fontsize=8.5, frameon=False)
    fig.tight_layout(rect=(0, 0.06, 1, 0.96))
    save(fig, "classes.png", 960)

    grey = (rgb24 * np.array([0.299, 0.587, 0.114])).sum(axis=2)
    for i in ("naive", "by_hand"):
        b, a = maps[i]
        od.save_png(change_image(b, a, grey), OUT / f"change_{i}.png", width=760, colors=64)

    def three(img_fn, name):
        pass

    # the legend of the change maps, as a small image beside the compare
    fig = plt.figure(figsize=(6.6, 0.5), dpi=100)
    fig.legend([plt.Rectangle((0, 0), 1, 1, fc=c) for c in (GAIN, LOSS, OTHER, "#ffffff")],
               ["became urban", "stopped being urban", "any other change", "no class (below min)"], loc="center", ncol=4,
               fontsize=8.5, frameon=False)
    save(fig, "change_legend.png", 660, colors=32)

    # ------------------------------------------------------------------------------------- the slider over min
    rows, frames = [], []
    for mn in (0.1, 0.2, 0.3, 0.4, 0.5):
        m = base + temporal("t", "hand.table", f"min = {mn}")
        oo = run(m, f"min{int(mn * 10)}", {"t.map": "t.tif", "t_b.map": "t_before.tif"})
        b, a = read_map(oo.parent / "t_before.tif"), read_map(oo.parent / "t.tif")
        s = stats(b, a)
        od.save_png(change_image(b, a, grey), OUT / f"min_{int(mn * 10)}.png", width=620, colors=64)
        rows.append({"min": mn, "changed": s["changed_share"], "no_class": round(s["none_ha"] / 3364, 3),
                     "became_urban_ha": s["became_urban_ha"], "left_urban_ha": s["left_urban_ha"]})
        frames.append((mn, s))
    figs.write_csv(DATA / "min.csv", rows)
    print("min", rows)

    # ------------------------------------------------------------- the patches that became urban, looked at
    b, a = maps["by_hand"]
    mask = (a == URB) & (b > 0) & (b != URB)
    lab, n = ndi.label(mask)
    sz = ndi.sum(mask, lab, range(1, n + 1))
    order = np.argsort(-sz)[:24]
    sizes = [int(sz[i]) for i in order]
    print("patches that became urban:", n, "the largest:", sizes)
    assert sizes[: len(FIRST_SIZES)] == FIRST_SIZES, "the ranking changed: judge the patches again"
    cols_, rws = 4, 6
    fig, axs = plt.subplots(rws, cols_ * 2, figsize=(10.4, 8.0), dpi=100)
    names_b = {}
    for k, i in enumerate(order):
        cy, cx = [int(v) for v in ndi.center_of_mass(lab == i + 1)]
        # the patch's cell nearest its centre (the centre of a ragged patch may lie outside it)
        ys, xs = np.nonzero(lab == i + 1)
        j = int(np.argmin((ys - cy) ** 2 + (xs - cx) ** 2))
        cy, cx = int(ys[j]), int(xs[j])
        names_b[k] = (cy, cx, ORDER[int(b[cy, cx]) - 1])
        r0, c0 = min(max(cy - 20, 0), 540), min(max(cx - 20, 0), 540)
        for jj, (im, t) in enumerate(((rgb19, "2019"), (rgb24, "2024"))):
            ax = axs[k // cols_, (k % cols_) * 2 + jj]
            ax.imshow(im[r0:r0 + 40, c0:c0 + 40])
            ax.set_title(f"#{k + 1} {t}" + (f"  {sizes[k] / 100:.1f} ha" if jj else ""), fontsize=7.5, loc="left")
            ax.set_axis_off()
            if jj:
                ax.contour(lab[r0:r0 + 40, c0:c0 + 40] == i + 1, levels=[0.5], colors="yellow", linewidths=0.9)
    fig.tight_layout(pad=0.5, h_pad=0.6, w_pad=0.2)
    save(fig, "patches.png", 1000)
    print("judged confident:", [k + 1 for k in REAL], "probable:", [k + 1 for k in UNCLEAR],
          "other:", [k + 1 for k in range(24) if k not in REAL and k not in UNCLEAR])

    # ----------------------------------- a matrix estimated from samples: the stable places and the changes verified
    pairs = [(c, c) for c, _, _ in train]  # the training places are stable places: the same class on both dates
    pairs += [(names_b[k][2], "urban") for k in REAL]
    body = ", ".join(f'["{p}", "{q}"]' for p, q in pairs)
    m = base + node("pairs", "table.of", "{ }", f'{{ columns = ["before", "after"], rows = [{body}] }}')
    m += node("est_s", "objects.transition_matrix", '{ samples = "pairs.table" }')
    m += temporal("from_s", "est_s.matrix")
    oo = run(m, "est_samples", {"est_s.matrix": "est_s.csv", "from_s.map": "from_s.tif", "from_s_b.map": "from_s_before.tif"},
             log=True)
    est_s = read_csv(oo / "est_s.csv")
    figs.write_csv(DATA / "matrix_from_samples.csv",
                   [{"From \\ to": r["class"], **{c: round(num(r[c]), 3) for c in ORDER}} for r in est_s])
    print("from samples", est_s)
    b2, a2 = read_map(oo.parent / "from_s_before.tif"), read_map(oo.parent / "from_s.tif")
    s = stats(b2, a2)
    s["variant"] = "Matrix estimated from stable places and verified changes"
    summary.append(s)
    figs.write_csv(DATA / "variants.csv", [{k: s[k] for k in ("variant", "changed_ha", "changed_share", "became_urban_ha",
                                                              "left_urban_ha", "none_ha")} for s in summary])
    figs.write_csv(DATA / "patch_samples.csv", [{"before": p, "after": q} for p, q in pairs])
    print("variants +samples", summary[-1])


if __name__ == "__main__":
    main()
