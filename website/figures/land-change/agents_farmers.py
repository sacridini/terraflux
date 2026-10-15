"""Figures of the tutorial "Agents" (land-change/agents): the template "Farmers on their parcels (agents)".

A farmer per parcel of a Voronoi mosaic, whose wealth follows the fertility of the parcel and a falling price,
who gives up farming when broke; run for 20 years, then again with other yearly costs.

    python website/figures/land-change/agents_farmers.py
"""

import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[2] / "tools"))
import figs  # noqa: E402
import numpy as np  # noqa: E402

SLUG = "land-change/agents"
FARMED = {0: ("Given up", "#d9a441"), 1: ("Farming", "#3b8a5a")}
COST_RULE = "wealth = wealth + income - 7"


def model(cost=7):
    m = figs.template("farmers-on-their-parcels-agents")
    assert COST_RULE in m
    return m.replace(COST_RULE, f"wealth = wealth + income - {cost}")


def main():
    out = figs.img_dir(SLUG)
    data = figs.data_dir(SLUG)

    work = figs.workdir(f"{SLUG}/base")
    m = figs.add_writes(model(), {
        "years.maps": "farmed.tif", "fertility.map": "fertility.tif", "parcels.map": "parcels.tif",
        "wealth.map": "wealth.tif", "years.summary": "summary.csv", "years.farmers": "farmers.csv",
    })
    m += '\n[[nodes]]\nid = "fig_parcel_fert"\nop = "agents.map"\ninputs = { agents = "farms.agents", parcels = "parcels.map" }\nparams = { column = "fertility" }\n'
    m += '\n[[nodes]]\nid = "fig_w"\nop = "raster.write"\ninputs = { value = "fig_parcel_fert.map" }\nparams = { path = "parcel_fertility.tif" }\n'
    figs.run(m, work)

    figs.raster(work / "fertility.tif", out / "fertility.png", cmap="YlGn", vmin=0, vmax=1, unit="fertility",
                title="Soil fertility (a Gaussian field)", width=700)
    parcels = figs.read(work / "parcels.tif")[0]
    ids = np.unique(parcels.compressed())
    rng = np.random.default_rng(3)
    shuffled = rng.permutation(len(ids))
    lut = {v: shuffled[i] for i, v in enumerate(ids)}
    pa = np.vectorize(lambda v: lut.get(v, -1))(parcels.filled(-1)).astype(float)
    figs.raster(None, out / "parcels.png", array=pa, cmap="tab20", legend=False, width=560,
                title="60 parcels (a Voronoi mosaic)")
    figs.raster(work / "parcel_fertility.tif", out / "parcel_fertility.png", cmap="YlGn", vmin=0, vmax=1,
                unit="mean fertility", title="Each farmer's land: the mean over its parcel", width=700)
    figs.raster(work / "wealth.tif", out / "wealth.png", cmap="RdBu", diverging=True, unit="wealth",
                title="Wealth after 20 years", width=700)

    # the years: the parcels still farmed, with the number of farmers and their mean wealth
    summary = figs.read_csv(work / "output" / "summary.csv")
    rows = []
    for i, r in enumerate(summary, start=1):
        rows.append({"year": i, "farming": int(float(r["farming_sum"])), "wealth": round(float(r["wealth_mean"]), 2),
                     "price": round(float(r["price_mean"]), 2)})
    figs.write_csv(data / "years.csv", rows)
    figs.frames(work / "farmed.tif", "year_{i}.png", out, palette=FARMED, legend=False, width=560)

    # each farmer after 20 years: the fertility of its parcel and its wealth
    farmers = figs.read_csv(work / "output" / "farmers.csv")
    figs.write_csv(data / "farmers.csv", [
        {"fertility": round(float(f["fertility"]), 4), "wealth": round(float(f["wealth"]), 2),
         "farming": int(float(f["farming"]))} for f in farmers])
    fert = np.array([float(f["fertility"]) for f in farmers])
    wealth = np.array([float(f["wealth"]) for f in farmers])
    farming = np.array([int(float(f["farming"])) for f in farmers])

    def draw(ax):
        ax.scatter(fert[farming == 1], wealth[farming == 1], s=26, color="#3b8a5a", label="Still farming")
        ax.scatter(fert[farming == 0], wealth[farming == 0], s=26, color="#d9a441", label="Gave up")
        ax.axhline(0, color=figs.INK2, lw=0.8)
        ax.set_xlabel("Mean fertility of the parcel")
        ax.set_ylabel("Wealth after 20 years")
        ax.legend(loc="upper left")

    figs.chart(out / "fertility_wealth.png", draw, width=820, height=440)

    # the yearly cost: 5 to 9
    cost_rows = []
    for cost in (5, 6, 7, 8, 9):
        w = figs.workdir(f"{SLUG}/cost_{cost}")
        figs.run(figs.add_writes(model(cost), {"years.maps": "farmed.tif", "years.summary": "summary.csv"}), w)
        n = figs.bands(w / "farmed.tif")
        figs.raster(w / "farmed.tif", out / f"cost_{cost}.png", band=n, palette=FARMED, legend=False, width=560)
        s = figs.read_csv(w / "output" / "summary.csv")
        r = {"cost": cost}
        for year in (5, 10, 15, 20):
            r[f"year_{year}"] = int(float(s[year - 1]["farming_sum"]))
        cost_rows.append(r)
        print(cost, r)
    figs.write_csv(data / "costs.csv", cost_rows)
    # farmers left, per year, for each cost (a chart)
    curves = []
    for year in range(1, 21):
        r = {"year": year}
        for cost in (5, 6, 7, 8, 9):
            s = figs.read_csv(figs.WEB / ".cache" / "work" / SLUG / f"cost_{cost}" / "output" / "summary.csv")
            r[f"cost {cost}"] = int(float(s[year - 1]["farming_sum"]))
        curves.append(r)
    figs.write_csv(data / "cost_curves.csv", curves)


if __name__ == "__main__":
    main()
