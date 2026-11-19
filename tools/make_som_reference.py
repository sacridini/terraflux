# SPDX-License-Identifier: GPL-3.0-or-later
"""What MiniSom (2.3.x) gives, for Terraflux's tests of the self-organizing map (crates/ops/src/multivar/som.rs and
codebook.rs, the operator ml.som): the cases of Zeit's parity fixtures (tests/data/make_som_parity_fixtures.py of the
zeit repository: online and batch trainings, the gaussian, bubble, triangle and mexican hat neighbourhoods, the three
decays, the hexagonal topology, 6, 24 and 130 variables) and some more of the hexagonal topology (an even and an odd
number of rows). Per case: the data, the weights before the training, the order of the records of an online
training, MiniSom's coordinates of the nodes and the trained weights, and on the trained weights MiniSom's winners,
quantization_error, topographic_error and distance_map (scaling "mean"). Every float is written as its 64 bits (a
whole number), so the tests compare bit by bit. Written to tests/data/reference/som.json:

    python tools/make_som_reference.py

Needs numpy and minisom (the cases were made with minisom 2.3.6).
"""

from __future__ import annotations

import json
from pathlib import Path

import minisom
import numpy as np
from minisom import MiniSom

ROOT = Path(__file__).resolve().parents[1]
REF = ROOT / "tests" / "data" / "reference"

# name: (MiniSom arguments, how the weights start, training method, its arguments)
CASES = {
    "online_sequential": (dict(x=5, y=5, sigma=1.2), "random", "train", dict(num_iteration=500)),
    "online_random_order": (dict(x=6, y=4, sigma=1.5), "none", "train", dict(num_iteration=800, random_order=True)),
    "online_epochs": (dict(x=4, y=4, sigma=1.0), "pca", "train", dict(num_iteration=3, use_epochs=True)),
    "online_hex_mexican": (
        dict(
            x=6,
            y=5,
            sigma=1.3,
            topology="hexagonal",
            neighborhood_function="mexican_hat",
            decay_function="linear_decay_to_zero",
            sigma_decay_function="linear_decay_to_one",
        ),
        "random",
        "train",
        dict(num_iteration=400),
    ),
    "online_bubble": (
        dict(
            x=5,
            y=5,
            sigma=2.0,
            neighborhood_function="bubble",
            decay_function="inverse_decay_to_zero",
            sigma_decay_function="inverse_decay_to_one",
        ),
        "random",
        "train",
        dict(num_iteration=400, random_order=True),
    ),
    "online_triangle_d24": (
        dict(x=5, y=6, sigma=2.0, neighborhood_function="triangle"),
        "random",
        "train",
        dict(num_iteration=400),
    ),
    "online_d130": (dict(x=3, y=3, sigma=1.0), "random", "train", dict(num_iteration=120)),
    "batch_gaussian": (dict(x=6, y=6, sigma=2.0), "random", "train_batch_offline", dict(num_iteration=15)),
    "batch_hex_d24": (
        dict(x=5, y=7, sigma=1.5, topology="hexagonal"),
        "random",
        "train_batch_offline",
        dict(num_iteration=10),
    ),
    # the hexagonal topology: an even number of rows (y), then an odd one, each with its neighbourhoods
    "online_hex_gaussian_even": (
        dict(x=7, y=6, sigma=2.0, topology="hexagonal", decay_function="asymptotic_decay"),
        "random",
        "train",
        dict(num_iteration=500, random_order=True),
    ),
    "online_hex_bubble_odd": (
        dict(x=6, y=5, sigma=2.0, topology="hexagonal", neighborhood_function="bubble"),
        "random",
        "train",
        dict(num_iteration=400),
    ),
    "batch_hex_gaussian_even": (
        dict(
            x=8,
            y=6,
            sigma=3.0,
            topology="hexagonal",
            decay_function="linear_decay_to_zero",
            sigma_decay_function="linear_decay_to_one",
        ),
        "pca",
        "train_batch_offline",
        dict(num_iteration=25),
    ),
    "batch_hex_triangle_odd": (
        dict(x=6, y=7, sigma=3.0, topology="hexagonal", neighborhood_function="triangle"),
        "random",
        "train_batch_offline",
        dict(num_iteration=12),
    ),
}


def make_data(name: str, n_features: int) -> np.ndarray:
    """Four groups of 75 records round random centres, in a random order (as Zeit's fixtures)."""
    rng = np.random.RandomState(sum(map(ord, name)))
    centers = rng.uniform(-4, 4, size=(4, n_features))
    x = np.vstack([c + rng.normal(0, 0.6, size=(75, n_features)) for c in centers])
    return x[rng.permutation(len(x))]


def bits(a: np.ndarray) -> list[int]:
    """The 64 bits of each float, flattened (C order)."""
    return np.ascontiguousarray(a, dtype=np.float64).view(np.uint64).ravel().tolist()


def main() -> None:
    out = {"minisom": getattr(minisom, "__version__", "2.3.6"), "cases": {}}
    for name, (kwargs, init, method, train_kwargs) in CASES.items():
        n_features = 24 if name.endswith("d24") else 130 if name.endswith("d130") else 6
        x = make_data(name, n_features)
        kwargs = dict(kwargs, input_len=n_features, learning_rate=0.5, random_seed=11)
        som = MiniSom(**kwargs)
        if init == "random":
            som.random_weights_init(x)
        elif init == "pca":
            som.pca_weights_init(x)
        start = som.get_weights().copy()
        case = {
            "som": {k: v for k, v in kwargs.items() if k not in ("random_seed",)},
            "method": method,
            "train": train_kwargs,
            "data": bits(x),
            "init": bits(start),
        }
        if method == "train":
            # the order of the records, drawn as train() draws it, from a copy of the generator's state
            gen = np.random.RandomState()
            gen.set_state(som._random_generator.get_state())
            use_epochs = train_kwargs.get("use_epochs", False)
            order = list(
                minisom._build_iteration_indexes(
                    len(x),
                    train_kwargs["num_iteration"],
                    False,
                    gen if train_kwargs.get("random_order") else None,
                    use_epochs,
                )
            )
            # with epochs the engine takes one epoch's order and repeats it
            case["order"] = [int(i) for i in (order[: len(x)] if use_epochs else order)]
        xx, yy = som.get_euclidean_coordinates()
        getattr(som, method)(x, **train_kwargs)
        weights = som.get_weights()
        case["xx"], case["yy"] = bits(xx), bits(yy)
        case["weights"] = bits(weights)
        case["winners"] = [int(i * kwargs["y"] + j) for i, j in (som.winner(r) for r in x)]
        case["quantization_error"] = float(som.quantization_error(x))
        case["topographic_error"] = float(som.topographic_error(x))
        case["distance_map"] = [float(v) for v in som.distance_map(scaling="mean").ravel()]
        out["cases"][name] = case
    (REF / "som.json").write_text(json.dumps(out), encoding="utf-8")
    print("wrote", REF / "som.json", len(out["cases"]), "cases")


if __name__ == "__main__":
    main()
