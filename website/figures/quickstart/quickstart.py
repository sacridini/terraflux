"""Figures of the Quickstart: the template 'MSPA of a forest' with its edge width 1 and 3 (for a before/after), the
template 'MSPA of your map' on the CORINE excerpt, and the real output of the commands the page shows (cli.txt)."""

import pathlib
import shutil
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1] / "getting-started"))
from _common import figs, np, CLC  # noqa: E402

work = figs.workdir("quickstart")
out = figs.WEB / "static" / "img" / "quickstart"
out.mkdir(parents=True, exist_ok=True)
log = []


def cli(*args, cwd=work):
    text = figs.cli(*args, cwd=cwd)
    log.append("$ terraflux " + " ".join(args) + "\n" + text.strip() + "\n")
    return text


# ------------------------------------------------------------------ the template, edge width 1 and 3
for ew in (1, 3):
    d = figs.workdir(f"quickstart/edge{ew}")
    m = figs.template("mspa-of-a-forest").replace("edge_width = 1", f"edge_width = {ew}")
    figs.run(figs.add_writes(m, {"mspa.map": "mspa.tif"}), d)
    figs.raster(d / "mspa.tif", out / f"edge{ew}.png", palette=figs.mspa_palette(), width=640, legend=False)
    a, _ = figs.read(d / "mspa.tif")
    print(ew, {int(v): int(c) for v, c in zip(*np.unique(a.compressed(), return_counts=True))})

# ------------------------------------------------------------------ MSPA of your map, on clc.tif, saved with a write
shutil.copy(CLC, work / "clc.tif")
mine = figs.template("mspa-of-your-map").replace('params = { path = "" }', 'params = { path = "clc.tif" }')
mine = mine.replace('params = { rules = "1 -> 2\\n* -> 1" }', 'params = { rules = "23..25 -> 2\\n* -> 1" }')
mine = mine.rstrip() + '''

[[nodes]]
id = "write"
op = "raster.write"
inputs = { value = "mspa.map" }
params = { path = "clc_mspa.tif" }
'''
assert "23..25" in mine
(work / "my-mspa.toml").write_text(mine, encoding="utf-8")
log.append("$ terraflux --version\n" + figs.cli("--version").strip() + "\n")
cli("check", "my-mspa.toml")
cli("run", "my-mspa.toml")
(work / "cli.txt").write_text("\n".join(log), encoding="utf-8")
print("\n".join(log))
figs.raster(work / "clc_mspa.tif", out / "clc_mspa.png", palette=figs.mspa_palette(), width=900, counts=True)
a, _ = figs.read(work / "clc_mspa.tif")
print({int(v): int(c) for v, c in zip(*np.unique(a.compressed(), return_counts=True))})
