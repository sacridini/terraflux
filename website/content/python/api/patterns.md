---
title: Habitat patterns
description: "MSPA, SPA, objects by size, parcels and contortion."
order: 30
---

Structural classes of habitat pixels (MSPA, SPA), objects by size,
parcellation and boundary contortion. Tutorial:
[habitat patterns](/tutorials/habitat-patterns/patterns-in-python/).

## MSPA

::: terraflux.mspa

::: terraflux.mspa_stats

### MSPA class codes

`tf.MSPA_CLASSES` maps each code of an MSPA map to its class:

| Code | Class | Code | Class |
|---|---|---|---|
| 17 | Core | 65 | Loop |
| 9 | Islet | 67 | Loop in edge |
| 5 | Perforation | 69 | Loop in perforation |
| 3 | Edge | 33 | Bridge |
| 1 | Branch | 35 | Bridge in edge |
| 0 | Background | 37 | Bridge in perforation |
| 100 | Core-opening | 220 | Border-opening |
| 129 | Missing | +100 | the class inside a core-opening (`internal=True`) |

## SPA

::: terraflux.spa

::: terraflux.SPAResult

## Objects by size

::: terraflux.accounting

::: terraflux.AccountingResult

## Parcels and shape

::: terraflux.parcellation

::: terraflux.ParcellationResult

::: terraflux.contortion
