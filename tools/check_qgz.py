"""Check a .qgz written by Terraflux in QGIS itself (PyQGIS): the project read, every layer valid, the map of
classes paletted, the points labelled by their name and drawn as squares, the features styled as rules (a rule
each class, then ELSE).

Run with QGIS's Python, after the ignored test `qgis::tests::a_project_for_pyqgis` wrote the project:

    set TERRAFLUX_QGIS_DIR=C:\\temp\\tf-qgis
    cargo test -j 4 -p terraflux-app --lib a_project_for_pyqgis -- --ignored
    "C:\\Program Files\\QGIS 3.34.14\\bin\\python-qgis-ltr.bat" tools\\check_qgz.py C:\\temp\\tf-qgis\\PyQGIS_check.qgz
"""

import sys

from qgis.core import QgsApplication, QgsProject, QgsRasterLayer, QgsVectorLayer

path = sys.argv[1]
app = QgsApplication([], False)
app.initQgis()
failed = []


def check(ok, what):
    print(("ok   " if ok else "FAIL ") + what)
    if not ok:
        failed.append(what)


project = QgsProject.instance()
check(project.read(path), f"the project read: {path}")
check(project.crs().authid() == "EPSG:32651" or "UTM zone 51N" in project.crs().description(),
      f"the project's coordinate system: {project.crs().authid()} {project.crs().description()}")
layers = {lyr.name(): lyr for lyr in project.mapLayers().values()}
check(sorted(layers) == ["land use", "plots", "uses"], f"the layers: {sorted(layers)}")
for name, lyr in layers.items():
    check(lyr.isValid(), f"{name}: valid")

land = layers.get("land use")
if isinstance(land, QgsRasterLayer):
    r = land.renderer()
    check(r.type() == "paletted", f"land use: renderer {r.type()}")
    classes = [c.value for c in r.classes()]
    check(classes == [1.0, 2.0, 3.0], f"land use: classes {classes}")

plots = layers.get("plots")
if isinstance(plots, QgsVectorLayer):
    check(plots.featureCount() == 2, f"plots: {plots.featureCount()} features")
    check(plots.labelsEnabled(), "plots: labels on")
    lab = plots.labeling()
    field = lab.settings().fieldName if lab is not None else None
    check(field == "name", f"plots: labelled by {field}")
    sym = plots.renderer().symbol() if plots.renderer().type() == "singleSymbol" else None
    shape = sym.symbolLayer(0).shape() if sym is not None else None
    from qgis.core import QgsSimpleMarkerSymbolLayerBase as M
    check(shape == M.Square, f"plots: marker {shape}")

uses = layers.get("uses")
if isinstance(uses, QgsVectorLayer):
    r = uses.renderer()
    check(r.type() == "RuleRenderer", f"uses: renderer {r.type()}")
    if r.type() == "RuleRenderer":
        rules = r.rootRule().children()
        filters = [x.filterExpression() for x in rules]
        check(len(rules) == 3, f"uses: rules {filters}")
        check(filters[-1] == "ELSE" or rules[-1].isElse(), "uses: the last rule ELSE")
        check(filters[0] == "\"use\" = 'forest'", f"uses: the first rule {filters[0]!r}")
        ctx_ok = all(x.symbol() is not None for x in rules)
        check(ctx_ok, "uses: a symbol each rule")

app.exitQgis()
print("FAILED: " + "; ".join(failed) if failed else "all checks passed")
sys.exit(1 if failed else 0)
