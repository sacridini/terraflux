"""Figures of "Neutral landscape models": a gallery of every nlm.* generator, the percolation threshold (the
largest cluster as the share of habitat grows, a slider with a linked chart from 10 seeds), the random cluster's
p and the fractal dimension of fBm (sliders), the Gaussian field's range, and the utilities."""

import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import _nlm as N  # noqa: E402
from _nlm import figs, np  # noqa: E402

SLUG = "neutral-landscapes/neutral-landscape-models"
w = N.work("neutral-landscape-models")
img = figs.img_dir(SLUG)
dat = figs.data_dir(SLUG)
R = {"rows": 200, "columns": 200}

# ---------------------------------------------------------------- the gallery
G = {
    "random": ("nlm.random", {**R, "seed": 1}),
    "percolation": ("nlm.percolation", {**R, "prob": 0.5, "seed": 1}),
    "planar_gradient": ("nlm.planar_gradient", {**R, "direction": 60}),
    "edge_gradient": ("nlm.edge_gradient", {**R, "direction": 60}),
    "distance_gradient": ("nlm.distance_gradient", {**R, "origin": [90, 110, 90, 110], "nlmr_compat": False}),
    "mpd": ("nlm.mpd", {**R, "roughness": 0.6, "seed": 2, "nlmr_compat": False}),
    "fbm": ("nlm.fbm", {**R, "dimension": 1.4, "seed": 3}),
    "gaussian_field": ("nlm.gaussian_field", {**R, "range": 20, "seed": 4}),
    "random_cluster": ("nlm.random_cluster", {**R, "p": 0.55, "weights": [0.3, 0.3, 0.4], "seed": 5}),
    "neigh": ("nlm.neigh", {**R, "p_neigh": 0.95, "p_empty": 0.001, "categories": 4, "rescale": False, "seed": 6}),
    "random_rectangular_cluster": ("nlm.random_rectangular_cluster", {**R, "minl": 5, "maxl": 30, "seed": 7}),
    "curds": ("nlm.curds", {"curds": [0.7, 0.7, 0.7], "steps": [5, 5, 8], "seed": 8}),
    "mosaic_tess": ("nlm.mosaic_tess", {**R, "germs": 50, "seed": 8}),
    "mosaic_gibbs": ("nlm.mosaic_gibbs", {**R, "germs": 60, "r": 10, "patch_classes": 5, "rescale": False, "seed": 9}),
    "mosaic_field": ("nlm.mosaic_field", {**R, "n": 20, "seed": 9}),
}
model = N.HEAD
writes = {}
for k, (op, p) in G.items():
    model += N.node(k, op, params=p)
    writes[f"{k}.map"] = f"g_{k}.tif"
N.run(model, w, writes, name="gallery.toml")
CLASSY = {"percolation": {0: ("Matrix", "#ece6d6"), 1: ("Habitat", figs.FOREST)},
          "curds": {0: ("Matrix", "#ece6d6"), 1: ("Habitat", figs.FOREST)},
          "random_cluster": None, "neigh": None, "mosaic_gibbs": None}
for k in G:
    a, info = figs.read(w / f"g_{k}.tif")
    print(k, info["shape"], float(a.min()), float(a.max()), len(np.unique(a.compressed())))
    if k in CLASSY:
        pal = CLASSY[k]
        if pal is None:
            vals = [int(v) for v in np.unique(a.compressed())]
            pal = {v: (str(v), figs.CLASSES[i]) for i, v in enumerate(vals)}
        figs.raster(w / f"g_{k}.tif", img / f"g_{k}.png", palette=pal, legend=False, width=340)
    else:
        figs.raster(w / f"g_{k}.tif", img / f"g_{k}.png", cmap="viridis", legend=False, width=340)

# ---------------------------------------------------------------- percolation: the largest cluster as habitat grows
PROBS = [round(float(x), 3) for x in np.arange(0.30, 0.801, 0.025)]
SEEDS = range(1, 11)
rows = []
for prob in PROBS:
    model = N.HEAD
    for s in SEEDS:
        model += N.node(f"m{s}", "nlm.percolation", params={**R, "prob": prob, "seed": s})
        for d in (4, 8):
            model += N.node(f"c{s}_{d}", "metrics.class", {"map": f"m{s}.map"},
                            {"metrics": ["pland", "lpi", "np"], "directions": d, "show": "lpi"})
    writes = {f"c{s}_{d}.table": f"perc_{s}_{d}.csv" for s in SEEDS for d in (4, 8)}
    N.run(model, w, writes, name="perc.toml")
    row = {"habitat": round(prob * 100, 1)}
    for d in (4, 8):
        lpis = []
        for s in SEEDS:
            t = [r for r in N.table(w, f"perc_{s}_{d}") if r["class"] == 1][0]
            lpis.append(100 * t["lpi"] / t["pland"])     # the share of the habitat in its largest cluster
        row[f"n{d}"] = round(float(np.mean(lpis)), 2)
    rows.append(row)
    print(row)
figs.write_csv(dat / "percolation.csv", rows)

# the frames: habitat in grey-green, its largest cluster (4 neighbours) in dark green
FR = [0.45, 0.50, 0.55, 0.575, 0.60, 0.625, 0.65, 0.70]
frames = []
for prob in FR:
    tag = str(prob).replace(".", "_")
    model = N.HEAD + N.node("m", "nlm.percolation", params={**R, "prob": prob, "seed": 1})
    model += N.node("p", "metrics.patch", {"map": "m.map"}, {"metrics": ["area"], "directions": 4, "show": "area"})
    N.run(model, w, {"m.map": f"pf_{tag}.tif", "p.map": f"pa_{tag}.tif"}, name="pf.toml")
    m = figs.read(w / f"pf_{tag}.tif")[0]
    area = figs.read(w / f"pa_{tag}.tif")[0]
    big = float(area[m == 1].max())
    out = np.where(m == 1, np.where(area == big, 3, 2), 1)
    frac = float((out == 3).sum()) / float((m == 1).sum()) * 100
    spans = bool(np.any(out[0] == 3) and np.any(out[-1] == 3)) or bool(np.any(out[:, 0] == 3) and np.any(out[:, -1] == 3))
    frames.append({"prob": prob, "largest": round(frac, 1), "spans": spans})
    figs.raster(None, img / f"perc_{tag}.png", array=np.ma.array(out),
                palette={1: ("Matrix", "#ece6d6"), 2: ("Habitat", "#9cc5a1"), 3: ("Largest cluster", "#1f5f3a")},
                legend=False, width=560)
    print(frames[-1])
figs.write_csv(dat / "percolation_frames.csv", frames)

# ---------------------------------------------------------------- the random cluster's p
for p in (0.05, 0.15, 0.25, 0.35, 0.45, 0.55, 0.65):
    tag = str(p).replace(".", "_")
    model = N.HEAD + N.node("m", "nlm.random_cluster", params={**R, "p": p, "weights": [0.4, 0.6], "seed": 3})
    model += N.node("c", "metrics.class", {"map": "m.map"}, {"metrics": ["pland", "np", "ai", "lpi"], "show": "pland"})
    N.run(model, w, {"m.map": f"rc_{tag}.tif", "c.table": f"rc_{tag}.csv"}, name="rc.toml")
    t = [r for r in N.table(w, f"rc_{tag}") if r["class"] == 1][0]
    print("p", p, {k: round(v, 2) for k, v in t.items()})
    figs.raster(w / f"rc_{tag}.tif", img / f"rc_{tag}.png", palette={1: ("Habitat", figs.FOREST), 2: ("Matrix", "#ece6d6")},
                legend=False, width=560)
    (dat / f"rc_{tag}.txt").write_text(f"{t['pland']:.1f},{int(t['np'])},{t['ai']:.1f}", encoding="utf-8")

# ---------------------------------------------------------------- fBm's fractal dimension, and the same cut to 40 % habitat
for dim in (0.2, 0.6, 1.0, 1.4, 1.8):
    tag = str(dim).replace(".", "_")
    model = N.HEAD + N.node("m", "nlm.fbm", params={**R, "dimension": dim, "seed": 3})
    model += N.node("h", "nlm.binarize", {"map": "m.map"}, {"breaks": 0.4})
    model += N.node("c", "metrics.class", {"map": "h.map"}, {"metrics": ["np", "ai"], "show": "ai"})
    N.run(model, w, {"m.map": f"fbm_{tag}.tif", "h.map": f"fbmh_{tag}.tif", "c.table": f"fbm_{tag}.csv"}, name="fbm.toml")
    t = [r for r in N.table(w, f"fbm_{tag}") if r["class"] == 2][0]
    print("dim", dim, t)
    a = figs.read(w / f"fbm_{tag}.tif")[0]
    h = figs.read(w / f"fbmh_{tag}.tif")[0]
    # the surface and its 40 % habitat side by side
    import matplotlib.pyplot as plt
    from matplotlib import colors as mcolors
    fig, axes = plt.subplots(1, 2, figsize=(8.4, 4.2), dpi=150)
    axes[0].imshow(a, cmap="viridis", interpolation="nearest")
    hab = np.where(h == 2, 1, 0)
    axes[1].imshow(hab, cmap=mcolors.ListedColormap(["#ece6d6", figs.FOREST]), interpolation="nearest")
    for ax, ttl in zip(axes, ("The surface (0 to 1)", "Cut to 40 % habitat")):
        ax.set_axis_off()
        ax.set_title(ttl, loc="left", fontsize=10.5)
    fig.tight_layout()
    fig.savefig(img / f"fbm_{tag}.png", dpi=150)
    plt.close(fig)
    figs._shrink(img / f"fbm_{tag}.png")
    (dat / f"fbm_{tag}.txt").write_text(f"{int(t['np'])},{t['ai']:.1f}", encoding="utf-8")

# ---------------------------------------------------------------- the Gaussian field's range
for rng in (3, 10, 30):
    model = N.HEAD + N.node("m", "nlm.gaussian_field", params={**R, "range": rng, "seed": 4})
    N.run(model, w, {"m.map": f"gf_{rng}.tif"}, name="gf.toml")
    figs.raster(w / f"gf_{rng}.tif", img / f"gf_{rng}.png", cmap="viridis", legend=False, width=340)

# ---------------------------------------------------------------- the utilities: merge, classify, binarize
model = (N.HEAD + N.node("gradient", "nlm.planar_gradient", params={**R, "direction": 90})
         + N.node("noise", "nlm.fbm", params={**R, "dimension": 1.2, "seed": 5})
         + N.node("merged", "nlm.merge", {"primary": "gradient.map", "secondary": "noise.map"}, {"scaling": 0.8})
         + N.node("classes", "nlm.classify", {"map": "merged.map"}, {"weights": [0.5, 0.3, 0.2]})
         + N.node("jenks", "nlm.classify", {"map": "merged.map"}, {"n": 4})
         + N.node("habitat", "nlm.binarize", {"map": "merged.map"}, {"breaks": 0.3}))
N.run(model, w, {"merged.map": "u_merged.tif", "classes.classes": "u_classes.tif", "jenks.classes": "u_jenks.tif",
                 "habitat.map": "u_habitat.tif"}, name="utils.toml")
figs.raster(w / "u_merged.tif", img / "u_merged.png", cmap="viridis", legend=False, width=340)
figs.raster(w / "u_classes.tif", img / "u_classes.png", palette={1: ("1", "#e9dca8"), 2: ("2", "#a7c957"), 3: ("3", figs.FOREST)},
            legend=False, width=340)
figs.raster(w / "u_jenks.tif", img / "u_jenks.png", palette={1: ("1", "#e9dca8"), 2: ("2", "#a7c957"), 3: ("3", "#3b8a5a"), 4: ("4", "#1f4f33")},
            legend=False, width=340)
figs.raster(w / "u_habitat.tif", img / "u_habitat.png", palette=N.HAB, legend=False, width=340)
for f in ("u_classes", "u_jenks", "u_habitat"):
    a = figs.read(w / f"{f}.tif")[0]
    v, c = np.unique(a.compressed(), return_counts=True)
    print(f, dict(zip(v.tolist(), np.round(c / c.sum() * 100, 1).tolist())))
