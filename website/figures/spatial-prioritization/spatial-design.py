"""Figures of 'Compact and connected plans, and zones' (spatial-prioritization/spatial-design).

prioritize.optimize on Serra Verde with a boundary penalty (a slider over its value, boundaries from
prioritize.evaluate), neighbour and contiguity constraints on blocks of 3 x 3 cells (prioritize.coarsen and refine),
a connectivity penalty along the tapir's habitat (prioritize.connectivity_matrix), H3 hexagons as planning units
(prioritize.h3_units) and two management zones.

    python website/figures/spatial-prioritization/spatial-design.py [boundary rules connectivity hexagons zones]
"""

import time

import numpy as np
from matplotlib import colors as mcolors
from matplotlib.patches import Patch
from scipy import ndimage

import sp_common as sp
from sp_common import figs, plt

SLUG = "spatial-design"
OUT = sp.img(SLUG)
DAT = sp.data(SLUG)
OPTIMUM = 37141.1
BLOCKS = '''
[[nodes]]
id = "blocks"
op = "prioritize.coarsen"
inputs = { cost = "cost.map", features = "features.stack" }
params = { factor = 3 }
'''


def patches(sel):
    return ndimage.label(sel > 0.5)[1]


# ------------------------------------------------------------------------------------------------ boundary
PENALTIES = [0, 0.001, 0.002, 0.005, 0.01, 0.02, 0.05]


def boundary():
    rows = []
    for p in PENALTIES:
        pen = f", boundary_penalty = {p}" if p else ""
        t = time.time()
        w = sp.run(SLUG, f"bp_{p}", f'''
[[nodes]]
id = "plan"
op = "prioritize.optimize"
inputs = {{ features = "features.stack", cost = "cost.map" }}
params = {{ objective = "min_set", relative_targets = 0.3, gap = 0.01{pen} }}

[[nodes]]
id = "evaluation"
op = "prioritize.evaluate"
inputs = {{ features = "features.stack", cost = "cost.map", solution = "plan.solution" }}
params = {{ relative_targets = 0.3 }}

[outputs.summary]
from = "plan.summary"
[outputs.eval]
from = "evaluation.summary"
''', {"plan.solution": "solution.tif"})
        secs = time.time() - t
        s, e = sp.table(w, "summary")[0], sp.table(w, "eval")[0]
        sol = sp.grid(w / "solution.tif")
        sp.plan_map(sol, OUT / f"boundary_{p}.png", legend=[], width=760)
        r = {"penalty": p, "cost": s["cost"] / 1000, "boundary": e["boundary"] / 1000, "cells": int(s["selected"]),
             "patches": patches(sol), "gap": s["gap"], "secs": secs}
        rows.append(r)
        print({k: (round(v, 4) if isinstance(v, float) else v) for k, v in r.items()}, "eval", e)
    sp.write_csv(DAT / "boundary.csv", [{"penalty": str(r["penalty"]) if r["penalty"] else "none",
                                         "Cost (% above the optimum)": round(100 * (r["cost"] * 1000 / OPTIMUM - 1), 1),
                                         "Boundary (% of no penalty)": round(100 * r["boundary"] / rows[0]["boundary"], 1)}
                                        for r in rows])
    sp.write_csv(DAT / "boundary-tradeoff.csv", [{"Boundary (km)": round(r["boundary"]), "Cost (million USD)":
                                                  round(r["cost"], 2)} for r in rows])


# ------------------------------------------------------------------------------------------------ rules on blocks
def blocks_plan(name, params, extra_inputs="", extra_nodes="", target=0.2, gap=0.01, tl=None):
    tlim = f", time_limit = {tl}" if tl else ""
    t = time.time()
    w = sp.run(SLUG, name, BLOCKS + extra_nodes + f'''
[[nodes]]
id = "plan"
op = "prioritize.optimize"
inputs = {{ features = "blocks.features", cost = "blocks.cost"{extra_inputs} }}
params = {{ objective = "min_set", relative_targets = {target}, gap = {gap}{tlim}{params} }}

[[nodes]]
id = "fine"
op = "prioritize.refine"
inputs = {{ selection = "plan.solution", like = "cost.map" }}

[outputs.summary]
from = "plan.summary"
''', {"fine.map": "solution.tif", "plan.solution": "coarse.tif"})
    secs = time.time() - t
    s = sp.table(w, "summary")[0]
    return w, s, secs


def rules():
    out = []
    for name, params, tl, gap in (("plain", "", None, 0.01), ("neighbors", ", neighbors = 2", 120, 0.01),
                                  ("contiguity", ", contiguity = true", 180, 0.05)):
        w, s, secs = blocks_plan(f"rule_{name}", params, tl=tl, gap=gap)
        coarse = sp.grid(w / "coarse.tif")
        fine = sp.grid(w / "solution.tif")
        out.append((name, fine))
        print(name, s, f"{secs:.1f} s", "blocks", int(np.nansum(coarse > 0.5)), "patches (4-conn, blocks)",
              ndimage.label(coarse > 0.5)[1])
    titles = {"plain": "No rule", "neighbors": "At least 2 selected neighbours", "contiguity": "One contiguous network"}
    sp.panels([f for _, f in out], [titles[n] for n, _ in out], OUT / "rules.png", cols=3, kinds=["plan"] * 3,
              width=1050)


# ------------------------------------------------------------------------------------------------ connectivity
def connectivity():
    tapir = '''
[[nodes]]
id = "tapir"
op = "raster.calc"
inputs = { f = "blocks.features" }
params = { expression = "f.lowland_tapir" }

[[nodes]]
id = "links"
op = "prioritize.connectivity_matrix"
inputs = { units = "blocks.cost", y = "tapir.map" }
'''
    res = {}
    for p in (0, 0.005, 0.02):
        pen = f", connectivity_penalty = {p}" if p else ""
        w, s, secs = blocks_plan(f"conn_{p}", pen, ', connectivity_data = "links.table"', tapir)
        # the connectivity held: evaluate the plan with the matrix
        w2 = sp.run(SLUG, f"conn_eval_{p}", BLOCKS + tapir + f'''
[[nodes]]
id = "coarse"
op = "raster.read"
params = {{ path = "{(w / 'coarse.tif').as_posix()}" }}

[[nodes]]
id = "evaluation"
op = "prioritize.evaluate"
inputs = {{ features = "blocks.features", cost = "blocks.cost", solution = "coarse.raster", connectivity_data = "links.table" }}
params = {{ relative_targets = 0.2 }}

[outputs.eval]
from = "evaluation.summary"
''')
        e = sp.table(w2, "eval")[0]
        fine = sp.grid(w / "solution.tif")
        res[p] = fine
        print("connectivity", p, s, e, f"{secs:.1f} s")
        tapir_plan(fine, OUT / f"connectivity_{p}.png")


def tapir_plan(sel, png):
    """A plan over the tapir's habitat: unselected land shaded by the tapir's suitability, selected cells green."""
    t = np.nan_to_num(sp.features()["lowland_tapir"])
    m = sp.land()
    rgba = np.zeros(t.shape + (4,))
    shade = plt.get_cmap("YlOrBr")(0.05 + 0.55 * t)
    rgba[m] = shade[m]
    rgba[m & (np.nan_to_num(sel) > 0.5)] = mcolors.to_rgba(sp.SELECTED)
    fig = plt.figure(figsize=(760 / 150, 760 / 150 * 60 / 90), dpi=150)
    ax = fig.add_axes([0, 0, 1, 1])
    ax.set_axis_off()
    ax.imshow(rgba, interpolation="nearest")
    sp.save(fig, png)


# ------------------------------------------------------------------------------------------------ hexagons
def hexagons():
    import geopandas as gpd
    for res in (7, 6):
        w = sp.run(SLUG, f"h3_{res}", f'''
[[nodes]]
id = "hexagons"
op = "prioritize.h3_units"
inputs = {{ cost = "cost.map" }}
params = {{ resolution = {res} }}

[[nodes]]
id = "plan"
op = "prioritize.optimize"
inputs = {{ polygons = "hexagons.units", features = "features.stack" }}
params = {{ objective = "min_set", relative_targets = 0.3, cost_column = "cost", gap = 0.01 }}

[[nodes]]
id = "write"
op = "vector.write"
inputs = {{ vector = "hexagons.units" }}
params = {{ path = "hexagons.gpkg" }}

[outputs.selection]
from = "plan.selection"
[outputs.summary]
from = "plan.summary"
''')
        hexes = gpd.read_file(w / "hexagons.gpkg")
        sel = sp.table(w, "selection")
        s = sp.table(w, "summary")[0]
        hexes["solution"] = [r["solution"] for r in sel]
        print("h3", res, len(hexes), "mean km2", round(hexes.geometry.area.mean() / 1e6, 2), s)
        draw_hexes(hexes, OUT / f"hexagons_{res}.png")


def draw_hexes(hexes, png):
    import rasterio
    with rasterio.open(sp.DATA / "cost.tif") as ds:
        b = ds.bounds
    fig = plt.figure(figsize=(900 / 150, 900 / 150 * 0.72 * 60 / 90 * 1.08), dpi=150)
    ax = fig.add_axes([0, 0, 0.72, 1])
    ax.set_axis_off()
    land = sp.land()
    rgba = np.zeros(land.shape + (4,))
    rgba[land] = mcolors.to_rgba("#f4f2ec")
    ax.imshow(rgba, extent=[b.left, b.right, b.bottom, b.top], interpolation="nearest")
    hexes[hexes["solution"] <= 0.5].plot(ax=ax, facecolor=sp.UNSELECTED, edgecolor="white", linewidth=0.4)
    hexes[hexes["solution"] > 0.5].plot(ax=ax, facecolor=sp.SELECTED, edgecolor="white", linewidth=0.4)
    ax.set_xlim(b.left, b.right)
    ax.set_ylim(b.bottom, b.top)
    fig.legend(handles=[Patch(facecolor=sp.SELECTED, label="Selected"), Patch(facecolor=sp.UNSELECTED, label="Not selected")],
               loc="center left", bbox_to_anchor=(0.73, 0.5), fontsize=9.5)
    sp.save(fig, png)


# ------------------------------------------------------------------------------------------------ zones
ZONE_COLORS = {1: ("Strict protection", "#2e7d4f"), 2: ("Sustainable use", "#a7c957")}


def zones():
    manual = ", ".join(f'{{ feature = "{f}", type = "relative", sense = ">=", target = 0.3 }}, '
                       f'{{ feature = "{f}", zone = "protect", type = "relative", sense = ">=", target = 0.2 }}'
                       for f in sp.FEATURES)
    for name, tg in (("zones", "relative_targets = 0.3"), ("zones_strict", f"manual_targets = [{manual}]")):
        w = sp.run(SLUG, name, f'''
[[nodes]]
id = "use_cost"
op = "raster.calc"
inputs = {{ c = "cost.map" }}
params = {{ expression = "c * 0.45" }}

[[nodes]]
id = "zone_costs"
op = "stack.of"
inputs = {{ protect = "cost.map", use = "use_cost.map" }}

[[nodes]]
id = "plan"
op = "prioritize.optimize"
inputs = {{ zone_costs = "zone_costs.stack", features = "features.stack" }}
params = {{ objective = "min_set", {tg}, contribution = {{ use = 0.5 }}, gap = 0.01 }}

[outputs.summary]
from = "plan.summary"
[outputs.coverage]
from = "plan.coverage"
''', {"plan.solution": "zones.tif"})
        z = sp.grid(w / "zones.tif")
        z = np.where(sp.land(), z, np.nan)
        s = sp.table(w, "summary")[0]
        u, c = np.unique(z[np.isfinite(z)], return_counts=True)
        print(name, s, dict(zip(u.tolist(), c.tolist())))
        cov = sp.table(w, "coverage")
        print("coverage", [(r.get("feature"), r.get("zone", ""), round(r["held"], 1), round(r["target"], 1)) for r in cov][:20])
        sp.plan_map((z == 1).astype(float), OUT / f"{name}.png",
                    legend=[(ZONE_COLORS[1][0], ZONE_COLORS[1][1]), (ZONE_COLORS[2][0], ZONE_COLORS[2][1]),
                            ("No zone", sp.UNSELECTED)],
                    extra=[(z == 2, ZONE_COLORS[2][1])], on=ZONE_COLORS[1][1])


if __name__ == "__main__":
    import sys
    for t in sys.argv[1:] or ["boundary", "rules", "connectivity", "hexagons", "zones"]:
        print(t, globals()[t]())
