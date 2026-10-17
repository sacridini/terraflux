"""Figures of "Roads that grow" (land-change/roads-that-grow): Dinamica EGO's endogenous road constructor
(dinamica.cost_map, dinamica.destinations, dinamica.pathways and dinamica.roads) on a frontier drawn by chance
beside a highway, one step by hand and then 20 years in a loop where the distance to the growing roads drives the
clearing (dinamica.step), against the same model with the roads frozen.

    python website/figures/land-change/roads-that-grow.py
"""

import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[2] / "tools"))
import figs  # noqa: E402
import numpy as np  # noqa: E402
from matplotlib.patches import Patch  # noqa: E402
from scipy import ndimage  # noqa: E402

plt = figs.plt
SLUG = "land-change/roads-that-grow"
out = figs.img_dir(SLUG)
data = figs.data_dir(SLUG)

DATA = """format = "terraflux/1"

[inputs.seed]
kind = "value"
value = 1

[[nodes]]
id = "highway"
op = "nlm.distance_gradient"
params = { rows = 250, columns = 250, resolution = 100, origin = [40, 40, 1, 250], nlmr_compat = false, rescale = false }
note = "The distance to a highway along column 40, in metres."

[[nodes]]
id = "noise"
op = "nlm.gaussian_field"
params = { rows = 250, columns = 250, resolution = 100, range = 15, seed = 21 }

[[nodes]]
id = "relief"
op = "nlm.gaussian_field"
params = { rows = 250, columns = 250, resolution = 100, range = 40, seed = 22 }

[[nodes]]
id = "value"
op = "nlm.gaussian_field"
params = { rows = 250, columns = 250, resolution = 100, range = 30, seed = 23 }
note = "What the forest is worth: timber, soil."

[[nodes]]
id = "roads0"
op = "map.calc"
inputs = { d = "highway.map" }
params = { expression = "if d == 0 then 1 else 0" }

[[nodes]]
id = "land0"
op = "map.calc"
inputs = { d = "highway.map", g = "noise.map" }
params = { expression = "if d < 500 + 2500 * g then 2 else 1" }
note = "1 forest, 2 cleared beside the highway."

[[nodes]]
id = "friction"
op = "map.calc"
inputs = { r = "relief.map" }
params = { expression = "1 + 30 * r * r * r" }
note = "Building is harder on rough ground."
"""

ONE_STEP = DATA + """
[[nodes]]
id = "attraction"
op = "map.calc"
inputs = { land = "land0.map", v = "value.map" }
params = { expression = "if land == 1 then v else 0" }

[[nodes]]
id = "near"
op = "dynamic.distance_to"
inputs = { landscape = "roads0.map" }
params = { classes = 1 }

[[nodes]]
id = "band"
op = "map.calc"
inputs = { a = "attraction.map", d = "near.distance" }
params = { expression = "if d >= 1500 and d <= 4000 then a else null" }

[[nodes]]
id = "ends"
op = "dinamica.destinations"
inputs = { attraction = "band.map", seed = 1 }
params = { quadrants = [2, 2], min_distance = 2000 }

[[nodes]]
id = "cost"
op = "dinamica.cost_map"
inputs = { sources = "roads0.map", friction = "friction.map" }

[[nodes]]
id = "paths"
op = "dinamica.pathways"
inputs = { network = "roads0.map", sources = "ends.map", cost = "cost.cost", seed = 2 }
params = { lottery = true }

[[nodes]]
id = "roads"
op = "dinamica.roads"
inputs = { network = "roads0.map", friction = "friction.map", attraction = "attraction.map", seed = 1 }
params = { quadrants = [2, 2], distance = [1500, 4000], spacing = 2000 }
"""

LOOP = DATA + """
[[nodes]]
id = "rates"
op = "markov.matrix"
params = { classes = [1, 2], single = [[0.985, 0.015], [0, 1]] }
note = "1.5% of the forest cleared a year."

[[nodes]]
id = "patches"
op = "patches.settings"
params = { transitions = [{ from = 1, to = 2, expansion = 0.7, expander = [12, 100, 1.5], patcher = [8, 50, 1.5] }] }

[[nodes]]
id = "years"
op = "control.repeat"
inputs = { times = 20, landscape = "land0.map", network = "roads0.map" }

[nodes.body]
carry = { landscape = "clearing.landscape", network = "roads.network" }
collect = { maps = "clearing.landscape", networks = "roads.network", info = "clearing.info" }

[[nodes.body.nodes]]
id = "attraction"
op = "map.calc"
inputs = { land = "@landscape", v = "value.map" }
params = { expression = "if land == 1 then v else 0" }

[[nodes.body.nodes]]
id = "roads"
op = "dinamica.roads"
inputs = { network = "@network", friction = "friction.map", attraction = "attraction.map", step = "@step", seed = "$seed" }
params = { quadrants = [2, 2], distance = [1500, 4000], spacing = 2000 }

[[nodes.body.nodes]]
id = "probability"
op = "map.calc"
inputs = { land = "@landscape", d = "roads.distance" }
params = { expression = "if land == 1 then exp(-d / 1000) else 0" }
note = "Clearing likely near the roads of this year."

[[nodes.body.nodes]]
id = "transitions"
op = "stack.of"
inputs = { 1_to_2 = "probability.map" }

[[nodes.body.nodes]]
id = "clearing"
op = "dinamica.step"
inputs = { landscape = "@landscape", probability = "transitions.stack", rates = "rates.markov", patches = "patches.patches", step = "@step", seed = "$seed" }
"""

# the same model with the roads frozen: the distance to the highway, computed once
STATIC = LOOP.replace("""[[nodes.body.nodes]]
id = "roads"
op = "dinamica.roads"
inputs = { network = "@network", friction = "friction.map", attraction = "attraction.map", step = "@step", seed = "$seed" }
params = { quadrants = [2, 2], distance = [1500, 4000], spacing = 2000 }
""", "").replace('d = "roads.distance"', 'd = "highway.map"').replace(
    'carry = { landscape = "clearing.landscape", network = "roads.network" }',
    'carry = { landscape = "clearing.landscape" }').replace(
    'collect = { maps = "clearing.landscape", networks = "roads.network", info = "clearing.info" }',
    'collect = { maps = "clearing.landscape", info = "clearing.info" }').replace(
    'inputs = { times = 20, landscape = "land0.map", network = "roads0.map" }',
    'inputs = { times = 20, landscape = "land0.map" }')
assert "dinamica.roads" not in STATIC and "highway.map\" }" in STATIC

W = {"land0.map": "land0.tif", "roads0.map": "roads0.tif", "friction.map": "friction.tif", "value.map": "value.tif",
     "highway.map": "highway.tif"}
LC = {0: ("Forest", "#3b8a5a"), 1: ("Cleared at the start", "#d9c9a3"), 2: ("Cleared by the model", "#d9a441"),
      3: ("Highway", "#1c2420"), 4: ("New roads", "#c0392b"), 5: ("Protected area", "#9ecbb0")}


def run(model, name, writes, *args):
    w = figs.workdir(f"land-change/roads-that-grow/{name}")
    figs.run(figs.add_writes(model, {**W, **writes}), w, *args)
    return w


def arr(path, band=1):
    return figs.read(path, band)[0].filled(np.nan)


def present(a):
    return ~np.isnan(a) & (a != 0)


def legend_strip(png, items, width=900):
    fig = plt.figure(figsize=(width / 150, 0.42), dpi=150)
    handles = [Patch(facecolor=c, edgecolor="#00000022", label=n) for n, c in items]
    fig.legend(handles=handles, loc="center", ncol=len(items), fontsize=9.5, handlelength=1.1, handleheight=1.1,
               columnspacing=1.4)
    fig.savefig(png, dpi=150)
    plt.close(fig)
    figs._shrink(png)


def composite(land0, land, roads0, net=None, pa=None):
    c = np.zeros(land0.shape)
    if pa is not None:
        c[pa] = 5
    c[land == 2] = 2
    c[land0 == 2] = 1
    if net is not None:
        c[present(net)] = 4
    c[present(roads0)] = 3
    return c


def road_km(net, roads0):
    """The length of the new roads: a cell's side per new road cell, sqrt 2 times more where it joins a diagonal."""
    new = present(net) & ~present(roads0)
    return new.sum() * 0.1


def interior(land, net, depth_m=1000):
    """The share of the map in forest more than depth_m from any clearing or road."""
    open_ = (land != 1) | present(net)
    d = ndimage.distance_transform_edt(~open_) * 100
    return float(((land == 1) & (d > depth_m)).mean() * 100)


# ---------------------------------------------------------------------------------------------- one step by hand

w1 = run(ONE_STEP, "one_step", {"band.map": "band.tif", "ends.map": "ends.tif", "cost.cost": "cost.tif",
                                 "paths.map": "paths.tif", "roads.network": "roads.tif", "roads.distance": "dist.tif",
                                 "attraction.map": "attraction.tif"})
land0, roads0 = arr(w1 / "land0.tif"), arr(w1 / "roads0.tif")
paths, roads = arr(w1 / "paths.tif"), arr(w1 / "roads.tif")
same = np.array_equal(present(paths), present(roads))
print("by hand == dinamica.roads:", same)
ends = arr(w1 / "ends.tif")
print("destinations:", int((~np.isnan(ends)).sum()), "new road km", road_km(paths, roads0))
band, cost = arr(w1 / "band.tif"), arr(w1 / "cost.tif")
friction = arr(w1 / "friction.tif")
print("friction", np.nanmin(friction), np.nanmax(friction), "cost max", np.nanmax(cost))

fig, axes = plt.subplots(1, 4, figsize=(12, 3.55), dpi=150)
bgc = np.zeros(land0.shape + (4,))
bgc[land0 == 1] = (0.231, 0.541, 0.353, 0.28)
bgc[land0 == 2] = (0.85, 0.79, 0.64, 1)
newr = ndimage.binary_dilation(present(paths) & ~present(roads0), np.ones((3, 3)))
hw = np.where(ndimage.binary_dilation(present(roads0), np.ones((1, 3))), 1.0, np.nan)
ii, jj = np.nonzero(~np.isnan(ends))
for ax in axes:
    ax.set_axis_off()
axes[0].imshow(bgc)
axes[0].imshow(band, cmap="YlGn", vmin=-0.2, vmax=1)
axes[0].set_title("1. Attraction 1.5-4 km away", loc="left", fontsize=10)
axes[1].imshow(bgc)
axes[1].imshow(np.where(np.isnan(band), np.nan, 1.0), cmap="Greys", vmin=0, vmax=4)
axes[1].axvline(124.5, color=figs.INK2, lw=0.9, ls="--")
axes[1].axhline(124.5, color=figs.INK2, lw=0.9, ls="--")
axes[1].scatter(jj, ii, s=46, c=figs.EMBER, edgecolors="white", linewidths=0.9, zorder=3)
axes[1].set_title("2. A destination per quadrant", loc="left", fontsize=10)
axes[2].imshow(cost, cmap="viridis")
axes[2].scatter(jj, ii, s=46, c=figs.EMBER, edgecolors="white", linewidths=0.9, zorder=3)
axes[2].set_title("3. Cost from the network", loc="left", fontsize=10)
axes[3].imshow(friction, cmap="Greys", vmin=-10, vmax=45)
axes[3].imshow(np.where(newr, 1.0, np.nan), cmap="Reds", vmin=0, vmax=1.2)
axes[3].scatter(jj, ii, s=46, c=figs.EMBER, edgecolors="white", linewidths=0.9, zorder=3)
axes[3].set_title("4. Down the cost: new roads", loc="left", fontsize=10)
for ax in axes:
    if ax is not axes[2]:
        ax.imshow(hw, cmap="Greys", vmin=0, vmax=1)
fig.tight_layout()
fig.savefig(out / "one_step.png", dpi=150)
plt.close(fig)
figs._shrink(out / "one_step.png")

figs.raster(None, out / "friction.png", array=friction, cmap="cividis", unit="friction", width=560)
figs.raster(None, out / "value.png", array=arr(w1 / "value.tif"), cmap="YlGn", unit="value", width=560)
figs.raster(None, out / "start.png", array=composite(land0, land0, roads0), palette=LC, legend=False, width=560)
legend_strip(out / "legend.png", [LC[k] for k in (0, 1, 2, 3, 4)], width=950)

# ---------------------------------------------------------------------------------------------- the loop


def series(w, n=20, nets=True):
    maps = [arr(w / "maps.tif", b) for b in range(1, n + 1)]
    ns = [arr(w / "networks.tif", b) for b in range(1, n + 1)] if nets else None
    return maps, ns


wl = run(LOOP, "loop", {"years.maps": "maps.tif", "years.networks": "networks.tif", "years.info": "info.csv"})
ws = run(STATIC, "static", {"years.maps": "maps.tif", "years.info": "info.csv"})
maps, nets = series(wl)
smaps, _ = series(ws, nets=False)
highway = arr(wl / "highway.tif")
rows = []
for k in range(20):
    rows.append({"step": k + 1, "roads": round(road_km(nets[k], roads0), 1),
                 "cleared": round(float((maps[k] == 2).mean() * 100), 2),
                 "cleared_static": round(float((smaps[k] == 2).mean() * 100), 2),
                 "far": round(float(((maps[k] == 2) & (land0 == 1) & (highway > 5000)).sum()), 0),
                 "far_static": round(float(((smaps[k] == 2) & (land0 == 1) & (highway > 5000)).sum()), 0),
                 "interior": round(interior(maps[k], nets[k]), 2), "interior_static": round(interior(smaps[k], roads0), 2)})
figs.write_csv(data / "years.csv", rows)
prev = present(roads0)
for k in range(20):
    if (k + 1) % 2 == 0:
        figs.raster(None, out / f"year_{k + 1}.png", array=composite(land0, maps[k], roads0, nets[k]), palette=LC,
                    legend=False, width=620)
        r = rows[k]
        new_now = road_km(nets[k], roads0) - (road_km(nets[k - 1], roads0) if k else 0)
        print(f"  - {{ value: {k + 1}, src: /img/tutorials/land-change/roads-that-grow/year_{k + 1}.png, "
              f"caption: \"Year {k + 1}\", stats: {{ \"New roads so far\": \"{r['roads']:.0f} km\", "
              f"\"Cleared\": \"{r['cleared']:.1f}%\", \"Forest interior\": \"{r['interior']:.1f}%\" }} }}")
print("final", rows[-1])
figs.raster(None, out / "final_growing.png", array=composite(land0, maps[-1], roads0, nets[-1]), palette=LC,
            legend=False, width=620)
figs.raster(None, out / "final_static.png", array=composite(land0, smaps[-1], roads0), palette=LC, legend=False,
            width=620)
for name, m in (("growing", maps[-1]), ("static", smaps[-1])):
    lab, k = ndimage.label(m == 1, structure=np.ones((3, 3)))
    sizes = np.bincount(lab.ravel())[1:]
    print(f"{name}: forest patches {k}, largest {sizes.max()} ha ({sizes.max() / (m == 1).sum() * 100:.1f}% of the "
          f"forest), new clearing beyond 5 km of the highway {int(((m == 2) & (land0 == 1) & (highway > 5000)).sum())} ha")

# ---------------------------------------------------------------------------------------------- settings


def final_net(model, name, *args):
    w = run(model, name, {"years.maps": "maps.tif", "years.networks": "networks.tif"}, *args)
    m = arr(w / "maps.tif", 20)
    n = arr(w / "networks.tif", 20)
    return m, n


print("distance range")
for lo, hi in ((500, 1500), (1500, 4000), (4000, 8000)):
    model = LOOP.replace("distance = [1500, 4000]", f"distance = [{lo}, {hi}]")
    m, n = final_net(model, f"dist_{lo}")
    figs.raster(None, out / f"dist_{lo}.png", array=composite(land0, m, roads0, n), palette=LC, legend=False,
                width=620)
    print(f"  - {{ value: \"{lo / 1000:g}-{hi / 1000:g} km\", src: /img/tutorials/land-change/roads-that-grow/dist_{lo}.png, "
          f"stats: {{ \"New roads\": \"{road_km(n, roads0):.0f} km\", \"Cleared\": \"{(m == 2).mean() * 100:.1f}%\", "
          f"\"Forest interior\": \"{interior(m, n):.1f}%\" }} }}")

print("quadrants")
for q in ((1, 1), (2, 2), (3, 3), (4, 4)):
    model = LOOP.replace("quadrants = [2, 2]", f"quadrants = [{q[0]}, {q[1]}]")
    m, n = final_net(model, f"quad_{q[0]}")
    figs.raster(None, out / f"quad_{q[0]}.png", array=composite(land0, m, roads0, n), palette=LC, legend=False,
                width=620)
    print(f"  - {{ value: \"{q[0]} x {q[1]}\", src: /img/tutorials/land-change/roads-that-grow/quad_{q[0]}.png, "
          f"stats: {{ \"New roads\": \"{road_km(n, roads0):.0f} km\", \"Cleared\": \"{(m == 2).mean() * 100:.1f}%\", "
          f"\"Forest interior\": \"{interior(m, n):.1f}%\" }} }}")

print("flat ground")
flat = LOOP.replace('params = { expression = "1 + 30 * r * r * r" }', 'params = { expression = "1 + 0 * r" }')
m, n = final_net(flat, "flat")
figs.raster(None, out / "flat.png", array=composite(land0, m, roads0, n), palette=LC, legend=False, width=620)
print(f"flat: roads {road_km(n, roads0):.0f} km")

print("protected area")
PA = """
[[nodes]]
id = "reserve"
op = "nlm.distance_gradient"
params = { rows = 250, columns = 250, resolution = 100, origin = [150, 235, 70, 180], nlmr_compat = false, rescale = false }

[[nodes]]
id = "outside"
op = "map.calc"
inputs = { d = "reserve.map" }
params = { expression = "if d == 0 then 0 else 1" }
"""
pa_mask = LOOP.replace("[[nodes]]\nid = \"rates\"", PA.strip() + "\n\n[[nodes]]\nid = \"rates\"", 1).replace(
    'attraction = "attraction.map", step = "@step"', 'attraction = "attraction.map", mask = "outside.map", step = "@step"')
pa_closed = pa_mask.replace('params = { expression = "1 + 30 * r * r * r" }',
                            'params = { expression = "if inside == 0 then null else 1 + 30 * r * r * r" }').replace(
    'inputs = { r = "relief.map" }', 'inputs = { r = "relief.map", inside = "reserve.map" }')
# the friction node comes before the reserve: move the reserve to the top of the nodes
pa_closed = pa_closed.replace(PA.strip() + "\n\n", "").replace('[[nodes]]\nid = "highway"', PA.strip() + '\n\n[[nodes]]\nid = "highway"', 1)
assert pa_closed.count('id = "reserve"') == 1 and pa_mask.count('id = "reserve"') == 1
pa = None
pa_rows = []
for name, model in (("none", LOOP), ("mask", pa_mask), ("closed", pa_closed)):
    w = run(model, f"pa_{name}", {"years.maps": "maps.tif", "years.networks": "networks.tif", "reserve.map": "reserve.tif"}
            if name != "none" else {"years.maps": "maps.tif", "years.networks": "networks.tif"})
    if name == "mask":
        pa = arr(w / "reserve.tif") == 0
    m, n = arr(w / "maps.tif", 20), arr(w / "networks.tif", 20)
    if name == "none":
        none_m, none_n = m, n
        continue
    figs.raster(None, out / f"pa_{name}.png", array=composite(land0, m, roads0, n, pa=pa & (m == 1)), palette=LC,
                legend=False, width=560)
    pa_rows.append((name, m, n))
figs.raster(None, out / "pa_none.png", array=composite(land0, none_m, roads0, none_n, pa=pa & (none_m == 1)),
            palette=LC, legend=False, width=560)
legend_strip(out / "legend_pa.png", [LC[k] for k in (0, 5, 1, 2, 3, 4)], width=1000)
for name, m, n in [("none", none_m, none_n)] + pa_rows:
    inside_cleared = ((m == 2) & pa).sum()
    inside_roads = (present(n) & pa).sum() * 0.1
    print(f"PA {name}: cleared inside {inside_cleared} ha ({inside_cleared / pa.sum() * 100:.1f}%), roads inside "
          f"{inside_roads:.1f} km, roads {road_km(n, roads0):.0f} km, cleared {(m == 2).mean() * 100:.1f}%")
print("images:", sum(p.stat().st_size for p in out.glob("*.png")) / 1e6, "MB")
