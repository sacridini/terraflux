---
title: Connectivity
description: "The indices of a habitat graph (Conefor and Makurhini): components, integral index and probability of connectivity, the importance of each patch and link, centrality, the change of connected area and ProtConn."
order: 70
---

::: metrics connectivity

## The example graph {#example}

The values on the cards were computed on the forests of the 50 x 50 km CORINE excerpt around Lake Steinhude and Hannover
that ships with the library and that the habitat networks tutorials use, on 100 m cells. The forests are the nodes (154
patches, 51 569 ha), their edge-to-edge distances the links, and the reserves of the ProtConn cards are the 18 forests of
500 ha or more taken as polygons (hypothetical, not real protected areas). `top` only shortens the tables of the
importance and centrality of the 154 nodes to the three highest; the call on each card is the one that makes them:

```python
import geopandas as gpd
import numpy as np
import rasterio.features
import shapely
import terraflux as tf

# CORINE land cover of the Steinhude-Hannover excerpt (EPSG:3035) on 100 m cells; the forests are the habitat
land = tf.reproject(tf.load_raster(tf.example_data("clc")), res=100, resampling="mode")
kw = dict(area_unit="ha", distance="edge", landscape_area=250_000)       # ha, edge-to-edge distances, a 50 x 50 km landscape
net = tf.HabitatGraph.from_raster(land, habitat=[23, 24, 25], **kw)
scrub = tf.HabitatGraph.from_raster(land, habitat=[23, 24, 25, 29], **kw)   # with transitional woodland-shrub as habitat too
nodes = tf.conn.node_table(land, habitat=[23, 24, 25], area_unit="ha")
candidate = (nodes["attribute"] >= 40).astype(int).to_numpy()            # 0 marks the forests under 40 ha, candidates to add

# hypothetical reserves: the forests of 500 ha or more as polygons, in the whole square and in a region inside it
ids = net.to_raster(nodes[nodes["attribute"] >= 500], "id")
grid = np.nan_to_num(ids.values).astype("int32")
shapes = rasterio.features.shapes(grid, mask=grid > 0, transform=land.rio.transform(), connectivity=8)
reserves = gpd.GeoDataFrame(geometry=[shapely.geometry.shape(s) for s, _ in shapes], crs=land.rio.crs)
x0, y0, x1, y1 = land.rio.bounds()
region = gpd.GeoDataFrame(geometry=[shapely.box(x0, y0, x1, y1)], crs=land.rio.crs)
inner = gpd.GeoDataFrame(geometry=[shapely.box(x0 + 10_000, y0 + 10_000, x1 - 10_000, y1 - 10_000)], crs=land.rio.crs)


def top(table, by, *cols, n=3):
    """The n rows of a table with the highest value of a column, with some other columns."""
    return table.nlargest(n, by)[["id", by, *cols]].round(4)

print(net.n_nodes, "forest patches,", int(nodes["attribute"].sum()), "ha;", len(reserves), "reserves,", int(reserves.area.sum() / 1e4), "ha")
ov = net.overall("PC", distance=1000, probability=0.5)
print(f"PC {ov['PC'][0]:.5f}, EC(PC) {ov['EC(PC)'][0]:.1f} ha")
print(top(net.node_importance("PC", distance=1000, probability=0.5), "dPC").to_string(index=False))
```

```text
154 forest patches, 51569 ha; 18 reserves, 39964 ha
PC 0.00513, EC(PC) 17910.7 ha
 id     dPC
 55 30.6234
110 24.0123
  8 14.2659
```

The landscape area (250 000 ha) is what makes PC and IIC small numbers: they divide by its square, so compare them only
between landscapes of the same size, and read the equivalent connected area (ECA, in hectares) instead. Node 55 is the
largest forest (9 151 ha) and carries about 31% of PC at 1 km. `distance` and `probability` are the two assumptions every
probabilistic card depends on: change them before comparing numbers with a study that used others.
