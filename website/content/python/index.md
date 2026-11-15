---
title: Terraflux for Python
short: Overview
description: The Python library of Terraflux, the same landscape-ecology methods and the same numbers as the application, to call from a script or a notebook.
order: 0
---

The library is the Python side of Terraflux: the methods of the application (habitat patterns, landscape metrics,
connectivity, spatial prioritization, species distributions, land change simulation) as functions that take and
return rasters, tables and plain Python objects. It is installed from PyPI and used as `tf`:

```bash
pip install terraflux
```

<!-- no-run -->
```python
import terraflux as tf

forest = tf.load_raster("forest.tif")
patterns = tf.mspa(forest, edge_width=2)
```

!!! note "Called landschaft until 1.58"
    The library was published as `landschaft` up to version 1.58. `pip install landschaft` now installs
    `terraflux`, and `import landschaft` keeps working for a while, with a deprecation warning. New code should
    use `import terraflux as tf`.

!!! tip "Application or library?"
    The application builds the same analyses as graphs of operators and runs them on one computer or a cluster.
    The library is for people who prefer to write code. Both give the same results; the
    [quickstart](/quickstart/) is the application's.

The pages of this section, written as the library's documentation is moved here:

- Getting started: installation, a first run, configuration, data and plotting.
- The [API reference](/python/api/): every function and class, from the docstrings of the library.
- The [changelog](/python/changelog/) of the library.
- [Learn landscape ecology](/learn/): the book that goes with the library.
