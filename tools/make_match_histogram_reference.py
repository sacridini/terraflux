# SPDX-License-Identifier: GPL-3.0-or-later
"""What scikit-image gives for raster.match_histogram, for Terraflux's tests: a source of three bands (40 x 50 cells,
values rounded to two decimals so that many are equal) and a reference of another size (30 x 60) and other
distributions, matched with skimage.exposure.match_histograms (channel_axis the bands); and the same with a mask, where
the cells of the mask make the distributions (the source's inside the mask, the whole reference) and the cells outside
take the value numpy's interp gives from the same two distributions (the extension raster.match_histogram makes).
Written to tests/data/reference/match_histogram.json:

    python -I tools/make_match_histogram_reference.py

Needs numpy and scikit-image.
"""

from __future__ import annotations

import json
from pathlib import Path

import numpy as np
import skimage
from skimage.exposure import match_histograms

ROOT = Path(__file__).resolve().parents[1]
REF = ROOT / "tests" / "data" / "reference"


def cdf(values: np.ndarray) -> tuple[np.ndarray, np.ndarray]:
    """np.unique and the cumulative share (what match_histograms makes of each side)."""
    v, counts = np.unique(values.ravel(), return_counts=True)
    return v, np.cumsum(counts) / values.size


def main() -> None:
    rng = np.random.default_rng(20261122)
    source = np.stack(
        [
            np.round(rng.normal(0.10, 0.03, (40, 50)), 2),
            np.round(rng.gamma(2.0, 0.05, (40, 50)), 2),
            np.round(rng.uniform(0.0, 0.5, (40, 50)), 2),
        ],
        axis=-1,
    )
    reference = np.stack(
        [
            np.round(rng.normal(0.25, 0.06, (30, 60)), 2),
            np.round(rng.gamma(3.0, 0.08, (30, 60)), 2),
            np.round(rng.uniform(0.1, 0.9, (30, 60)), 2),
        ],
        axis=-1,
    )
    matched = match_histograms(source, reference, channel_axis=-1)
    mask = rng.uniform(size=(40, 50)) < 0.6
    masked = np.empty_like(source)
    for b in range(3):
        s, r = source[..., b], reference[..., b]
        inside = match_histograms(s[mask], r.ravel())
        sv, sq = cdf(s[mask])
        tv, tq = cdf(r)
        out = np.interp(np.interp(s, sv, sq), tq, tv)
        out[mask] = inside
        masked[..., b] = out
    data = {
        "skimage": skimage.__version__,
        "source": source.tolist(),
        "reference": reference.tolist(),
        "matched": matched.tolist(),
        "mask": mask.astype(int).tolist(),
        "masked": masked.tolist(),
    }
    REF.mkdir(parents=True, exist_ok=True)
    (REF / "match_histogram.json").write_text(json.dumps(data), encoding="utf-8")
    print("written", REF / "match_histogram.json")


if __name__ == "__main__":
    main()
