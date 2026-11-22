# SPDX-License-Identifier: GPL-3.0-or-later
"""What Spectral Python gives for the minimum noise fraction, for Terraflux's tests of ml.pca with method mnf: an image
of five bands (smooth signals of two sources with noise of another size in each band), the noise statistics by the
difference with the right-hand neighbour (`noise_from_diffs` with direction "right": the covariance of the differences,
over 2, with n - 1) and by the mean of that and of the difference with the lower neighbour, `mnf`, and the scores
(`reduce`), the eigenvalues and the denoised image (`denoise`), written to tests/data/reference/mnf.json:

    python -I tools/make_mnf_reference.py

Needs numpy and spectral (pip install spectral, in a virtual environment). The eigenvectors are given up to sign, so
the test aligns each component's sign before it compares.
"""

from __future__ import annotations

import json
from pathlib import Path

import numpy as np
import spectral
from spectral.algorithms.algorithms import GaussianStats, calc_stats, mnf, noise_from_diffs

ROOT = Path(__file__).resolve().parents[1]
REF = ROOT / "tests" / "data" / "reference"
ROWS, COLS = 24, 20


def image(rng: np.random.Generator) -> np.ndarray:
    """Five bands: two smooth sources mixed, plus white noise of a different size in each band."""
    y, x = np.mgrid[0:ROWS, 0:COLS]
    s1 = np.sin(x / 4.0) + 0.5 * np.cos(y / 5.0)
    s2 = (x + y) / 20.0 + np.sin(y / 3.0)
    mix = np.array([[40.0, 5.0], [10.0, 25.0], [0.3, 0.2], [-20.0, 15.0], [8.0, -9.0]])
    sd = np.array([4.0, 2.0, 0.05, 6.0, 1.0])
    bands = [mix[b, 0] * s1 + mix[b, 1] * s2 + 100.0 * b + rng.normal(scale=sd[b], size=(ROWS, COLS)) for b in range(5)]
    return np.stack(bands, axis=2)


def result(X: np.ndarray, noise: GaussianStats, k: int) -> dict:
    signal = calc_stats(X)
    r = mnf(signal, noise)
    return {
        "noise_cov": noise.cov.tolist(),
        "signal_cov": signal.cov.tolist(),
        "mean": signal.mean.tolist(),
        "eigenvalues": np.real(r.napc.eigenvalues).tolist(),
        # the scores of every cell, row by row, all the components; the loadings are the matrix that makes them
        "scores": np.real(r.reduce(X, num=X.shape[2])).reshape(-1, X.shape[2]).tolist(),
        "loadings": np.real(r.get_reduction_transform(num=X.shape[2])._A).tolist(),
        "denoised": np.real(r.denoise(X, num=k)).reshape(-1, X.shape[2]).tolist(),
        "k": k,
    }


def main() -> None:
    rng = np.random.default_rng(20261123)
    X = image(rng)
    right = noise_from_diffs(X, "right")
    lower = noise_from_diffs(X, "lower")
    both = GaussianStats(mean=right.mean, cov=(right.cov + lower.cov) / 2.0, nsamples=right.nsamples)
    out = {
        "spectral": spectral.__version__,
        "rows": ROWS,
        "cols": COLS,
        "bands": ["b1", "b2", "b3", "b4", "b5"],
        "image": X.reshape(-1, 5).tolist(),
        "shift": result(X, right, 2),
        "shift_both": result(X, both, 2),
    }
    (REF / "mnf.json").write_text(json.dumps(out), encoding="utf-8")
    print("wrote", REF / "mnf.json")


if __name__ == "__main__":
    main()
