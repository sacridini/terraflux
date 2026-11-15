---
title: Networks from MSPA
description: "Network components, node and link importance and Conefor files from the cores and bridges of an MSPA map."
order: 20
---

The same in the app: [Networks and change](/tutorials/habitat-patterns/networks-and-change/) and [A network from MSPA](/tutorials/habitat-networks/mspa-network/). API: [`tf.gtn_components`][terraflux.gtn_components], [`tf.gtn_importance`][terraflux.gtn_importance] and [`tf.conefor_inputs`][terraflux.conefor_inputs], on the [habitat networks](/python/api/networks/) page. It needs an [MSPA map](/python/algorithms/mspa-and-spa/); the example map of the library is used here.

GuidosToolbox's *Topological Network Analysis* (GTN) identifies functional components and ranks the structural importance of individual habitat nodes and corridors. The cores of an MSPA map are the nodes, its bridges the links.

```python
import terraflux as tf

data = tf.load_raster(tf.example_data("input"))

# Step 1: Run MSPA to get segmented classes
mspa_map = tf.mspa(data, edge_width=2)

# Step 2: Extract connected network components
net = tf.gtn_components(mspa_map)
print(net.summary())
# Output includes:
# - Equivalent Connected Area (ECA) and ECA_max
# - Degree of Coherence (DOC [%])
# - Number of network components and isolated cores

# Step 3: Compute node and link importance (Probability of Connectivity - dPC)
importance = tf.gtn_importance(mspa_map)
print(importance.nodes.head())          # stepping-stone nodes: node_id, dpc, degree, normalized_importance
print(importance.links.head())          # links: link_id, dpc, connected_nodes, normalized_importance
# importance.importance_map: dPC on the pixels of every link (positive) and node (negative)

# Step 4: Export to Conefor Sensinode
nodes, links = tf.conefor_inputs(mspa_map, output_dir="./conefor_files")
# Writes mspa_nodes_mspa.txt and mspa_links_mspa.txt; nodes and links are also returned as tables
```

The importance of a link is the loss of connectivity (dPC, the drop of the numerator of the probability of connectivity, in %) when it is cut; the importance of a node with two or more links is its contribution to connecting the others. The maps classify the top 1 %, 5 % and 10 %.

Conefor's own indices (NC, NL, H, CCP, LCP, IIC, F, AWF, PC) and node importances on
any table of nodes and distances are in [habitat connectivity graphs](/python/algorithms/connectivity-graphs/).

## References

- Vogt, P., & Riitters, K. (2017). GuidosToolbox: universal digital image object analysis. *European Journal of Remote Sensing*, 50(1), 352–361. https://doi.org/10.1080/22797254.2017.1330650
- Saura, S., & Pascual-Hortal, L. (2007). A new habitat availability index to integrate connectivity in landscape conservation planning: comparison with existing indices and application to a case study. *Landscape and Urban Planning*, 83(2–3), 91–103. https://doi.org/10.1016/j.landurbplan.2007.03.005
- Saura, S., & Rubio, L. (2010). A common currency for the different ways in which patches and links can contribute to habitat availability and connectivity in the landscape. *Ecography*, 33(3), 523–537. https://doi.org/10.1111/j.1600-0587.2009.05760.x
- Saura, S., Vogt, P., Velázquez, J., Hernando, A., & Tejera, R. (2011). Key structural forest connectors can be identified by combining landscape spatial pattern and network analyses. *Forest Ecology and Management*, 262(2), 150–160. https://doi.org/10.1016/j.foreco.2011.03.017

More references, with how the studies used each method, are at the end of the application's tutorials of the same subject (linked at the top of this page).
