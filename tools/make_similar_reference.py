# SPDX-License-Identifier: GPL-3.0-or-later
"""What numpy gives for objects.similar, for Terraflux's tests: 30 objects with four embedding columns (emb_1..emb_4)
and four attributes of other scales (ndvi, mean_nir, sd_nir, compactness), two examples (the objects 3 and 17), and
for each object the best cosine, and the best 1 / (1 + Euclidean distance), to the examples: on the embeddings as they
are, and on the attributes standardised (z-scores, the standard deviation of the population). Written to
tests/data/reference/similar.json:

    python -I tools/make_similar_reference.py

Needs numpy.
"""

from __future__ import annotations

import json
from pathlib import Path

import numpy as np

ROOT = Path(__file__).resolve().parents[1]
REF = ROOT / "tests" / "data" / "reference"


def best(x: np.ndarray, ex: list[int]) -> tuple[list[float], list[float]]:
    e = x[[k - 1 for k in ex]]
    unit = lambda a: a / np.linalg.norm(a, axis=1, keepdims=True)  # noqa: E731
    cos = (unit(x) @ unit(e).T).max(axis=1)
    dist = np.linalg.norm(x[:, None, :] - e[None, :, :], axis=2)
    return cos.tolist(), (1.0 / (1.0 + dist)).max(axis=1).tolist()


def main() -> None:
    rng = np.random.default_rng(20261123)
    n = 30
    emb = rng.normal(size=(n, 4))
    attrs = np.stack(
        [rng.uniform(-0.2, 0.9, n), rng.uniform(0.0, 0.4, n), rng.uniform(0.0, 0.05, n), rng.uniform(1.0, 2.5, n)],
        axis=1,
    )
    examples = [3, 17]
    z = (attrs - attrs.mean(axis=0)) / attrs.std(axis=0)
    emb_cos, emb_euc = best(emb, examples)
    z_cos, z_euc = best(z, examples)
    data = {
        "emb": emb.tolist(),
        "attrs": attrs.tolist(),
        "examples": examples,
        "emb_cosine": emb_cos,
        "emb_euclidean": emb_euc,
        "std_cosine": z_cos,
        "std_euclidean": z_euc,
    }
    REF.mkdir(parents=True, exist_ok=True)
    (REF / "similar.json").write_text(json.dumps(data), encoding="utf-8")
    print("written", REF / "similar.json")


if __name__ == "__main__":
    main()
