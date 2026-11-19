"""Reference for objects.embed: a tiny ONNX model (Conv 3x3 + ReLU + global average pool) and numpy's embeddings of
a fixed image and objects, as the operator does them (bands standardised, patches of P x P padded with 0, the mean of
the patches an object's cells fall in weighted by the cells).

Needs numpy and onnx (the model is also run by onnxruntime when it is installed, as a cross-check).
Writes tests/data/reference/embed_conv.onnx and embed.json. Run: python tools/make_embed_reference.py
"""

import json
import os

import numpy as np
import onnx
from onnx import TensorProto, helper, numpy_helper

OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "tests", "data", "reference")
C, P, D = 3, 16, 4
W, H = 40, 24

rng = np.random.default_rng(20261119)
weight = rng.normal(size=(D, C, 3, 3)).astype(np.float32)
bias = rng.normal(size=(D,)).astype(np.float32)

x = helper.make_tensor_value_info("x", TensorProto.FLOAT, [1, C, P, P])
y = helper.make_tensor_value_info("y", TensorProto.FLOAT, [1, D])
graph = helper.make_graph(
    [
        helper.make_node("Conv", ["x", "w", "b"], ["c"], pads=[1, 1, 1, 1], kernel_shape=[3, 3]),
        helper.make_node("Relu", ["c"], ["r"]),
        helper.make_node("GlobalAveragePool", ["r"], ["g"]),
        helper.make_node("Flatten", ["g"], ["y"], axis=1),
    ],
    "embed_conv",
    [x],
    [y],
    [numpy_helper.from_array(weight, "w"), numpy_helper.from_array(bias, "b")],
)
model = helper.make_model(graph, opset_imports=[helper.make_opsetid("", 13)])
model.ir_version = 8
onnx.checker.check_model(model)
onnx.save(model, os.path.join(OUT, "embed_conv.onnx"))


def run_model(patch):
    """The model in numpy: patch is C x P x P float32."""
    pad = np.pad(patch, ((0, 0), (1, 1), (1, 1)))
    out = np.zeros((D, P, P), np.float32)
    for d in range(D):
        acc = np.full((P, P), bias[d], np.float32)
        for c in range(C):
            for i in range(3):
                for j in range(3):
                    acc += weight[d, c, i, j] * pad[c, i : i + P, j : j + P]
        out[d] = np.maximum(acc, 0)
    return out.reshape(D, -1).mean(axis=1)


# the image: three bands of values with 3 decimals (so JSON holds them exactly), a cell without a value
bands = np.round(rng.uniform(0, 100, size=(C, H, W)), 3)
bands[1, 3, 5] = np.nan
# the objects: rectangles, numbered by their first cell in row-major order; 0 is none
labels = np.zeros((H, W), np.int64)
labels[2:10, 3:20] = 1
labels[2:12, 24:38] = 2
labels[14:23, 4:30] = 3
labels[16:22, 33:39] = 4

std = []
for b in bands:
    v = b[~np.isnan(b)]
    s = v.std() or 1.0
    std.append(np.where(np.isnan(b), 0.0, (b - v.mean()) / s).astype(np.float32))
std = np.stack(std)

count = int(labels.max())
acc = np.zeros((count, D))
weights = np.zeros(count)
for r0 in range(0, H, P):
    for c0 in range(0, W, P):
        lab = labels[r0 : r0 + P, c0 : c0 + P]
        if not lab.any():
            continue
        patch = np.zeros((C, P, P), np.float32)
        patch[:, : lab.shape[0], : lab.shape[1]] = std[:, r0 : r0 + P, c0 : c0 + P]
        e = run_model(patch)
        for k in range(1, count + 1):
            n = int((lab == k).sum())
            acc[k - 1] += e * n
            weights[k - 1] += n
emb = acc / weights[:, None]

try:
    import onnxruntime as ort

    s = ort.InferenceSession(os.path.join(OUT, "embed_conv.onnx"))
    p = np.zeros((1, C, P, P), np.float32)
    p[0] = std[:, :P, :P]
    got = s.run(None, {"x": p})[0][0]
    assert np.allclose(got, run_model(p[0]), atol=1e-5), "numpy and onnxruntime differ"
except ImportError:
    pass

json.dump(
    {
        "width": W,
        "height": H,
        "patch": P,
        "bands": [[None if np.isnan(v) else float(v) for v in b.reshape(-1)] for b in bands],
        "labels": labels.reshape(-1).tolist(),
        "embeddings": emb.tolist(),
    },
    open(os.path.join(OUT, "embed.json"), "w"),
)
print("embeddings", emb)
