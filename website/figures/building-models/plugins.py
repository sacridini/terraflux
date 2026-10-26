"""Figures of 'Plugins in WebAssembly': a forest stand's growth (plugins/stand.wat) run cell by cell by plugin.wasm on
a site productivity drawn by a neutral landscape model; its biomass over the years, with and without fellings; the
plugin checked against the same loop in numpy; and the sandbox stopping a function that never ends."""

import pathlib
import shutil
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[2] / "tools"))
import figs  # noqa: E402
import numpy as np  # noqa: E402

SLUG = "building-models/plugins"
HERE = pathlib.Path(__file__).resolve().parent
work = figs.workdir(SLUG)
out = figs.img_dir(SLUG)
data = figs.data_dir(SLUG)
shutil.copy(HERE / "plugins" / "stand.wat", work / "stand.wat")


def model(years, function="grow", cycle=20):
    extra = ', cycle = %d' % cycle if function == "harvested" else ""
    args = '["r", "b", "years", "cycle"]' if function == "harvested" else '["r", "b", "years"]'
    return f'''format = "terraflux/1"
name = "Stand growth"

[[nodes]]
id = "site"
op = "nlm.gaussian_field"
params = {{ rows = 200, columns = 200, range = 30, seed = 21 }}
note = "Site productivity, 0 to 1 (in its place, a map of site index or of soils)."

[[nodes]]
id = "rate"
op = "raster.calc"
inputs = {{ s = "site.map" }}
params = {{ expression = "None if s < 0.15 else 0.03 + 0.12 * s" }}
note = "The growth rate per year, 0.03 to 0.15; no forest (no value) on the poorest sites."

[[nodes]]
id = "growth"
op = "plugin.wasm"
inputs = {{ r = "rate.map", b = 20, years = {years}{extra} }}
params = {{ path = "stand.wat", function = "{function}", args = {args} }}
note = "The biomass after the years, from 20 t/ha, by the plugin's loop."

[outputs.biomass]
from = "growth.map"

[outputs.rate]
from = "rate.map"
'''


rows = []
for y in range(0, 65, 5):
    row = {"years": y}
    for fn in ("grow", "harvested"):
        d = figs.workdir(f"{SLUG}/{fn}_{y}")
        shutil.copy(work / "stand.wat", d / "stand.wat")
        figs.run(model(y, fn), d)
        a, _ = figs.read(d / "output" / "biomass.tif")
        row[fn] = round(float(a.mean()), 2)
        if fn == "grow" and y % 10 == 0:
            figs.raster(d / "output" / "biomass.tif", out / f"biomass_{y}.png", cmap="YlGn", vmin=0, vmax=300,
                        unit="biomass (t/ha)", width=640)
        if y == 60:
            figs.raster(d / "output" / "biomass.tif", out / f"{fn}_60.png", cmap="YlGn", vmin=0, vmax=300,
                        legend=False, width=560)
    rows.append(row)
    print(row)
figs.write_csv(data / "biomass.csv", rows)
(work / "stand.toml").write_text(model(30), encoding="utf-8")

# the rate map, and the plugin checked against numpy
d = work.parent / "plugins" / "grow_30"
r, _ = figs.read(d / "output" / "rate.tif")
figs.raster(d / "output" / "rate.tif", out / "rate.png", cmap="viridis", unit="growth rate per year", width=640)
b = np.full(r.shape, 20.0)
for _ in range(30):
    b = b + r.filled(np.nan) * b * (1 - b / 300)
g, _ = figs.read(d / "output" / "biomass.tif")
diff = np.nanmax(np.abs(g.filled(np.nan) - b))
print("max difference from numpy (float32 file):", diff, "missing cells", int(g.mask.sum()), "of", g.size)

# the sandbox: a function that never ends
d = figs.workdir(f"{SLUG}/forever")
shutil.copy(work / "stand.wat", d / "stand.wat")
m = model(1).replace('function = "grow", args = ["r", "b", "years"]', 'function = "forever", args = ["r"]')
r = figs.run(m, d, check=False)
msg = (r.stdout + r.stderr).strip()
r2 = figs.run(model(1).replace('function = "grow"', 'function = "grows"'), d, check=False)
msg2 = (r2.stdout + r2.stderr).strip()
r3 = figs.run(model(1).replace('args = ["r", "b", "years"]', 'args = ["r", "b"]'), d, check=False)
msg3 = (r3.stdout + r3.stderr).strip()
(work / "messages.txt").write_text(f"{msg}\n---\n{msg2}\n---\n{msg3}\n", encoding="utf-8")
print(msg, "\n---\n", msg2, "\n---\n", msg3)
