---
title: Benchmarks
short: Overview
description: How Terraflux's results are checked against the tools they reproduce, how much faster they run, how many tests there are, and how to repeat it all.
order: 0
---

Terraflux makes two promises: **the same results as the established tools**, and **much less waiting for them**. These
pages show how both are checked, with the numbers.

Each method is re-implemented from its publication and its source code. It is then run on the same maps as the original
program (GuidosToolbox, landscapemetrics, Makurhini, Circuitscape, Zonation, prioritizr, NLMR, Dinamica EGO and others)
and the two outputs are compared, pixel for pixel or value for value. The times are taken on the same maps too. The
comparisons are part of the test suite, so they run again at every change.

!!! note "Whose numbers these are"
    The speed and accuracy tables on these pages are those of **the Python library**
    ([terraflux-py](https://github.com/sacridini/terraflux-py), `import terraflux as tf`, version {{pyversion}}), which
    holds the methods and their test suite. None was measured again for these pages: the machine and the version of
    every original tool are stated with each number.

    The application runs the library's own C++ code for land change simulation, landscape metrics, neutral landscape
    models and the GuidosToolbox analyses, and Rust ports of the other methods, tested against the library. Its checks
    are on [the application's own tests](/benchmarks/application/). **The application has no timing table of its own
    yet**: no number here is a measured run time of the application.

## In numbers

| Measure | Result |
|---|---|
| Timed comparisons with an original tool | **121**, in 13 areas ([speed](/benchmarks/speed/)) |
| Median speed-up over the original | **10 times**, from 1.2 to more than 6,000 times |
| Groups of methods compared with their original | **47** ([accuracy](/benchmarks/accuracy/)) |
| Of them identical to the original | **25** identical, **13** identical except some values within a stated tolerance, **7** within a tolerance, **2** partly different |
| The library's tests | **4,678** collected, in 171 files; 81 run the original tool |
| The application's tests | **1,518**: 692 integration and 826 unit tests, plus 83 comparisons with stored references ([application](/benchmarks/application/)) |

## How much faster

Speed-up is the original's time divided by Terraflux's, on the same case. The largest gains are against programs that
work cell by cell in R; the smallest are against programs already written in compiled code, such as WhiteboxTools. The
dot is the median of a domain's cases; the line reaches its smallest and largest.

```tfchart
type: dot
log: true
format: "{v}×"
rule: { value: 1, label: same speed }
labels_at_end: true
data: /data/benchmarks/view/speed_domains.csv
x: domain
y: [median, min, max]
labels: { median: Median, min: Smallest, max: Largest }
xlabel: Times faster than the original tool
caption: "Speed-up of the Python library over the tool it reproduces, by area: the median, smallest and largest of the area's cases (on a logarithmic scale). Spatial prioritization counts each case on 1 and on 12 threads; optimized plans leave out the four cases where the reference gave no answer in its time limit."
```

```tftable
data: /data/benchmarks/view/speed_domains_table.csv
sortable: true
align: { domain: left }
digits: 3
format: { min: "{v}×", median: "{v}×", max: "{v}×" }
labels: { domain: Area, cases: Cases, min: Smallest, median: Median, max: Largest }
caption: "The same numbers as a table. Click a header to sort. [Every case is on the speed page](/benchmarks/speed/)."
```

Times depend on the computer, the map and the options: read them as orders of magnitude.

## How close are the results

The results are compared with the originals in the strictest way the original allows. Integer maps and counts must be
identical. Decimal results are compared bit for bit where the original's arithmetic can be reproduced, and otherwise to a
stated tolerance, usually the original's own precision.

```tftable
sortable: true
align: { class: left, meaning: left }
badge:
  class:
    identical: good
    "identical, some values to a tolerance": info
    "within a tolerance": info
    "partly different": warning
caption: "The 47 comparison groups of the accuracy page, by how they agree with the original."
csv: |
  class,groups,meaning
  identical,25,"Every map, count and value has the same bits as the original's."
  "identical, some values to a tolerance",13,"Maps and counts are identical; some decimal values agree to a stated tolerance (for example 1e-12)."
  "within a tolerance",7,"Values agree to a stated tolerance, mostly 1e-9 to 1e-14, where the original's own arithmetic cannot be reproduced."
  "partly different",2,"The original is closed or approximate in part (CLUMondo's 2017 options, ties in Dinamica EGO's road directions)."
```

Where Terraflux deliberately differs from an original, the page on [accuracy](/benchmarks/accuracy/#known-differences)
says so. Nothing is left out of it.

## The pages

| Page | What it holds |
|---|---|
| [Speed](/benchmarks/speed/) | The timings of every area against its original tool, with a chart and a sortable table for each, how they were measured, and Terraflux's own times on larger problems. |
| [Accuracy](/benchmarks/accuracy/) | How results are compared, the results of every tool, the GuidosToolbox reference programs, what is checked in other ways, the details by module and the known differences. |
| [The application's own tests](/benchmarks/application/) | How the application is checked against the library, every comparison and the script that writes its reference, the test counts per crate, and the other verifications. |
| [Reproduce](/benchmarks/reproduce/) | The commands that repeat the library's and the application's tests, and the variables that point the library's live tests to the original tools. |

## The test suite of the library

As of terraflux-py {{pyversion}}, the library's test suite has **171 test files with 1,665 test functions**, which pytest
collects as **4,678 tests** once the parametrised cases are counted one by one. 81 of them carry the `live_reference`
marker: they run the original tool, so they need it installed. The old guide of the library said "324 test cases";
that figure is out of date.

```bash
git grep -h "^def test_\|^    def test_" -- tests | wc -l          # test functions: 1665
pytest -p no:xdist --collect-only -q tests | tail -1              # collected tests: 4678 tests collected
pytest -p no:xdist --collect-only -q tests -m live_reference | tail -1   # 81 of them need an original tool
```

The numbers of cases inside the tables (for example "53 reference cases" for gdistance) are the cases of one
comparison, not of the whole suite.
