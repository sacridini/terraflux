"""Figures of 'Coordinate systems and reprojection': a map in degrees and in metres, the area of a cell in degrees
along the latitudes, a forest's area counted three ways, nearest against bilinear on classes, and slopes of a
surface in degrees."""

import rasterio
from pyproj import Geod

from _common import figs, np, plt, CLC, ELEVATION, FOREST_WGS84, SIBUYAN, WINDOW, posix, clc_palette, classes_png, read
from _common import save, CLC_LEGEND

SLUG = "getting-started/coordinate-systems"
work = figs.workdir(SLUG)
out = figs.img_dir(SLUG)
data = figs.data_dir(SLUG)

model = f'''format = "terraflux/1"

[[nodes]]
id = "elevation"
op = "raster.read"
params = {{ path = "{posix(ELEVATION)}" }}

[[nodes]]
id = "elevation_utm"
op = "raster.reproject"
inputs = {{ raster = "elevation.raster" }}
params = {{ crs = "EPSG:32610", resolution = 30, path = "elevation_utm.tif" }}

[[nodes]]
id = "slope_degrees"
op = "raster.terrain"
inputs = {{ raster = "elevation.raster" }}
params = {{ path = "slope_degrees.tif" }}

[[nodes]]
id = "slope_degrees_s"
op = "raster.terrain"
inputs = {{ raster = "elevation.raster" }}
params = {{ options = "-s 111120", path = "slope_degrees_s.tif" }}

[[nodes]]
id = "slope_utm"
op = "raster.terrain"
inputs = {{ raster = "elevation_utm.raster" }}
params = {{ path = "slope_utm.tif" }}

[[nodes]]
id = "forest"
op = "raster.read"
params = {{ path = "{posix(FOREST_WGS84)}" }}

[[nodes]]
id = "forest_laea"
op = "raster.reproject"
inputs = {{ raster = "forest.raster" }}
params = {{ crs = "EPSG:3035", resolution = 25, path = "forest_laea.tif" }}

[[nodes]]
id = "landcover"
op = "raster.read"
params = {{ path = "{posix(CLC)}" }}

[[nodes]]
id = "window"
op = "raster.clip"
inputs = {{ raster = "landcover.raster" }}
params = {{ extent = {WINDOW}, path = "window.tif" }}

[[nodes]]
id = "nearest"
op = "raster.reproject"
inputs = {{ raster = "window.raster" }}
params = {{ crs = "EPSG:4326", resampling = "near", path = "window_nearest.tif" }}

[[nodes]]
id = "bilinear"
op = "raster.reproject"
inputs = {{ raster = "window.raster" }}
params = {{ crs = "EPSG:4326", resampling = "bilinear", path = "window_bilinear.tif" }}

[[nodes]]
id = "auto"
op = "raster.reproject"
inputs = {{ raster = "window.raster" }}
params = {{ crs = "EPSG:4326", path = "window_auto.tif" }}

[[nodes]]
id = "sibuyan"
op = "raster.read"
params = {{ path = "{posix(SIBUYAN)}" }}

[[nodes]]
id = "sibuyan_wgs84"
op = "raster.reproject"
inputs = {{ raster = "sibuyan.raster" }}
params = {{ crs = "EPSG:4326", source_crs = "EPSG:32651", path = "sibuyan_wgs84.tif" }}
'''
figs.run(model, work)
for f in ("slope_degrees.tif", "slope_degrees_s.tif", "slope_utm.tif", "elevation_utm.tif", "forest_laea.tif",
          "window_nearest.tif", "window_bilinear.tif", "window_auto.tif", "sibuyan_wgs84.tif"):
    assert (work / f).exists(), f

# ------------------------------------------------------------------ 1. the same surface in degrees and in metres
def framed(path, png, unit_label, xlabel, ylabel, fmt):
    with rasterio.open(path) as d:
        a = d.read(1, masked=True)
        b = d.bounds
    fig, ax = plt.subplots(figsize=(4.6, 4.6), dpi=150)
    ax.imshow(a, extent=[b.left, b.right, b.bottom, b.top], cmap="terrain", vmin=300, vmax=1500,
              interpolation="nearest")
    ax.set_xlabel(xlabel)
    ax.set_ylabel(ylabel)
    ax.grid(False)
    ax.ticklabel_format(useOffset=False, style="plain")
    ax.xaxis.set_major_formatter(plt.FuncFormatter(fmt))
    ax.yaxis.set_major_formatter(plt.FuncFormatter(fmt))
    ax.tick_params(labelsize=7.5)
    for lab in ax.get_xticklabels():
        lab.set_rotation(30)
    ax.set_title(unit_label, loc="left", fontsize=10)
    save(fig, png)


framed(ELEVATION, out / "elevation_degrees.png", "EPSG:4326: degrees", "longitude (°)", "latitude (°)",
       lambda v, _: f"{v:.2f}")
framed(work / "elevation_utm.tif", out / "elevation_utm.png", "EPSG:32610: metres (UTM zone 10 N)", "x (m)", "y (m)",
       lambda v, _: f"{v:,.0f}")
with rasterio.open(ELEVATION) as d:
    eb, ew, eh = d.bounds, d.width, d.height
with rasterio.open(work / "elevation_utm.tif") as d:
    ub, uw, uh = d.bounds, d.width, d.height
print("elevation degrees", eb, ew, eh, "utm", ub, uw, uh, "km", (ub.right - ub.left) / 1000, (ub.top - ub.bottom) / 1000)

# ------------------------------------------------------------------ 2. the area of a cell of 1 arc-second along the latitudes
geod = Geod(ellps="WGS84")
rows = []
for lat in range(0, 81, 5):
    s = 1 / 3600
    area, _ = geod.polygon_area_perimeter([0, s, s, 0], [lat, lat, lat + s, lat + s])
    ew_m = geod.inv(0, lat, s, lat)[2]
    ns_m = geod.inv(0, lat, 0, lat + s)[2]
    rows.append({"latitude": lat, "east_west": round(ew_m, 2), "north_south": round(ns_m, 2),
                 "area": round(abs(area), 1)})
figs.write_csv(data / "cell_area.csv", rows)
print("cell area rows", rows[0], rows[9], rows[-1])

# ------------------------------------------------------------------ 3. the forest's area, three ways
with rasterio.open(FOREST_WGS84) as d:
    f = d.read(1)
    T = d.transform
    res = T.a
n_forest = int((f == 2).sum())
# (a) the naive way: the cell's side taken as 0.000305 degrees x 111.32 km at the equator, squared
side = res * 111320
naive = n_forest * side * side / 1e6
# (b) the truth: every cell's own area on the ellipsoid, by its latitude
lats = T.f + (np.arange(f.shape[0]) + 0.5) * T.e
cell_area = np.array([abs(geod.polygon_area_perimeter([0, res, res, 0], [la - res / 2, la - res / 2, la + res / 2,
                                                                         la + res / 2])[0]) for la in lats])
true = float(((f == 2).sum(axis=1) * cell_area).sum() / 1e6)
# (c) counting cells after reprojecting to an equal-area projection (ETRS89-LAEA, cells of 25 m)
fl = read(work / "forest_laea.tif")[0]
laea = float((fl == 2).sum() * 625 / 1e6)
print(f"forest cells {n_forest}; naive {naive:.2f} km2 (side {side:.2f} m); geodesic {true:.2f} km2; laea {laea:.2f} km2")
print("cell area at the map's centre", cell_area[len(cell_area) // 2], "m2; ew", geod.inv(0, lats[500], res, lats[500])[2],
      "ns", geod.inv(0, lats[500], 0, lats[500] + res)[2])

# ------------------------------------------------------------------ 4. nearest against bilinear on classes
near = read(work / "window_nearest.tif")[0]
bil = read(work / "window_bilinear.tif")[0]
auto = read(work / "window_auto.tif")[0]
win = read(work / "window.tif")[0]
orig = set(int(v) for v in np.unique(win.compressed()))
inside = (near.filled(0) > 0)
changed = (near != bil) & inside
invented = set(int(v) for v in np.unique(bil[inside].compressed())) - orig
cells_invented = int(np.isin(bil.filled(0), list(invented)).sum())
print("orig classes", sorted(orig), "bilinear invented", sorted(invented), "cells with another class", int(changed.sum()),
      "of", int(inside.sum()), f"{changed.sum() / inside.sum() * 100:.1f}%", "cells of invented classes", cells_invented,
      f"{cells_invented / inside.sum() * 100:.1f}%", "auto == near:", bool((auto == near).all()))
# a zoom where the river, a town and forest meet
r0, c0, h, w = 150, 80, 70, 100
allv = sorted(orig | invented | {0})
pal = {v: CLC_LEGEND[v] for v in allv if v in CLC_LEGEND}
for name, arr in (("nearest", near), ("bilinear", bil)):
    sub = np.ma.masked_equal(arr[r0:r0 + h, c0:c0 + w].filled(0), 0)
    classes_png(sub, out / f"classes_{name}.png", pal, width=820, legend=False)
# the classes the bilinear made up, as a list for the page
print("invented names", [(v, CLC_LEGEND[v][0]) for v in sorted(invented) if v in CLC_LEGEND])
sub_n = near[r0:r0 + h, c0:c0 + w]
sub_b = bil[r0:r0 + h, c0:c0 + w]
print("zoom: classes nearest", sorted(set(np.unique(sub_n.compressed()).tolist())), "bilinear",
      sorted(set(np.unique(sub_b.compressed()).tolist())))
classes_png(np.ma.masked_equal(near.filled(0), 0), out / "window_degrees.png", clc_palette(orig), width=820,
            legend=False)

# ------------------------------------------------------------------ 5. slopes
s_deg = read(work / "slope_degrees.tif")[0]
s_s = read(work / "slope_degrees_s.tif")[0]
s_utm = read(work / "slope_utm.tif")[0]
print("slope means: degrees default", float(s_deg.mean()), "with -s 111120", float(s_s.mean()), "utm", float(s_utm.mean()))
print("slope max:", float(s_deg.max()), float(s_s.max()), float(s_utm.max()))
print("slope p90:", np.percentile(s_deg.compressed(), 90), np.percentile(s_s.compressed(), 90),
      np.percentile(s_utm.compressed(), 90))
with rasterio.open(work / "sibuyan_wgs84.tif") as d:
    print("sibuyan reprojected", d.crs, d.bounds, d.res)
