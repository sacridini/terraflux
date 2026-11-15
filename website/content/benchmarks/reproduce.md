---
title: Reproduce
short: Reproduce
description: The commands that repeat the library's comparisons and timings and the application's tests, and the variables that point them to the original tools.
order: 40
---

Every number on these pages can be made again. The library's comparisons are part of its test suite; the application's are part
of its own. Run only what you need: some timings take minutes and need the original tool installed.

## The Python library's tests

The comparisons are part of the library's test suite ([terraflux-py](https://github.com/sacridini/terraflux-py)):

```bash
pip install -e ".[test]"
pytest                              # every test, stored references included
pytest -m live_reference            # also run the originals that are installed
```

Keep the number of cores down, as for any long job: `OMP_NUM_THREADS=4 RAYON_NUM_THREADS=4 pytest`. The live runs look for the
originals in these environment variables:

| Variable | Original |
|---|---|
| `TERRAFLUX_MSPA_EXE` | GuidosToolbox's `mspa` executable (`MSPAstandalone/mspa_win64.exe`) |
| `TERRAFLUX_GDL_EXE` | GDL, to run GuidosToolbox's IDL code |
| `TERRAFLUX_SPATCON_EXE`, `TERRAFLUX_GRAYSPATCON_EXE`, `TERRAFLUX_GGEO_EXE` | GuidosToolbox's helper programs |
| `TERRAFLUX_RSCRIPT`, `TERRAFLUX_R_LIBS` | R with landscapemetrics and Makurhini |
| `TERRAFLUX_JULIA`, `TERRAFLUX_JULIA_PROJECT` | Julia with Circuitscape and Omniscape |

[`tools/make_reference*.py`](https://github.com/sacridini/terraflux-py/tree/main/tools) regenerate the stored references from
the originals, and `python tools/benchmark.py --exe <mspa executable>` repeats the MSPA timings.

- [`tools/make_reference_optimize.py`](https://github.com/sacridini/terraflux-py/blob/main/tools/make_reference_optimize.py) and
  [`tools/benchmark_optimize.py`](https://github.com/sacridini/terraflux-py/blob/main/tools/benchmark_optimize.py) use R with
  prioritizr, prioriactions and gurobi (`TERRAFLUX_RSCRIPT`, `TERRAFLUX_R_LIBS`); Marxan's references come from its executable
  in WSL.
- [`tools/make_reference_prioritize.py`](https://github.com/sacridini/terraflux-py/blob/main/tools/make_reference_prioritize.py)
  and [`tools/benchmark_prioritize.py`](https://github.com/sacridini/terraflux-py/blob/main/tools/benchmark_prioritize.py) look
  for Zonation 5's `z5.exe` and `z5_16bit.exe` in `TERRAFLUX_Z5_DIR`.
- [`tools/make_reference_sdm.py`](https://github.com/sacridini/terraflux-py/blob/main/tools/make_reference_sdm.py) and
  [`tools/benchmark_sdm.py`](https://github.com/sacridini/terraflux-py/blob/main/tools/benchmark_sdm.py) use R with the packages
  listed on the [speed page](/benchmarks/speed/#species-distributions) in `TERRAFLUX_SDM_R_LIBS` (default
  `%LOCALAPPDATA%/R/landschaft-sdm-lib`).
- [`tools/make_reference_simulate.py`](https://github.com/sacridini/terraflux-py/blob/main/tools/make_reference_simulate.py) and
  [`tools/benchmark_simulate.py`](https://github.com/sacridini/terraflux-py/blob/main/tools/benchmark_simulate.py) use R with
  lulcc, OpenLand and diffeR (`TERRAFLUX_SIM_R_LIBS`, default `%LOCALAPPDATA%/R/landschaft-sim-lib`), CLUMondo
  (`TERRAFLUX_CLUMONDO`, `TERRAFLUX_CLUMONDO_FIXED`), GRASS 8 with the `r.futures` addons in WSL, Dinamica EGO's console
  (`TERRAFLUX_DINAMICA`) and MOLUSCE's sources (`TERRAFLUX_MOLUSCE`, `TERRAFLUX_MOLUSCE_FIXED`);
  [`tools/benchmark_dinamica.py`](https://github.com/sacridini/terraflux-py/blob/main/tools/benchmark_dinamica.py) times the
  Dinamica EGO functors added in 1.46 on 4 cores.
- The other timings of the [speed page](/benchmarks/speed/) have their scripts named in their sections:
  `benchmark_surface.py`, `benchmark_hydro.py`, `benchmark_connect.py`, `benchmark_zonal.py` and `benchmark_plot.py`.

The default names of the R libraries still say "landschaft", the library's name when they were made.

## The application's tests

Clone the application with its submodule (the C++ kernels come from the library) and test the crate you want, with the cores
limited so the computer stays usable:

```bash
git clone --recurse-submodules https://github.com/sacridini/terraflux.git
cd terraflux
cargo test -p terraflux-ops -j 4 -- --test-threads=4 name_of_a_test
RAYON_NUM_THREADS=4 OMP_NUM_THREADS=4 cargo test -p terraflux-exec -j 4
```

`-j 4` limits the build, `--test-threads=4` (or the variable `RUST_TEST_THREADS=4`) the tests, and the two variables the
libraries inside Terraflux. The crates are `terraflux-app`, `terraflux-ops`, `terraflux-exec`, `terraflux-graph`,
`terraflux-raster` and others (the [counts per crate](/benchmarks/application/#the-tests-of-the-application) list them). The
tests of the whole workspace take a long time: test the crate you changed, with a name filter. The
[contributing guide](/about/contributing/#build-and-test-the-code) has the same advice.

Some parity tests differ in the last bit on macOS and Linux, because their references were written on Windows; the application's
README lists them.

### The ignored tests

Long tests and the tests that need the network are marked `#[ignore]` and do not run by default. The speed-and-result test of
the optimizer, which compares the application with the library on 122 cases, needs a release build, one test at a time and four
threads, or its times mean nothing:

```bash
RAYON_NUM_THREADS=4 OMP_NUM_THREADS=4 cargo test -p terraflux-ops --release --test ops -j 4 -- --ignored --test-threads 1 --nocapture parity::
```

`PARITY_ONLY=small_raster,medium_h3` runs only the cases whose names contain one of those words.

### The reference scripts

The references the application's tests read are in `tests/data/reference`. They were written by the scripts of the
application's [`tools`](https://github.com/sacridini/terraflux/tree/main/tools) folder: 72 `make_*.py` scripts, each with a
comment at its top that says what it writes and how to run it, for example `python tools/make_nlm_reference.py`. They need the
Python library installed (and, for a few, R, numpy, scipy or an original program); the
[table of comparisons](/benchmarks/application/#every-comparison) names the script of each group of operators. The tests read
the stored files, so you do not need to run a script to run a test.
