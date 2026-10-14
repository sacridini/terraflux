# SPDX-License-Identifier: GPL-3.0-or-later
"""CoordinateCleaner's reference tables (landschaft's reference_data, read from CoordinateCleaner 3.0.1 on CRAN,
GPL-3) as the gzipped CSV files Terraflux carries inside its executable, crates/ops/data/sdm/*.csv.gz:

    python tools/make_sdm_refdata.py

countryref (the centroids of the countries and provinces and the capitals), institutions (biodiversity
institutions) and aohi (artificial hotspots of occurrences): the columns the cleaning tests use and the names.
Numbers are written as Python's repr (they are read back to the bit), missing values as empty fields.
"""

from __future__ import annotations

import csv
import gzip
import io
import math
from pathlib import Path

import landschaft as ls

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "crates" / "ops" / "data" / "sdm"

COLUMNS = {
    "countryref": ["iso3", "iso2", "name", "type", "centroid.lon", "centroid.lat", "capital", "capital.lon",
                   "capital.lat"],
    "institutions": ["name", "type", "country", "decimalLongitude", "decimalLatitude"],
    "aohi": ["ID", "taxa", "category", "decimalLongitude", "decimalLatitude"],
}


def cell(v) -> str:
    if v is None:
        return ""
    if isinstance(v, float):
        return "" if math.isnan(v) else repr(v)
    s = str(v)
    return "" if s in ("<NA>", "nan") else s


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    for name, cols in COLUMNS.items():
        d = ls.sdm.reference_data(name)[cols]
        buf = io.StringIO()
        w = csv.writer(buf, lineterminator="\n")
        w.writerow(cols)
        for row in d.itertuples(index=False):
            w.writerow([cell(v) for v in row])
        path = OUT / f"{name}.csv.gz"
        with open(path, "wb") as f, gzip.GzipFile(fileobj=f, mode="wb", mtime=0, filename="") as z:
            z.write(buf.getvalue().encode("utf-8"))
        print(path, len(d), path.stat().st_size)


if __name__ == "__main__":
    main()
