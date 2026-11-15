---
title: The application's own tests
short: The application
description: How the application is checked against the Python library and the original tools, every comparison and the script that writes its reference, the test counts per crate, and the other verifications.
order: 30
---

The speed and accuracy pages give the numbers of the Python library. This page is about the application: how its results are
made to agree with the library's, how many tests there are, and what else was verified. The application and the library agree
because they share their kernels and because the application is checked against the library, which is itself checked against
the original tools ([accuracy](/benchmarks/accuracy/)).

!!! note "No timing of the application yet"
    The application has no table of run times against the library or against an original tool. The only timings recorded for
    it are those of the interface and a few engines, in [the other verifications](#other-verifications). Where a method is
    timed on the [speed page](/benchmarks/speed/), the time is the library's.

## Terraflux, the application

- **Shared kernels.** For land change simulation, landscape metrics, neutral landscape models and the GuidosToolbox analyses,
  the application runs the library's own C++ code (its `terraflux-ls-sys` crate is built from the library's sources), so a model
  run in the application gives the same bits as the library with the same seed.
- **Rust ports, each tested against the library.** The application's other operators are Rust ports. The tests under
  `crates/*/tests` of the [application's repository](https://github.com/sacridini/terraflux) read references stored in
  `tests/data/reference`, which scripts named `tools/make_*.py` write: there are **72** of them, 60 named
  `make_*_reference.py`. Most run the library; a few take their references from numpy, scipy or R directly, and a few only
  write input data or source files. The comparison is to the bit wherever the library's arithmetic can be reproduced, and to a
  rounding stated in the test where the library takes its result from numpy's BLAS, SIMD exponentials or LAPACK (the habitat
  graphs' tests say which columns; the circuit tests compare the maps to the bit).
- **Models saved by the library.** 13 models written by the library's `Model.save` (CARS, CLUE, CLUMondo, Dinamica, FUTURES,
  MOLUSCE and others) are run again by the application and must give the library's maps bit for bit.
- **Original tools, in two hops.** For most comparisons the application is checked against the library, not against the
  original: application = library (this page), library = original ([accuracy](/benchmarks/accuracy/)). Thirteen of the
  comparisons read stored outputs of original tools that the library keeps (landscapemetrics, prioritizr, Zonation 5, proxirr,
  Marxan, plangea, exactextractr, GuidosToolbox's `mspa` and others). Six scripts run an original directly: R's mapaccuracy,
  gstat, MetaLandSim and riverconn, MigClim and UDef-ARP.

The scripts are in the application's [`tools`](https://github.com/sacridini/terraflux/tree/main/tools) folder; each one starts
with a comment that says what it writes and how to run it. Some test comments and changelog lines still say "landschaft", the
name the library had when the references were written; the references are those of the library ({{pyversion}} today).

## What the comparisons are

The table below lists **83** comparisons, one for each group of operators tested together: 36 compare bit for bit, 31 bit for
bit with some values to a stated tolerance, 7 only to a tolerance, 7 have no external reference (consistency checks and sums
done by hand), and 2 are source files written by a script rather than comparisons. The charts count the first four kinds,
by kind and by area (Other: vectors, statistics, data).

```tfchart
type: bar
horizontal: true
labels_at_end: true
data: /data/benchmarks/view/app_classes.csv
x: class
y: [rows]
labels: { rows: Comparisons }
xlabel: Comparisons (of 81 that compare something)
caption: "The application's comparisons by how they agree: bit for bit, bit for bit with some values to a stated tolerance, only to a tolerance, or with no external reference (consistency checks on the application itself, or sums done by hand)."
```

```tfchart
type: bar
horizontal: true
stacked: true
labels_at_end: true
data: /data/benchmarks/view/app_by_domain.csv
x: group
y: [c0, c1, c2, c3]
labels: { c0: bit for bit, c1: "bit for bit, some values to a tolerance", c2: to a tolerance, c3: no external reference }
xlabel: Comparisons
caption: "The same comparisons by area. Prioritization (Zonation, prioritizr, Marxan and the optimized plans) and land change have the most. The tolerance of each comparison is stated in the table below."
```

## Every comparison

For each group of operators: what it was compared with, how the two agree, the tolerance where there is one, the number of
reference cases (left empty where the test does not count them), the script that writes the reference and the test that reads
it. Click a header to sort, for example by "Result" to see every comparison that is not bit for bit.

```tftable
data: /data/benchmarks/view/app_view.csv
columns: [domain, operators, against, class, comparison, tolerance, cases, script, test]
sortable: true
align: { domain: left, operators: left, against: left, class: left, comparison: left, tolerance: left, cases: right, script: left, test: left }
digits: 0
labels: { domain: Area, operators: Operators, against: Compared with, class: Result, comparison: How they agree, tolerance: Tolerance, cases: Cases, script: Script, test: Test }
badge:
  class:
    bit for bit: good
    "bit for bit, some values to a tolerance": info
    to a tolerance: info
    no external reference: info
    generated source file: info
link: { script: "https://github.com/sacridini/terraflux/blob/main/" }
caption: "\"Compared with\" says what the reference is: the library's output (53 groups), a stored output of an original tool (13), an original run by the script (6), numpy, scipy or scikit-learn (4), or nothing outside the application (7)."
```

!!! note "Counts that differ by one"
    The application's Zonation tests run **65** cases; the library's table on the accuracy page says 64 reference projects, and
    the changelog entry for the output files says 64. The difference is one case, which we have not traced. Likewise the
    changelog gives 18 cases for `swat.watershed` and its reference file holds 19.

## The tests of the application

The application's repository has **1,518 tests**: 692 integration tests (in `crates/*/tests`) and 826 unit tests (inside the
source files). They were counted by searching the source for `#[test]` and `#[tokio::test]`, so the number a test run reports
can differ (macros, ignored tests). One integration test often loops over many reference cases, which is why the 83
comparisons above hold far more cases than tests. The search finds 44 `#[ignore]` marks (long runs, tests that need the network, and the speed-and-result test of the optimizer; a few may be in comments).

```tfchart
type: bar
horizontal: true
labels_at_end: true
data: /data/benchmarks/view/tests_by_crate.csv
x: crate
y: [integration, unit]
labels: { integration: Integration tests, unit: Unit tests }
xlabel: Tests
caption: "Tests of each crate of the workspace, by kind. Crates with no tests are left out."
```

```tftable
data: /data/benchmarks/tests.csv
sortable: true
align: { scope: left, kind: left, notes: left }
digits: 0
labels: { scope: Scope, kind: Kind, files: Files, tests: Tests, ignored: Ignored, notes: Note }
caption: "Counts by crate, then the totals, the reference data and the library's own counts. The application's reference data are 316 files in all (182 top-level entries); 13 bridge fixtures are models saved by the library."
```

## Other verifications

Besides the stored references, the changelog records what was checked by other means: the same operators in memory against out
of core, models run in QGIS, the interface's frame times, and the speed of the optimizer's engines. Timings here are the
application's, on the machine of the day, and are not comparisons with another tool unless the row says so.

```tftable
data: /data/benchmarks/view/app_checks_view.csv
columns: [version, date, kind, area, checked, against, result]
sortable: true
align: { version: left, date: left, kind: left, area: left, checked: left, against: left, result: left }
badge:
  kind:
    comparison: good
    timing: info
    external check: info
labels: { version: Version, date: Date, kind: Kind, area: Area, checked: What was checked, against: Against what, result: Result }
caption: "From the changelog, newest first. \"Comparison\" rows compare results; \"timing\" rows record a time before and after a change, or a scale figure; the \"external check\" opened the exported QGIS project in QGIS 3.34 itself."
```

### The speed-and-result test of the optimizer

One test compares the application with the library on **122 optimization cases**: every objective on every kind of planning
unit (raster cells, tables, Marxan's rij, polygons, H3 hexagons, management zones), with the same solver (HiGHS, 4 threads).
Each case must give the library's compiled model (size and sums), its status and objective (within the gap asked), its selected
units where the optimum is unique, and a time **at most 1.5 times the library's plus a quarter of a second**. It is marked
`#[ignore]` because it takes minutes and needs a release build; what is kept is that it passes, not a table of the seconds. The library's
seconds on those cases (HiGHS, median of 3, 4 threads, library 1.58.1) are kept in
[`optimize_parity_cases.csv`](/data/benchmarks/optimize_parity_cases.csv), so a table of the application's own seconds next to
them can be made the next time the test is run in a release build. This page does not have it yet.

## What is not here yet

- **Timings of the application** against the library or an original: none, apart from the rows above.
- **Linux and macOS timings:** none. The results are the same bits as on Windows, except for the differences in the
  [known differences](/benchmarks/accuracy/#known-differences); some parity tests of the application differ in the last bit on
  macOS and Linux because their references were written on Windows (the README lists them).
- **Cases not counted:** 20 comparisons do not state their number of cases; they are left empty above.
