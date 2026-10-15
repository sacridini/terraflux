"""The model of Serra Verde's original vegetation and area of habitat (the tutorial original-vegetation-and-habitat),
which the restoration plans start from."""

HABITAT = """format = "terraflux/1"

[[nodes]]
id = "cover"
op = "raster.read"
params = { path = "land_cover.tif" }

[[nodes]]
id = "past"
op = "raster.read"
params = { path = "past_cover.tif" }

[[nodes]]
id = "ecoregions"
op = "raster.read"
params = { path = "ecoregions.tif" }

[[nodes]]
id = "eco_forest"
op = "map.calc"
inputs = { e = "ecoregions.map" }
params = { expression = "if e == 1 then 0.31 else if e == 2 then 0.98 else if e == 3 then 0.91 else 0.2" }

[[nodes]]
id = "eco_savanna"
op = "map.calc"
inputs = { e = "ecoregions.map" }
params = { expression = "if e == 1 then 0 else if e == 2 then 0.01 else if e == 3 then 0.09 else 0.8" }

[[nodes]]
id = "eco_mangrove"
op = "map.calc"
inputs = { e = "ecoregions.map" }
params = { expression = "if e == 1 then 0.69 else if e == 2 then 0.01 else 0" }

[[nodes]]
id = "eco_shares"
op = "stack.of"
inputs = { forest = "eco_forest.map", savanna = "eco_savanna.map", mangrove = "eco_mangrove.map" }

[[nodes]]
id = "vegetation"
op = "restoration.original_vegetation"
inputs = { land_cover = "cover.stack", past = "past.stack", ecoregions = "eco_shares.stack" }
params = { natural = ["forest", "savanna", "mangrove"], anthropic = ["pasture", "cropland"] }

[[nodes]]
id = "ranges"
op = "raster.read"
params = { path = "features.tif" }

[[nodes]]
id = "habitat"
op = "sdm.area_of_habitat"
inputs = { ranges = "ranges.stack", land_cover = "cover.stack", original = "vegetation.original", restore = "vegetation.restore", anthropic = "vegetation.anthropic" }
params = { natural = ["forest", "savanna", "mangrove"], suitability = { jaguar = ["forest", "savanna"], lowland_tapir = ["forest"], harpy_eagle = ["forest"], maned_wolf = ["savanna"], golden_lion_tamarin = ["forest"], neotropical_otter = ["forest", "mangrove"], araucaria_forest = ["forest"], mangrove = ["mangrove"] } }
"""
