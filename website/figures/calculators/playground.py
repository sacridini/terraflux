"""The data of the calculators' playgrounds (calculators/raster, calculators/vector): Sibuyan Island (Philippines),
its land use of 1997 and 2011 and its drivers at 250 m (lulcc's and landschaft's example data).

- The raster calculator's: the land use (a stack, its layers 1997 and 2011) and some of the drivers (a stack), as
  the files hold them, to website/static/data/calculators/raster.js.
- The vector calculator's: the patches of the land use of 2011 as polygons (vector.polygonize), their mean
  elevation, slope, distance to roads and population density (vector.zonal, vector.join) and the name of their
  class as a text (vector.calc), in a real run of Terraflux, to website/static/data/calculators/vector.js.

The playgrounds compute with Terraflux's own map algebra built to WebAssembly (website/tools/build_calc.py); at the
end the engine is checked against `terraflux run` on the same scripts, when Node.js is there.

    python website/figures/calculators/playground.py
"""

import json
import re
import os
import pathlib
import shutil
import subprocess
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[2] / "tools"))
import figs  # noqa: E402
import numpy as np  # noqa: E402

if os.environ.get("TERRAFLUX_BIN"):
    figs.binary = lambda: pathlib.Path(os.environ["TERRAFLUX_BIN"])

NAME = "calculators/playground"
work = figs.workdir(NAME)
INPUTS = figs.ROOT / "tests" / "data" / "inputs"
shutil.copy(INPUTS / "sibuyan_landuse.tif", work / "landuse.tif")
shutil.copy(INPUTS / "sibuyan_drivers.tif", work / "drivers.tif")
DATA = figs.WEB / "static" / "data" / "calculators"
DATA.mkdir(parents=True, exist_ok=True)

LAND = {1: ("Forest", "#3b8a5a"), 2: ("Coconut", "#d9a441"), 3: ("Grassland", "#a7c957"), 4: ("Rice", "#7fb7d9"),
        5: ("Other", "#b4508a")}
DRIVERS = ["elevation", "slope", "dist_roads", "dist_urban", "dist_streams", "dist_sea", "pop_density"]
# how many decimals each driver keeps in the page's data
DECIMALS = {"elevation": 0, "slope": 1, "dist_roads": 0, "dist_urban": 0, "dist_streams": 0, "dist_sea": 0,
            "pop_density": 1}


def write_data(name: str, data: dict):
    """The data of a playground as a script (read from the site's files too): TFCalcData[name]."""
    text = json.dumps(data, separators=(",", ":"))
    (DATA / f"{name}.js").write_text(f"(window.TFCalcData = window.TFCalcData || {{}})[{name!r}] = {text};\n",
                                     encoding="utf-8")


def cells(a: np.ndarray, decimals: int) -> list:
    out = []
    for x in np.ma.filled(a.astype("float64"), np.nan).ravel():
        if np.isnan(x):
            out.append(None)
        elif decimals == 0:
            out.append(int(round(x)))
        else:
            out.append(round(float(x), decimals))
    return out


# ------------------------------------------------------------------------------------------------ the rasters

lu = [figs.read(work / "landuse.tif", b)[0] for b in (1, 2)]
_, dinfo = figs.read(work / "drivers.tif", 1)
names = list(dinfo["descriptions"])
drivers = {n: figs.read(work / "drivers.tif", names.index(n) + 1)[0] for n in DRIVERS}
h, w = lu[0].shape
raster = {
    "about": "Sibuyan Island (Philippines), 128 x 108 cells of 250 m: its land use in 1997 and 2011 (lu) and drivers "
             "(d), lulcc's example data.",
    "width": w,
    "height": h,
    "cell": 250,
    "inputs": {
        "lu": {"stack": {"layers": ["1997", "2011"], "cells": [cells(a, 0) for a in lu]}},
        "d": {"stack": {"layers": DRIVERS, "cells": [cells(drivers[n], DECIMALS[n]) for n in DRIVERS]}},
    },
    "classes": {"lu": {str(k): list(v) for k, v in LAND.items()}},
}
write_data("raster", raster)

# ------------------------------------------------------------------------------------------------ the features

CLASSES = " else ".join(f'"{n.lower()}" if value == {k}' for k, (n, _) in LAND.items() if k < 5) + ' else "other"'
# each driver's mean in each patch (vector.zonal of the driver's band, joined to the patches and named), then the
# name of each patch's class
MEANS = {"elevation": "round(mean)", "slope": "round(mean * 10) / 10", "dist_roads": "round(mean)",
         "pop_density": "round(mean * 10) / 10"}
nodes = [
    ("land", "raster.read", None, '{ path = "landuse.tif", band = "2011" }'),
    ("patches", "vector.polygonize", '{ map = "land.raster" }', None),
]
last = "patches"
for n, value in MEANS.items():
    nodes += [
        (f"{n}_map", "raster.read", None, f'{{ path = "drivers.tif", band = "{n}" }}'),
        (f"{n}_zonal", "vector.zonal", f'{{ vector = "patches.vector", map = "{n}_map.raster" }}',
         '{ stats = ["mean"] }'),
        (f"{n}_join", "vector.join", f'{{ vector = "{last}.vector", table = "{n}_zonal.table" }}', None),
        (f"{n}_named", "vector.calc", f'{{ vector = "{n}_join.vector" }}', f'{{ expression = "out {n} = {value}" }}'),
    ]
    last = f"{n}_named"
nodes += [
    ("named", "vector.calc", f'{{ vector = "{last}.vector" }}', "{ expression = '''out cover = " + CLASSES + "''' }"),
    ("save", "vector.write", '{ vector = "named.vector" }', '{ path = "patches.geojson" }'),
]
MODEL = 'format = "terraflux/1"\n'
for id_, op, inputs, params in nodes:
    MODEL += f'\n[[nodes]]\nid = "{id_}"\nop = "{op}"\n'
    if inputs:
        MODEL += f"inputs = {inputs}\n"
    if params:
        MODEL += f"params = {params}\n"
figs.run(MODEL, work)
gj = json.loads((work / "patches.geojson").read_text(encoding="utf-8"))
KEEP = ["cover", "value", "cells", "area", "elevation", "slope", "dist_roads", "pop_density"]
feats = gj["features"]


def coords(c):
    if isinstance(c[0], (int, float)):
        return [round(c[0], 1), round(c[1], 1)]
    return [coords(x) for x in c]


geometries = [{"type": f["geometry"]["type"], "coordinates": coords(f["geometry"]["coordinates"])} for f in feats]
attributes = []
for k in KEEP:
    vals = [f["properties"].get(k) for f in feats]
    if k == "cover":
        attributes.append({"name": k, "texts": [v or "" for v in vals]})
    else:
        attributes.append({"name": k, "numbers": vals})
xs = [p[0] for g in geometries for ring in (g["coordinates"] if g["type"] == "Polygon" else [r for poly in g["coordinates"] for r in poly]) for p in ring]
ys = [p[1] for g in geometries for ring in (g["coordinates"] if g["type"] == "Polygon" else [r for poly in g["coordinates"] for r in poly]) for p in ring]
vector = {
    "about": f"The {len(feats)} patches of Sibuyan's land use in 2011 as polygons, with their class's name (cover), "
             "their class (value), cells and area (m²) from vector.polygonize, and the mean elevation (m), slope (°), "
             "distance to roads (m) and population density in each from vector.zonal; coordinates in metres (UTM 51N).",
    "features": {"geometries": geometries, "attributes": attributes},
    "bounds": [min(xs), min(ys), max(xs), max(ys)],
    "classes": {"cover": {n.lower(): c for _, (n, c) in LAND.items()}},
}
write_data("vector", vector)
print(f"raster.js: {(DATA / 'raster.js').stat().st_size // 1024} kB; vector.js: "
      f"{(DATA / 'vector.js').stat().st_size // 1024} kB, {len(feats)} features")

# ------------------------------------------------------------------------------------------------ the engine checked

# Every example of the playgrounds' pages (their ```tfcalc blocks) computed by the engine of the page and by
# `terraflux run` on the same data (the drivers as the page has them, rounded): the same outputs.


def examples():
    import yaml

    for md in sorted((figs.WEB / "content" / "calculators").glob("*.md")):
        for block in re.findall(r"^```tfcalc\n(.*?)^```", md.read_text(encoding="utf-8"), re.S | re.M):
            spec = yaml.safe_load(block)
            for p in spec["presets"]:
                yield md.name, spec["kind"], p


def toml_value(v):
    return json.dumps(v) if isinstance(v, str) else repr(v)


def same(a, b, tol=1e-5):
    a = np.array([np.nan if x is None else x for x in a], dtype="float64")
    b = np.array([np.nan if x is None else x for x in b], dtype="float64")
    if a.shape != b.shape or not np.array_equal(np.isnan(a), np.isnan(b)):
        return False
    m = ~np.isnan(a)
    return bool(np.allclose(a[m], b[m], rtol=tol, atol=tol))


def check():
    import rasterio

    node = shutil.which("node")
    engine = figs.WEB / "static" / "js" / "calc-engine.js"
    if not node or not engine.exists():
        print("(the engine not checked: no Node.js, or website/static/js/calc-engine.js not built)")
        return
    # the data as the page has it, for terraflux run
    with rasterio.open(work / "drivers.tif") as src:
        profile = src.profile | {"count": len(DRIVERS), "dtype": "float64", "nodata": None}
    with rasterio.open(work / "drivers_web.tif", "w", **profile) as dst:
        for i, (n, c) in enumerate(zip(DRIVERS, raster["inputs"]["d"]["stack"]["cells"]), 1):
            dst.write(np.array([np.nan if x is None else x for x in c], dtype="float64").reshape(h, w), i)
            dst.set_band_description(i, n)
    fc = {"type": "FeatureCollection", "features": [
        {"type": "Feature", "geometry": g,
         "properties": {a["name"]: (a["texts"][i] if "texts" in a else a["numbers"][i]) for a in attributes}}
        for i, g in enumerate(geometries)]}
    (work / "features_web.geojson").write_text(json.dumps(fc), encoding="utf-8")

    cases = list(examples())
    requests = []
    for _, kind, p in cases:
        if kind == "raster":
            requests.append({"fn": "raster.run", "data": "raster", "text": p["script"], "inputs": p.get("inputs", {})})
        else:
            requests.append({"fn": "vector.run", "data": "vector", "text": p["script"], "op": p.get("op", "calc"),
                             "column": p.get("column", ""), "inputs": p.get("inputs", {})})
    (work / "requests.json").write_text(json.dumps(requests), encoding="utf-8")
    r = subprocess.run([node, str(figs.WEB / "tools" / "check_calc.mjs"), str(work / "requests.json")],
                       capture_output=True, text=True, encoding="utf-8")
    if r.returncode != 0:
        raise SystemExit(r.stderr)
    answers = json.loads(r.stdout)
    bad = []
    for k, ((page, kind, p), a) in enumerate(zip(cases, answers)):
        case = work / f"case_{k}"
        case.mkdir(exist_ok=True)
        # the node's other inputs: a number as it is, a text from an input of the model (a text in a node's
        # inputs is a reference)
        given = p.get("inputs", {})
        extra = "".join(f", {n} = " + (f'"${n}"' if isinstance(v, str) else toml_value(v)) for n, v in given.items())
        texts = "".join(f'\n[inputs.{n}]\nkind = "value"\nvalue = {toml_value(v)}\n'
                        for n, v in given.items() if isinstance(v, str))
        script = p["script"]
        if kind == "raster":
            model = f'''format = "terraflux/1"
{texts}
[inputs.lu]
kind = "stack"
path = "../landuse.tif"

[inputs.d]
kind = "stack"
path = "../drivers_web.tif"

[[nodes]]
id = "calc"
op = "raster.calc"
inputs = {{ lu = "$lu", d = "$d"{extra} }}
params = {{ expression = \'\'\'{script}\'\'\' }}
'''
        else:
            op = p.get("op", "calc")
            column = f', column = "{p["column"]}"' if p.get("column") else ""
            model = f'''format = "terraflux/1"
{texts}
[[nodes]]
id = "features"
op = "vector.read"
params = {{ path = "../features_web.geojson" }}

[[nodes]]
id = "calc"
op = "vector.{op}"
inputs = {{ vector = "features.vector"{extra} }}
params = {{ expression = \'\'\'{script}\'\'\'{column} }}

[[nodes]]
id = "save"
op = "vector.write"
inputs = {{ vector = "calc.vector" }}
params = {{ path = "out.geojson" }}
'''
        if "error" in a:
            run = figs.run(model, case, check=False)
            if run.returncode == 0:
                bad.append(f"{page} {p['name']!r}: the page fails ({a['error']}), terraflux run does not")
            continue
        ok = a["ok"]
        if kind == "raster":
            writes = {}
            for o in ok["outputs"]:
                writes[f"calc.{o['name']}"] = f"{o['name']}.tif" if "number" not in o else f"{o['name']}.txt"
            run = figs.run(figs.add_writes(model, writes), case, check=False)
            if run.returncode != 0:
                bad.append(f"{page} {p['name']!r}: terraflux run fails: {run.stderr.strip()[-300:]}")
                continue
            for o in ok["outputs"]:
                if "number" in o:
                    got = next((case / "output").glob(f"{o['name']}*")).read_text(encoding="utf-8").strip()
                    if not same([o["number"]], [float(json.loads(got) if got[:1] in "[{" else got)]):
                        bad.append(f"{page} {p['name']!r}: {o['name']} = {o['number']}, terraflux run {got}")
                    continue
                layers = [o["map"]] if "map" in o else [l["cells"] for l in o["stack"]]
                for i, cells in enumerate(layers, 1):
                    arr = figs.read(case / f"{o['name']}.tif", i)[0]
                    if not same(cells, np.ma.filled(arr, np.nan).ravel().tolist()):
                        bad.append(f"{page} {p['name']!r}: {o['name']} (layer {i}) differs")
        else:
            run = figs.run(model, case, check=False)
            if run.returncode != 0:
                bad.append(f"{page} {p['name']!r}: terraflux run fails: {run.stderr.strip()[-300:]}")
                continue
            got = [f["properties"] for f in json.loads((case / "out.geojson").read_text(encoding="utf-8"))["features"]]
            if "kept" in ok:
                want = [{a["name"]: (a["texts"] if "texts" in a else a["numbers"])[i] for a in attributes} for i in ok["kept"]]
                if [(g["value"], g["area"]) for g in got] != [(x["value"], x["area"]) for x in want]:
                    bad.append(f"{page} {p['name']!r}: kept {len(ok['kept'])}, terraflux run {len(got)}")
                continue
            for c in ok["attributes"]:
                if c["name"] not in ok["given"]:
                    continue
                theirs = [g.get(c["name"]) for g in got]
                if "texts" in c:
                    if [t or None for t in c["texts"]] != [t or None for t in theirs]:
                        bad.append(f"{page} {p['name']!r}: the texts of {c['name']} differ")
                elif not same(c["numbers"], theirs):
                    bad.append(f"{page} {p['name']!r}: {c['name']} differs")
    print(f"the playgrounds' {len(cases)} examples: the page and terraflux run "
          + ("give the same" if not bad else "differ:\n  " + "\n  ".join(bad)))
    if bad:
        raise SystemExit(1)


check()
