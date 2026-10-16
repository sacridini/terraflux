"""Figures of "Protected-area connectivity (ProtConn)" (habitat networks, part 5): a network of hypothetical
reserves (the forests of 500 ha or more of the CORINE landscape), its ProtConn at several dispersal distances, and
the same with smaller forests added as reserves."""

import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import _clc  # noqa: E402
from _clc import figs, np, plt  # noqa: E402

SLUG = "habitat-networks/protconn"
work = figs.workdir(SLUG)
out = figs.img_dir(SLUG)
data = figs.data_dir(SLUG)
_clc.setup(work)

DISTANCES = [1000, 2000, 5000, 10000, 20000]


def model(threshold, extra=None):
    expr = f"if a >= {threshold} then id else null"
    if extra:
        expr = f"if a >= {threshold} or (a >= {extra[0]} and a < {threshold} and imp >= {extra[1]}) then id else null"
    return f"""format = "terraflux/1"

[[nodes]]
id = "land"
op = "raster.read"
params = {{ path = "clc_100m.tif" }}

[[nodes]]
id = "graph"
op = "conn.graph_from_raster"
inputs = {{ map = "land.raster" }}
params = {{ habitat = [23, 24, 25], area_unit = "ha", distance = "edge" }}

[[nodes]]
id = "patches"
op = "conn.node_importance"
inputs = {{ graph = "graph.graph" }}
params = {{ distance = 2000, probability = 0.5, metrics = ["A", "PC"] }}

[[nodes]]
id = "area"
op = "conn.to_map"
inputs = {{ graph = "graph.graph", table = "patches.table" }}
params = {{ column = "dA" }}

[[nodes]]
id = "imp"
op = "conn.to_map"
inputs = {{ graph = "graph.graph", table = "patches.table" }}
params = {{ column = "dPCconnector" }}

[[nodes]]
id = "id"
op = "conn.to_map"
inputs = {{ graph = "graph.graph", table = "patches.table" }}
params = {{ column = "id" }}

[[nodes]]
id = "reserves"
op = "map.calc"
inputs = {{ a = "area.map", id = "id.map", imp = "imp.map" }}
params = {{ expression = "{expr}" }}

[[nodes]]
id = "region"
op = "map.calc"
inputs = {{ l = "land.raster" }}
params = {{ expression = "1" }}

[[nodes]]
id = "protconn"
op = "conn.protconn_raster"
inputs = {{ protected = "reserves.map", region_map = "region.map" }}
params = {{ distance = {DISTANCES}, area_unit = "ha" }}
"""


# dA is the share of the forest area in the patch (%): 500 ha of 51,569 ha is 0.9696 %
FOREST = 51569.0
pct = lambda ha: round(100 * ha / FOREST, 6)  # noqa: E731

import json  # noqa: E402
import os  # noqa: E402
import shutil  # noqa: E402
import subprocess  # noqa: E402

GDAL = shutil.which("gdal") or r"C:\Program Files\GDAL\gdal.exe"
os.environ.setdefault("PROJ_DATA", r"C:\Program Files\GDAL\projlib")
REGION = {"type": "FeatureCollection", "crs": {"type": "name", "properties": {"name": "urn:ogc:def:crs:EPSG::3035"}},
          "features": [{"type": "Feature", "properties": {"name": "study area"}, "geometry": {"type": "Polygon",
                        "coordinates": [[[4261250, 3232500], [4311250, 3232500], [4311250, 3282500],
                                         [4261250, 3282500], [4261250, 3232500]]]}}]}

VECTOR = f"""format = "terraflux/1"

[[nodes]]
id = "reserves"
op = "vector.read"
params = {{ path = "reserves.gpkg" }}

[[nodes]]
id = "region"
op = "vector.read"
params = {{ path = "region.geojson" }}

[[nodes]]
id = "protconn"
op = "conn.protconn"
inputs = {{ region = "region.vector", protected = "reserves.vector" }}
params = {{ distance = {DISTANCES}, area_unit = "ha", delta = true }}

[[nodes]]
id = "save_delta"
op = "vector.write"
inputs = {{ vector = "protconn.delta" }}
params = {{ path = "delta.geojson" }}
"""


def scenario(w, expr_model):
    """The reserves made a raster by terraflux, polygons by GDAL, then ProtConn by terraflux."""
    _clc.setup(w)
    figs.run(figs.add_writes(expr_model, {"reserves.map": "reserves.tif", "id.map": "ids.tif",
                                          "patches.table": "patches.csv"}), w)
    if (w / "reserves.gpkg").exists():
        (w / "reserves.gpkg").unlink()
    subprocess.run([GDAL, "raster", "polygonize", "-q", "--connect-diagonal-pixels", "--attribute-name", "patch",
                    "-f", "GPKG", str(w / "reserves.tif"), str(w / "reserves.gpkg")], check=True,
                   capture_output=True)
    (w / "region.geojson").write_text(json.dumps(REGION), encoding="utf-8")
    figs.run(figs.add_writes(VECTOR, {"protconn.table": "protconn.csv"}), w, name="protconn.toml")
    return figs.read_csv(w / "output" / "protconn.csv")


base = scenario(work, model(pct(500)))
w2 = figs.workdir(SLUG + "/added")
# the scenario: the forests of 40 to 500 ha that matter most as stepping stones (dPCconnector >= 0.5 %) protected too
added = scenario(w2, model(pct(500), extra=(pct(40), 0.5)))
shutil.copy2(work / "reserves.gpkg", data / "reserves.gpkg")
shutil.copy2(work / "region.geojson", data / "region.geojson")

ids = figs.read(work / "ids.tif")[0]
r1 = figs.read(work / "reserves.tif")[0]
r2 = figs.read(w2 / "reserves.tif")[0]
cls = np.zeros(ids.shape)
cls[~np.ma.getmaskarray(ids)] = 1
cls[~np.ma.getmaskarray(r2)] = 3
cls[~np.ma.getmaskarray(r1)] = 2
cls = np.ma.masked_equal(cls, 0)
figs.raster(None, out / "reserves.png", array=cls,
            palette={1: ("Other forest", "#b9d7c1"), 2: ("Reserves (forests of 500 ha or more)", figs.FOREST),
                     3: ("Stepping stones added", figs.EMBER)}, width=980)
n1 = len(np.unique(r1.compressed()))
n2 = len(np.unique(r2.compressed()))
print("reserves:", n1, "with stepping stones:", n2, "area", r1.count(), r2.count())

# the importance of every reserve at 5 km (dProtConn), painted on the reserves
delta = json.loads((work / "delta.geojson").read_text(encoding="utf-8"))
r1f = np.ma.filled(r1, 0).astype(int)
dmap = np.full(r1.shape, np.nan)
# the features are the reserves in the order of the polygons: join them by their area
pc_by_area = {}
for f in delta["features"]:
    p = f["properties"]
    if int(round(p["distance"])) == 5000:
        pc_by_area[round(p["attribute"])] = p["dProtConn"]
vals, counts = np.unique(r1f[r1f > 0], return_counts=True)
for v, c in zip(vals, counts):
    if round(c) in pc_by_area:
        dmap[r1f == v] = pc_by_area[round(c)]
print("dProtConn at 5 km:", sorted(((round(v, 1), k) for k, v in pc_by_area.items()), reverse=True)[:6])
_clc.network_map(out / "dprotconn.png", ids, values=dmap, cmap="viridis", unit="dProtConn at 5 km (%)", width=900)

rows, rows2 = [], []
for b, a in zip(base, added):
    d = int(float(b["distance"]))
    lab = f"{d / 1000:g} km"
    rows.append({"distance": lab, "ProtConn": round(float(b["ProtConn"]), 2), "ProtUnconn": round(float(b["ProtUnconn"]), 2),
                 "ProtConn2": round(float(a["ProtConn"]), 2), "ProtUnconn2": round(float(a["ProtUnconn"]), 2)})
    rows2.append({"distance": lab, "Prot": round(float(b["ProtConn_Prot"]), 1), "Unprot": round(float(b["ProtConn_Unprot"]), 1),
                  "Trans": round(float(b["ProtConn_Trans"]), 1)})
    print(lab, {k: round(float(b[k]), 2) for k in ("Prot", "ProtConn", "ProtUnconn", "RelConn", "ECA", "ProtConn_Prot", "ProtConn_Unprot")},
          "| added:", {k: round(float(a[k]), 2) for k in ("Prot", "ProtConn", "ProtUnconn", "RelConn")})
figs.write_csv(data / "protconn.csv", [{k: r[k] for k in ("distance", "ProtConn", "ProtUnconn")} for r in rows])
figs.write_csv(data / "protconn-added.csv", [{"distance": r["distance"], "ProtConn": r["ProtConn"],
                                              "ProtConn2": r["ProtConn2"]} for r in rows])
figs.write_csv(data / "fractions.csv", rows2)
