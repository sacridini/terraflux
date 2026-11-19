"""Figures of "What an object is: segmentation" (tutorials/object-based-image-analysis/what-is-an-object.md).

One model of Terraflux, run on the Sentinel-2 clip of the series (obia_data.py): the data.stac node, the segmenters
of the page and objects.to_map to write each one's objects as a map of ids; the figures draw the outlines of those ids
over the RGB. Run with: python website/figures/object-based-image-analysis/what-is-an-object.py
"""

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
SLUG = "what-is-an-object"
OUT = figs.img_dir(f"{od.CATEGORY}/{SLUG}")
DATA = figs.data_dir(f"{od.CATEGORY}/{SLUG}")

# the part of the clip the figures show (rows, columns of the clip): lake edge, houses, gallery forest, cerrado
R0, R1, C0, C1 = 90, 360, 200, 470
SCALES = [1, 1.5, 2, 3, 4, 6, 8]
CELL_HA = 0.01  # a cell of 10 m, in hectares

SEGMENTERS = [
    # id, op, settings (TOML), image
    ("mr", "segment.multiresolution", "{ scale = 2 }", "refl.image"),
    ("slic", "segment.slic", "{ size = 12 }", "refl.image"),
    ("fz", "segment.felzenszwalb", "{ k = 2 }", "refl.image"),
    ("ws", "segment.watershed", "{ }", "refl.image"),
    ("th", "segment.threshold", "{ classes = 2 }", "ndwi.map"),
]
SHAPES = [("sh1", 0.1, 0.5), ("sh5", 0.5, 0.5), ("sh9", 0.9, 0.5), ("co1", 0.5, 0.1), ("co9", 0.5, 0.9)]


def node(i, op, inputs, params):
    return f'\n[[nodes]]\nid = "{i}"\nop = "{op}"\ninputs = {inputs}\nparams = {params}\n'


def build():
    m = od.sentinel2_model()
    m += node("ndwi", "raster.calc", '{ s = "refl.image" }', '{ expression = "(s.green - s.nir) / (s.green + s.nir)" }')
    writes = {}
    segs = [(i, op, f"{{ image = \"{img}\" }}", p) for i, op, p, img in SEGMENTERS]
    segs += [(f"mr_s{str(s).replace('.', '_')}", "segment.multiresolution", '{ image = "refl.image" }', f"{{ scale = {s} }}")
             for s in SCALES]
    segs += [(i, "segment.multiresolution", '{ image = "refl.image" }', f"{{ scale = 2, shape = {sh}, compactness = {co} }}")
             for i, sh, co in SHAPES]
    for i, op, inp, p in segs:
        m += node(i, op, inp, p)
        m += node(f"{i}_map", "objects.to_map", f'{{ objects = "{i}.objects" }}', '{ what = "id" }')
        writes[f"{i}_map.map"] = f"{i}.tif"
    return figs.add_writes(m, writes), [s[0] for s in segs]


def labels(work, i):
    with rasterio.open(work / f"{i}.tif") as ds:
        a = ds.read(1, masked=True).astype("float64")
    return np.ma.filled(a, -1).astype("int64")


def count(lab):
    return len(np.unique(lab[lab >= 0]))


def crop(a):
    return a[R0:R1, C0:C1]


def main():
    od.sentinel2()
    work = figs.workdir(f"{od.CATEGORY}/{SLUG}")
    model, ids = build()
    r = od.run(model, work, "--profile")
    print(r.stdout[:600])
    rgb = od.rgb("sentinel2")
    lab = {i: labels(work, i) for i in ids}
    n = {i: count(lab[i]) for i in ids}
    print(n)

    # the clip, with the part the other figures show
    fig = plt.figure(figsize=(5.4, 5.4), dpi=150)
    ax = fig.add_axes([0, 0, 1, 1])
    ax.imshow(rgb)
    ax.add_patch(plt.Rectangle((C0, R0), C1 - C0, R1 - R0, fill=False, ec="#ffd91a", lw=1.6))
    ax.text(C0 + 8, R1 - 10, "shown in the next figures", color="#ffd91a", fontsize=8, fontweight="bold")
    ax.set_axis_off()
    fig.savefig(OUT / "clip.png", dpi=150)
    plt.close(fig)
    od.save_png(np.asarray(plt.imread(OUT / "clip.png"))[..., :3], OUT / "clip.png", width=760, colors=160)

    # pixels and objects: a small window, the cells and then the objects filled with their mean colour
    pr0, pr1, pc0, pc1 = 195, 235, 270, 310
    sub = rgb[pr0:pr1, pc0:pc1]
    sl = lab["mr"][pr0:pr1, pc0:pc1]
    mean = np.zeros_like(sub)
    for k in np.unique(sl):
        mean[sl == k] = sub[sl == k].mean(axis=0)
    k = 10
    big = np.kron(sub, np.ones((k, k, 1)))
    grid = big.copy()
    grid[::k, :] = grid[::k, :] * 0.55 + 0.45 * 0.9
    grid[:, ::k] = grid[:, ::k] * 0.55 + 0.45 * 0.9
    obj = od.outlines(mean, sl, color=(1, 1, 1), scale=k, thick=2)
    both = np.concatenate([grid, np.ones((grid.shape[0], 14, 3)), obj], axis=1)
    od.save_png(both, OUT / "pixels-objects.png", width=900, colors=200)

    # the five segmenters, outlines over the same part
    names = {"mr": "multiresolution", "slic": "SLIC", "fz": "Felzenszwalb", "ws": "watershed", "th": "threshold"}
    pos = {"mr": "mr", "slic": "slic", "fz": "fz", "ws": "ws", "th": "th"}
    for i in pos:
        img = od.outlines(crop(rgb), crop(lab[i]), scale=2, thick=2)
        od.save_png(img, OUT / f"seg_{i}.png", width=560, colors=128)

    # shape and compactness
    for i, _, _ in SHAPES:
        img = od.outlines(crop(rgb), crop(lab[i]), scale=2, thick=2)
        od.save_png(img, OUT / f"{i}.png", width=480, colors=96)

    # the scale slider
    rows = []
    for s in SCALES:
        i = f"mr_s{str(s).replace('.', '_')}"
        img = od.outlines(crop(rgb), crop(lab[i]), scale=2, thick=2)
        od.save_png(img, OUT / f"scale_{str(s).replace('.', '_')}.png", width=600, colors=112)
        cells = int((lab[i] >= 0).sum())
        rows.append({"scale": s, "objects": n[i], "mean_ha": round(cells / n[i] * CELL_HA, 2)})
    figs.write_csv(DATA / "scale.csv", rows)
    print(rows)

    summary = {"segmenters": {i: n[i] for i in pos}, "shapes": {i: n[i] for i, _, _ in SHAPES}, "scales": rows}
    import json

    (work / "counts.json").write_text(json.dumps(summary, indent=1), encoding="utf-8")
    cells = {i: int((lab[i] >= 0).sum()) for i in pos}
    print({i: round(cells[i] / n[i] * CELL_HA, 2) for i in pos})


if __name__ == "__main__":
    main()
