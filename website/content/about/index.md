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
  Conefor, Circuitscape, gdistance, Zonation, Marxan, MaxNet, Biodiverse, Dinamica EGO,
  lulcc, PLUS and others), most to the last bit: see [validation and benchmarks](/about/validation/).
- **Any scale.** The same model runs on a laptop, on the computers of a lab, on a university cluster or in the
  cloud, and maps larger than memory run in tiles with the same result whatever the number of machines.

## How it is built

| Part | Technology |
|---|---|
| The application | Rust, with an egui and wgpu interface drawn by the GPU |
| Simulation and analysis kernels | C++17 and OpenMP from [terraflux-py](https://github.com/sacridini/terraflux-py), and Rust |
| Geographic data | GDAL (bundled with the installers), and a GeoTIFF reader and writer of its own |
| Many machines | gRPC between a coordinator and its workers, TLS with pairing codes |
| Python | The Python library terraflux (`import terraflux as tf`) writes and reads Terraflux models (`tf.simulate`) |

**terraflux-py** is Terraflux's sister project: a Python library (`pip install terraflux-py`) with the same methods, for those who prefer to
work in notebooks. A model fitted in Python runs in Terraflux unchanged, and the other way round.

## License

Terraflux is free software under the **GNU General Public License, version 3 or later** (GPL-3.0-or-later). You
may use it for any purpose, study and change its source code, and share it, changed or not, under the same
license. The full text is the [LICENSE](https://github.com/sacridini/terraflux/blob/main/LICENSE) file of the
repository.

The [Python library](/python/) is under the same license, GPL-3.0-or-later, and comes without any warranty, as the
license says; its [LICENSE](https://github.com/sacridini/terraflux-py/blob/main/LICENSE) is in its repository. The
methods it implements come from published research and open-source tools (GuidosToolbox, landscapemetrics,
Conefor Sensinode, Makurhini, Circuitscape, Omniscape, and the others named on its pages), whose licenses are
compatible with the GPL. Its C++ core uses [Eigen](https://eigen.tuxfamily.org) (MPL-2.0) and
[pybind11](https://github.com/pybind/pybind11) (BSD). It ports code from GuidosToolbox and miallib, (c) European
Union (Joint Research Centre), authors Pierre Soille, Peter Vogt et al., both GPL-3.0-or-later; the example maps
in the library come from the [GTB repository](https://github.com/ec-jrc/GTB). GuidosToolbox's `mspa` notes that its
interface is made available for non-commercial purposes: contact the JRC for commercial use of MSPA.

## Supported systems and versions

Each release is built and tested for these systems:

| System | Supported |
|---|---|
| Windows | Windows 10 and 11, 64-bit |
| macOS | Apple silicon (M1 or later) |
| Linux | x86-64, glibc 2.39 or newer (Ubuntu 24.04 and later, Debian 13 and later, Fedora 40 and later, Arch) |

GDAL is bundled with the packages. A build from source on another system may work, but is not tested.

**Only the latest release is supported.** A bug, or a security problem, is fixed in the next release; fixes are not
back-ported to older ones. Terraflux tells you when a newer version is out (see [Privacy and network
access](/about/privacy/)), and the [release notes](/about/changelog/) say what changed.

**Version numbers** follow [Semantic Versioning](https://semver.org/). Since version 1.0, **a model saved by an
earlier version opens in later ones**; a change that would break that, in the model format, the command line or the
Python interface, comes only with a new major version. New features and fixes come in minor and patch versions.

## Taking part

- **Report a problem or ask for a feature** in the [issues](https://github.com/sacridini/terraflux/issues) of the repository: say what you
  did, what you expected, what happened, and attach the model file (`.toml`) if you can. [Contributing](/about/contributing/)
  says what makes a report easy to act on.
- **Improve this documentation:** every page has a link at its end, "Suggest a change to this page".
- **Contribute code:** [Contributing](/about/contributing/) explains how to build Terraflux, run its tests and send a change.
- **Behave kindly:** everyone follows the [Code of Conduct](https://github.com/sacridini/terraflux/blob/main/CODE_OF_CONDUCT.md).
- **Report a security problem in private**, as the [security policy](https://github.com/sacridini/terraflux/blob/main/SECURITY.md) says.
- **Know what Terraflux sends over the network:** [Privacy and network access](/about/privacy/).

## Citing

See [Cite Terraflux](/about/citing/): cite the software and, above all, the papers of the methods you used.
