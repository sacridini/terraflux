---
title: Batch processing and raster collections
description: "Any tool over many rasters, bounded memory and resuming, and sharing the work with the other computers of a lab."
order: 70
---

The same in the app: [Batch processing](/tutorials/large-data/batch-processing/) and [Rasters larger than memory](/tutorials/large-data/larger-than-memory/); the application's many-computers pages are in [Parallel computing](/parallel/). The same as a worked tutorial in Python: [Large data in Python](/tutorials/large-data/large-data-in-python/). API: [`tf.batch`][terraflux.batch], [`tf.mspa_batch`][terraflux.mspa_batch], [`tf.BatchResult`][terraflux.BatchResult] and [`tf.helpers`][terraflux.parallel.helpers] on the [large data and batch](/python/api/batch/) page. Settings that apply to every batch (threads, `max_memory`) are on the [configuration](/python/getting-started/configuration/#memory) page.

The examples need a folder of rasters. This one makes four tiles from the example map:

```python
import os
import terraflux as tf

data = tf.load_raster(tf.example_data("input"))
os.makedirs("tiles", exist_ok=True)
for i, (r, c) in enumerate([(0, 0), (0, 500), (500, 0), (500, 500)]):
    tf.write_raster(data[r:r + 500, c:c + 500], f"tiles/tile{i}.tif")


def my_function(raster):                      # any function of one raster
    return tf.fad(raster, window_size=7)
```

## Batch of any tool

Any tool can run on many rasters (GTB's *Batch Process*):

```python
results = tf.batch("accounting", "tiles/*.tif", "out/", highlight_largest=True, max_memory="16GB")
tf.batch("spa", "tiles/", "out/", variant=6)
tf.batch(my_function, "tiles/", "out/")     # any function of one raster
[r.outputs for r in results]                # GeoTIFF maps, CSV tables, TXT report, JSON record
```

Outputs are named `<name>_<tool>[_<field>].<ext>`. The JSON record
(`<name>_<tool>.json`) holds the parameters, the output files and the scalar
results, and lets a rerun skip finished inputs (`overwrite=True` recomputes them).

## Bounded memory

Process thousands of raster tiles efficiently without exhausting system RAM:

```python
# Header-only scan of directory
tiles = tf.load_raster("tiles/")

# Batch process with bounded memory (max 16 GB RAM used concurrently)
results = tf.mspa_batch(
    tiles,
    output_dir="./mspa_output",
    edge_width=2,
    n_workers=4,
    max_memory="16GB",
    overwrite=False,  # resumes if interrupted
)
```

## Several computers

Many independent tasks can be shared with the other computers of a lab. Each
helper joins the group once (`terraflux-py helpers invite` on the main computer,
`terraflux-py worker --join <code>` on the helper) and then lends itself with
`terraflux-py worker`. Functions that run many tasks take `on=`:

<!-- no-run -->
```python
with tf.helpers() as pool:
    tf.batch("fad", "tiles/", "fad_out/", on=pool)
    ens = tf.simulate.scenarios(model, start, drivers, replicates=200, seed=1, on=pool)
```

The results are the same as on one computer. The full walk-through (pairing a helper, shared folders, what runs where) is in [many computers in Python](/parallel/many-computers-in-python/) and, for a cluster, [HPC with Slurm in Python](/parallel/hpc-slurm-in-python/). Helpers run only terraflux's own
operations (a task is an operation's name, its JSON parameters and its input
files, never code), connect out over TLS and prove the group's secret; input
files come from their cache, a shared folder (NAS) or the network.
