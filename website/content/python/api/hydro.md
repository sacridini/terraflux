---
title: Terrain hydrology
description: "Filling, flow directions and accumulation, streams, watersheds, HAND, wetness indices, river networks."
order: 90
---

Where water goes on a terrain: depressions filled so that every cell drains,
the direction each cell drains to, how much area drains through it, the
stream network, the height above the nearest drainage (HAND) and the wetness,
stream power and sediment transport indices. They live in the `tf.hydro`
module.

The directions and accumulations are those of WhiteboxTools 2.4 cell for cell
(`D8Pointer`, `DInfPointer`, `D8FlowAccumulation`, `DInfFlowAccumulation`,
`FD8FlowAccumulation`, `ElevationAboveStream`); the filling is its
`FillDepressions` with water let out at the map's edge and at missing cells,
which WhiteboxTools does not do, and the breaching its
`BreachDepressionsLeastCost` with the channels that reach the edge cut. Flat
areas drain by the method of Barnes, Lehman & Mulla (2014). The `"mfd"`
method and `distance_to_channel` are the routing of the InVEST models
(pygeoprocessing 2.4's `flow_dir_mfd`, `flow_accumulation_mfd`,
`distance_to_channel_mfd`), cell for cell; its `fill_pits` gives the same DEM
as `fill`.

<!-- no-run -->
```python
import terraflux as tf

dem = tf.load_raster("srtm.tif")
filled = tf.hydro.fill(dem)
acc = tf.hydro.flow_accumulation(filled)                 # D8, in cells
rivers = tf.hydro.streams(acc, 1000)
height = tf.hydro.hand(filled, rivers)                   # height above the drainage
wetness = tf.hydro.twi(filled)                           # FD8 contributing area
```

## Conditioning

::: terraflux.hydro.fill

::: terraflux.hydro.breach

## Flow

::: terraflux.hydro.flow_direction

::: terraflux.hydro.flow_accumulation

::: terraflux.hydro.streams

::: terraflux.hydro.distance_to_channel

`tf.hydro.D8_CODES` maps the D8 codes to their directions: 1 east, 2
south-east, 4 south, 8 south-west, 16 west, 32 north-west, 64 north, 128
north-east (0: no lower neighbour).

## Stream links and watersheds

The division of a stream network into links (between junctions and outlets),
the area that drains to each seed and outlets moved onto the streams: the
delineation steps of the watershed model (see
[`tf.hydro.swat.watershed`](/python/api/swat/#from-the-landscape)).

::: terraflux.hydro.stream_links

::: terraflux.hydro.watersheds

::: terraflux.hydro.snap_outlets

## Rivers as a network

The tools of hydrographr on the Hydrography90m and of R's riverconn, on the D8
directions: points (gauges, dams, records) moved onto the streams, the land
upstream of every link or point summarised (the mean slope, the share of
forest upstream), the catchment of every point as a polygon, distances along
the river between points, the stretches up and down the river from each point,
the centrality of the links, and the dendritic connectivity index of a network
cut by barriers (Cote et al. 2009) with the gain of removing each barrier.

::: terraflux.hydro.snap_points

::: terraflux.hydro.upstream

::: terraflux.hydro.Upstream

::: terraflux.hydro.upstream_catchment

::: terraflux.hydro.Catchments

::: terraflux.hydro.network_distance

::: terraflux.hydro.along_network

::: terraflux.hydro.NetworkReach

::: terraflux.hydro.centrality

::: terraflux.hydro.dci

::: terraflux.hydro.DCI

## Height above the nearest drainage

::: terraflux.hydro.hand

## Topographic indices

::: terraflux.hydro.twi

::: terraflux.hydro.spi

::: terraflux.hydro.sti

## Sediment connectivity

The index of connectivity of Borselli et al. (2008) and Cavalli et al. (2013), as
InVEST's sediment delivery ratio model computes it
([`tf.services.sdr`](/python/api/services/#sediment-delivery-ratio)).

::: terraflux.hydro.connectivity_index
