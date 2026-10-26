"""Figures of "Gravity potentials as drivers" (land-change/gravity-potentials): Dinamica EGO's interaction
potential (surface.gravity) of towns drawn by chance, its exponent, its units, the mask and the two methods, and
the potential as a driver of one step of clearing (dinamica.step) beside the distance to the nearest town.

    python website/figures/land-change/gravity-potentials.py
"""

import pathlib
import sys
import time

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[2] / "tools"))
import figs  # noqa: E402
import numpy as np  # noqa: E402
from matplotlib.patches import Patch  # noqa: E402

plt = figs.plt
SLUG = "land-change/gravity-potentials"
out = figs.img_dir(SLUG)
data = figs.data_dir(SLUG)

TOWNS = """format = "terraflux/1"

[[nodes]]
id = "spots"
op = "nlm.percolation"
params = { rows = 250, columns = 250, resolution = 100, prob = 0.0004, seed = 31 }
note = "Where the towns are: about 1 cell in 2,500."

[[nodes]]
id = "size"
op = "nlm.gaussian_field"
params = { rows = 250, columns = 250, resolution = 100, range = 60, seed = 32 }

[[nodes]]
id = "towns"
op = "raster.calc"
inputs = { s = "spots.map", g = "size.map" }
params = { expression = "round(500 + 40000 * g * g * g) if s == 1 else None" }
note = "Each town's population; no value elsewhere."

[[nodes]]
id = "pull"
op = "surface.gravity"
inputs = { sources = "towns.map" }
params = { exponent = 2 }
"""

DRIVER = TOWNS + """
[[nodes]]
id = "is_town"
op = "raster.calc"
inputs = { t = "towns.map" }
params = { expression = "0 if isnull(t) else 1" }

[[nodes]]
id = "nearest"
op = "dynamic.distance_to"
inputs = { landscape = "is_town.map" }
params = { classes = 1 }

[[nodes]]
id = "land"
op = "raster.calc"
inputs = { d = "nearest.distance" }
params = { expression = "2 if d <= 300 else 1" }
note = "1 forest, 2 cleared around the towns."

[[nodes]]
id = "by_pull"
op = "raster.calc"
inputs = { land = "land.map", p = "pull.map" }
params = { expression = "p / (p + 500) if land == 1 else 0" }

[[nodes]]
id = "transitions"
op = "stack.of"
inputs = { 1_to_2 = "by_pull.map" }

[[nodes]]
id = "rates"
op = "markov.matrix"
params = { classes = [1, 2], single = [[0.97, 0.03], [0, 1]] }

[[nodes]]
id = "patches"
op = "patches.settings"
params = { transitions = [{ from = 1, to = 2, expansion = 0.5, expander = [10, 60, 1.5], patcher = [10, 60, 1.5] }] }

[[nodes]]
id = "clearing"
op = "dinamica.step"
inputs = { landscape = "land.map", probability = "transitions.stack", rates = "rates.markov", patches = "patches.patches", step = 1, seed = 1 }
"""

# the same step with the probability from the distance to the nearest town
BY_DISTANCE = DRIVER.replace("""[[nodes]]
id = "by_pull"
op = "raster.calc"
inputs = { land = "land.map", p = "pull.map" }
params = { expression = "p / (p + 500) if land == 1 else 0" }""", """[[nodes]]
id = "by_pull"
op = "raster.calc"
inputs = { land = "land.map", d = "nearest.distance" }
params = { expression = "exp(-d / 1500) if land == 1 else 0" }""")
assert BY_DISTANCE != DRIVER


def run(model, name, writes, *args):
    w = figs.workdir(f"land-change/gravity-potentials/{name}")
    t = time.perf_counter()
    figs.run(figs.add_writes(model, writes), w, *args)
    return w, time.perf_counter() - t


def arr(path, band=1):
    return figs.read(path, band)[0].filled(np.nan)


def legend_strip(png, items, width=900):
    fig = plt.figure(figsize=(width / 150, 0.42), dpi=150)
    handles = [Patch(facecolor=c, edgecolor="#00000022", label=n) for n, c in items]
    fig.legend(handles=handles, loc="center", ncol=len(items), fontsize=9.5, handlelength=1.1, handleheight=1.1,
               columnspacing=1.4)
    fig.savefig(png, dpi=150)
    plt.close(fig)
    figs._shrink(png)


def town_map(ax, towns, scale=1.0):
    ii, jj = np.nonzero(~np.isnan(towns))
    ax.scatter(jj, ii, s=6 + towns[ii, jj] / 400 * scale, facecolors="none", edgecolors="white", linewidths=0.9)


# ---------------------------------------------------------------------------------------------- a toy, explained

fig, axes = plt.subplots(1, 3, figsize=(10.5, 3.6), dpi=150)
toy = np.full((40, 60), np.nan)
toy[12, 14], toy[28, 22], toy[20, 46] = 10000, 2000, 4000
yy, xx = np.mgrid[0:40, 0:60]
for ax, beta in zip(axes, (1, 2, 3)):
    p = np.zeros(toy.shape)
    for (i, j) in zip(*np.nonzero(~np.isnan(toy))):
        d = np.hypot(yy - i, xx - j)
        d[i, j] = 1.0
        p += toy[i, j] / d ** beta
    ax.imshow(np.log10(p), cmap="magma")
    for (i, j) in zip(*np.nonzero(~np.isnan(toy))):
        ax.scatter(j, i, s=8 + toy[i, j] / 120, facecolors="none", edgecolors="white", linewidths=1)
    ax.set_axis_off()
    ax.set_title(f"β = {beta}", loc="left")
fig.tight_layout()
fig.savefig(out / "toy.png", dpi=150)
plt.close(fig)
figs._shrink(out / "toy.png")

# ---------------------------------------------------------------------------------------------- the towns, the pull

w, _ = run(TOWNS, "towns", {"towns.map": "towns.tif", "pull.map": "pull.tif"})
towns, pull = arr(w / "towns.tif"), arr(w / "pull.tif")
v = towns[~np.isnan(towns)]
print(f"{v.size} towns, {v.min():.0f} to {v.max():.0f} people, total {v.sum():.0f}")
fig = plt.figure(figsize=(620 / 150, 620 / 150 * 0.9), dpi=150)
ax = fig.add_axes([0, 0, 0.8, 1])
im = ax.imshow(np.log10(pull), cmap="magma")
town_map(ax, towns)
ax.set_axis_off()
cax = fig.add_axes([0.83, 0.1, 0.035, 0.76])
cb = fig.colorbar(im, cax=cax)
cb.outline.set_visible(False)
cb.set_label("log10 of the potential", fontsize=9.5, color=figs.INK2)
fig.savefig(out / "pull.png", dpi=150, bbox_inches="tight", pad_inches=0.08)
plt.close(fig)
figs._shrink(out / "pull.png")

# a transect through the largest town, for each exponent
big = np.unravel_index(np.nanargmax(towns), towns.shape)
print("largest town at", big)
tr = []
cols = list(range(250))
for beta in (0.5, 1, 2, 3):
    m = TOWNS.replace("params = { exponent = 2 }", f"params = {{ exponent = {beta} }}")
    wb, _ = run(m, f"beta_{beta}", {"pull.map": "pull.tif"})
    pb = arr(wb / "pull.tif")
    fig = plt.figure(figsize=(620 / 150, 620 / 150 * 0.9), dpi=150)
    ax = fig.add_axes([0, 0, 0.8, 1])
    im = ax.imshow(np.log10(pb), cmap="magma")
    town_map(ax, towns)
    ax.axhline(big[0], color="#5ad1ff", lw=0.7, ls=(0, (3, 3)))
    ax.set_axis_off()
    cax = fig.add_axes([0.83, 0.1, 0.035, 0.76])
    cb = fig.colorbar(im, cax=cax)
    cb.outline.set_visible(False)
    cb.set_label("log10 of the potential", fontsize=9.5, color=figs.INK2)
    fig.savefig(out / f"beta_{beta}.png", dpi=150, bbox_inches="tight", pad_inches=0.08)
    plt.close(fig)
    figs._shrink(out / f"beta_{beta}.png")
    row = pb[big[0], :]
    tr.append(row / np.nanmax(row))
    print(f"beta {beta}: min {np.nanmin(pb):.3g} max {np.nanmax(pb):.3g} ratio {np.nanmax(pb) / np.nanmin(pb):.3g}; "
          f"share of the map above 10% of the max of the row: {(row > 0.1 * row.max()).mean() * 100:.0f}%")
rows = [{"km": round(c * 0.1, 1), **{f"b{b}": round(float(t[c]), 5) for b, t in zip((0.5, 1, 2, 3), tr)}}
        for c in cols]
figs.write_csv(data / "transect.csv", rows)

# units: cells against map units
m = TOWNS.replace("params = { exponent = 2 }", 'params = { exponent = 2, unit = "map" }')
wm, _ = run(m, "unit_map", {"pull.map": "pull.tif"})
pm = arr(wm / "pull.tif")
print("map / cells:", np.nanmin(pm / pull), np.nanmax(pm / pull))

# ---------------------------------------------------------------------------------------------- the mask

MASKED = TOWNS.replace("""[[nodes]]
id = "pull"
op = "surface.gravity"
inputs = { sources = "towns.map" }
params = { exponent = 2 }""", """[[nodes]]
id = "frame"
op = "nlm.distance_gradient"
params = { rows = 250, columns = 250, resolution = 100, origin = [51, 200, 51, 200], nlmr_compat = false, rescale = false }

[[nodes]]
id = "study"
op = "raster.calc"
inputs = { d = "frame.map" }
params = { expression = "1 if d == 0 else None" }
note = "The study area; no value around it."

[[nodes]]
id = "inside"
op = "raster.calc"
inputs = { t = "towns.map", s = "study.map" }
params = { expression = "None if isnull(s) else t" }
note = "Only the towns of the study area."

[[nodes]]
id = "pull"
op = "surface.gravity"
inputs = { sources = "towns.map", mask = "study.map" }
params = { exponent = 2 }

[[nodes]]
id = "pull_inside"
op = "surface.gravity"
inputs = { sources = "inside.map", mask = "study.map" }
params = { exponent = 2 }""")
wk, _ = run(MASKED, "mask", {"pull.map": "pull.tif", "pull_inside.map": "inside.tif", "study.map": "study.tif"})
pk, pi = arr(wk / "pull.tif"), arr(wk / "inside.tif")
loss = (pk - pi) / pk * 100
print(f"mask: towns inside {np.count_nonzero(~np.isnan(towns[50:200, 50:200]))}; potential missed without the towns "
      f"around: median {np.nanmedian(loss):.1f}%, max {np.nanmax(loss):.1f}%; cells off by > 25%: "
      f"{np.mean(loss[~np.isnan(loss)] > 25) * 100:.1f}%")
for name, a in (("mask_all", pk), ("mask_inside", pi)):
    fig = plt.figure(figsize=(560 / 150, 560 / 150 * 0.9), dpi=150)
    ax = fig.add_axes([0, 0, 0.8, 1])
    im = ax.imshow(np.log10(a), cmap="magma", vmin=np.nanmin(np.log10(pi)), vmax=np.nanmax(np.log10(pk)))
    town_map(ax, towns if name == "mask_all" else np.where(np.isnan(arr(wk / "study.tif")), np.nan, towns))
    ax.set_xlim(-0.5, 249.5)
    ax.set_ylim(249.5, -0.5)
    ax.plot([49.5, 199.5, 199.5, 49.5, 49.5], [49.5, 49.5, 199.5, 199.5, 49.5], color=figs.INK2, lw=0.8)
    ax.set_axis_off()
    cax = fig.add_axes([0.83, 0.1, 0.035, 0.76])
    cb = fig.colorbar(im, cax=cax)
    cb.outline.set_visible(False)
    cb.set_label("log10 of the potential", fontsize=9.5, color=figs.INK2)
    fig.savefig(out / f"{name}.png", dpi=150, bbox_inches="tight", pad_inches=0.08)
    plt.close(fig)
    figs._shrink(out / f"{name}.png")
figs.raster(None, out / "mask_loss.png", array=loss, cmap="YlOrRd", vmin=0, vmax=100,
            unit="% of the potential missed", width=560)

# ---------------------------------------------------------------------------------------------- as a driver

LC = {1: ("Forest", "#3b8a5a"), 2: ("Cleared at the start", "#d9c9a3"), 3: ("Cleared in the step", "#b4508a")}
res = {}
for name, model in (("pull", DRIVER), ("distance", BY_DISTANCE)):
    wd, _ = run(model, f"driver_{name}", {"land.map": "land.tif", "clearing.landscape": "after.tif",
                                          "by_pull.map": "prob.tif", "nearest.distance": "near.tif",
                                          "clearing.info": "info.csv"})
    land, after = arr(wd / "land.tif"), arr(wd / "after.tif")
    near = arr(wd / "near.tif")
    new = (after == 2) & (land == 1)
    c = np.where(new, 3, land)
    figs.raster(None, out / f"driver_{name}.png", array=c, palette=LC, legend=False, width=620)
    figs.raster(None, out / f"prob_{name}.png", array=np.where(land == 1, arr(wd / "prob.tif"), np.nan),
                cmap="viridis", vmin=0, vmax=1, unit="probability", width=560)
    # the town nearest each new cell, and its size
    from scipy import ndimage

    idx = ndimage.distance_transform_edt(np.isnan(towns), return_distances=False, return_indices=True)
    pop = towns[idx[0], idx[1]]
    res[name] = new
    big_share = (pop[new] >= 5000).mean() * 100
    print(f"{name}: new {new.sum()} ha; near a town of >= 5000: {big_share:.0f}%; mean distance to the nearest town "
          f"{near[new].mean() / 1000:.2f} km; towns with clearing within 2 km: "
          f"{len(np.unique(idx[0][new & (near <= 2000)] * 250 + idx[1][new & (near <= 2000)]))}")
legend_strip(out / "legend.png", [LC[k] for k in (1, 2, 3)], width=700)
print("towns >= 5000:", int((v >= 5000).sum()), "of", v.size, "; their people", v[v >= 5000].sum() / v.sum() * 100)

# ---------------------------------------------------------------------------------------------- the methods

MANY = """format = "terraflux/1"

[[nodes]]
id = "land"
op = "nlm.random_cluster"
params = { rows = 250, columns = 250, resolution = 100, p = 0.5, weights = [0.8, 0.2], seed = 4 }

[[nodes]]
id = "cleared"
op = "raster.calc"
inputs = { l = "land.map" }
params = { expression = "1 if l == 2 else None" }

[[nodes]]
id = "pull"
op = "surface.gravity"
inputs = { sources = "cleared.map" }
params = { exponent = 2, method = "direct" }
"""
tt = {}
for method in ("direct", "fft"):
    mm = MANY.replace('method = "direct"', f'method = "{method}"')
    best = 1e9
    for _ in range(3):
        wt, dt = run(mm, f"method_{method}", {"pull.map": "pull.tif", "cleared.map": "cleared.tif"}, "--threads", "4")
        best = min(best, dt)
    tt[method] = (best, arr(wt / "pull.tif"))
    print(method, f"{best:.2f} s")
ns = np.count_nonzero(~np.isnan(arr(wt / "cleared.tif")))
d = np.nanmax(np.abs(tt["fft"][1] - tt["direct"][1])) / np.nanmax(tt["direct"][1])
print(f"{ns} sources; fft vs direct: largest difference {d:.2g} of the largest value (float32 files)")
print("images:", sum(p.stat().st_size for p in out.glob("*.png")) / 1e6, "MB")
