"""Reference for segment.sam's prompts: the same ONNX weights run by Python's onnxruntime (no torch), the prompt
tensors built the way segment.sam builds them (points 1 inside and 0 outside plus a padding point of label -1, or a
box by its two corners as labels 2 and 3), on the test scene of crates/ops/tests/objects_dl.rs. Writes
tests/data/reference/sam_prompts.json: per case the prompts (in cells) and the cells of the mask the decoder gives
(runs of the row-major cell numbers), cut to the connected area that holds the first point.

    python tools/make_sam_prompts_reference.py [encoder.onnx decoder.onnx]

Without arguments the weights of Terraflux's cache (the quantised ViT-B) are used.
"""

import json
import os
import sys
from pathlib import Path

import numpy as np
import onnxruntime as ort
from scipy import ndimage

W, H = 400, 300
FRAME = (683, 1024)


def scene():
    shapes = [
        (80.0, 90.0, 45.0, True, [220.0, 60.0, 50.0]),
        (90.0, 250.0, 40.0, False, [50.0, 80.0, 210.0]),
        (200.0, 100.0, 38.0, False, [240.0, 220.0, 60.0]),
        (190.0, 240.0, 50.0, True, [40.0, 190.0, 80.0]),
    ]
    bands = np.zeros((3, H, W))
    for r in range(H):
        for c in range(W):
            y, x = float(r), float(c)
            k = None
            for j, (cy, cx, s, disc, _) in enumerate(shapes):
                inside = np.hypot(y - cy, x - cx) <= s if disc else abs(y - cy) <= s and abs(x - cx) <= s
                if inside:
                    k = j
                    break
            tex = ((r * 31 + c * 17) % 13) - 6.0 + 4.0 * (np.sin(x / 9.0) + np.cos(y / 7.0))
            base = shapes[k][4] if k is not None else [90.0, 100.0, 95.0]
            for b in range(3):
                bands[b, r, c] = min(max(base[b] + tex, 0.0), 255.0)
    return bands


def stretch(band):
    s = np.sort(band.ravel())
    at = lambda q: s[int(np.floor((len(s) - 1) * q + 0.5))]
    lo, hi = at(0.02), at(0.98)
    return (lo, hi) if hi > lo else (lo, lo + 1.0)


def resized(bands):
    scale = min(FRAME[1] / W, FRAME[0] / H)
    nw = min(max(int(np.floor(W * scale + 0.5)), 1), FRAME[1])
    nh = min(max(int(np.floor(H * scale + 0.5)), 1), FRAME[0])

    def axis(n, length):
        x = np.clip((np.arange(n) + 0.5) * length / n - 0.5, 0, length - 1)
        i0 = np.floor(x).astype(int)
        return i0, np.minimum(i0 + 1, length - 1), (x - i0).astype(np.float32)

    (y0, y1, fy), (x0, x1, fx) = axis(nh, H), axis(nw, W)
    out = []
    for b in range(3):
        lo, hi = stretch(bands[b])
        src = (np.clip((bands[b] - lo) / (hi - lo), 0, 1) * 255.0).astype(np.float32)
        a = src[y0][:, x0] * (1 - fx) + src[y0][:, x1] * fx
        c = src[y1][:, x0] * (1 - fx) + src[y1][:, x1] * fx
        out.append(a * (1 - fy)[:, None] + c * fy[:, None])
    img = np.stack(out, axis=-1).astype(np.float32)
    size = (round(FRAME[0] / scale), round(FRAME[1] / scale))
    return img, scale, size


def tensors(prompts, scale):
    coords, labels = [], []
    box = None
    for p in prompts:
        if p["kind"] == "point":
            coords.append([p["x"] * scale, p["y"] * scale])
            labels.append(1.0 if p["positive"] else 0.0)
        elif box is None:
            box = p
    if box is not None:
        coords += [[box["x0"] * scale, box["y0"] * scale], [box["x1"] * scale, box["y1"] * scale]]
        labels += [2.0, 3.0]
    else:
        coords.append([0.0, 0.0])
        labels.append(-1.0)
    return np.array([coords], dtype=np.float32), np.array([labels], dtype=np.float32)


def anchor(prompts):
    for p in prompts:
        if p["kind"] == "point" and p["positive"]:
            return p["x"], p["y"]
    for p in prompts:
        if p["kind"] == "box":
            return (p["x0"] + p["x1"]) / 2, (p["y0"] + p["y1"]) / 2
    return None


def runs(mask):
    flat = np.flatnonzero(mask.ravel())
    out = []
    for i in flat:
        if out and out[-1][0] + out[-1][1] == i:
            out[-1][1] += 1
        else:
            out.append([int(i), 1])
    return out


def main():
    if len(sys.argv) == 3:
        enc_path, dec_path = sys.argv[1:]
    else:
        base = Path(os.environ["LOCALAPPDATA"]) / "Terraflux" / "cache" / "sam" / "vit_b_quant"
        enc_path, dec_path = base / "encoder.onnx", base / "decoder.onnx"
    opts = ort.SessionOptions()
    opts.intra_op_num_threads = 4
    enc = ort.InferenceSession(str(enc_path), opts, providers=["CPUExecutionProvider"])
    dec = ort.InferenceSession(str(dec_path), opts, providers=["CPUExecutionProvider"])
    bands = scene()
    img, scale, size = resized(bands)
    emb = enc.run(["image_embeddings"], {"input_image": img})[0]
    pt = lambda x, y, positive=True: {"kind": "point", "x": x, "y": y, "positive": positive}
    box = lambda x0, y0, x1, y1: {"kind": "box", "x0": x0, "y0": y0, "x1": x1, "y1": y1}
    cases = {
        "point": [pt(90.5, 80.5)],
        "point_and_outside": [pt(100.5, 200.5), pt(300.5, 40.5, False)],
        "box": [box(190.0, 140.0, 290.0, 240.0)],
        "box_and_point": [box(190.0, 140.0, 290.0, 240.0), pt(240.5, 190.5)],
    }
    result = {"width": W, "height": H, "cases": []}
    for name, prompts in cases.items():
        coords, labels = tensors(prompts, scale)
        feeds = {
            "image_embeddings": emb,
            "point_coords": coords,
            "point_labels": labels,
            "mask_input": np.zeros((1, 1, 256, 256), np.float32),
            "has_mask_input": np.zeros(1, np.float32),
            "orig_im_size": np.array(size, np.float32),
        }
        masks, iou = dec.run(["masks", "iou_predictions"], feeds)
        k = int(np.argmax(iou.ravel()))
        inside = masks[0, k, :H, :W] > 0
        # the connected area (4 neighbours) that holds the anchor, else the largest
        lab, n = ndimage.label(inside)
        ax, ay = anchor(prompts)
        at = lab[int(ay), int(ax)]
        if at == 0 and n > 0:
            at = 1 + int(np.argmax(ndimage.sum(inside, lab, range(1, n + 1))))
        keep = (lab == at) if at else np.zeros_like(inside)
        result["cases"].append({"name": name, "prompts": prompts, "iou": float(iou.ravel()[k]), "runs": runs(keep)})
        print(name, int(keep.sum()), float(iou.ravel()[k]))
    out = Path(__file__).resolve().parent.parent / "tests" / "data" / "reference" / "sam_prompts.json"
    out.write_text(json.dumps(result))
    print("wrote", out)


if __name__ == "__main__":
    main()
