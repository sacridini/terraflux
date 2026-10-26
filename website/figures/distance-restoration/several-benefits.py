"""Figures of tutorials/distance-restoration/several-benefits: restoration plans for species, carbon and intactness
weighed against each other, ecosystems' risk of collapse, budgets and targets per region (Serra Verde)."""

import numpy as np

import _common as C
from _common import figs
from _habitat_model import HABITAT

SLUG = "distance-restoration/several-benefits"
out = figs.img_dir(SLUG)
data = figs.data_dir(SLUG)
FILES = ("land_cover", "past_cover", "features", "ecoregions", "carbon_potential", "footprint", "cost", "regions")

MORE = """
[[nodes]]
id = "carbon_t_ha"
op = "raster.read"
params = { path = "carbon_potential.tif" }

[[nodes]]
id = "carbon"
op = "raster.calc"
inputs = { c = "carbon_t_ha.raster" }
params = { expression = "c * 100 / 1000000" }

[[nodes]]
id = "linear"
op = "stack.of"
inputs = { carbon = "carbon.map" }

[[nodes]]
id = "footprint"
op = "raster.read"
params = { path = "footprint.tif" }

[[nodes]]
id = "pasture"
op = "raster.read"
params = { path = "land_cover.tif", band = "pasture" }

[[nodes]]
id = "cropland"
op = "raster.read"
params = { path = "land_cover.tif", band = "cropland" }

[[nodes]]
id = "reduction"
op = "raster.calc"
inputs = { p = "pasture.raster", c = "cropland.raster" }
params = { expression = "(4 * p + 7 * c) / (p + c) if p + c > 0 else 0" }

[[nodes]]
id = "cost"
op = "raster.read"
params = { path = "cost.tif" }

[[nodes]]
id = "regions"
op = "raster.read"
params = { path = "regions.tif" }

[[nodes]]
id = "ecosystems"
op = "restoration.ecosystem_areas"
inputs = { land_cover = "cover.stack", ecoregions = "ecoregions.map" }
params = { natural = ["forest", "savanna", "mangrove"], anthropic = ["pasture", "cropland"] }

[[nodes]]
id = "eco_targets"
op = "restoration.ecosystem_targets"
inputs = { table = "ecosystems.table" }
params = { target = 100 }
"""

PLAN = """
[[nodes]]
id = "plan"
op = "restoration.plan"
inputs = { INPUTS }
params = { PARAMS }
"""
SPECIES_IN = 'restorable = "vegetation.anthropic", table = "habitat.table", gain = "habitat.gain"'
CARBON_IN = 'restorable = "vegetation.anthropic", linear = "linear.stack"'
INTACT_IN = 'restorable = "vegetation.anthropic", hfi = "footprint.raster", reduction = "reduction.map"'
ECO_IN = 'restorable = "vegetation.anthropic", table = "ecosystems.table", gain = "ecosystems.gain"'
ALL_IN = ('restorable = "vegetation.anthropic", table = "habitat.table", gain = "habitat.gain", linear = "linear.stack", '
          'hfi = "footprint.raster", reduction = "reduction.map"')
SC_IN = 'restorable = "vegetation.anthropic", table = "habitat.table", gain = "habitat.gain", linear = "linear.stack"'
W = 700


def run(name, inputs, params, writes=None):
    w = figs.workdir(f"{SLUG}/{name}")
    C.serra(w, *FILES)
    m = HABITAT + MORE + PLAN.replace("INPUTS", inputs).replace("PARAMS", params)
    wr = {"plan.summary": "summary.csv", "plan.benefits": "benefits.csv", "plan.restored": "restored.tif"}
    wr.update(writes or {})
    figs.run(figs.add_writes(m, wr), w)
    (figs.WEB / ".cache" / "work" / SLUG / f"{name}.toml").write_text(m, encoding="utf-8")
    return w, C.table(w, "summary")[0], [r["value"] for r in C.table(w, "benefits")]


def plan_map(w, png, legend=True):
    r = figs.read(w / "restored.tif")[0]
    C.share_map(r, png, width=W)
    return r


# ------------------------------------------------------------------ each benefit alone, 100 km2
w_s, s_s, b_s = run("species", SPECIES_IN, "target = 100")
w_c, s_c, b_c = run("carbon", CARBON_IN, "target = 100",
                    {"carbon_t_ha.raster": "carbon.tif", "ecosystems.table": "eco.csv", "eco_targets.targets": "eco_targets.csv"})
w_i, s_i, b_i = run("intactness", INTACT_IN, "target = 100")
w_e, s_e, b_e = run("ecosystems", ECO_IN, "target = 100")
best = {"species": b_s[0], "carbon": b_c[0], "intactness": b_i[0], "ecosystems": b_e[0]}
print("best alone", best)
for name, w in (("species", w_s), ("carbon", w_c), ("intactness", w_i), ("ecosystems", w_e)):
    plan_map(w, out / f"alone_{name}.png", legend=False)
ct = figs.read(w_c / "carbon.tif")[0]
figs.raster(w_c / "carbon.tif", out / "carbon.png", cmap="YlGn", unit="carbon of restored vegetation (t/ha)", width=W)
print("ecosystems", C.table(w_c, "eco"))
print("ecosystem targets", C.table(w_c, "eco_targets"))
figs.write_csv(data / "ecosystems.csv",
               [{"ecoregion": {1: "Coast", 2: "Lowland forest", 3: "Montane forest", 4: "Savanna"}[int(r["code"])],
                 "current": round(r["current"], 1), "potential": round(r["potential"], 1),
                 "left": round(r["current"] / r["potential"] * 100, 1),
                 "target": round(t["target"], 1)}
                for r, t in zip(C.table(w_c, "eco"), C.table(w_c, "eco_targets"))])

# how each single-benefit plan fares on the others: evaluate by giving all benefits weight ~0 except one is not
# possible; instead solve the combined problem with one weight 1 and the others 1e-9
cross = []
for name in ("species", "carbon", "intactness"):
    ws = {"species": 1e-9, "carbon": 1e-9, "intactness": 1e-9}
    ws[name] = 1 / best[name]
    _, s, b = run(f"cross_{name}", ALL_IN, f"target = 100, weight = {ws['species']}, linear_weights = [{ws['carbon']}], "
                  f"intactness_weight = {ws['intactness']}")
    cross.append({"plan": f"{name} alone", "species": round(b[0] / best["species"] * 100, 1),
                  "carbon": round(b[1] / best["carbon"] * 100, 1), "intactness": round(b[2] / best["intactness"] * 100, 1)})
w_a, s_a, b_a = run("all", ALL_IN, f"target = 100, weight = {1 / best['species']}, "
                    f"linear_weights = [{1 / best['carbon']}], intactness_weight = {1 / best['intactness']}")
cross.append({"plan": "all three", "species": round(b_a[0] / best["species"] * 100, 1),
              "carbon": round(b_a[1] / best["carbon"] * 100, 1), "intactness": round(b_a[2] / best["intactness"] * 100, 1)})
plan_map(w_a, out / "all_three.png")
print("cross", cross)
figs.write_csv(data / "cross.csv", cross)

# ------------------------------------------------------------------ the weight of species against carbon
frames = []
for wt in (0.0, 0.1, 0.2, 0.3, 0.5, 0.7, 0.9, 1.0):
    ws, wc = wt / best["species"], (1 - wt) / best["carbon"]
    w, s, b = run(f"w{wt}", SC_IN, f"target = 100, weight = {ws}, linear_weights = [{wc}]")
    plan_map(w, out / f"weight_{wt}.png")
    frames.append({"w": wt, "species": round(b[0] / best["species"] * 100, 1),
                   "carbon": round(b[1] / best["carbon"] * 100, 1)})
print("weights", frames)
figs.write_csv(data / "weights.csv", frames)

# ------------------------------------------------------------ the sets of weights compared (Strassburg et al. 2020)
n_s, n_c, n_i = (float(f"{1 / best[k]:.3g}") for k in ("species", "carbon", "intactness"))
print("normalising weights", n_s, n_c, n_i)
combos = [(1, 0, 0), (0, 1, 0), (0, 0, 1), (1, 1, 1), (2, 1, 1), (1, 1, 0)]
sets = "weights = [" + ", ".join(
    f"{{ extinction_risk = {a * n_s:.4g}, carbon = {b * n_c:.4g}, intactness = {c * n_i:.4g} }}" for a, b, c in combos) + "]"
print(sets)
pre = "target = 100, "
w_w, s_w, b_w = run("sets", ALL_IN, pre + sets, {"plan.weights": "weights.csv"})
wt = C.table(w_w, "weights")
for r in wt:
    print({k: round(v, 4) if isinstance(v, float) else v for k, v in r.items()})
figs.write_csv(data / "sets.csv", [{"set": "%d-%d-%d" % combos[i], "species": round(r["extinction_risk"] / best["species"] * 100, 1),
                                    "carbon": round(r["carbon"] / best["carbon"] * 100, 1),
                                    "intactness": round(r["intactness"] / best["intactness"] * 100, 1),
                                    "score": round(r["score"], 3), "rank": r["rank"]} for i, r in enumerate(wt)])

# ------------------------------------------------------------------ budgets
brows = []
for budget in (1000, 2000, 4000, 6000, 8000, 10000, 12000):
    w, s, b = run(f"budget{budget}", SPECIES_IN.replace("restorable", 'cost = "cost.raster", restorable'),
                  f"target = 100, budget = {budget}")
    brows.append({"budget": budget / 1000, "area": round(s["area"], 1), "reduction": round(s["risk_reduction"], 4),
                  "cost": round(s["cost"] / 1000, 2)})
print("budgets", brows)
figs.write_csv(data / "budget.csv", brows)
cost_first = float(np.nansum(figs.read(w_s / "restored.tif")[0].filled(0) *
                             figs.read(figs.WEB / ".cache/work" / SLUG / "budget1000/cost.tif")[0].filled(0)))
w_m, s_m, b_m = run("mincost", SPECIES_IN.replace("restorable", 'cost = "cost.raster", restorable'),
                    f"target = 100, min_benefit = {0.95 * best['species']}")
print("first plan cost", cost_first, "min cost for 95%", s_m)
plan_map(w_m, out / "cheapest.png")
figs.raster(w_m / "cost.tif", out / "cost.png", cmap="cividis", unit="cost (thousand USD per km²)", width=W)

# ------------------------------------------------------------------ targets per region
w_r, s_r, b_r = run("regions", SPECIES_IN.replace("restorable", 'regions = "regions.raster", restorable'),
                    'target = { 1 = 30, 2 = 40, 3 = 30 }')
r0 = figs.read(w_s / "restored.tif")[0].filled(0)
r1 = figs.read(w_r / "restored.tif")[0].filled(0)
reg = figs.read(w_r / "regions.tif")[0].filled(0)
print("regions", [(k, round(float(r0[reg == k].sum()), 1), round(float(r1[reg == k].sum()), 1)) for k in (1, 2, 3)],
      "benefit", b_r[0], "vs", best["species"])
plan_map(w_r, out / "regions_plan.png")
figs.raster(w_r / "regions.tif", out / "regions.png",
            palette={1: ("Region 1", "#cfe3c4"), 2: ("Region 2", "#9cc0d9"), 3: ("Region 3", "#e9c99a")}, width=W)
figs.write_csv(data / "regions.csv", [{"region": k, "without": round(float(r0[reg == k].sum()), 1),
                                       "with": round(float(r1[reg == k].sum()), 1)} for k in (1, 2, 3)])
print("done")
