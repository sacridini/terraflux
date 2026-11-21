---
title: Circuit connectivity
description: "Omniscape and Circuitscape: current, resistance distance, networks."
order: 70
---

Connectivity from electrical circuit theory: omnidirectional current with
moving windows (Omniscape), current and effective resistance between focal
points, sources and grounds, on rasters and on networks (Circuitscape).
Tutorial: [circuit connectivity](/tutorials/circuit-connectivity/).

`tf.omniscape` is at the top level; the other functions are in the
`tf.circuit` module (`from terraflux import circuit`).

!!! tip "Resistance, conductance and current"
    A resistance map says how hard each cell is to cross (1 = easy, larger =
    harder); conductance is its inverse. Current is injected at sources and
    leaves at grounds; the current through a cell shows how much movement
    funnels through it. The effective resistance between two points falls
    when there are more, shorter or easier paths between them.

## Omnidirectional current

::: terraflux.circuit.omniscape.omniscape
    options:
      show_root_full_path: false

::: terraflux.circuit.omniscape_ini

::: terraflux.circuit.source_from_resistance

::: terraflux.circuit.read_reclass_table

::: terraflux.circuit.reclassify

::: terraflux.circuit.parse_ini

## Focal points on rasters

::: terraflux.circuit.circuitscape_pairwise

::: terraflux.circuit.circuitscape_one_to_all

::: terraflux.circuit.circuitscape_ini

::: terraflux.circuit.CircuitscapeResult
    options:
      members: false

::: terraflux.circuit.FocalPoints

::: terraflux.circuit.IncludedPairs

## Sources and grounds

::: terraflux.circuit.circuitscape_advanced

::: terraflux.circuit.advanced.advanced
    options:
      show_root_full_path: false

::: terraflux.circuit.AdvancedResult
    options:
      members: false

## Networks

::: terraflux.circuit.circuitscape_network_pairwise

::: terraflux.circuit.circuitscape_network_advanced

::: terraflux.circuit.NetworkResult
    options:
      members: false

::: terraflux.circuit.read_graph
