"""Figures of 'Systematic conservation planning: the ideas' (spatial-prioritization/systematic-conservation-planning).

Four sites and four species (complementarity, by Zonation and an exact plan), Serra Verde's costs and features,
the richest cells against an optimized plan for the same money, and a ranking beside a plan.
"""

import numpy as np
import rasterio
from rasterio.transform import from_origin

import sp_common as sp
from sp_common import figs, plt

SLUG = "systematic-conservation-planning"
OUT = sp.img(SLUG)
DAT = sp.data(SLUG)


# ------------------------------------------------------------------------------------------------ four sites
def toy():
    sites = ["A", "B", "C", "D"]
    occ = np.array([[1, 1, 0, 1],   # species 1 in A, B and D
                    [1, 1, 0, 0],   # species 2 in A and B
                    [1, 0, 0, 0],   # species 3 only in A
                    [0, 0, 1, 0]],  # species 4 only in C
                   dtype="float32")
    w = sp.work(SLUG, "toy")
    prof = dict(driver="GTiff", height=1, width=4, count=4, dtype="float32", crs="EPSG:3857",
                transform=from_origin(0, 1000, 1000, 1000), nodata=None)
    with rasterio.open(w / "species.tif", "w", **prof) as ds:
        for i in range(4):
            ds.write(occ[i][None, :], i + 1)
            ds.set_band_description(i + 1, f"species_{i + 1}")
    prof.update(count=1)
    with rasterio.open(w / "ones.tif", "w", **prof) as ds:
        ds.write(np.ones((1, 4), "float32"), 1)
    model = '''format = "terraflux/1"

[[nodes]]
id = "species"
op = "raster.read"
params = { path = "species.tif" }

[[nodes]]
id = "cost"
op = "raster.read"
params = { path = "ones.tif" }

[[nodes]]
id = "zonation"
op = "prioritize.zonation"
inputs = { features = "species.raster" }
params = { rule = "CAZMAX" }

[[nodes]]
id = "two_sites"
op = "prioritize.optimize"
inputs = { features = "species.raster", cost = "cost.raster" }
params = { objective = "max_cover", budget = 2, gap = 0 }
'''
    model = figs.add_writes(model, {"zonation.rank": "rank.tif", "two_sites.solution": "two.tif"})
    figs.run(model, w, "-q")
    rank = sp.grid(w / "rank.tif")[0]
    two = sp.grid(w / "two.tif")[0]
    rich = occ.sum(axis=0)
    print("toy: richness", rich, "rank", rank.round(3), "two sites", two)

    fig, axs = plt.subplots(1, 2, figsize=(8.4, 3.9), dpi=150)
    picks = [("Richest two sites", [0, 1]), ("Complementary two sites", [int(i) for i in np.flatnonzero(two > 0.5)])]
    for ax, (title, pick) in zip(axs, picks):
        ax.set_xlim(-0.6, 3.6)
        ax.set_ylim(-1.55, 3.6)
        ax.invert_yaxis()
        ax.set_aspect("equal")
        ax.set_axis_off()
        for j in range(4):
            if j in pick:
                ax.add_patch(plt.Rectangle((j - 0.45, -0.5), 0.9, 4.0, color="#2e7d4f22", lw=0))
            ax.text(j, -0.9, sites[j], ha="center", va="center", fontsize=13, fontweight="bold",
                    color=figs.INK if j in pick else figs.INK2)
            ax.text(j, -1.35, f"{int(rich[j])} sp.", ha="center", va="center", fontsize=9, color=figs.INK2)
        held = occ[:, pick].max(axis=1) > 0
        for i in range(4):
            ax.text(-0.55, i, f"Sp. {i + 1}", ha="right", va="center", fontsize=10,
                    color=figs.INK if held[i] else "#c0392b")
            for j in range(4):
                if occ[i, j]:
                    ax.add_patch(plt.Circle((j, i), 0.22, color=sp.FCOL[sp.FEATURES[i]] if j in pick else "#b9c0ba"))
        ax.set_title(f"{title}: {int(held.sum())} of 4 species", loc="left", fontsize=11.5, pad=14)
    fig.tight_layout()
    sp.save(fig, OUT / "complementarity.png")
    return rank, two


# ------------------------------------------------------------------------------------------------ Serra Verde
def landscape():
    cost = sp.grid(sp.DATA / "cost.tif")
    prot = sp.grid(sp.DATA / "protected.tif") > 0.5
    urb = sp.grid(sp.DATA / "urban.tif") > 0.5
    mask = sp.land()
    print("Serra Verde:", int(mask.sum()), "planning units; cost", np.nanmin(cost), np.nanmax(cost),
          "total", np.nansum(cost))
    fig, axs = plt.subplots(1, 2, figsize=(9.6, 3.6), dpi=150)
    cm = plt.get_cmap("magma_r").copy()
    cm.set_bad((1, 1, 1, 0))
    im = axs[0].imshow(np.ma.masked_invalid(cost), cmap=cm, interpolation="nearest")
    axs[0].set_title("Cost (thousand USD per km²)", loc="left", fontsize=11)
    cb = fig.colorbar(im, ax=axs[0], fraction=0.035, pad=0.02)
    cb.outline.set_visible(False)
    cb.ax.tick_params(labelsize=8.5)
    ctx = np.zeros(cost.shape + (4,))
    from matplotlib import colors as mc
    ctx[mask] = mc.to_rgba(sp.UNSELECTED)
    ctx[prot] = mc.to_rgba(sp.PROTECTED)
    ctx[urb] = mc.to_rgba(sp.URBAN)
    axs[1].imshow(ctx, interpolation="nearest")
    axs[1].set_title("Protected areas and cities", loc="left", fontsize=11)
    from matplotlib.patches import Patch
    axs[1].legend(handles=[Patch(color=sp.PROTECTED, label="Protected area"), Patch(color=sp.URBAN, label="City"),
                           Patch(color=sp.UNSELECTED, label="Other land")], loc="lower left", fontsize=8.5,
                  frameon=True, framealpha=0.9, edgecolor="none")
    for ax in axs:
        ax.set_axis_off()
    fig.tight_layout()
    sp.save(fig, OUT / "serra-verde.png")


def feature_maps():
    f = sp.features()
    sp.panels([f[k] for k in sp.FEATURES], [sp.NAMES[k] for k in sp.FEATURES], OUT / "features.png", cols=4,
              vmin=0, vmax=1, colorbar="Habitat suitability", width=1050)
    rows = []
    for k in sp.FEATURES:
        a = f[k]
        rows.append({"feature": sp.NAMES[k], "total": round(float(np.nansum(a)), 1),
                     "cells": int(np.nansum(a > 0))})
    return rows


# ------------------------------------------------------------------------------------------------ hotspots
def hotspots():
    plan_w = sp.run(SLUG, "plan", '''
[[nodes]]
id = "plan"
op = "prioritize.optimize"
inputs = { features = "features.stack", cost = "cost.raster" }
params = { objective = "min_set", relative_targets = 0.3, gap = 0.001 }

[outputs.summary]
from = "plan.summary"
''', {"plan.solution": "plan.tif"})
    s = sp.table(plan_w, "summary")[0]
    budget = s["cost"]
    plan = sp.grid(plan_w / "plan.tif")
    cost = sp.grid(sp.DATA / "cost.tif")
    f = sp.features()
    rich = sum(np.nan_to_num(f[k]) for k in sp.FEATURES)
    rich = np.where(np.isfinite(cost), rich, -np.inf)
    order = np.argsort(-rich, axis=None, kind="stable")
    spent = np.cumsum(np.nan_to_num(cost.ravel()[order]))
    hot = np.where(np.isfinite(cost), 0.0, np.nan)
    hot.ravel()[order[spent <= budget]] = 1.0
    print(f"plan: {int(s['selected'])} cells, cost {budget:.0f}; hotspots: {int(np.nansum(hot))} cells, "
          f"cost {np.nansum(cost * (hot > 0.5)):.0f}")

    # evaluate both with Terraflux
    w = sp.work(SLUG, "evaluate")
    with rasterio.open(sp.DATA / "cost.tif") as src:
        prof = src.profile
    for name, arr in (("hotspots.tif", hot), ("plan.tif", plan)):
        with rasterio.open(w / name, "w", **prof) as ds:
            ds.write(arr.astype("float32"), 1)
    model = sp.HEAD + sp.read_node("hot", "hotspots.tif") + sp.read_node("plan", "plan.tif") + '''
[[nodes]]
id = "eval_hot"
op = "prioritize.evaluate"
inputs = { features = "features.stack", cost = "cost.raster", solution = "hot.map" }
params = { relative_targets = 0.3 }

[[nodes]]
id = "eval_plan"
op = "prioritize.evaluate"
inputs = { features = "features.stack", cost = "cost.raster", solution = "plan.map" }
params = { relative_targets = 0.3 }

[outputs.hot]
from = "eval_hot.coverage"
[outputs.plan]
from = "eval_plan.coverage"
[outputs.hot_summary]
from = "eval_hot.summary"
[outputs.plan_summary]
from = "eval_plan.summary"
'''
    figs.run(model, w, "-q")
    ch, cp = sp.table(w, "hot"), sp.table(w, "plan")
    rows = []
    for k, a, b in zip(sp.FEATURES, ch, cp):
        rows.append({"feature": sp.SHORT[k], "Richest cells": round(100 * a["relative_held"], 1),
                     "Optimized plan": round(100 * b["relative_held"], 1), "Target": 30})
    sp.write_csv(DAT / "hotspots.csv", rows)
    print("hotspots met", sum(int(r["met"]) for r in ch), "plan met", sum(int(r["met"]) for r in cp))
    print(rows)
    print(sp.table(w, "hot_summary"), sp.table(w, "plan_summary"))

    fig, axs = plt.subplots(1, 2, figsize=(9.6, 3.5), dpi=150)
    for ax, arr, t in ((axs[0], hot, "A. The richest cells"), (axs[1], plan, "B. An optimized plan")):
        ax.imshow(sp.rgba_of(arr), interpolation="nearest")
        ax.set_axis_off()
        ax.set_title(t, loc="left", fontsize=11.5)
    fig.text(0.5, 0.01, f"Both cost {budget / 1000:.1f} million USD", ha="center", fontsize=9.5, color=figs.INK2)
    fig.tight_layout(rect=(0, 0.04, 1, 1))
    sp.save(fig, OUT / "hotspots-vs-plan.png")
    return budget, rows, ch, cp, s


def families():
    w = sp.run(SLUG, "families", '''
[[nodes]]
id = "zonation"
op = "prioritize.zonation"
inputs = { features = "features.stack", cost = "cost.raster" }

[[nodes]]
id = "plan"
op = "prioritize.optimize"
inputs = { features = "features.stack", cost = "cost.raster" }
params = { objective = "min_set", relative_targets = 0.3, gap = 0.001 }
''', {"zonation.rank": "rank.tif", "plan.solution": "plan.tif"})
    rank = sp.grid(w / "rank.tif")
    plan = sp.grid(w / "plan.tif")
    fig, axs = plt.subplots(1, 2, figsize=(9.6, 3.7), dpi=150)
    cm = plt.get_cmap("viridis").copy()
    cm.set_bad((1, 1, 1, 0))
    im = axs[0].imshow(np.ma.masked_invalid(rank), cmap=cm, vmin=0, vmax=1, interpolation="nearest")
    axs[0].set_title("Ranking (Zonation): a priority per cell", loc="left", fontsize=11)
    cb = fig.colorbar(im, ax=axs[0], fraction=0.035, pad=0.02)
    cb.outline.set_visible(False)
    cb.ax.tick_params(labelsize=8.5)
    axs[1].imshow(sp.rgba_of(plan), interpolation="nearest")
    axs[1].set_title("Optimization (prioritizr): one plan", loc="left", fontsize=11)
    for ax in axs:
        ax.set_axis_off()
    fig.tight_layout()
    sp.save(fig, OUT / "two-families.png")


def targets():
    """The cheapest plan for targets of 10 % to 50 %: frames of a slider and the cost curve."""
    rows, frames = [], []
    for t in (10, 15, 20, 25, 30, 35, 40, 45, 50):
        w = sp.run(SLUG, f"target_{t}", f'''
[[nodes]]
id = "plan"
op = "prioritize.optimize"
inputs = {{ features = "features.stack", cost = "cost.raster" }}
params = {{ objective = "min_set", relative_targets = {t / 100}, gap = 0.005 }}

[outputs.summary]
from = "plan.summary"
''', {"plan.solution": "plan.tif"})
        s = sp.table(w, "summary")[0]
        sel = sp.grid(w / "plan.tif")
        sp.plan_map(sel, OUT / f"target_{t}.png", legend=[], width=760)
        rows.append({"target": t, "cost": round(s["cost"] / 1000, 2), "area": round(100 * s["selected"] / 4825, 1)})
        print("target", t, s)
    sp.write_csv(DAT / "targets.csv", rows)
    return rows


if __name__ == "__main__":
    toy()
    landscape()
    print(feature_maps())
    hotspots()
    families()
    print(targets())
    sp.serra_verde_zip(SLUG)
