"""Make the views of the accuracy table from website/static/data/benchmarks/accuracy_tools.csv.

    python website/tools/make_accuracy_view.py            # writes view/accuracy_view.csv, accuracy_by_domain.csv, accuracy_classes.csv
    python website/tools/make_accuracy_view.py --check    # only says whether the views are up to date

accuracy_tools.csv is the table the library's comparisons are written into, one row per comparison group. The page
(content/benchmarks/accuracy.md) reads three views of it: the table of groups, the groups per area by result class (the
stacked bars) and the groups per class. The numbers of groups in the captions of accuracy.md and index.md are the length
of the table: change them with it.
"""

import argparse
import csv
import io
import pathlib
import sys

DATA = pathlib.Path(__file__).resolve().parent.parent / "static" / "data" / "benchmarks"
VIEW = DATA / "view"

# the area as the page names it, by the domain of the table
AREA = {
    "MSPA / GuidosToolbox": "GuidosToolbox",
    "Circuit connectivity": "Circuits",
    "Surface metrics": "Surface and terrain",
    "Terrain hydrology": "Hydrology",
    "Spatial prioritization": "Zonation",
    "Optimized prioritization": "Optimized plans",
    "Neutral landscape models": "Neutral models",
    "Watershed model (SWAT+)": "SWAT+",
}
# the chart names this one shorter
CHART = {"Grid distances": "Grid"}
# the result class as the page words it
CLASS = {"identical": "identical", "mixed": "identical, some values to a tolerance", "tolerance": "within a tolerance"}
CLASSES = list(CLASS.values()) + ["partly different"]
# the rows of class "mixed" that differ in what they compare: the page calls them "partly different"
PARTLY = ("Land change: land systems (CLUMondo)", "Land change: roads (friction_cost, pathways)")


def read():
    with open(DATA / "accuracy_tools.csv", encoding="utf8", newline="") as f:
        return list(csv.DictReader(f))


CRLF, LF = chr(13) + chr(10), chr(10)


def write(eol, header, rows):
    out = io.StringIO(newline="")
    w = csv.writer(out, lineterminator=eol)
    w.writerow(header)
    w.writerows(rows)
    return out.getvalue()


def views():
    rows = read()
    table, per_area, counts = [], {}, dict.fromkeys(CLASSES, 0)
    for r in rows:
        area = AREA.get(r["domain"], r["domain"])
        cls = "partly different" if r["terraflux_methods"] in PARTLY else CLASS[r["result_class"]]
        version = "" if r["reference_version"] == "?" else r["reference_version"]
        tol = f"≤ {r['loosest_tolerance']}" if r["loosest_tolerance"] else ""
        table.append([area, r["terraflux_methods"], r["comparison"], version, r["cases"], cls, tol, r["result"]])
        per_area.setdefault(CHART.get(area, area), dict.fromkeys(CLASSES, 0))[cls] += 1
        counts[cls] += 1
    return {
        # the table keeps CRLF, the two small ones LF, as they were committed
        "accuracy_view.csv": write(CRLF, ["domain", "methods", "reference", "version", "cases", "class", "tolerance", "result"], table),
        "accuracy_by_domain.csv": write(LF, ["domain", "c0", "c1", "c2", "c3"],
                                        [[a, *c.values()] for a, c in per_area.items()]),
        "accuracy_classes.csv": write(LF, ["class", "groups"], [[k, v] for k, v in counts.items()]),
    }


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--check", action="store_true")
    args = ap.parse_args()
    bad = 0
    for name, text in views().items():
        path = VIEW / name
        old = path.read_bytes().decode("utf8") if path.exists() else None
        if old == text:
            continue
        bad += 1
        if args.check:
            print(f"{name}: out of date")
        else:
            path.write_bytes(text.encode("utf8"))
            print(f"wrote {name}")
    if args.check:
        sys.exit(1 if bad else 0)
    print(f"{len(read())} groups")


if __name__ == "__main__":
    main()
