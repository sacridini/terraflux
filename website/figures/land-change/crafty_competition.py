"""Figures of the tutorial "CRAFTY" (land-change/crafty): the template "CRAFTY: farmers and foresters".

Farmers and foresters compete for the cells of a landscape of two capitals (fertility, wood) to meet a demand
for food that grows and a demand for timber that stays; then the same with other shares of cells offered each
step (search), other giving-in thresholds and other giving-up rates.

    python website/figures/land-change/crafty_competition.py
"""

import pathlib
import re
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[2] / "tools"))
import figs  # noqa: E402
import numpy as np  # noqa: E402

SLUG = "land-change/crafty"
COVER = {0: ("Unmanaged", "#e9e4d8"), 1: ("Farmers", "#d9a441"), 2: ("Foresters", "#3b8a5a")}


def model(search=0.2, giving_in=0.1, rate=0.2, giving_up=0.05):
    m = figs.template("crafty-farmers-and-foresters")
    m = m.replace("search = 0.2", f"search = {search}").replace("giving_up_rate = 0.2", f"giving_up_rate = {rate}")
    m = m.replace("giving_in = 0.1", f"giving_in = {giving_in}").replace("giving_up = 0.05,", f"giving_up = {giving_up},")
    return m


def run(name, m):
    work = figs.workdir(f"{SLUG}/{name}")
    figs.run(figs.add_writes(m, {"simulation.cover": "cover.tif", "simulation.info": "info.csv",
                                 "capitals.stack": "capitals.tif", "start.map": "start.tif"}), work)
    info = figs.read_csv(work / "output" / "info.csv")
    rows = []
    for k in range(0, len(info), 2):
        food, timber = info[k], info[k + 1]
        assert food["service"] == "0" and timber["service"] == "1"
        rows.append({"step": k // 2 + 1, "food_demand": float(food["demand"]), "food_supply": round(float(food["supply"]), 1),
                     "timber_demand": float(timber["demand"]), "timber_supply": round(float(timber["supply"]), 1)})
    return work, rows


def shares(path, band):
    a = figs.read(path, band)[0]
    total = a.count()
    return {COVER[v][0]: f"{(a == v).sum() / total * 100:.1f}%" for v in COVER}


def main():
    out = figs.img_dir(SLUG)
    data = figs.data_dir(SLUG)
    work, rows = run("base", model())
    figs.write_csv(data / "supply.csv", rows)
    n = figs.bands(work / "cover.tif")
    figs.frames(work / "cover.tif", "step_{i}.png", out, palette=COVER, legend=False, width=560)
    figs.raster(work / "start.tif", out / "start.png", palette={1: COVER[1], 2: COVER[2]}, title="Who manages each cell at the start",
                width=700)
    figs.raster(work / "cover.tif", out / "cover_last.png", band=n, palette=COVER, counts=True,
                title="After 20 steps", width=700)
    figs.raster(work / "capitals.tif", out / "fertility.png", band=1, cmap="YlOrBr", vmin=0, vmax=1, unit="capital",
                title="Capital: fertility", width=640)
    figs.raster(work / "capitals.tif", out / "wood.png", band=2, cmap="YlGn", vmin=0, vmax=1, unit="capital",
                title="Capital: wood", width=640)
    # what each AFT would produce on each cell: production x capital ^ sensitivity (the template's settings)
    fert = figs.read(work / "capitals.tif", 1)[0]
    wood = figs.read(work / "capitals.tif", 2)[0]
    figs.raster(None, out / "prod_farmer.png", array=1.0 * np.clip(fert, 0, 1) ** 2.0, cmap="YlOrBr", vmin=0, vmax=1,
                unit="food per cell", title="A farmer's food: 1 x fertility^2", width=640)
    figs.raster(None, out / "prod_forester.png", array=1.0 * np.clip(wood, 0, 1) ** 1.0, cmap="YlGn", vmin=0, vmax=1,
                unit="timber per cell", title="A forester's timber: 1 x wood^1", width=640)
    print("frames", n, "last", shares(work / "cover.tif", n))
    print(rows[0], rows[-1])

    # the share of the cells offered each step
    curves = {}
    for s in (0.05, 0.1, 0.2, 0.4):
        w, r = run(f"search_{s}", model(search=s))
        figs.raster(w / "cover.tif", out / f"search_{s}.png", band=figs.bands(w / "cover.tif"), palette=COVER,
                    legend=False, width=560)
        curves[s] = r
        print("search", s, r[-1], shares(w / "cover.tif", figs.bands(w / "cover.tif")))
    figs.write_csv(data / "search.csv", [
        {"step": i + 1, "demand": curves[0.05][i]["food_demand"],
         **{f"search {s}": curves[s][i]["food_supply"] for s in curves}} for i in range(len(curves[0.05]))])

    # the giving-in threshold
    gi = []
    for g in (0.0, 0.1, 0.3, 0.6):
        w, r = run(f"giving_in_{g}", model(giving_in=g))
        figs.raster(w / "cover.tif", out / f"giving_in_{g}.png", band=figs.bands(w / "cover.tif"), palette=COVER,
                    legend=False, width=560)
        # how many cells changed manager over the 20 steps
        prev = figs.read(w / "start.tif")[0]
        changes = 0
        for b in range(1, figs.bands(w / "cover.tif") + 1):
            cur = figs.read(w / "cover.tif", b)[0]
            changes += int((cur != prev).sum())
            prev = cur
        gi.append({"giving_in": g, "changes": changes, "food_gap": round(r[-1]["food_demand"] - r[-1]["food_supply"], 1),
                   "timber_gap": round(r[-1]["timber_demand"] - r[-1]["timber_supply"], 1)})
        print("giving_in", g, gi[-1], shares(w / "cover.tif", figs.bands(w / "cover.tif")))
    figs.write_csv(data / "giving_in.csv", gi)

    # stats of the step slider
    stats = [shares(work / "cover.tif", b) for b in range(1, n + 1)]
    import json
    (data / "stats.json").write_text(json.dumps(stats), encoding="utf-8")


if __name__ == "__main__":
    main()
