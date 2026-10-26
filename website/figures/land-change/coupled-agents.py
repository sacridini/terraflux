"""Figures of "A coupled model: macro agents, parcel managers and CRAFTY" (land-change/coupled-agents).

MODEL is the coupled model of docs/agents.md and crates/exec/tests/abm.rs, its inputs drawn in the model as the
test draws them (a 40 x 40 grid; fertility rising to the east, wood to the north, four parcels, the quadrants;
nobody managing the land at the start): macro agents (a population growing by 2% a step) give the demand for
food and timber, parcel managers decide subsidies that raise the fertility of their parcels, and CRAFTY's farmers
and foresters compete for the cells. Runs: with the first manager's subsidy and without it, other growth rates and
other subsidies.

COUNCILS puts agents in lulcc's CLUE-S model of Plum Island (part 4 of the CLUE-S series): two councils, the west
and the east of the map, the east freezing its land from step 5 (agents.map as lulcc.step's restrictions).

    python website/figures/land-change/coupled-agents.py
"""

import pathlib
import shutil
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[2] / "tools"))
import figs  # noqa: E402
import matplotlib.pyplot as plt  # noqa: E402
import numpy as np  # noqa: E402
from matplotlib.patches import FancyArrowPatch, FancyBboxPatch  # noqa: E402

SLUG = "land-change/coupled-agents"
out = figs.img_dir(SLUG)
data = figs.data_dir(SLUG)
COVER = {0: ("Unmanaged", "#e9e4d8"), 2: ("Farmers", "#d9a441"), 3: ("Foresters", "#3b8a5a")}
PIE = {1: ("Forest", "#3b8a5a"), 2: ("Built", "#b4508a"), 3: ("Other", "#d9a441")}

MODEL = """format = "terraflux/1"
name = "Agents and land: a coupled model"

[[nodes]]
id = "east"
op = "nlm.planar_gradient"
params = { rows = 40, columns = 40, direction = 90 }

[[nodes]]
id = "north"
op = "nlm.planar_gradient"
params = { rows = 40, columns = 40, direction = 0 }

[[nodes]]
id = "fertility"
op = "raster.calc"
inputs = { e = "east.map" }
params = { expression = "0.0125 + 0.975 * e" }

[[nodes]]
id = "wood"
op = "raster.calc"
inputs = { n = "north.map" }
params = { expression = "0.0125 + 0.975 * n" }

[[nodes]]
id = "capitals"
op = "stack.of"
inputs = { fertility = "fertility.map", wood = "wood.map" }

[[nodes]]
id = "parcels"
op = "raster.calc"
inputs = { e = "east.map", n = "north.map" }
params = { expression = "1 + (e > 0.5) + 2 * (n < 0.5)" }

[[nodes]]
id = "land"
op = "raster.calc"
inputs = { e = "east.map" }
params = { expression = "e * 0" }

[[nodes]]
id = "people"
op = "agents.table"
params = { agents = [{ id = 1, population = 1000, food_per_head = 0.25, timber_per_head = 0.15 }] }

[[nodes]]
id = "managers"
op = "agents.parcels"
inputs = { parcels = "parcels.map" }

[[nodes]]
id = "simulation"
op = "control.repeat"
inputs = { times = 30, land = "land.map", people = "people.agents" }

[nodes.body]
carry = { land = "alloc.land", people = "grow.agents" }
collect = { cover = "alloc.cover", info = "alloc.info", demand = "need.table" }

[[nodes.body.nodes]]
id = "grow"
op = "agents.update"
inputs = { agents = "@people" }
params = { rules = "population = population * 1.02\\nfood = population * food_per_head\\ntimber = population * timber_per_head" }

[[nodes.body.nodes]]
id = "need"
op = "agents.summary"
inputs = { agents = "grow.agents" }

[[nodes.body.nodes]]
id = "decide"
op = "agents.update"
inputs = { agents = "managers.agents", step = "@step" }
params = { rules = "subsidy = 3 if id == 1 and step > 5 else 1" }

[[nodes.body.nodes]]
id = "boost"
op = "agents.map"
inputs = { agents = "decide.agents", parcels = "parcels.map" }
params = { column = "subsidy" }

[[nodes.body.nodes]]
id = "fert"
op = "raster.calc"
inputs = { caps = "capitals.stack", boost = "boost.map" }
params = { expression = "min(caps.fertility * boost, 1)" }

[[nodes.body.nodes]]
id = "caps"
op = "stack.set"
inputs = { stack = "capitals.stack", map = "fert.map" }
params = { name = "fertility" }

[[nodes.body.nodes]]
id = "alloc"
op = "crafty.allocate"
inputs = { land = "@land", capitals = "caps.stack", demand = "need.table", step = "@step", seed = 3 }
params = { services = { food = "food_sum", timber = "timber_sum" }, search = 0.2, giving_up_rate = 0.2, afts = [
  { name = "farmer", class = 2, production = { food = 1.0 }, sensitivity = { fertility = 2.0 }, giving_up = 0.05, giving_in = 0.1 },
  { name = "forester", class = 3, production = { timber = 1.0 }, sensitivity = { wood = 1.0 }, giving_up = 0.05, giving_in = 0.1 },
] }

[outputs.info]
from = "simulation.info"

[outputs.demand]
from = "simulation.demand"
"""
RULE = "subsidy = 3 if id == 1 and step > 5 else 1"
GROWTH = "population = population * 1.02"

COUNCILS = """format = "terraflux/1"

[inputs.observed]
kind = "stack"
path = "pie_landuse.tif"

[inputs.start]
kind = "map"
path = "pie_landuse.tif"
band = "1985"

[inputs.drivers]
kind = "stack"
path = "pie_drivers.tif"

[[nodes]]
id = "regression"
op = "glm.fit"
inputs = { landscape = "$start", drivers = "$drivers" }
params = { formulas = { 1 = ["elevation", "slope"], 2 = ["elevation", "slope", "dist_built_1985"], 3 = ["elevation", "slope"] } }

[[nodes]]
id = "suitability"
op = "glm.predict"
inputs = { model = "regression.model", drivers = "$drivers", landscape = "$start" }

[[nodes]]
id = "demand"
op = "demand.from_maps"
inputs = { maps = "$observed" }
params = { times = [1985, 1991, 1999] }

[[nodes]]
id = "rng"
op = "rng.r"
inputs = { seed = 1 }

[[nodes]]
id = "history"
op = "lulcc.history"
inputs = { landscape = "$start" }

[[nodes]]
id = "west_east"
op = "nlm.planar_gradient"
inputs = { like = "$start" }
params = { direction = 90 }

[[nodes]]
id = "towns"
op = "raster.calc"
inputs = { g = "west_east.map" }
params = { expression = "1 + (g > 0.5)" }

[[nodes]]
id = "councils"
op = "agents.parcels"
inputs = { parcels = "towns.map" }

[[nodes]]
id = "simulation"
op = "control.repeat"
inputs = { times = "demand.steps", landscape = "$start", history = "history.history" }

[nodes.body]
carry = { landscape = "allocate.landscape", history = "allocate.history" }
collect = { maps = "allocate.landscape", info = "allocate.info" }

[[nodes.body.nodes]]
id = "zoning"
op = "agents.update"
inputs = { agents = "councils.agents", step = "@step" }
params = { rules = "frozen = 1 if id == 2 and step > 4 else 0" }

[[nodes.body.nodes]]
id = "frozen"
op = "agents.map"
inputs = { agents = "zoning.agents", parcels = "towns.map" }
params = { column = "frozen" }

[[nodes.body.nodes]]
id = "allocate"
op = "lulcc.step"
inputs = { landscape = "@landscape", history = "@history", suitability = "suitability.suitability", demand = "demand.demand", step = "@step", rng = "rng.rng", restrictions = "frozen.map" }
params = { allocation = { method = "clue", jitter = 0.0002, scale = 0.000001, max_iter = 1000, max_diff = 50, mean_diff = 50 }, elasticity = [0.2, 0.2, 0.2] }

[outputs.maps]
from = "simulation.maps"

[outputs.steps]
from = "simulation.info"
"""


def run(name, model):
    work = figs.workdir(f"{SLUG}/{name}")
    figs.run(figs.add_writes(model, {"simulation.cover": "cover.tif", "parcels.map": "parcels.tif",
                                     "capitals.stack": "capitals.tif"}), work)
    info = figs.read_csv(work / "output" / "info.csv")
    rows = []
    for k in range(0, len(info), 2):
        f, t = info[k], info[k + 1]
        rows.append({"step": k // 2 + 1, "food_demand": round(float(f["demand"]), 1),
                     "food_supply": round(float(f["supply"]), 1), "timber_demand": round(float(t["demand"]), 1),
                     "timber_supply": round(float(t["supply"]), 1)})
    return work, rows


def cover_counts(work, band):
    a = figs.read(work / "cover.tif", band)[0]
    return {name: int((a == v).sum()) for v, (name, _) in COVER.items()}


def schematic():
    """Who drives whom in the coupled model: three levels of agents and the land."""
    fig, ax = plt.subplots(figsize=(7.6, 3.6), dpi=150)
    ax.set_xlim(0, 10)
    ax.set_ylim(0, 5)
    ax.set_axis_off()
    boxes = {
        "people": (0.2, 3.4, 3.0, 1.2, "Macro agents\n(the population)", "#cfe0ef"),
        "managers": (0.2, 0.4, 3.0, 1.2, "Parcel managers\n(four, one per quadrant)", "#f3dcc0"),
        "crafty": (6.6, 1.9, 3.2, 1.2, "CRAFTY\nfarmers and foresters", "#d6e6d2"),
    }
    for x, y, w, h, t, c in boxes.values():
        ax.add_patch(FancyBboxPatch((x, y), w, h, boxstyle="round,pad=0.05,rounding_size=0.15", fc=c, ec="#48534d",
                                    lw=0.8))
        ax.text(x + w / 2, y + h / 2, t, ha="center", va="center", fontsize=10)

    def arrow(a, b, text, tx, ty, rad=0.0):
        ax.add_patch(FancyArrowPatch(a, b, arrowstyle="-|>", mutation_scale=14, color="#1c2420", lw=1.1,
                                     connectionstyle=f"arc3,rad={rad}"))
        ax.text(tx, ty, text, ha="center", va="center", fontsize=9, color="#48534d")

    arrow((3.3, 4.0), (6.6, 2.75), "demand for food and timber\n(agents.summary)", 5.1, 4.05, rad=-0.15)
    arrow((3.3, 1.0), (6.6, 2.15), "subsidy raises the fertility\ncapital (agents.map, stack.set)", 5.1, 0.9, rad=0.15)
    ax.text(8.2, 1.45, "who manages each cell,\nthe supply of each service", ha="center", va="top", fontsize=9,
            color="#48534d")
    ax.text(1.7, 2.5, "each step: the population grows;\nthe managers decide", ha="center", va="center",
            fontsize=9, color="#48534d", style="italic")
    fig.tight_layout()
    fig.savefig(out / "schematic.png", dpi=150)
    plt.close(fig)
    figs._shrink(out / "schematic.png")


def main():
    schematic()
    w_with, r_with = run("with", MODEL)
    w_without, r_without = run("without", MODEL.replace(RULE, "subsidy = 1"))
    (data / "coupled.toml").write_text(MODEL, encoding="utf-8")
    parcels = figs.read(w_with / "parcels.tif")[0]
    n = figs.bands(w_with / "cover.tif")

    figs.raster(w_with / "capitals.tif", out / "fertility.png", band=1, cmap="YlOrBr", vmin=0, vmax=1,
                unit="capital", title="Fertility", width=520)
    figs.raster(w_with / "capitals.tif", out / "wood.png", band=2, cmap="YlGn", vmin=0, vmax=1, unit="capital",
                title="Wood", width=520)
    figs.raster(w_with / "parcels.tif", out / "parcels.png",
                palette={1: ("Parcel 1", "#e8901f"), 2: ("Parcel 2", "#9fb7c9"),
                         3: ("Parcel 3", "#c8d6b9"), 4: ("Parcel 4", "#d7c4d9")},
                title="Parcels", width=520)
    fert = figs.read(w_with / "capitals.tif", 1)[0]
    boosted = np.minimum(fert * np.where(parcels == 1, 3, 1), 1)
    figs.raster(None, out / "fertility_subsidised.png", array=boosted, cmap="YlOrBr", vmin=0, vmax=1, unit="capital",
                title="Fertility, subsidy on", width=520)
    # where the gradients go (printed, to check the captions)
    print("fertility first column, last column:", float(fert[:, 0].mean()), float(fert[:, -1].mean()))
    wood = figs.read(w_with / "capitals.tif", 2)[0]
    print("wood first row, last row:", float(wood[0].mean()), float(wood[-1].mean()))
    print("parcel ids at the corners (NW, NE, SW, SE):", parcels[0, 0], parcels[0, -1], parcels[-1, 0], parcels[-1, -1])
    # the food the landscape could give if every cell were farmed: sum of fertility squared
    print("food from every cell farmed:", float((fert ** 2).sum()), "with the subsidy:", float((boosted ** 2).sum()),
          "timber from every cell:", float(wood.sum()))

    for name, w in (("with", w_with), ("without", w_without)):
        figs.raster(w / "cover.tif", out / f"final_{name}.png", band=n, palette=COVER, legend=False, width=520)
    figs.raster(None, out / "cover_legend.png", array=np.ma.masked_invalid(np.array([[0.0, 2, 3]])), palette=COVER,
                width=300)

    figs.frames(w_with / "cover.tif", "step_{i}.png", out, palette=COVER, legend=False, width=520)
    figs.write_csv(data / "supply.csv", r_with)
    # the slider's frames, printed for the page
    for b in range(1, n + 1):
        c = cover_counts(w_with, b)
        a = figs.read(w_with / "cover.tif", b)[0]
        p1 = int(((a == 2) & (parcels == 1)).sum())
        r = r_with[b - 1]
        print(f'  - {{ value: {b}, src: /img/tutorials/{SLUG}/step_{b}.png, stats: {{ "Food (demand)": '
              f'"{r["food_supply"]:.0f} ({r["food_demand"]:.0f})", "Timber (demand)": "{r["timber_supply"]:.0f} '
              f'({r["timber_demand"]:.0f})", Farmers: "{c["Farmers"]}", Foresters: "{c["Foresters"]}", '
              f'Unmanaged: "{c["Unmanaged"]}", "Farmers in parcel 1": "{p1}" }} }}')

    rows = []
    for b in range(1, n + 1):
        a = figs.read(w_with / "cover.tif", b)[0]
        c = figs.read(w_without / "cover.tif", b)[0]
        rows.append({"step": b, "with": int(((a == 2) & (parcels == 1)).sum()),
                     "without": int(((c == 2) & (parcels == 1)).sum())})
    figs.write_csv(data / "parcel1.csv", rows)
    print("farmers in parcel 1 at step 30:", rows[-1])
    print("last supply with:", r_with[-1], "without:", r_without[-1])
    for name, w in (("with", w_with), ("without", w_without)):
        a = figs.read(w / "cover.tif", n)[0]
        print(name, "step 30", cover_counts(w, n),
              {f"parcel {p}": {nm: int(((a == v) & (parcels == p)).sum()) for v, (nm, _) in COVER.items()}
               for p in (1, 2, 3, 4)})
    print("without, supply", r_without[-6:])

    # settings: the growth of the population
    grows = []
    for g in (0.0, 0.01, 0.02, 0.03, 0.04):
        m = MODEL.replace(GROWTH, f"population = population * {1 + g:.2f}")
        w, r = run(f"growth_{g}", m)
        figs.raster(w / "cover.tif", out / f"growth_{g}.png", band=n, palette=COVER, legend=False, width=520)
        c = cover_counts(w, n)
        last = r[-1]
        short = sum(1 for x in r[10:] if x["food_supply"] < 0.95 * x["food_demand"])
        grows.append({"growth": g, "food_demand": last["food_demand"], "food_supply": last["food_supply"],
                      "timber_demand": last["timber_demand"], "timber_supply": last["timber_supply"], **c,
                      "short_steps": short})
        print("growth", grows[-1])
    figs.write_csv(data / "growth.csv", grows)
    gl = []
    for g in (0.0, 0.01, 0.02, 0.03, 0.04):
        w = figs.WEB / ".cache" / "work" / SLUG / f"growth_{g}"
        info = figs.read_csv(w / "output" / "info.csv")
        for k in range(0, len(info), 2):
            if len(gl) <= k // 2:
                gl.append({"step": k // 2 + 1})
            gl[k // 2][f"demand {int(g * 100)}%"] = round(float(info[k]["demand"]), 1)
            gl[k // 2][f"supply {int(g * 100)}%"] = round(float(info[k]["supply"]), 1)
    figs.write_csv(data / "growth_food.csv", gl)

    # agents in a CLUE-S model: two councils, the east freezing its land from step 5
    work = figs.workdir(f"{SLUG}/councils")
    for f in ("pie_landuse.tif", "pie_drivers.tif"):
        shutil.copy(figs.ROOT / "tests" / "data" / "inputs" / f, work / f)
    (data / "councils.toml").write_text(COUNCILS, encoding="utf-8")
    figs.run(figs.add_writes(COUNCILS, {"towns.map": "towns.tif", "simulation.maps": "maps.tif"}), work)
    # the same without councils, for comparison
    free = COUNCILS.replace(', restrictions = "frozen.map"', "")
    wf = figs.workdir(f"{SLUG}/councils_free")
    for f in ("pie_landuse.tif", "pie_drivers.tif"):
        shutil.copy(figs.ROOT / "tests" / "data" / "inputs" / f, wf / f)
    figs.run(figs.add_writes(free, {"simulation.maps": "maps.tif"}), wf)
    start = figs.read(work / "pie_landuse.tif", 1)[0]
    towns = figs.read(work / "towns.tif")[0].filled(0)
    print("towns: west cells", int(((towns == 1) & ~start.mask).sum()), "east", int(((towns == 2) & ~start.mask).sum()))
    steps = figs.read_csv(work / "output" / "steps.csv")
    print("converged", [int(float(s["converged"])) for s in steps])
    rows = []
    prev, prevf = start, start
    for b in range(1, 15):
        m = figs.read(work / "maps.tif", b)[0]
        mf = figs.read(wf / "maps.tif", b)[0]
        ch = (m != prev).filled(False)
        chf = (mf != prevf).filled(False)
        rows.append({"year": 1985 + b, "West, east frozen from 1990": int((ch & (towns == 1)).sum()),
                     "East, frozen from 1990": int((ch & (towns == 2)).sum()),
                     "West, no council": int((chf & (towns == 1)).sum()),
                     "East, no council": int((chf & (towns == 2)).sum())})
        prev, prevf = m, mf
    figs.write_csv(data / "councils.csv", rows)
    for r in rows:
        print(r)
    m4 = figs.read(work / "maps.tif", 4)[0]
    last = figs.read(work / "maps.tif", 14)[0]
    code = np.where((m4 != last).filled(False), last.filled(0), np.where(towns == 2, 5, 4)).astype(float)
    code = np.ma.masked_where(np.ma.getmaskarray(start), code)
    pal = {4: ("West council", "#eef1ec"), 5: ("East council (frozen from 1990)", "#c9d3dc"),
           1: ("to forest, 1990-1999", PIE[1][1]), 2: ("to built, 1990-1999", PIE[2][1]),
           3: ("to other, 1990-1999", PIE[3][1])}
    figs.raster(None, out / "councils_changes.png", array=code, palette=pal, counts=False, width=860,
                title="Change after the east council froze its land")
    lastf = figs.read(wf / "maps.tif", 14)[0]
    for nm, l in (("councils", last), ("free", lastf)):
        nb = ((l == 2) & (start != 2)).filled(False)
        print(nm, "new built 1985-1999 west", int((nb & (towns == 1)).sum()), "east", int((nb & (towns == 2)).sum()))
    print("images:", round(sum(p.stat().st_size for p in out.glob("*.png")) / 1e6, 2), "MB")


if __name__ == "__main__":
    main()
