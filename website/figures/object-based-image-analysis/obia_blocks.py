"""City blocks of the Sentinel-2 clip of the series (Lago Paranoa, Brasilia), made from OpenStreetMap for the page
"Classes by parcel" (classes-by-parcel.py).

OpenStreetMap has no layer of lots for this area (the few `landuse` ways are single shops, five lots of Lago Sul and a
military site), but its streets are well drawn: the polygons that the streets close are the blocks. `blocks()` asks the
Overpass API for the streets of the clip's box (highway=residential, tertiary, secondary, primary and their links),
cuts the plane with the streets and the box's edge (shapely), and keeps the polygons of 1 to 80 ha that are not slivers
(a double carriageway leaves slivers, and the open land is one polygon over 80 ha). Each block has `block` (1..n, from
the north-west corner), `faces` (the kind of street along most of its boundary: primary, secondary, tertiary or
residential) and `area_ha`. The answers of the API are kept in website/.cache/obia/osm/ (the data change: the date of
the data is in the attribution). The result is written as website/static/data/tutorials/object-based-image-analysis/classes-by-parcel/
blocks.geojson in EPSG:31983, the clip's coordinate system.

Data (c) OpenStreetMap contributors, Open Database License (ODbL) 1.0, https://www.openstreetmap.org/copyright. The blocks
are a derived database and carry the same licence.
"""

from __future__ import annotations

import json
import math
import pathlib
import time
import urllib.parse
import urllib.request

import obia_data as od
from pyproj import Transformer
from shapely.geometry import LineString, Polygon, box, mapping
from shapely.ops import linemerge, polygonize, unary_union

OVERPASS = "https://overpass-api.de/api/interpreter"
UA = "terraflux-docs/1.0 (tutorial figures; https://terraflux.org)"
CACHE = od.CACHE / "osm"
BBOX = (-15.89, -47.89, -15.835, -47.835)  # south, west, north, east (degrees): the clip's box
STREETS = ('way["highway"]["highway"!~"footway|path|track|steps|cycleway|pedestrian|bridleway|corridor|proposed|'
           'construction|service"](%s,%s,%s,%s);' % BBOX)
MIN_HA, MAX_HA = 1.0, 80.0
MIN_ROUND = 0.3  # 4 pi area / perimeter^2: 1 for a circle, 0.78 for a square, near 0 for a sliver


def overpass(name: str, query: str) -> dict:
    """The answer of the Overpass API to `query` (cached in website/.cache/obia/osm/<name>.json), with retries."""
    f = CACHE / f"{name}.json"
    if f.exists():
        return json.loads(f.read_text(encoding="utf-8"))
    CACHE.mkdir(parents=True, exist_ok=True)
    data = urllib.parse.urlencode({"data": query}).encode()
    for k in range(6):
        try:
            req = urllib.request.Request(OVERPASS, data=data, headers={"User-Agent": UA})
            with urllib.request.urlopen(req, timeout=110) as r:
                txt = r.read().decode("utf-8")
            if txt.lstrip().startswith("{"):
                f.write_text(txt, encoding="utf-8")
                return json.loads(txt)
        except OSError:
            pass
        time.sleep(15 * (k + 1))
    raise SystemExit("the Overpass API did not answer")


def streets() -> tuple[list[tuple[LineString, str]], str]:
    """The streets inside the clip's box as (line, kind), and the date of the data."""
    d = overpass("streets", f"[out:json][timeout:90];({STREETS});out tags geom;")
    t = Transformer.from_crs(4326, 31983, always_xy=True)
    x0, y0, x1, y1 = od.S2_UTM_EXTENT
    clip = box(x0, y0, x1, y1)
    out = []
    for e in d["elements"]:
        if len(e.get("geometry", [])) > 1:
            ln = LineString([t.transform(p["lon"], p["lat"]) for p in e["geometry"]]).intersection(clip)
            if not ln.is_empty:
                out.append((ln, e["tags"]["highway"].replace("_link", "")))
    return out, d["osm3s"]["timestamp_osm_base"]


def blocks() -> tuple[list[dict], str]:
    """The blocks as GeoJSON features, and the date of the OpenStreetMap data."""
    lines, stamp = streets()
    x0, y0, x1, y1 = od.S2_UTM_EXTENT
    cut = unary_union([ln for ln, _ in lines] + [box(x0, y0, x1, y1).boundary])
    polys = [p for p in polygonize(cut)
             if MIN_HA <= p.area / 1e4 <= MAX_HA and 4 * math.pi * p.area / p.length ** 2 >= MIN_ROUND]
    # numbered from the north-west corner: by the row of the top, then the column
    polys.sort(key=lambda p: (-round(p.bounds[3] / 400), p.bounds[0]))
    feats = []
    for k, p in enumerate(polys, 1):
        # the kind of street along most of its boundary
        near = p.exterior.buffer(6)
        length = {}
        for ln, kind in lines:
            length[kind] = length.get(kind, 0) + ln.intersection(near).length
        faces = max(length, key=length.get)
        feats.append({"type": "Feature", "geometry": mapping(p),
                      "properties": {"block": k, "faces": faces, "area_ha": round(p.area / 1e4, 2)}})
    return feats, stamp


def write(path: pathlib.Path) -> dict:
    """Writes blocks.geojson (EPSG:31983) and returns {"count", "timestamp"}."""
    feats, stamp = blocks()
    gj = {"type": "FeatureCollection", "name": "blocks",
          "crs": {"type": "name", "properties": {"name": "urn:ogc:def:crs:EPSG::31983"}}, "features": feats}
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(gj, separators=(",", ":")), encoding="utf-8")
    return {"count": len(feats), "timestamp": stamp}


if __name__ == "__main__":
    print(write(pathlib.Path("blocks.geojson")))
