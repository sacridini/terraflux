"""Figures and terminal outputs of 'Checks, tests and reproducible runs': Sibuyan's forest lost between 1997 and
2011 (lulcc's data), three check.assert nodes, two [[tests]], terraflux test (--update), the manifest of a run and
terraflux reproduce. The outputs printed are those the page shows."""

import pathlib
import shutil
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[2] / "tools"))
import figs  # noqa: E402

SLUG = "building-models/checks-and-tests"
work = figs.workdir(SLUG)
out = figs.img_dir(SLUG)
shutil.copy(figs.ROOT / "tests" / "data" / "inputs" / "sibuyan_landuse.tif", work / "sibuyan_landuse.tif")

MODEL = '''format = "terraflux/1"
name = "Forest lost on Sibuyan"

[inputs.lu1997]
kind = "map"
path = "sibuyan_landuse.tif"
band = "1997"
note = "The land use of 1997: 1 forest, 2 coconut, 3 grassland, 4 rice, 5 other; the sea has no value."

[inputs.lu2011]
kind = "map"
path = "sibuyan_landuse.tif"
band = "2011"
note = "The land use of 2011, the same classes."

[[nodes]]
id = "classes"
op = "check.assert"
inputs = { a = "$lu1997", b = "$lu2011" }
params = { expression = "1 <= a <= 5 and 1 <= b <= 5", message = "a cell outside the five classes" }
note = "Every cell of the island is one of the five classes, at both dates."

[[nodes]]
id = "loss"
op = "raster.calc"
inputs = { a = "$lu1997", b = "$lu2011" }
params = { expression = "None if isnull(a) else a == 1 and b != 1" }
note = "1 where the forest of 1997 was gone in 2011, 0 on the rest of the island."

[[nodes]]
id = "forest"
op = "raster.calc"
inputs = { a = "$lu1997", b = "$lu2011" }
params = { expression = """
out before = (a == 1).sum()
out after = (b == 1).sum()
""" }
note = "The cells of forest at each date."

[[nodes]]
id = "shrank"
op = "check.assert"
inputs = { before = "forest.before", after = "forest.after" }
params = { expression = "after <= before", message = "the forest grew" }
note = "Sibuyan's forest shrank between the dates: a gain would mean the bands were swapped."

[[nodes]]
id = "tenth"
op = "check.assert"
inputs = { before = "forest.before", after = "forest.after" }
params = { expression = "(before - after) / before < 0.1", message = "more than a tenth of the forest was lost", warn = true }
note = "Only a warning: a loss above 10 % is possible, but worth a look."

[outputs.loss]
from = "loss.map"

[outputs.forest2011]
from = "forest.after"

[[tests]]
from = "loss.map"
expected = "expected/loss.tif"
note = "The map of the forest lost, cell by cell."

[[tests]]
from = "forest.after"
expected = "expected/forest2011.json"
note = "The cells of forest in 2011."
'''
(work / "model.toml").write_text(MODEL, encoding="utf-8")


def show(*args):
    print(f"> terraflux {' '.join(args)}")
    print(figs.cli("--threads", "4", *args, cwd=work))


show("check", "./model.toml")
show("run", "./model.toml")
show("run", "./model.toml", "--set", "tenth.warn=false", "--out", "strict")
show("test", "./model.toml")
show("test", "./model.toml", "--update")
show("test", "./model.toml")
show("reproduce", "output/manifest.json")
print((work / "output" / "manifest.json").read_text(encoding="utf-8"))

# an edit: only the forest that became grassland; the test and the manifest both see it
(work / "model.toml").write_text(MODEL.replace("a == 1 and b != 1", "a == 1 and b == 3"), encoding="utf-8")
show("test", "./model.toml")
show("reproduce", "output/manifest.json")
(work / "model.toml").write_text(MODEL, encoding="utf-8")

loss, _ = figs.read(work / "output" / "loss.tif")
print(f"forest lost: {int((loss == 1).sum())} cells of the island's {int((loss == loss).sum())}")
figs.raster(work / "output" / "loss.tif", out / "loss.png", colors={0: "#e9e4d8", 1: figs.EMBER},
            labels={0: "Not forest lost", 1: "Forest lost"}, counts=True, width=800)
