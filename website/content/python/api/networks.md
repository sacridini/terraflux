---
title: Habitat networks
description: "Habitat graphs, IIC/PC/ECA, importance, ProtConn, grids, metapopulations, MSPA networks."
order: 60
---

Graphs of habitat patches linked by distance or by dispersal probability,
their connectivity indices (IIC, PC, ECA...), the importance of every patch
and link, protected-area connectivity (ProtConn), grids and time series, and
the network of MSPA cores and bridges. Tutorial:
[habitat networks](/tutorials/habitat-networks/).

The graph class is `tf.HabitatGraph`; the other functions are in the
`tf.conn` module (`import terraflux as tf`, then `tf.conn.protconn(...)`).

## Habitat graphs

::: terraflux.HabitatGraph
    options:
      members:
        - from_raster
        - from_vector
        - from_samc
        - from_conefor
        - n_nodes
        - overall
        - node_importance
        - link_importance
        - link_change
        - components
        - centrality
        - to_raster
        - to_geodataframe
        - to_conefor

## Nodes and distances

::: terraflux.conn.node_table

::: terraflux.conn.distance_table

::: terraflux.conn.settlement_probabilities

::: terraflux.conn.boundary_vertices

## Protected-area connectivity

::: terraflux.conn.protconn

::: terraflux.conn.protconn_regions

::: terraflux.conn.protconn_raster

::: terraflux.conn.protconn_nodes

::: terraflux.conn.ProtConnNodes
    options:
      members: false

::: terraflux.conn.protconn_indicators

## Grids

::: terraflux.conn.make_grid

::: terraflux.conn.connectivity_grid

::: terraflux.conn.eca_change_grid

## Change and neighbourhoods

::: terraflux.conn.eca_change

::: terraflux.conn.focal_nodes

## Metapopulations

Hanski's incidence function model as R's MetaLandSim (`spom`) iterates it: the colonisation and extinction of the
patches of a network over time, the same run as R's for a seed.

::: terraflux.conn.metapopulation

::: terraflux.conn.Metapopulation

## Networks of MSPA cores and bridges

::: terraflux.gtn_components

::: terraflux.GTNComponentsResult

::: terraflux.gtn_importance

::: terraflux.GTNImportanceResult

::: terraflux.conefor_inputs
