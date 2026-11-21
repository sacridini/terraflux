---
title: Metrics
description: "Every metric Terraflux computes, by family, theme and level: what it measures, its formula, the Python call with a real value, and the setting that selects it in the application."
order: 1
---

Terraflux computes hundreds of metrics, from the area of a patch to the importance of a node in a habitat network. This
section details each of them on a card with four parts:

1. **What it measures**: the idea in plain words, its unit and range, and when to use it or avoid it.
2. **Formula**: the equation (the one the code computes) with each symbol explained.
3. **In Python**: the call, a real value computed on a small example, and a link to the library's reference.
4. **In the application**: the operator that computes it and the setting that selects it.

The metrics are grouped in **families** by the tool or the field they come from; the families listed below have their
cards today, and the others follow. Inside a family the cards are grouped by theme (area and edge, shape, aggregation,
diversity...). To find one, search below by name, abbreviation (`ED`, `CONTAG`, `Sa`) or by what it measures, and narrow
the list by family, theme or level (a patch, a class, the whole landscape, a map...). The site's own search (Ctrl K)
finds the metrics too, with the cards.

::: metrics-index
