---
title: About Terraflux
short: About
description: What Terraflux is for, how it is built, and how to take part.
order: 0
---

Terraflux is a free, open-source desktop application and command line for **landscape ecology, conservation
planning and land use and land cover change modelling**. It brings into one program methods that ecologists
usually run in a dozen tools, in as many languages: habitat patterns, landscape metrics, connectivity, spatial
prioritization, restoration planning, species distributions, biodiversity analysis and land change simulation.

## What it is for

- **One model, many front-ends.** An analysis is a graph of operators saved as a small text file. The guided
  wizard, the node editor, the command line, Python and the file itself all read and write the same graph.
- **Easy without hiding anything.** Every automatic choice of the wizard becomes an explicit node with a note
  saying why, so a beginner can accept sensible defaults and an expert can change any of them.
- **The same numbers as the reference tools.** Methods are re-implemented from their publications and source
  code, and the test suite checks each against its reference implementation (GuidosToolbox, landscapemetrics,
  Conefor, Circuitscape, gdistance, Zonation, Marxan, prioritizr, plangea, MaxNet, Biodiverse, Dinamica EGO,
  lulcc, PLUS and others), most to the last bit.
- **Any scale.** The same model runs on a laptop, on the computers of a lab, on a university cluster or in the
  cloud, and maps larger than memory run in tiles with the same result whatever the number of machines.

## How it is built

| Part | Technology |
|---|---|
| The application | Rust, with an egui and wgpu interface drawn by the GPU |
| Simulation and analysis kernels | C++17 and OpenMP from [landschaft](https://github.com/sacridini/landschaft), and Rust |
| Geographic data | GDAL (bundled with the installers), and a GeoTIFF reader and writer of its own |
| Many machines | gRPC between a coordinator and its workers, TLS with pairing codes |
| Python | landschaft writes and reads Terraflux models (`ls.simulate`) |

**landschaft** is Terraflux's sister project: a Python library with the same methods, for those who prefer to
work in notebooks. A model fitted in Python runs in Terraflux unchanged, and the other way round.

## License

Terraflux is free software under the **GNU General Public License, version 3 or later** (GPL-3.0-or-later). You
may use it for any purpose, study and change its source code, and share it, changed or not, under the same
license.

## Taking part

- **Report a problem or ask for a feature** in the [issues](https://github.com/sacridini/terraflux/issues) of the repository: say what you
  did, what you expected, what happened, and attach the model file (`.toml`) if you can.
- **Improve this documentation:** every page has a link at its end, "Suggest a change to this page".
- **Contribute code:** the repository's README explains how to build Terraflux and run its tests.

## Citing

See [Cite Terraflux](/about/citing/): cite the software and, above all, the papers of the methods you used.
