//! Models to start from: small graphs that run as they are (their data drawn by neutral landscape models) or
//! once a file is chosen, to see how a flow is built and to check that it works.

use terraflux_graph::{Data, DataKind, Model, Node, Source};
use terraflux_ops::presets;

/// A model to start from: its name, its group in the menu, what it shows, and the model (written, or built from a
/// preset).
pub struct Template {
    pub name: &'static str,
    pub group: &'static str,
    pub about: &'static str,
    toml: &'static str,
    build: Option<fn() -> Model>,
}

impl Template {
    pub fn model(&self) -> Model {
        match self.build {
            Some(f) => f(),
            None => Model::from_toml(self.toml).expect("a template is a model"),
        }
    }
}

const PATTERNS: &str = "Patterns (GuidosToolbox)";
const METRICS: &str = "Landscape metrics";
const CONNECTIVITY: &str = "Connectivity";
const CONSERVATION: &str = "Conservation and restoration";
const SPECIES: &str = "Species";
const LAND_CHANGE: &str = "Land change";
const SURFACES: &str = "Surfaces and uncertainty";
const OBJECTS: &str = "Objects (OBIA)";

/// The templates' groups, in the menu's order.
pub const GROUPS: [&str; 8] = [PATTERNS, METRICS, CONNECTIVITY, CONSERVATION, SPECIES, LAND_CHANGE, SURFACES, OBJECTS];

/// The classes of the land use drawn for the land change templates.
const LAND_CLASSES: [(i64, &str); 3] = [(1, "forest"), (2, "farming"), (3, "water")];

/// Two dates of land use drawn for the land change templates: classes 1 (forest), 2 (farming) and 3 (water) by a
/// random cluster, and ten years later farming grown into the forest beside it where a suitability is high; the
/// suitability and a roughness, the drivers of the change. Nodes `land_2000`, `land_2010`, `drivers`.
fn land_change_data(m: &mut Model) {
    m.nodes.push(
        Node::new("land_2000", "nlm.random_cluster")
            .param("rows", 160)
            .param("columns", 160)
            .param("p", 0.5)
            .param("weights", serde_json::json!([0.55, 0.35, 0.10]))
            .param("seed", 11)
            .note("Land use drawn by chance: 1 forest, 2 farming, 3 water. In its place, a raster.read of a map of yours.")
            .data(Data::new("a land use map, the first date", DataKind::Classes).classes(&LAND_CLASSES)),
    );
    m.nodes.push(
        Node::new("aptitude", "nlm.gaussian_field")
            .param("rows", 160)
            .param("columns", 160)
            .param("range", 25)
            .param("seed", 12)
            .note("A driver: how suited each cell is to farming (0 to 1)."),
    );
    m.nodes.push(
        Node::new("roughness", "nlm.fbm")
            .param("rows", 160)
            .param("columns", 160)
            .param("dimension", 1.4)
            .param("seed", 13)
            .note("A driver of no effect on the change, as real models have some."),
    );
    m.nodes.push(
        Node::new("land_2010", "raster.calc")
            .input("t0", "land_2000.map")
            .input("suit", "aptitude.map")
            .param("expression", "2 if t0 == 1 and nbsum(t0 == 2, 1) >= 2 and suit > 0.45 else t0")
            .note("Ten years later: farming grown into the forest beside it, where it is suited.")
            .data(Data::new("a land use map, the second date", DataKind::Classes).classes(&LAND_CLASSES)),
    );
    m.nodes.push(
        Node::new("drivers", "stack.of")
            .input("aptitude", "aptitude.map")
            .input("roughness", "roughness.map")
            .note("The drivers of the change, a layer each.")
            .data(Data::new(
                "the drivers of the change, a layer each (here a suitability for farming and a roughness)",
                DataKind::Stack,
            )),
    );
}

/// The references to inputs (`$name`) of nodes (and of their bodies) replaced by references to nodes, the seed by
/// a number.
fn rewire(nodes: &mut [Node], to: &[(&str, &str)]) {
    for n in nodes {
        for src in n.inputs.values_mut() {
            if let Source::Ref(r) = src {
                if r == "$seed" {
                    *src = Source::Literal(7.into());
                } else if let Some((_, by)) = to.iter().find(|(from, _)| *from == r.as_str()) {
                    *r = by.to_string();
                }
            }
        }
        if let Some(b) = &mut n.body {
            rewire(&mut b.nodes, to);
        }
    }
}

/// Dinamica EGO's classic model on the data drawn: Markov rates, weights of evidence, Expander and Patcher.
fn dinamica() -> Model {
    let mut m = Model::new("Dinamica EGO");
    m.description = Some(
        "Dinamica EGO's classic model, calibrated on two dates of land use drawn by chance (2000 and 2010) and \
         run 3 steps of 10 years from 2010."
            .into(),
    );
    land_change_data(&mut m);
    presets::dinamica(&mut m, &presets::Dinamica { start: "t1".into(), steps: 1, simulate: 3, ..Default::default() });
    rewire(&mut m.nodes, &[("$t0", "land_2000.map"), ("$t1", "land_2010.map"), ("$drivers", "drivers.stack")]);
    m.nodes.push(Node::new("plot", "view.plot").input("value", "simulation.maps").param("title", "Simulated land use"));
    m
}

/// PLUS on the data drawn: the land expansion analysis (a random forest per class) and CARS in a loop.
fn plus() -> Model {
    let mut m = Model::new("PLUS (LEAS and CARS)");
    m.description = Some(
        "PLUS (Liang et al. 2021): its land expansion analysis (LEAS, a random forest per class of where it \
         appeared between two dates of land use drawn by chance, 2000 and 2010) gives each class's growth \
         probability, and CARS allocates three steps of 10 years from 2010, the demand projected by the Markov \
         chain of the period."
            .into(),
    );
    land_change_data(&mut m);
    m.nodes.push(
        Node::new("leas", "ml.potential")
            .input("landscape", "land_2000.map")
            .input("later", "land_2010.map")
            .input("drivers", "drivers.stack")
            .param("classes", serde_json::json!([2]))
            .param("n_estimators", 50)
            .param("seed", 1)
            .note(
                "PLUS's land expansion analysis: for every class that grows (here farming, the only one), a random \
                 forest of where it appeared between the two dates (among the cells without it) on the drivers. \
                 potential is its growth probability, importance each driver's contribution.",
            ),
    );
    m.nodes.push(
        Node::new("markov", "markov.fit")
            .input("t0", "land_2000.map")
            .input("t1", "land_2010.map")
            .literal("steps", 1)
            .note("The transition probabilities of the period, which project the demand."),
    );
    m.nodes.push(
        Node::new("demand", "plus.demand")
            .input("start", "land_2010.map")
            .input("markov", "markov.markov")
            .param("steps", 3)
            .note("The cells of each class at each of three steps: the Markov chain projected from 2010 (PLUS's default)."),
    );
    let body = terraflux_graph::Body {
        nodes: vec![
            Node::new("cars", "cars.step")
                .input("landscape", "@landscape")
                .input("probability", "leas.potential")
                .input("demand", "demand.demand")
                .input("step", "@step")
                .literal("seed", 7)
                .param("step_size", 50)
                .note("CARS: the classes compete for the cells by their growth probability, their neighbours and the gap to the demand, and random seeds start new patches."),
        ],
        carry: [("landscape".to_string(), "cars.landscape".to_string())].into(),
        collect: [("maps".to_string(), "cars.landscape".to_string()), ("info".to_string(), "cars.info".to_string())]
            .into(),
        ..Default::default()
    };
    m.nodes.push(
        Node::new("simulation", "control.repeat")
            .input("times", "demand.steps")
            .input("landscape", "land_2010.map")
            .note("One iteration per step, the map carried from step to step.")
            .body(body),
    );
    m.nodes.push(Node::new("plot", "view.plot").input("value", "simulation.maps").param("title", "Simulated land use"));
    m
}

/// CLUE-S (lulcc's CluesModel) on the data drawn: logistic regressions, a linear demand, CLUE's allocation.
fn clue_s() -> Model {
    let mut m = Model::new("CLUE-S");
    m.description = Some(
        "CLUE-S as lulcc's CluesModel: the regressions of every class on the drivers, the demand extended from \
         two dates of land use drawn by chance (2000 and 2010), the allocation of three steps of 10 years."
            .into(),
    );
    land_change_data(&mut m);
    m.nodes.push(
        Node::new("observed", "stack.of")
            .input("t2000", "land_2000.map")
            .input("t2010", "land_2010.map")
            .note("The observed maps, a layer per date, for the demand."),
    );
    presets::lulcc(
        &mut m,
        &presets::Lulcc {
            observed: "observed".into(),
            drivers: "drivers".into(),
            seed: "seed".into(),
            start: "start".into(),
            restrictions: None,
            history: None,
            times: vec![2000.0, 2010.0],
            formulas: None,
            // the iteration moves by the difference in cells times the scale: lulcc's 0.0005 overshoots on a
            // map of 25,600 cells and never meets the demand; a smaller step does in a few hundred iterations
            allocation: serde_json::json!({"method": "clue", "scale": 0.000005, "max_iter": 5000}),
            elasticity: Some(serde_json::json!([0.5, 0.2, 1.0])),
            allowed: None,
            neighbourhood: None,
        },
    );
    rewire(
        &mut m.nodes,
        &[("$observed", "observed.stack"), ("$start", "land_2010.map"), ("$drivers", "drivers.stack")],
    );
    // three steps of ten years from 2010
    if let Some(d) = m.nodes.iter_mut().find(|n| n.id == "demand") {
        d.params.insert("out_times".into(), serde_json::json!([2010, 2020, 2030, 2040]));
    }
    m.nodes.push(Node::new("plot", "view.plot").input("value", "simulation.maps").param("title", "Simulated land use"));
    m
}

/// The SWAT+ model of the template "SWAT+ from rasters" (its weather is added by [`swat_from_rasters`]).
const SWAT_FROM_RASTERS: &str = r#"
format = "terraflux/1"
name = "SWAT+ from rasters"
description = "A SWAT+ project made from maps, as QSWAT+ and the SWAT+ Editor make it, and run: a terrain, a land use and a soil drawn by chance (a valley of 3 km by 3 km, cells of 100 m, 700 to 1,000 m) are divided into subbasins, channels and HRUs; with made-up soils and two years of made-up daily weather they become a SWAT+ project (the Editor's files), which is changed (the curve number and the water the soil holds, by a percentage the model's inputs give: what a batch or a calibration varies) and simulated by the Python library's port of SWAT+ 62. The results come back on the map: the soil lost and the runoff of each HRU, the flow of each channel and its daily discharge."

[inputs.cn2_change]
kind = "value"
value = 0
note = "The curve number (cn2) of every HRU changed by this percentage (-10 is 10% lower, less runoff): what SWAT+'s hard calibration does with a row of calibration.cal. A batch or a calibration varies it."

[inputs.awc_change]
kind = "value"
value = 0
note = "The water the soil holds (awc, in every layer) changed by this percentage."

[[nodes]]
id = "relief"
op = "nlm.fbm"
params = { rows = 30, columns = 30, dimension = 1.4, seed = 91, resolution = 100, crs = "+proj=tmerc +lat_0=-15 +lon_0=-47 +k=1 +x_0=0 +y_0=0 +datum=WGS84 +units=m" }
note = "The terrain's shape, in cells of 100 m on a made-up place (a transverse Mercator at 15 S, 47 W: the model needs latitudes for the sun)."

[[nodes]]
id = "dem"
op = "raster.calc"
inputs = { f = "relief.map" }
params = { expression = "700 + 300 * f" }
note = "The elevation, 700 to 1,000 m. In its place, a DEM in a projected CRS: raster.read, or data.download's Copernicus DEM reprojected."
data = { what = "an elevation model (DEM) in a projected coordinate system", kind = "values", expect = "metres" }

[[nodes]]
id = "cover"
op = "nlm.random_cluster"
inputs = { like = "relief.map" }
params = { p = 0.5, weights = [0.6, 0.4], seed = 92 }
note = "The land use: 1 forest, 2 pasture (in its place, a land use map; the codes name the plants in the lookup below)."
data = { what = "a land use map: its codes name the plants of the land use lookup", kind = "classes", classes = { 1 = "forest (FRSD)", 2 = "pasture (PAST)" } }

[[nodes]]
id = "soils"
op = "raster.calc"
inputs = { f = "relief.map" }
params = { expression = "10 if f > 0.4 else 20" }
note = "The soils: 10 on the uplands (above 820 m), 20 in the lower valley (in its place, a soil map: a raster.read of the soil units)."
data = { what = "a soil map: a code per soil unit, named in the soil lookup", kind = "classes", classes = { 10 = "upland loam", 20 = "valley clay loam" } }

[[nodes]]
id = "landuse_lookup"
op = "table.of"
params = { columns = ["code", "name"], rows = [[1, "FRSD"], [2, "PAST"]] }
note = "What each land use code is in SWAT+: the name of a plant (plants.plt) or an urban land use (urban.urb). FRSD is deciduous forest, PAST pasture."

[[nodes]]
id = "soil_lookup"
op = "table.of"
params = { columns = ["code", "name"], rows = [[10, "UPLAND_LOAM"], [20, "VALLEY_CLAY_LOAM"]] }
note = "What each soil code is: the name of a soil of the table below."

[[nodes]]
id = "soil_table"
op = "table.of"
note = "Two made-up soils, a row per layer, in SWAT+'s soils.sol columns: a loam of hydrologic group B and a clay loam of group C. In its place, a usersoil table (ArcSWAT, QSWAT+) read by table.read."

[nodes.params]
columns = ["name", "hyd_grp", "dp_tot", "anion_excl", "perc_crk", "texture", "dp", "bd", "awc", "soil_k", "carbon", "clay", "silt", "sand", "rock", "alb", "usle_k", "ec", "caco3", "ph"]
rows = [
    ["UPLAND_LOAM", "B", 1500, 0.5, 0.5, "L", 200, 1.35, 0.15, 20.0, 2.5, 18, 42, 40, 5, 0.12, 0.28, 0, 0, 6.0],
    ["UPLAND_LOAM", "B", 1500, 0.5, 0.5, "L", 700, 1.45, 0.13, 12.0, 1.0, 22, 40, 38, 5, 0.12, 0.28, 0, 0, 6.0],
    ["UPLAND_LOAM", "B", 1500, 0.5, 0.5, "L", 1500, 1.55, 0.11, 6.0, 0.4, 25, 38, 37, 5, 0.12, 0.28, 0, 0, 6.0],
    ["VALLEY_CLAY_LOAM", "C", 1500, 0.5, 0.5, "CL", 250, 1.30, 0.17, 6.0, 2.0, 32, 36, 32, 5, 0.12, 0.30, 0, 0, 6.0],
    ["VALLEY_CLAY_LOAM", "C", 1500, 0.5, 0.5, "CL", 900, 1.40, 0.15, 3.0, 0.8, 36, 34, 30, 5, 0.12, 0.30, 0, 0, 6.0],
    ["VALLEY_CLAY_LOAM", "C", 1500, 0.5, 0.5, "CL", 1500, 1.50, 0.13, 1.5, 0.3, 38, 34, 28, 5, 0.12, 0.30, 0, 0, 6.0],
]

[[nodes]]
id = "watershed"
op = "swat.watershed"
inputs = { dem = "dem.map", landuse = "cover.map", soils = "soils.map", landuse_lookup = "landuse_lookup.table", soil_lookup = "soil_lookup.table", soil_table = "soil_table.table" }
params = { stream_threshold = 0.3 }
note = "QSWAT+'s watershed: the DEM filled and drained (D8), a stream where 0.3 km2 drain, a channel and a subbasin per stream link, and an HRU per subbasin, the largest combination of land use, soil and slope (QSWAT+'s default; hru_method = filter keeps every combination above thresholds)."
why = "0.3 km2 gives a handful of subbasins on 9 km2: enough to see a network, quick to run. With real data, the threshold sets how fine the network is (QSWAT+ proposes 1% of the area)."

[[nodes]]
id = "stations"
op = "table.of"
params = { columns = ["name", "lat", "lon", "elev"], rows = [["valley", -14.98, -46.98, 850]] }
note = "The weather station, at the valley (in its place, the stations of the series: name, lat, lon, elev)."

[[nodes]]
id = "pcp"
op = "table.select"
inputs = { table = "weather.table" }
params = { columns = ["year", "day", "pcp"], rename = { pcp = "valley" } }
note = "The daily precipitation of the station (mm): a row per day, year and day of the year, a column per station."

[[nodes]]
id = "tmax"
op = "table.select"
inputs = { table = "weather.table" }
params = { columns = ["year", "day", "tmax"], rename = { tmax = "valley" } }

[[nodes]]
id = "tmin"
op = "table.select"
inputs = { table = "weather.table" }
params = { columns = ["year", "day", "tmin"], rename = { tmin = "valley" } }

[[nodes]]
id = "wgn"
op = "swat.wgn"
inputs = { pcp = "pcp.table", tmax = "tmax.table", tmin = "tmin.table" }
params = { monthly = { pcp_hhr = 8, slr_ave = [4, 7, 11, 16, 20, 23, 25, 21, 15, 9, 5, 3], dew_ave = 4, wnd_ave = 2.5 } }
note = "The weather generator's statistics (SWAT+ fills the days the series lack with it) computed from the series, by month: temperatures, rain, the chance of a wet day. What daily data cannot give (the largest half-hour rain, the sun, dew point and wind means) is given by month."
why = "swat.from_tables computes the same statistics when wgn is not connected; the node is here to show them and to replace them with the ones of SWAT+'s weather generator database."

[[nodes]]
id = "project"
op = "swat.from_tables"
inputs = { watershed = "watershed.watershed", stations = "stations.table", pcp = "pcp.table", tmax = "tmax.table", tmin = "tmin.table", wgn = "wgn.wgn" }
note = "The SWAT+ project as the SWAT+ Editor writes it: the TxtInOut files (HRUs, channels, aquifers, soils, land uses, weather, time.sim, file.cio) from the watershed and the weather; it keeps the watershed for the maps of the results. The simulated period is the days the weather covers."

[[nodes]]
id = "curve_number"
op = "swat.set"
inputs = { project = "project.project" }
params = { parameter = "cn2", change = "pctchg", value = "$cn2_change" }
note = "The curve number changed in every HRU by the percentage of the model's input (a row of calibration.cal, which the run applies before it starts)."

[[nodes]]
id = "soil_water"
op = "swat.set"
inputs = { project = "curve_number.project" }
params = { parameter = "awc", change = "pctchg", value = "$awc_change" }
note = "The water the soil holds changed in every layer, by the percentage of the model's input."

[[nodes]]
id = "yearly"
op = "swat.run"
inputs = { project = "soil_water.project" }
params = { step = "aa", tables = ["hru_wb", "hru_ls"] }
note = "SWAT+ run for the two years; the tables of the water balance and the soil lost of each HRU, as the average annual (aa) values. The project skips the first year (nyskip 1, the SWAT+ Editor's default): the soil water and the aquifers settle in it."
why = "A run prints its tables at one step: the HRUs' at the average annual one for the maps, the channels' daily below for the discharge."

[[nodes]]
id = "daily"
op = "swat.run"
inputs = { project = "soil_water.project" }
params = { step = "day", tables = ["channel_sd"] }
note = "The same project run again, printing the channels' table by day (of the second year: the first is skipped). Set nyskip to 0 to print both."

[[nodes]]
id = "sediment"
op = "swat.hru_map"
inputs = { project = "soil_water.project", hru_ls = "yearly.hru_ls" }
params = { variable = "sedyld", how = "annual" }
note = "The soil lost (t/ha a year, MUSLE's sediment yield) of each HRU on the map: each cell takes its HRU's value."

[[nodes]]
id = "runoff"
op = "swat.hru_map"
inputs = { project = "soil_water.project", hru_wb = "yearly.hru_wb" }
params = { variable = "surq_gen", how = "annual" }
note = "The surface runoff (mm a year) of each HRU on the map."

[[nodes]]
id = "flow"
op = "swat.channel_map"
inputs = { project = "soil_water.project", channel_sd = "daily.channel_sd" }
params = { column = "flo_out", how = "mean" }
note = "The mean discharge (m3/s) leaving each channel, on the stream cells."

[[nodes]]
id = "discharge"
op = "swat.reach_discharge"
inputs = { project = "soil_water.project", channel_sd = "daily.channel_sd" }
note = "The daily discharge of each channel (m3/s): a row per day, a column per channel."

[[nodes]]
id = "plot_sediment"
op = "view.plot"
inputs = { value = "sediment.map" }
params = { title = "Soil lost by HRU (t/ha a year)" }

[[nodes]]
id = "plot_runoff"
op = "view.plot"
inputs = { value = "runoff.map" }
params = { title = "Surface runoff by HRU (mm a year)" }

[[nodes]]
id = "plot_flow"
op = "view.plot"
inputs = { value = "flow.map" }
params = { title = "Mean discharge of each channel (m3/s)" }

[[nodes]]
id = "plot_discharge"
op = "view.plot"
inputs = { table = "discharge.table" }
params = { title = "Daily discharge (m3/s)" }
"#;

/// Two years of daily weather made up for the SWAT+ template: a rainy season and a dry one (rain falling on most
/// days of the summer, 9 mm on average), temperatures with the seasons, from a fixed generator.
fn swat_weather() -> Node {
    let mut state: u64 = 2001;
    let mut draw = move || {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((state >> 11) as f64 + 0.5) / (1u64 << 53) as f64
    };
    let round1 = |x: f64| (x * 10.0).round() / 10.0;
    let mut rows = Vec::new();
    for year in [2001, 2002] {
        for day in 1..=365 {
            // 1 in the middle of the austral summer, -1 in winter
            let season = (2.0 * std::f64::consts::PI * (f64::from(day) - 15.0) / 365.0).cos();
            let wet = draw() < 0.40 + 0.28 * season;
            let rain = if wet { round1(-9.0 * draw().ln()) } else { 0.0 };
            let tmax = round1(27.0 + 3.0 * season + 1.5 * (draw() - 0.5));
            let tmin = round1(tmax - 10.0 - 3.0 * draw());
            rows.push(serde_json::json!([year, day, rain, tmax, tmin]));
        }
    }
    Node::new("weather", "table.of")
        .param("columns", serde_json::json!(["year", "day", "pcp", "tmax", "tmin"]))
        .param("rows", rows)
        .note(
            "Two years of daily weather made up (2001 and 2002): rain (mm), maximum and minimum temperature (deg C), \
             wet summers and dry winters. In its place, the series of the stations (table.read of a CSV with a \
             date or year and day, and a column per station).",
        )
}

/// A SWAT+ project from rasters on the data drawn, run and mapped.
fn swat_from_rasters() -> Model {
    let mut m = Model::from_toml(SWAT_FROM_RASTERS).expect("a template is a model");
    m.nodes.insert(0, swat_weather());
    m
}

pub const TEMPLATES: [Template; 72] = [
    Template {
        name: "MSPA of a forest",
        group: PATTERNS,
        build: None,
        about: "a map of three classes drawn by chance, its forest made MSPA's foreground by a reclassification, \
                MSPA, and its map in the Viewer",
        toml: r#"
format = "terraflux/1"
name = "MSPA of a forest"
description = "GuidosToolbox's MSPA on a forest map: read (here drawn), reclassify, MSPA, plot."

[[nodes]]
id = "landscape"
op = "nlm.random_cluster"
params = { rows = 300, columns = 300, p = 0.55, weights = [0.3, 0.15, 0.55], seed = 7 }
note = "A map of three classes drawn by chance (1 farming, 2 water, 3 forest): in its place, a raster.read of a map of yours."
data = { what = "a land cover map with a forest class (the reclassification makes the forest MSPA's foreground)", kind = "classes", classes = { 1 = "farming", 2 = "water", 3 = "forest" } }

[[nodes]]
id = "forest"
op = "raster.reclassify"
inputs = { map = "landscape.map" }
params = { rules = "3 -> 2\n* -> 1" }
note = "MSPA reads 2 as the foreground and 1 as the background: the forest (3) becomes 2, every other class 1."

[[nodes]]
id = "mspa"
op = "landscape.mspa"
inputs = { map = "forest.map" }
params = { edge_width = 1, connectivity = 8 }
note = "Core, islet, perforation, edge, loop, bridge and branch, as GuidosToolbox computes them."

[[nodes]]
id = "plot"
op = "view.plot"
inputs = { value = "mspa.map" }
params = { title = "MSPA" }
note = "Its map in the Viewer, in GuidosToolbox's colours, refreshed at every run."
"#,
    },
    Template {
        name: "MSPA of your map",
        group: PATTERNS,
        build: None,
        about: "the same flow on a raster of yours: choose its file in the node read, and in the reclassification \
                the classes of its forest",
        toml: r#"
format = "terraflux/1"
name = "MSPA of your map"
description = "GuidosToolbox's MSPA on a map of yours: read, reclassify, MSPA, plot."

[[nodes]]
id = "read"
op = "raster.read"
params = { path = "" }
note = "Choose your map's file (Node > path > Choose...)."

[[nodes]]
id = "forest"
op = "raster.reclassify"
inputs = { map = "read.raster" }
params = { rules = "1 -> 2\n* -> 1" }
note = "Your forest's classes become 2 (the foreground), the rest 1 (the background): put your forest's classes in place of 1 (A rule per class lists the map's classes)."

[[nodes]]
id = "mspa"
op = "landscape.mspa"
inputs = { map = "forest.map" }
params = { edge_width = 1, connectivity = 8 }
note = "Core, islet, perforation, edge, loop, bridge and branch, as GuidosToolbox computes them."

[[nodes]]
id = "plot"
op = "view.plot"
inputs = { value = "mspa.map" }
params = { title = "MSPA" }
note = "Its map in the Viewer, refreshed at every run."
"#,
    },
    Template {
        name: "Forest from a surface",
        group: PATTERNS,
        build: None,
        about: "a smooth surface drawn by chance cut at a threshold into forest and the rest (a reclassification \
                by ranges), then MSPA",
        toml: r#"
format = "terraflux/1"
name = "Forest from a surface"
description = "A continuous surface reclassified by ranges into a forest map, and its MSPA."

[[nodes]]
id = "surface"
op = "nlm.gaussian_field"
params = { rows = 300, columns = 300, range = 15, seed = 3 }
note = "A smooth surface from 0 to 1 drawn by chance: a suitability, say."
data = { what = "a continuous surface, such as a suitability: the forest is where it is above the threshold", kind = "values", expect = "0 to 1" }

[[nodes]]
id = "forest"
op = "raster.reclassify"
inputs = { map = "surface.map" }
params = { rules = "0..0.45 -> 1\n0.45..1 -> 2" }
note = "Ranges: up to 0.45 the background (1), above it the forest (2). Move the threshold and see MSPA change."

[[nodes]]
id = "mspa"
op = "landscape.mspa"
inputs = { map = "forest.map" }
params = { edge_width = 2 }
note = "An edge two cells wide."

[[nodes]]
id = "plot_surface"
op = "view.plot"
inputs = { value = "surface.map" }
params = { title = "Surface" }
note = "A continuous map: a ramp of colours."

[[nodes]]
id = "plot_mspa"
op = "view.plot"
inputs = { value = "mspa.map" }
params = { title = "MSPA" }
note = "MSPA's classes in GuidosToolbox's colours."
"#,
    },
    Template {
        name: "Fragmentation of a forest",
        group: PATTERNS,
        build: None,
        about: "one forest map, its density at five scales (FAD-MS) and its connectivity at one (FOS), in GuidosToolbox's colours",
        toml: r#"
format = "terraflux/1"
name = "Fragmentation of a forest"
description = "GuidosToolbox's fragmentation of a forest map: its density at five scales (FAD-MS) and FOS."

[[nodes]]
id = "landscape"
op = "nlm.random_cluster"
params = { rows = 300, columns = 300, p = 0.55, weights = [0.3, 0.15, 0.55], seed = 7 }
note = "A map of three classes drawn by chance (1 farming, 2 water, 3 forest): in its place, a raster.read of a map of yours."
data = { what = "a land cover map with a forest class: the fragmentation is of the forest (3)", kind = "classes", classes = { 1 = "farming", 2 = "water", 3 = "forest" } }

[[nodes]]
id = "fad"
op = "fragmentation.fad_multiscale"
inputs = { map = "landscape.map" }
params = { foreground = 3 }
note = "The forest's share (%) in windows of 7, 13, 27, 81 and 243 cells and their average; scales has each, stats the shares of the classes (rare to intact). The foreground is the forest's class: no reclassification needed."

[[nodes]]
id = "fos"
op = "fragmentation.fos"
inputs = { map = "landscape.map" }
params = { foreground = 3, window = 27, method = "FAC", classes = 5 }
note = "The connectivity of the forest at one scale (27 cells), by FAC; summary has GTB's report (ECA, coherence)."

[[nodes]]
id = "plot_fad"
op = "view.plot"
inputs = { value = "fad.map" }
params = { title = "FAD, the average of five scales" }

[[nodes]]
id = "plot_fos"
op = "view.plot"
inputs = { value = "fos.map" }
params = { title = "FOS (FAC, 27 cells)" }
"#,
    },
    Template {
        name: "Patterns and objects",
        group: PATTERNS,
        build: None,
        about: "one forest map, its simplified pattern (SPA6) and its objects by size (Accounting)",
        toml: r#"
format = "terraflux/1"
name = "Patterns and objects"
description = "GuidosToolbox's SPA and Accounting on a forest map: a coarser MSPA, and the forest's objects by size."

[[nodes]]
id = "landscape"
op = "nlm.random_cluster"
params = { rows = 300, columns = 300, p = 0.5, weights = [0.5, 0.5], seed = 5 }
note = "Two classes drawn by chance: 1 the background, 2 the forest (GuidosToolbox's coding already)."
data = { what = "a map of the forest and the background", kind = "classes", classes = { 1 = "background", 2 = "forest" } }

[[nodes]]
id = "spa"
op = "landscape.spa"
inputs = { map = "landscape.map" }
params = { variant = 6 }
note = "Core, islet, edge, perforation and margin: MSPA's main classes at less cost; stats has their areas, porosity the share of core-openings."

[[nodes]]
id = "objects"
op = "landscape.accounting"
inputs = { map = "landscape.map" }
params = { thresholds = [10, 100, 1000, 5000], highlight_largest = true }
note = "The forest's objects in five size classes (up to 10 cells, 100, 1000, 5000, larger), the three largest painted apart; stats counts each class."

[[nodes]]
id = "plot_spa"
op = "view.plot"
inputs = { value = "spa.map" }
params = { title = "SPA6" }

[[nodes]]
id = "plot_objects"
op = "view.plot"
inputs = { value = "objects.map" }
params = { title = "Objects by size" }
"#,
    },
    Template {
        name: "Landscape mosaic",
        group: PATTERNS,
        build: None,
        about: "a land cover of agriculture, natural and developed land, its landscape mosaic, and the natural \
                land's contagion and entropy",
        toml: r#"
format = "terraflux/1"
name = "Landscape mosaic"
description = "GuidosToolbox's landscape mosaic of a land cover, and the contagion and entropy of its natural land."

[[nodes]]
id = "cover"
op = "nlm.random_cluster"
params = { rows = 300, columns = 300, p = 0.5, weights = [0.35, 0.5, 0.15], seed = 9 }
note = "A land cover drawn by chance: 1 agriculture, 2 natural, 3 developed (the mosaic's coding already). With a map of yours, give the classes of each in the mosaic's settings."
data = { what = "a land cover map of agriculture, natural land and developed land", kind = "classes", classes = { 1 = "agriculture", 2 = "natural", 3 = "developed" } }

[[nodes]]
id = "mosaic"
op = "fragmentation.landscape_mosaic"
inputs = { map = "cover.map" }
params = { window = 27 }
note = "The mix of the three around each cell (27 cells): which dominates, pairs, mosaics; stats has the share of each class."

[[nodes]]
id = "contagion"
op = "fragmentation.contagion"
inputs = { map = "cover.map" }
params = { foreground = 2, window = 7 }
note = "How much of the natural land's edges join natural land, around each cell."

[[nodes]]
id = "entropy"
op = "fragmentation.entropy"
inputs = { map = "cover.map" }
params = { foreground = 2 }
note = "How mixed the natural land and the rest are over the whole map (%)."

[[nodes]]
id = "plot_mosaic"
op = "view.plot"
inputs = { value = "mosaic.map" }
params = { title = "Landscape mosaic" }

[[nodes]]
id = "plot_contagion"
op = "view.plot"
inputs = { value = "contagion.map" }
params = { title = "Contagion of the natural land" }
"#,
    },
    Template {
        name: "Landscape metrics",
        group: METRICS,
        build: None,
        about: "a land cover's metrics per class (with the share of each as a map) and of the whole landscape, \
                and its diversity in a moving window",
        toml: r#"
format = "terraflux/1"
name = "Landscape metrics"
description = "landscapemetrics' metrics of a land cover: per class, of the landscape, and in a moving window."

[[nodes]]
id = "cover"
op = "nlm.random_cluster"
params = { rows = 200, columns = 200, p = 0.5, weights = [0.4, 0.35, 0.25], seed = 12 }
note = "A land cover of three classes drawn by chance: in its place, a raster.read of a map of yours (its cells' size gives the areas: hectares for metres)."
data = { what = "a land cover map of any classes (the metrics are per class); its cells' size gives the areas", kind = "classes" }

[[nodes]]
id = "classes"
op = "metrics.class"
inputs = { map = "cover.map" }
params = { metrics = ["ca", "pland", "np", "ed", "area_mn", "ai", "cohesion"], show = "pland" }
note = "A row per class: area, share, patches, edge density, mean patch area, aggregation and cohesion. Click a column to sort, choose one to see as bars; map paints the share of each class."

[[nodes]]
id = "landscape"
op = "metrics.landscape"
inputs = { map = "cover.map" }
params = { type = "diversity" }
note = "The diversity of the whole landscape: Shannon's, Simpson's, richness..."

[[nodes]]
id = "window"
op = "metrics.window"
inputs = { map = "cover.map" }
params = { metrics = ["shdi", "ed"], window = 9 }
note = "Shannon's diversity and the edge density of the 9 x 9 cells around each cell, a map each."

[[nodes]]
id = "plot_share"
op = "view.plot"
inputs = { value = "classes.map" }
params = { title = "Share of each class (%)" }

[[nodes]]
id = "plot_window"
op = "view.plot"
inputs = { value = "window.maps" }
params = { title = "Diversity around each cell" }
"#,
    },
    Template {
        name: "Tables: filter, summarise, join and pivot",
        group: METRICS,
        build: None,
        about: "the tables operators give worked on as in a spreadsheet: a land cover's patches filtered, a column \
                computed, summarised by class, joined to a table of values, the largest kept, and its change from \
                one date to the next turned into a matrix",
        toml: r#"
format = "terraflux/1"
name = "Tables: filter, summarise, join and pivot"
description = "A land cover's patches (metrics.patch) and its transitions (change.transitions) worked on as tables: filtered, a column computed, summarised by class, joined to the value of each class, sorted, reshaped into a matrix."

[[nodes]]
id = "cover"
op = "nlm.random_cluster"
params = { rows = 120, columns = 120, p = 0.5, weights = [0.45, 0.35, 0.2], seed = 12 }
note = "A land cover of three classes drawn by chance: 1 forest, 2 farming, 3 water. In its place, a raster.read of a map of yours."
data = { what = "a land use map, the first date", kind = "classes", classes = { 1 = "forest", 2 = "farming", 3 = "water" } }

[[nodes]]
id = "later"
op = "raster.calc"
inputs = { cover = "cover.map" }
note = "Ten years later: farming grown into the forest beside it."
data = { what = "a land use map, the second date", kind = "classes", classes = { 1 = "forest", 2 = "farming", 3 = "water" } }
[nodes.params]
expression = """
# the forest (1) with three cells of farming (2) or more around it becomes farming
grown = cover
grown[cover == 1 and nbsum(cover == 2, 1) >= 3] = 2
out map = grown
"""

[[nodes]]
id = "patches"
op = "metrics.patch"
inputs = { map = "cover.map" }
params = { metrics = ["area", "perim", "shape"] }
note = "A row per patch: its class, area (hectares), perimeter (metres) and shape."

[[nodes]]
id = "large"
op = "table.filter"
inputs = { table = "patches.table" }
params = { expression = "area >= 0.005" }
note = "The patches of 50 cells or more (0.005 ha at cells of 1 m): the small ones left out."

[[nodes]]
id = "edge"
op = "table.calc"
inputs = { table = "large.table" }
params = { column = "edge_per_ha", expression = "perim / area" }
note = "A column computed from the others, as a spreadsheet's: the metres of edge per hectare."

[[nodes]]
id = "by_class"
op = "table.aggregate"
inputs = { table = "edge.table" }
params = { by = "class", columns = ["area", "edge_per_ha"], stats = ["sum", "mean", "max"] }
note = "A row per class: its patches (n), their area in all, on average and the largest, and their edge per hectare."

[[nodes]]
id = "values"
op = "table.of"
params = { columns = ["class", "value_per_ha"], rows = [[1, 1200], [2, 300], [3, 50]] }
note = "What a hectare of each class is worth (in its place, a table.read of yours)."

[[nodes]]
id = "worth"
op = "table.join"
inputs = { left = "by_class.table", right = "values.table" }
params = { on = "class", how = "left" }
note = "Each class's summary beside its value per hectare, by the class."

[[nodes]]
id = "total"
op = "table.calc"
inputs = { table = "worth.table" }
params = { column = "worth", expression = "area_sum * value_per_ha" }
note = "What the large patches of each class are worth."

[[nodes]]
id = "largest"
op = "table.sort"
inputs = { table = "edge.table" }
params = { by = "area", descending = true, top = 10 }
note = "The ten largest patches."

[[nodes]]
id = "short"
op = "table.select"
inputs = { table = "largest.table" }
params = { columns = ["class", "area", "edge_per_ha"], rename = { area = "hectares" } }
note = "Only the columns to report, one renamed."

[[nodes]]
id = "dates"
op = "stack.of"
inputs = { t2000 = "cover.map", t2010 = "later.map" }

[[nodes]]
id = "change"
op = "change.transitions"
inputs = { maps = "dates.stack" }
note = "A row per pair of classes (from, to) with its cells, from one date to the next."

[[nodes]]
id = "matrix"
op = "table.pivot"
inputs = { table = "change.one_step" }
params = { direction = "wider", id = "from", names = "to", values = "cells", fill = 0 }
note = "The transitions as a matrix: a row per class before, a column per class after (to_1, to_2...)."

[[nodes]]
id = "plot_worth"
op = "view.plot"
inputs = { table = "total.table" }
params = { title = "Worth of each class", chart = "bars", x = "class", y = "worth" }
"#,
    },
    Template {
        name: "Raster statistics: summaries, zones and correlation",
        group: SURFACES,
        build: None,
        about: "the statistics of rasters: a map summarised with its histogram, three years reduced cell by cell, \
                the mean of each zone as a table and as a map, the hectares of each class by zone, the correlation \
                of two drivers over the map and in a moving window, and the drivers put on one scale",
        toml: r#"
format = "terraflux/1"
name = "Raster statistics: summaries, zones and correlation"
description = "A rainfall map summarised (raster.stats), three years of it reduced at every cell (stack.reduce), its mean in every zone of a map of zones (raster.zonal), the land cover's hectares by zone (raster.tabulate), its correlation with the elevation over the map (raster.correlation) and in a moving window (raster.local_correlation), and both on a common scale (raster.standardize)."

[[nodes]]
id = "surface"
op = "nlm.gaussian_field"
params = { rows = 120, columns = 120, range = 20, seed = 4 }

[[nodes]]
id = "elevation"
op = "raster.calc"
inputs = { s = "surface.map" }
params = { expression = "200 + 800 * s" }
note = "An elevation of 200 to 1000 m drawn by chance (in its place, a raster.read of a DEM)."
data = { what = "an elevation model (DEM)", kind = "values", expect = "metres" }

[[nodes]]
id = "noise"
op = "nlm.gaussian_field"
params = { rows = 120, columns = 120, range = 6, seed = 9 }

[[nodes]]
id = "rain"
op = "raster.calc"
inputs = { elevation = "elevation.map", noise = "noise.map" }
params = { expression = "600 + 0.8 * elevation + 400 * noise" }
note = "A year's rainfall (mm), wetter up the hills."
data = { what = "a year's rainfall", kind = "values", expect = "mm" }

[[nodes]]
id = "summary"
op = "raster.stats"
inputs = { map = "rain.map" }
params = { bins = 12 }
note = "The rainfall summarised: its mean, standard deviation, percentiles..., and its histogram."

[[nodes]]
id = "dry"
op = "raster.calc"
inputs = { rain = "rain.map", noise = "noise.map" }
params = { expression = "rain * (0.7 + 0.2 * noise)" }
note = "A dry year."

[[nodes]]
id = "wet"
op = "raster.calc"
inputs = { rain = "rain.map" }
params = { expression = "rain * 1.25" }
note = "A wet year."

[[nodes]]
id = "years"
op = "stack.of"
inputs = { y2020 = "rain.map", y2021 = "dry.map", y2022 = "wet.map" }

[[nodes]]
id = "mean_rain"
op = "stack.reduce"
inputs = { maps = "years.stack" }
params = { stat = "mean" }
note = "The mean of the three years at every cell."

[[nodes]]
id = "spread"
op = "stack.reduce"
inputs = { maps = "years.stack" }
params = { stat = "std" }
note = "How much a cell's rainfall changes from year to year."

[[nodes]]
id = "zones"
op = "nlm.random_cluster"
params = { rows = 120, columns = 120, p = 0.55, weights = [0.3, 0.3, 0.4], seed = 21 }
note = "Three zones (in their place, municipalities or watersheds rasterized by vector.rasterize)."
data = { what = "the zones to summarise by (municipalities, watersheds), a code each", kind = "classes" }

[[nodes]]
id = "by_zone"
op = "raster.zonal"
inputs = { map = "mean_rain.map", zones = "zones.map" }
params = { stats = ["count", "mean", "min", "max", "std"], to_map = "mean" }
note = "The rainfall of each zone, and a map of every cell given its zone's mean."

[[nodes]]
id = "cover"
op = "nlm.random_cluster"
params = { rows = 120, columns = 120, p = 0.5, weights = [0.5, 0.3, 0.2], seed = 12 }
note = "A land cover: 1 forest, 2 farming, 3 water."
data = { what = "a land cover map", kind = "classes", classes = { 1 = "forest", 2 = "farming", 3 = "water" } }

[[nodes]]
id = "hectares"
op = "raster.tabulate"
inputs = { zones = "zones.map", classes = "cover.map" }
params = { units = "share" }
note = "The share of each class in every zone (units = hectares for its area, of a map in metres)."

[[nodes]]
id = "drivers"
op = "stack.of"
inputs = { elevation = "elevation.map", rain = "mean_rain.map" }

[[nodes]]
id = "correlation"
op = "raster.correlation"
inputs = { x = "drivers.stack" }
note = "How strongly rainfall goes with the elevation over the whole map."

[[nodes]]
id = "local"
op = "raster.local_correlation"
inputs = { x = "elevation.map", y = "mean_rain.map" }
params = { radius = 5, shape = "circle" }
note = "The same in a circle of 5 cells around every cell: where the relation holds and where it does not."

[[nodes]]
id = "scaled"
op = "raster.standardize"
inputs = { map = "drivers.stack" }
params = { method = "zscore" }
note = "Both drivers as z-scores, to be compared or combined (a model's predictors)."

[[nodes]]
id = "plot_zones"
op = "view.plot"
inputs = { table = "by_zone.table" }
params = { title = "Mean rainfall by zone", chart = "bars", x = "zone", y = "mean" }
"#,
    },
    Template {
        name: "Cleaning a map of classes: filter, sieve, label and grow",
        group: METRICS,
        build: None,
        about: "a classified map rid of its specks as before its patches are measured: smoothed in a window, \
                cleaned by the majority filter, its patches under a size merged into their neighbours, numbered, \
                and a class grown and shrunk by a cell",
        toml: r#"
format = "terraflux/1"
name = "Cleaning a map of classes: filter, sieve, label and grow"
description = "A classification full of specks (a satellite image's, say) cleaned: the majority filter (raster.majority), the patches of fewer than 20 cells merged into their largest neighbour (raster.sieve, as gdal_sieve), the patches numbered (raster.label), a class grown and shrunk (raster.expand, raster.shrink), and a smoothing in a window (raster.focal)."

[[nodes]]
id = "field"
op = "nlm.gaussian_field"
params = { rows = 150, columns = 150, range = 12, seed = 5 }
data = { what = "a continuous surface to smooth", kind = "values" }

[[nodes]]
id = "noise"
op = "nlm.random"
params = { rows = 150, columns = 150, seed = 6 }

[[nodes]]
id = "classified"
op = "raster.calc"
inputs = { field = "field.map", noise = "noise.map" }
params = { expression = "1 if field + 0.35 * noise < 0.55 else 2 if field + 0.35 * noise < 0.8 else 3" }
note = "A land cover classified pixel by pixel, with the specks of a classification (1 forest, 2 farming, 3 town)."
data = { what = "a land cover map classified pixel by pixel: the one to clean", kind = "classes", classes = { 1 = "forest", 2 = "farming", 3 = "town" } }

[[nodes]]
id = "smooth"
op = "raster.focal"
inputs = { map = "field.map" }
params = { stat = "mean", shape = "circle", radius = 3 }
note = "A surface smoothed: the mean in a circle of 3 cells (median, std, a Gaussian kernel... the same way)."

[[nodes]]
id = "filtered"
op = "raster.majority"
inputs = { map = "classified.map" }
params = { neighbours = 8, replace = "majority", iterations = 2 }
note = "Every cell given the class most of its neighbours hold, twice."

[[nodes]]
id = "sieved"
op = "raster.sieve"
inputs = { map = "filtered.map" }
params = { threshold = 20, neighbours = 4 }
note = "The patches of fewer than 20 cells merged into the largest one they touch, as gdal_sieve."

[[nodes]]
id = "patches"
op = "raster.label"
inputs = { map = "sieved.map" }
params = { neighbours = 4 }
note = "Every patch numbered, with a table of its class, cells and area."

[[nodes]]
id = "town_grown"
op = "raster.expand"
inputs = { map = "sieved.map" }
params = { classes = [3], cells = 2 }
note = "The town grown two cells into what is around it."

[[nodes]]
id = "forest_core"
op = "raster.shrink"
inputs = { map = "sieved.map" }
params = { classes = [1], cells = 1 }
note = "The forest shrunk by a cell: its edge given to its neighbours."

[[nodes]]
id = "before"
op = "metrics.class"
inputs = { map = "classified.map" }
params = { metrics = ["np", "area_mn"] }
note = "The patches of each class before the cleaning..."

[[nodes]]
id = "after"
op = "metrics.class"
inputs = { map = "sieved.map" }
params = { metrics = ["np", "area_mn"] }
note = "...and after it: far fewer, larger."
"#,
    },
    Template {
        name: "Vector tools: polygons, overlay and points",
        group: METRICS,
        build: None,
        about: "the vector tools at work: a land cover's forest turned into polygons and measured, the large ones \
                kept, plots drawn inside them, the plots counted in squares, and the forest cut by the squares",
        toml: r#"
format = "terraflux/1"
name = "Vector tools: polygons, overlay and points"
description = "A land cover's forest as polygons (vector.polygonize), measured (vector.geometry), the large patches kept (vector.filter) and given a column (vector.calc), random plots inside them (vector.points), counted in squares (vector.spatial_join), and the forest cut by the squares (vector.overlay)."

[[nodes]]
id = "cover"
op = "nlm.random_cluster"
params = { rows = 100, columns = 100, p = 0.5, weights = [0.5, 0.3, 0.2], seed = 12 }
note = "A land cover: 1 forest, 2 farming, 3 water (in its place, a raster.read of yours)."
data = { what = "a land cover map to turn into polygons", kind = "classes", classes = { 1 = "forest", 2 = "farming", 3 = "water" } }

[[nodes]]
id = "clean"
op = "raster.sieve"
inputs = { map = "cover.map" }
params = { threshold = 8 }
note = "Its specks merged into their neighbours first: fewer, simpler polygons."

[[nodes]]
id = "polygons"
op = "vector.polygonize"
inputs = { map = "clean.map" }
note = "A polygon per patch, with its value, cells and area."

[[nodes]]
id = "measured"
op = "vector.geometry"
inputs = { vector = "polygons.vector" }
note = "Each polygon's area, perimeter and centroid as attributes."

[[nodes]]
id = "forest"
op = "vector.filter"
inputs = { vector = "measured.vector" }
params = { expression = "value == 1 and area >= 40" }
note = "The forest patches of 40 cells or more."

[[nodes]]
id = "shape"
op = "vector.calc"
inputs = { vector = "forest.vector" }
params = { column = "edge_ratio", expression = "perimeter / area" }
note = "A column computed from the others: the edge per unit of area."

[[nodes]]
id = "plots"
op = "vector.points"
inputs = { vector = "shape.vector" }
params = { method = "random", n = 3, seed = 2 }
note = "Three field plots drawn inside every large patch, with its attributes."

[[nodes]]
id = "squares"
op = "vector.grid_zones"
inputs = { map = "cover.map" }
params = { size = 25 }
note = "Squares of 25 cells' side over the map."

[[nodes]]
id = "plots_per_square"
op = "vector.spatial_join"
inputs = { vector = "squares.zones", other = "plots.vector" }
params = { mode = "summary", fields = ["edge_ratio"] }
note = "The plots in every square counted, and their patches' edge ratio averaged."

[[nodes]]
id = "forest_by_square"
op = "vector.overlay"
inputs = { vector = "shape.vector", other = "squares.zones" }
params = { how = "intersection" }
note = "The forest cut by the squares: a piece per patch and square, with the attributes of both."

[[nodes]]
id = "pieces"
op = "vector.geometry"
inputs = { vector = "forest_by_square.vector" }
note = "The area of every piece."

[[nodes]]
id = "forest_area"
op = "table.aggregate"
inputs = { table = "pieces.table" }
params = { by = "zone", columns = ["area"], stats = ["sum"] }
note = "The forest's area in every square."
"#,
    },
    Template {
        name: "Spatial analysis: density, contours, viewshed and classes",
        group: SURFACES,
        build: None,
        about: "the classic tools of spatial analysis on a terrain and villages: the villages' kernel density and \
                count per cell, the terrain's contour lines, what three villages see, its classes by natural \
                breaks, and the true area of its cells",
        toml: r#"
format = "terraflux/1"
name = "Spatial analysis: density, contours, viewshed and classes"
description = "Villages on a terrain: their kernel density and count per cell (vector.density), the terrain's contours (raster.contour), what three villages see from a tower (raster.viewshed), the terrain in classes by Jenks's natural breaks (raster.classify) and the true area of every cell (raster.cell_area)."

[[nodes]]
id = "surface"
op = "nlm.gaussian_field"
params = { rows = 120, columns = 120, range = 20, seed = 4 }

[[nodes]]
id = "elevation"
op = "raster.calc"
inputs = { s = "surface.map" }
params = { expression = "200 + 800 * s" }
note = "A terrain of 200 to 1000 m drawn by chance (in its place, a raster.read of a DEM)."
data = { what = "an elevation model (DEM)", kind = "values", expect = "metres" }

[[nodes]]
id = "region"
op = "vector.grid_zones"
inputs = { map = "elevation.map" }
params = { size = 120 }
note = "The map's extent as a polygon."

[[nodes]]
id = "villages"
op = "vector.points"
inputs = { vector = "region.zones" }
params = { method = "random", n = 60, seed = 4 }
note = "Sixty villages drawn by chance (in their place, a vector.read or vector.from_table of yours)."

[[nodes]]
id = "density"
op = "vector.density"
inputs = { vector = "villages.vector", map = "elevation.map" }
params = { kernel = "quartic", radius = 15 }
note = "The villages' kernel density (quartic, a radius of 15 cells): villages per square unit of the map."

[[nodes]]
id = "count"
op = "vector.density"
inputs = { vector = "villages.vector", map = "elevation.map" }
params = { kernel = "count" }
note = "The villages in every cell."

[[nodes]]
id = "contours"
op = "raster.contour"
inputs = { map = "elevation.map" }
params = { interval = 100 }
note = "A contour line every 100 m."

[[nodes]]
id = "towers"
op = "vector.filter"
inputs = { vector = "villages.vector" }
params = { expression = "feature <= 3" }
note = "Three villages with a watchtower."

[[nodes]]
id = "seen"
op = "raster.viewshed"
inputs = { map = "elevation.map", points = "towers.vector" }
params = { observer_height = 20, radius = 50 }
note = "How many towers see every cell, from 20 m up and 50 cells away at most."

[[nodes]]
id = "classes"
op = "raster.classify"
inputs = { map = "elevation.map" }
params = { method = "jenks", n = 5 }
note = "The terrain in five classes by Jenks's natural breaks (quantiles, equal intervals, standard deviations alike)."

[[nodes]]
id = "cell_area"
op = "raster.cell_area"
inputs = { map = "elevation.map" }
params = { units = "m2" }
note = "Every cell's true area: on a map of longitudes and latitudes, smaller towards the poles."
"#,
    },
    Template {
        name: "Ecological intactness",
        group: CONSERVATION,
        build: None,
        about: "a human footprint drawn by chance, the ecological intactness it leaves (Beyer et al., 2020), \
                and where restoring would raise it most",
        toml: r#"
format = "terraflux/1"
name = "Ecological intactness"
description = "Beyer et al.'s ecological intactness of a human footprint, and its gradient as cells are restored."

[[nodes]]
id = "field"
op = "nlm.gaussian_field"
params = { rows = 120, columns = 120, range = 12, seed = 8 }
note = "A smooth surface from 0 to 1 drawn by chance."

[[nodes]]
id = "footprint"
op = "raster.calc"
inputs = { f = "field.map" }
params = { expression = "f * f * 30" }
note = "A human footprint of 0 to 30 (in its place, a raster.read of the Human Footprint Index)."
data = { what = "a human footprint (the Human Footprint Index)", kind = "values", expect = "0 to 30" }

[[nodes]]
id = "intactness"
op = "restoration.intactness"
inputs = { footprint = "footprint.map" }
params = { radius = 3, cell_size = 1.0 }
note = "The habitat's quality exp(-gamma footprint) of each cell and of those within 3 cells: 1 intact, 0 lost. cell_size: a cell's side in km (a projected map gives it)."

[[nodes]]
id = "reduction"
op = "raster.calc"
inputs = { f = "footprint.map" }
params = { expression = "f / 2" }
note = "How much a cell's footprint falls when it is restored: half of it, here."

[[nodes]]
id = "gradient"
op = "restoration.intactness_gradient"
inputs = { footprint = "footprint.map", reduction = "reduction.map" }
params = { radius = 3, cell_size = 1.0 }
note = "How much the total intactness rises per unit of each cell restored: where restoring pays most."

[[nodes]]
id = "plot_intactness"
op = "view.plot"
inputs = { value = "intactness.map" }
params = { title = "Ecological intactness" }

[[nodes]]
id = "plot_gradient"
op = "view.plot"
inputs = { value = "gradient.map" }
params = { title = "Where restoring raises it most" }
"#,
    },
    Template {
        name: "Zonation",
        group: CONSERVATION,
        build: None,
        about: "four species' distributions drawn by chance and a cost, Zonation's priority rank of the cells and \
                its performance curves",
        toml: r#"
format = "terraflux/1"
name = "Zonation"
description = "Zonation 5's priority ranking of four features with a cost: the rank map and the performance curves."

[[nodes]]
id = "bird"
op = "nlm.gaussian_field"
params = { rows = 100, columns = 120, range = 15, seed = 1 }
note = "A species' suitability drawn by chance (in its place, a raster.read of a distribution map)."
data = { what = "a species' distribution map (the bird's): its suitability or amount in each cell", kind = "values", expect = "0 or more" }

[[nodes]]
id = "frog"
op = "nlm.gaussian_field"
params = { rows = 100, columns = 120, range = 6, seed = 2 }
data = { what = "a species' distribution map (the frog's): its suitability or amount in each cell", kind = "values", expect = "0 or more" }

[[nodes]]
id = "orchid"
op = "nlm.gaussian_field"
params = { rows = 100, columns = 120, range = 25, seed = 3 }
data = { what = "a species' distribution map (the orchid's): its suitability or amount in each cell", kind = "values", expect = "0 or more" }

[[nodes]]
id = "wetland"
op = "nlm.fbm"
params = { rows = 100, columns = 120, dimension = 1.4, seed = 4 }
data = { what = "a feature's map (the wetland's): its amount in each cell", kind = "values", expect = "0 or more" }

[[nodes]]
id = "features"
op = "stack.of"
inputs = { bird = "bird.map", frog = "frog.map", orchid = "orchid.map", wetland = "wetland.map" }
note = "The features, a layer each."

[[nodes]]
id = "cost"
op = "nlm.gaussian_field"
params = { rows = 100, columns = 120, range = 30, seed = 9 }
note = "What each cell costs (land prices, say): Zonation keeps the cheap cells where they serve as well."
data = { what = "what each cell costs (land prices, say)", kind = "values", expect = "above 0" }

[[nodes]]
id = "zonation"
op = "prioritize.zonation"
inputs = { features = "features.stack", cost = "cost.map" }
params = { rule = "CAZ2", weights = [2.0, 1.0, 1.0, 1.5] }
note = "CAZ2: the cells removed first are those whose loss the features feel least (weights in the order of the features' names). curves: how much of each feature remains as cells are removed."

[[nodes]]
id = "plot_rank"
op = "view.plot"
inputs = { value = "zonation.rank" }
params = { title = "Priority rank" }
"#,
    },
    Template {
        name: "Marxan and irreplaceability",
        group: CONSERVATION,
        build: None,
        about: "four species and a cost: Marxan's reserves for a third of each, how often each cell was chosen, \
                and ProxIRR's irreplaceability of the cells",
        toml: r#"
format = "terraflux/1"
name = "Marxan and irreplaceability"
description = "Marxan 4's reserves from maps (every cell a planning unit), and ProxIRR's irreplaceability."

[[nodes]]
id = "bird"
op = "nlm.gaussian_field"
params = { rows = 40, columns = 50, range = 8, seed = 1 }
note = "Species' amounts drawn by chance (in their place, raster.read of distribution maps)."
data = { what = "a species' distribution map (the bird's): its suitability or amount in each cell", kind = "values", expect = "0 or more" }

[[nodes]]
id = "frog"
op = "nlm.gaussian_field"
params = { rows = 40, columns = 50, range = 4, seed = 2 }
data = { what = "a species' distribution map (the frog's): its suitability or amount in each cell", kind = "values", expect = "0 or more" }

[[nodes]]
id = "orchid"
op = "nlm.gaussian_field"
params = { rows = 40, columns = 50, range = 12, seed = 3 }
data = { what = "a species' distribution map (the orchid's): its suitability or amount in each cell", kind = "values", expect = "0 or more" }

[[nodes]]
id = "wetland"
op = "nlm.fbm"
params = { rows = 40, columns = 50, dimension = 1.4, seed = 4 }
data = { what = "a feature's map (the wetland's): its amount in each cell", kind = "values", expect = "0 or more" }

[[nodes]]
id = "features"
op = "stack.of"
inputs = { bird = "bird.map", frog = "frog.map", orchid = "orchid.map", wetland = "wetland.map" }

[[nodes]]
id = "cost"
op = "nlm.gaussian_field"
params = { rows = 40, columns = 50, range = 15, seed = 9 }
note = "What each cell costs."
data = { what = "what each cell costs (land prices, say)", kind = "values", expect = "above 0" }

[[nodes]]
id = "marxan"
op = "prioritize.marxan"
inputs = { features = "features.stack", cost = "cost.map" }
params = { prop = [0.3, 0.3, 0.3, 0.3], blm = 0.2, num_reps = 10, num_iters = 200000, num_temp = 1000, seed = 7 }
note = "Reserves holding 30% of every species at the least cost and boundary (blm: how much compactness weighs), 10 runs. map: the best run; frequency_map: how often each cell was chosen."

[[nodes]]
id = "irreplaceability"
op = "prioritize.proxirr"
inputs = { features = "features.stack" }
params = { prop = 0.3 }
note = "ProxIRR: beta, the chance each cell is needed to reach the targets; gamma, how many targets need it."

[[nodes]]
id = "plot_best"
op = "view.plot"
inputs = { value = "marxan.map" }
params = { title = "Best reserve" }

[[nodes]]
id = "plot_frequency"
op = "view.plot"
inputs = { value = "marxan.frequency_map" }
params = { title = "Selection frequency" }

[[nodes]]
id = "plot_beta"
op = "view.plot"
inputs = { value = "irreplaceability.beta" }
params = { title = "Irreplaceability (beta)" }
"#,
    },
    Template {
        name: "Conservation planning (linear programming)",
        group: CONSERVATION,
        build: None,
        about: "four species and a cost: the cheapest set of cells that holds 30% of each, solved as \
                an integer linear program (HiGHS), and how much of each it holds",
        toml: r#"
format = "terraflux/1"
name = "Conservation planning (linear programming)"
description = "The minimum set problem as an integer linear program: the cheapest cells that meet every species' target, solved exactly."

[[nodes]]
id = "bird"
op = "nlm.gaussian_field"
params = { rows = 30, columns = 40, range = 8, seed = 1 }
note = "Species' amounts drawn by chance (in their place, raster.read of distribution maps)."
data = { what = "a species' distribution map (the bird's): its suitability or amount in each cell", kind = "values", expect = "0 or more" }

[[nodes]]
id = "frog"
op = "nlm.gaussian_field"
params = { rows = 30, columns = 40, range = 4, seed = 2 }
data = { what = "a species' distribution map (the frog's): its suitability or amount in each cell", kind = "values", expect = "0 or more" }

[[nodes]]
id = "orchid"
op = "nlm.gaussian_field"
params = { rows = 30, columns = 40, range = 12, seed = 3 }
data = { what = "a species' distribution map (the orchid's): its suitability or amount in each cell", kind = "values", expect = "0 or more" }

[[nodes]]
id = "wetland"
op = "nlm.fbm"
params = { rows = 30, columns = 40, dimension = 1.4, seed = 4 }
data = { what = "a feature's map (the wetland's): its amount in each cell", kind = "values", expect = "0 or more" }

[[nodes]]
id = "features"
op = "stack.of"
inputs = { bird = "bird.map", frog = "frog.map", orchid = "orchid.map", wetland = "wetland.map" }

[[nodes]]
id = "cost"
op = "nlm.gaussian_field"
params = { rows = 30, columns = 40, range = 15, seed = 9 }
note = "What each cell costs (no value: not a planning unit)."
data = { what = "what each cell costs (land prices, say)", kind = "values", expect = "above 0; no value: not a planning unit" }

[[nodes]]
id = "plan"
op = "prioritize.optimize"
inputs = { features = "features.stack", cost = "cost.map" }
params = { objective = "min_set", relative_targets = 0.3, gap = 0.01 }
note = "The cheapest cells holding 30% of each species: an integer linear program, solved by HiGHS within 1% of the optimum. Try objective = max_features with a budget, or decisions = proportion. coverage: what each species gets."

[[nodes]]
id = "plot_solution"
op = "view.plot"
inputs = { value = "plan.solution" }
params = { title = "Selected cells" }
"#,
    },
    Template {
        name: "Restoration plan",
        group: CONSERVATION,
        build: None,
        about: "a forest and its cropland drawn by chance, three species' ranges: where to restore 100 km² for \
                the most reduction of their extinction risk (Strassburg et al., 2020: an exact plan, a proven bound)",
        toml: r#"
format = "terraflux/1"
name = "Restoration plan"
description = "The restoration problem of Strassburg et al. (2020): the cells to restore for the most reduction of the species' extinction risk."

[[nodes]]
id = "field"
op = "nlm.gaussian_field"
params = { rows = 40, columns = 50, range = 10, seed = 5 }
note = "A smooth surface from 0 to 1 drawn by chance."

[[nodes]]
id = "forest"
op = "raster.calc"
inputs = { f = "field.map" }
params = { expression = "f * 0.9" }
note = "The share of forest in each cell (in its place, a raster.read of land cover shares)."
data = { what = "the share of forest in each cell (land cover shares)", kind = "values", expect = "0 to 1" }

[[nodes]]
id = "crops"
op = "raster.calc"
inputs = { f = "field.map" }
params = { expression = "1 - f * 0.9" }
note = "The rest is cropland: the land restoration can return to forest."
data = { what = "the share of cropland in each cell: the land restoration can return to forest", kind = "values", expect = "0 to 1" }

[[nodes]]
id = "land_cover"
op = "stack.of"
inputs = { forest = "forest.map", crops = "crops.map" }

[[nodes]]
id = "vegetation"
op = "restoration.original_vegetation"
inputs = { land_cover = "land_cover.stack" }
params = { natural = ["forest"], anthropic = ["crops"] }
note = "What the converted land was before (Strassburg et al., 2020): forest here, and the share of anthropic land."

[[nodes]]
id = "toucan"
op = "nlm.gaussian_field"
params = { rows = 40, columns = 50, range = 15, seed = 11 }

[[nodes]]
id = "jaguar"
op = "nlm.gaussian_field"
params = { rows = 40, columns = 50, range = 20, seed = 12 }

[[nodes]]
id = "frog"
op = "nlm.gaussian_field"
params = { rows = 40, columns = 50, range = 6, seed = 13 }

[[nodes]]
id = "ranges"
op = "raster.calc"
inputs = { t = "toucan.map" }
params = { expression = "t > 0.4" }
note = "A species' range: the cells above 0 (in its place, raster.read of range maps)."
data = { what = "a species' range (the toucan's): 1 where it lives", kind = "mask" }

[[nodes]]
id = "ranges2"
op = "raster.calc"
inputs = { j = "jaguar.map" }
params = { expression = "j > 0.5" }
data = { what = "a species' range (the jaguar's): 1 where it lives", kind = "mask" }

[[nodes]]
id = "ranges3"
op = "raster.calc"
inputs = { f = "frog.map" }
params = { expression = "f > 0.3" }
data = { what = "a species' range (the frog's): 1 where it lives", kind = "mask" }

[[nodes]]
id = "species"
op = "stack.of"
inputs = { toucan = "ranges.map", jaguar = "ranges2.map", frog = "ranges3.map" }

[[nodes]]
id = "habitat"
op = "sdm.area_of_habitat"
inputs = { ranges = "species.stack", land_cover = "land_cover.stack", original = "vegetation.original", restore = "vegetation.restore", anthropic = "vegetation.anthropic" }
params = { natural = ["forest"], cell_area = 1.0 }
note = "Every species' habitat now and before conversion (km²), and what restoring a cell gives it."

[[nodes]]
id = "plan"
op = "restoration.plan"
inputs = { restorable = "vegetation.anthropic", table = "habitat.table", gain = "habitat.gain" }
params = { target = 100, cell_area = 1.0, z_sd = 0.1 }
note = "The share of every cell to restore, 100 km² in all, for the most reduction of the species' extinction risk (exact within 1e-7; method = steps for the stepwise procedure). Try levels = [0.25, 0.5, 1] for a trade-off curve. summary: the risk reduction and its deviation from z's."

[[nodes]]
id = "plot_plan"
op = "view.plot"
inputs = { value = "plan.restored" }
params = { title = "Share of each cell restored" }
"#,
    },
    Template {
        name: "Species distribution model",
        group: SPECIES,
        build: None,
        about: "three predictors drawn by chance, presences drawn where a species finds its conditions, their \
                preparation in one node (a record per cell, the predictors without collinearity, background \
                points, spatial blocks), MaxNet fitted with cross-validation and its map of suitability",
        toml: r#"
format = "terraflux/1"
name = "Species distribution model"
description = "A species distribution model with MaxNet: the records and predictors prepared in one node, the model fitted over spatial folds, the suitability mapped."

[[nodes]]
id = "temperature"
op = "nlm.gaussian_field"
params = { rows = 60, columns = 80, range = 14, seed = 21 }
note = "A predictor drawn by chance (in its place, raster.read of a climate layer)."
data = { what = "a climate predictor: the temperature", kind = "values" }

[[nodes]]
id = "rainfall"
op = "nlm.gaussian_field"
params = { rows = 60, columns = 80, range = 22, seed = 22 }
data = { what = "a climate predictor: the rainfall", kind = "values" }

[[nodes]]
id = "relief"
op = "nlm.gaussian_field"
params = { rows = 60, columns = 80, range = 6, seed = 23 }
data = { what = "a terrain predictor: the relief", kind = "values" }

[[nodes]]
id = "predictors"
op = "stack.of"
inputs = { temperature = "temperature.map", rainfall = "rainfall.map", relief = "relief.map" }

[[nodes]]
id = "suitability"
op = "raster.calc"
inputs = { t = "temperature.map", r = "rainfall.map" }
params = { expression = "exp(-((t - 0.6) ** 2) / 0.02 - ((r - 0.5) ** 2) / 0.05)" }
note = "Where the species finds its conditions: warm cells of middling rainfall."

[[nodes]]
id = "presences"
op = "sdm.background"
inputs = { env = "predictors.stack", bias = "suitability.map" }
params = { n = 120, seed = 3 }
note = "Presences drawn by the suitability (in their place, vector.read of the records, e.g. a CSV of GBIF)."

[[nodes]]
id = "data"
op = "sdm.prepare"
inputs = { occurrences = "presences.points", env = "predictors.stack" }
params = { clean = false, background = 800, folds = 4, seed = 1 }
note = "The preparation (the Python library's prepare, `tf.sdm.prepare`): a record per cell, the predictors without collinearity (VIF 10), 800 background points, 4 folds of spatial blocks. With lon/lat records clean = true runs CoordinateCleaner's tests; area = a distance in metres draws the background near the records. report: what each step removed."

[[nodes]]
id = "fit"
op = "sdm.fit"
inputs = { records = "data.records" }
note = "MaxNet fitted on every fold and tested on the others (AUC, TSS, Boyce...), then on every record."

[[nodes]]
id = "predict"
op = "sdm.maxnet_predict"
inputs = { model = "fit.model", env = "data.env" }

[[nodes]]
id = "plot"
op = "view.plot"
inputs = { value = "predict.prediction" }
params = { title = "Suitability (MaxNet, cloglog)" }
"#,
    },
    Template {
        name: "Multi-action planning",
        group: CONSERVATION,
        build: None,
        about: "six planning units, two species and a threat: which units to monitor and where to act against \
                the threat so that both species recover, at the least cost (Salgado-Rojas et al., 2020)",
        toml: r#"
format = "terraflux/1"
name = "Multi-action planning"
description = "The minimum cost model of multi-action planning (minimizeCosts): the cheapest monitoring and actions that meet the species' recovery targets."

[[nodes]]
id = "pu"
op = "table.of"
params = { columns = ["id", "monitoring_cost"], rows = [[1, 2], [2, 1], [3, 3], [4, 1], [5, 2], [6, 2]] }
note = "The planning units and what monitoring them costs (in its place, a table read from a file)."

[[nodes]]
id = "features"
op = "table.of"
params = { columns = ["id", "target_recovery"], rows = [[1, 1], [2, 1]] }
note = "Two species and the recovery each must reach."

[[nodes]]
id = "dist_features"
op = "table.of"
params = { columns = ["pu", "feature", "amount"], rows = [[1, 1, 1], [2, 1, 1], [3, 1, 1], [3, 2, 1], [4, 2, 1], [5, 2, 1], [6, 1, 1]] }
note = "Where each species lives."

[[nodes]]
id = "threats"
op = "table.of"
params = { columns = ["id"], rows = [[1]] }

[[nodes]]
id = "dist_threats"
op = "table.of"
params = { columns = ["pu", "threat", "amount", "action_cost"], rows = [[1, 1, 1, 1], [2, 1, 1, 4], [3, 1, 1, 2], [4, 1, 1, 1], [5, 1, 1, 2]] }
note = "Where the threat is, and what acting against it costs there."

[[nodes]]
id = "plan"
op = "prioritize.actions"
inputs = { pu = "pu.table", features = "features.table", dist_features = "dist_features.table", threats = "threats.table", dist_threats = "dist_threats.table" }
params = { model = "minimizeCosts" }
note = "An integer linear program, solved exactly. Try model = maximizeBenefits with a budget. summary: every unit's actions; benefit: what each species gets."
"#,
    },
    Template {
        name: "MSPA's edge width",
        group: PATTERNS,
        build: None,
        about: "one forest map, MSPA with edges of 1 and of 4 cells side by side",
        toml: r#"
format = "terraflux/1"
name = "MSPA's edge width"
description = "How the edge's width changes MSPA's classes: the same forest, edges of 1 and 4 cells."

[[nodes]]
id = "landscape"
op = "nlm.random_cluster"
params = { rows = 300, columns = 300, p = 0.6, weights = [0.4, 0.6], seed = 11 }
note = "Two classes drawn by chance: 1 the background, 2 the forest (MSPA's coding already)."
data = { what = "a map of the forest and the background", kind = "classes", classes = { 1 = "background", 2 = "forest" } }

[[nodes]]
id = "edge_1"
op = "landscape.mspa"
inputs = { map = "landscape.map" }
params = { edge_width = 1 }

[[nodes]]
id = "edge_4"
op = "landscape.mspa"
inputs = { map = "landscape.map" }
params = { edge_width = 4 }

[[nodes]]
id = "plot_1"
op = "view.plot"
inputs = { value = "edge_1.map" }
params = { title = "Edge of 1 cell" }

[[nodes]]
id = "plot_4"
op = "view.plot"
inputs = { value = "edge_4.map" }
params = { title = "Edge of 4 cells" }
"#,
    },
    Template {
        name: "Neutral landscapes",
        group: SURFACES,
        build: None,
        about: "the three neutral landscape models side by side: patches of classes, a smooth surface, a fractal \
                one (the viewer colours each as what it is)",
        toml: r#"
format = "terraflux/1"
name = "Neutral landscapes"
description = "NLMR's models: a random cluster (classes), a Gaussian field and a fractional Brownian motion (0 to 1)."

[[nodes]]
id = "clusters"
op = "nlm.random_cluster"
params = { rows = 256, columns = 256, p = 0.4, weights = [0.25, 0.25, 0.5], seed = 4 }
note = "Classes: a colour each."

[[nodes]]
id = "field"
op = "nlm.gaussian_field"
params = { rows = 256, columns = 256, range = 20, seed = 4 }
note = "Values from 0 to 1: a ramp."

[[nodes]]
id = "fractal"
op = "nlm.fbm"
params = { rows = 256, columns = 256, dimension = 1.5, seed = 4 }

[[nodes]]
id = "plot_clusters"
op = "view.plot"
inputs = { value = "clusters.map" }
params = { title = "Random cluster" }

[[nodes]]
id = "plot_field"
op = "view.plot"
inputs = { value = "field.map" }
params = { title = "Gaussian field" }

[[nodes]]
id = "plot_fractal"
op = "view.plot"
inputs = { value = "fractal.map" }
params = { title = "Fractal" }
"#,
    },
    Template {
        name: "Dinamica EGO",
        group: LAND_CHANGE,
        about: "Dinamica EGO's classic model (Markov rates, weights of evidence, Expander and Patcher) on two dates \
                of land use drawn by chance, run three steps ahead",
        toml: "",
        build: Some(dinamica),
    },
    Template {
        name: "CLUE-S",
        group: LAND_CHANGE,
        about: "CLUE-S as lulcc's (logistic regressions, a linear demand, CLUE's allocation) on two dates of land use \
                drawn by chance, run three steps ahead",
        toml: "",
        build: Some(clue_s),
    },
    Template {
        name: "PLUS (LEAS and CARS)",
        group: LAND_CHANGE,
        about: "PLUS on two dates of land use drawn by chance: the land expansion analysis (a random forest per class) gives the growth probabilities, CARS allocates three steps ahead",
        toml: "",
        build: Some(plus),
    },
    Template {
        name: "Forest gain and loss",
        group: PATTERNS,
        build: None,
        about: "a forest at two dates: its simple and morphological change (the real one told from the \
                shifts of its edges) and the change of its fragmentation (FAD at four scales)",
        toml: r#"
format = "terraflux/1"
name = "Forest gain and loss"
description = "GuidosToolbox's change analysis of a forest at two dates: simple change, morphological change (the real one told from the shifts of its edges) and the change of its fragmentation (FAD)."

[[nodes]]
id = "before"
op = "nlm.random_cluster"
params = { rows = 250, columns = 250, p = 0.55, weights = [0.4, 0.6], seed = 27 }
note = "The forest at the first date, drawn by chance: 1 the background, 2 the forest (in its place, a raster.read of a map of yours)."
data = { what = "a map of the forest, the first date", kind = "classes", classes = { 1 = "background", 2 = "forest" } }

[[nodes]]
id = "pressure"
op = "nlm.gaussian_field"
params = { rows = 250, columns = 250, range = 30, seed = 28 }
note = "Where clearing presses, from 0 to 1."

[[nodes]]
id = "after"
op = "raster.calc"
inputs = { f = "before.map", p = "pressure.map" }
params = { expression = "1 if f == 2 and p > 0.6 and nbsum(f == 1, 1) >= 1 else 2 if f == 1 and p < 0.2 and nbsum(f == 2, 1) >= 5 else f" }
note = "The second date: the forest cleared from its edges where the pressure is high, regrown in the gaps where it is low."
data = { what = "a map of the forest, the second date", kind = "classes", classes = { 1 = "background", 2 = "forest" } }

[[nodes]]
id = "simple"
op = "change.simple"
inputs = { before = "before.map", after = "after.map" }
note = "Every cell's change: stable background, gain, loss, stable forest; stats counts them."

[[nodes]]
id = "morphological"
op = "change.morphological"
inputs = { before = "before.map", after = "after.map" }
note = "The same change with the spurious one at the forest's edges (a cell's shift) told apart; stats has the net change and the elasticity."

[[nodes]]
id = "fad"
op = "change.fad"
inputs = { before = "before.map", after = "after.map" }
params = { foreground = 2, windows = [7, 13, 27, 81] }
note = "FAD at four scales at both dates: how many forest cells moved to a more fragmented class or a less fragmented one (stats), and the matrix of the classes."

[[nodes]]
id = "plot_simple"
op = "view.plot"
inputs = { value = "simple.map" }
params = { title = "Simple change" }

[[nodes]]
id = "plot_morphological"
op = "view.plot"
inputs = { value = "morphological.map" }
params = { title = "Morphological change" }
"#,
    },
    Template {
        name: "Patches and their shapes",
        group: METRICS,
        build: None,
        about: "every patch of a land cover measured (area, shape, core, nearest neighbour), GuidosToolbox's \
                parcellation and the contortion of the forest's outlines",
        toml: r#"
format = "terraflux/1"
name = "Patches and their shapes"
description = "Every patch of a land cover measured (landscapemetrics' patch level): its area, shape and core; GuidosToolbox's parcellation (the effective mesh size) and the contortion of the forest's outlines."

[[nodes]]
id = "cover"
op = "nlm.random_cluster"
params = { rows = 200, columns = 200, resolution = 30, p = 0.5, weights = [0.5, 0.3, 0.2], seed = 71 }
note = "A land cover of three classes drawn by chance on cells of 30 m: 1 forest, 2 grassland, 3 farming (in its place, a raster.read of a map of yours)."
data = { what = "a land cover map", kind = "classes", classes = { 1 = "forest", 2 = "grassland", 3 = "farming" } }

[[nodes]]
id = "patches"
op = "metrics.patch"
inputs = { map = "cover.map" }
params = { metrics = ["area", "perim", "shape", "core", "enn"], show = "area" }
note = "A row per patch: its area (ha), perimeter, shape index, core area and the distance to its nearest neighbour of the same class. Sort a column by a click; map paints every patch with its area."

[[nodes]]
id = "parcellation"
op = "landscape.parcellation"
inputs = { map = "cover.map" }
note = "GuidosToolbox's parcellation: per class its parcels, their mean size (APS), the effective mesh size (AWAPS) and the division."

[[nodes]]
id = "contortion"
op = "landscape.contortion"
inputs = { map = "cover.map" }
params = { foreground = 1, min_area = 20 }
note = "How contorted the outline of every forest patch of 20 cells or more is."

[[nodes]]
id = "plot_area"
op = "view.plot"
inputs = { value = "patches.map" }
params = { title = "Area of each patch (ha)" }

[[nodes]]
id = "plot_contortion"
op = "view.plot"
inputs = { value = "contortion.map" }
params = { title = "Contortion of the forest" }
"#,
    },
    Template {
        name: "Kinds of landscapes",
        group: METRICS,
        build: None,
        about: "a land cover cut into squares described by their metrics (spatial signatures), their \
                principal components and the squares grouped into kinds of landscapes (k-means)",
        toml: r#"
format = "terraflux/1"
name = "Kinds of landscapes"
description = "A land cover cut into squares, every square described by its metrics (pylandstats' spatial signatures), their principal components, and the squares grouped by k-means into kinds of landscapes."

[[nodes]]
id = "gradient"
op = "nlm.planar_gradient"
params = { rows = 240, columns = 240, direction = 120, seed = 81 }
note = "A gradient across the map (a climate, an altitude...)."

[[nodes]]
id = "patchiness"
op = "nlm.gaussian_field"
params = { rows = 240, columns = 240, range = 6, seed = 82 }

[[nodes]]
id = "surface"
op = "nlm.merge"
inputs = { primary = "gradient.map", secondary = "patchiness.map" }
params = { scaling = 1.2 }
note = "The gradient with patches on it."

[[nodes]]
id = "cover"
op = "nlm.classify"
inputs = { map = "surface.map" }
params = { weights = [0.3, 0.3, 0.2, 0.2] }
note = "Four classes of 30, 30, 20 and 20 % of the cells, which change along the gradient (in its place, a raster.read of a land cover of yours)."
data = { what = "a land cover map of any classes (four here): the kinds of landscape are found from its patterns", kind = "classes" }

[[nodes]]
id = "signatures"
op = "metrics.signatures"
inputs = { maps = "cover.classes" }
params = { grid = 30, landscape_metrics = ["shdi", "ed", "lpi", "contag"], class_metrics = ["pland"] }
note = "The map tiled by squares of 30 cells: a row per square, its diversity, edge density, largest patch, contagion and the share of every class."

[[nodes]]
id = "pca"
op = "metrics.pca"
inputs = { table = "signatures.table" }
params = { components = 3 }
note = "The principal components of the metrics: loadings tells what each means, scores places every square on them."

[[nodes]]
id = "kinds"
op = "metrics.cluster"
inputs = { table = "signatures.table" }
params = { k = 4, seed = 0 }
note = "The squares in 4 groups of like metrics (k-means of their z-scores): kinds of landscape."

[[nodes]]
id = "plot_cover"
op = "view.plot"
inputs = { value = "cover.classes" }
params = { title = "Land cover" }
"#,
    },
    Template {
        name: "Network of a forest (MSPA)",
        group: CONNECTIVITY,
        build: None,
        about: "MSPA's cores and bridges as a network: its components, the equivalent connected area and the \
                importance of every link and stepping stone (GuidosToolbox)",
        toml: r#"
format = "terraflux/1"
name = "Network of a forest (MSPA)"
description = "GuidosToolbox's network analysis: MSPA's cores (the nodes) and bridges (the links), their components and the importance of every link and stepping stone."

[[nodes]]
id = "landscape"
op = "nlm.random_cluster"
params = { rows = 250, columns = 250, resolution = 30, p = 0.55, weights = [0.45, 0.55], seed = 19 }
note = "Two classes drawn by chance on cells of 30 m: 1 the background, 2 the forest (MSPA's coding already)."
data = { what = "a map of the forest and the background", kind = "classes", classes = { 1 = "background", 2 = "forest" } }

[[nodes]]
id = "mspa"
op = "landscape.mspa"
inputs = { map = "landscape.map" }
params = { edge_width = 2 }
note = "The cores and the bridges between them."

[[nodes]]
id = "components"
op = "networks.gtn_components"
inputs = { map = "mspa.map" }
note = "The networks of cores joined by bridges: summary has their number, the equivalent connected area (ECA) and the coherence (ECA over its largest, %)."

[[nodes]]
id = "importance"
op = "networks.gtn_importance"
inputs = { map = "mspa.map" }
note = "The loss of connectivity (dPC) without each bridge and each stepping-stone core; the top 1, 5 and 10 % in GTB's colours. links and nodes rank them."

[[nodes]]
id = "plot_mspa"
op = "view.plot"
inputs = { value = "mspa.map" }
params = { title = "MSPA" }

[[nodes]]
id = "plot_components"
op = "view.plot"
inputs = { value = "components.map" }
params = { title = "Networks" }

[[nodes]]
id = "plot_importance"
op = "view.plot"
inputs = { value = "importance.map" }
params = { title = "The most important links and nodes" }
"#,
    },
    Template {
        name: "Habitat network (Conefor)",
        group: CONNECTIVITY,
        build: None,
        about: "the forest's patches as a graph: the connectivity of the landscape at three distances (IIC, \
                PC, the equivalent connected area) and the importance of every patch (dPC), as Conefor and \
                Makurhini",
        toml: r#"
format = "terraflux/1"
name = "Habitat network (Conefor)"
description = "A graph of habitat patches: the connectivity of the landscape (IIC, PC and their equivalent connected areas) and the importance of every patch to it, as Conefor and Makurhini."

[[nodes]]
id = "landscape"
op = "nlm.random_cluster"
params = { rows = 150, columns = 150, resolution = 100, p = 0.4, weights = [0.75, 0.25], seed = 23 }
note = "Two classes drawn by chance on cells of 100 m: 1 the matrix, 2 the forest (in its place, a raster.read of a map of yours)."
data = { what = "a map of the habitat (forest) and the matrix around it", kind = "classes", classes = { 1 = "matrix", 2 = "forest" } }

[[nodes]]
id = "graph"
op = "conn.graph_from_raster"
inputs = { map = "landscape.map" }
params = { habitat = 2, area_unit = "ha", distance = "edge" }
note = "Every patch of forest a node, its area its attribute; between every two, the gap between their edges (m)."

[[nodes]]
id = "overall"
op = "conn.overall"
inputs = { graph = "graph.graph" }
params = { distance = [500, 1000, 2000], probability = 0.5 }
note = "The landscape's connectivity for species that cross 500 m, 1 and 2 km of matrix with a probability of one half: a row per distance (IIC, PC, the equivalent connected area...)."

[[nodes]]
id = "importance"
op = "conn.node_importance"
inputs = { graph = "graph.graph" }
params = { distance = 1000, probability = 0.5 }
note = "How much of PC (and IIC) is lost without each patch: dPC, and its fractions intra (its own area), flux and connector (a stepping stone)."

[[nodes]]
id = "dpc"
op = "conn.to_map"
inputs = { graph = "graph.graph", table = "importance.table" }
params = { column = "dPC" }
note = "Every patch painted with its dPC."

[[nodes]]
id = "connector"
op = "conn.to_map"
inputs = { graph = "graph.graph", table = "importance.table" }
params = { column = "dPCconnector" }
note = "And with the part of it as a stepping stone between the others."

[[nodes]]
id = "plot_dpc"
op = "view.plot"
inputs = { value = "dpc.map" }
params = { title = "Importance of each patch (dPC, %)" }

[[nodes]]
id = "plot_connector"
op = "view.plot"
inputs = { value = "connector.map" }
params = { title = "As a stepping stone (dPCconnector)" }
"#,
    },
    Template {
        name: "Circuit connectivity",
        group: CONNECTIVITY,
        build: None,
        about: "a resistance and its best habitat: Circuitscape's currents between the patches (pairwise) \
                and Omniscape's omnidirectional connectivity",
        toml: r#"
format = "terraflux/1"
name = "Circuit connectivity"
description = "Circuitscape's currents between habitat patches, and Omniscape's omnidirectional connectivity, over one resistance."

[[nodes]]
id = "quality"
op = "nlm.gaussian_field"
params = { rows = 100, columns = 100, range = 20, seed = 31 }
note = "A habitat's quality from 0 to 1 drawn by chance (in its place, a raster.read of a suitability map)."
data = { what = "a habitat's quality, or a suitability map", kind = "values", expect = "0 to 1" }

[[nodes]]
id = "resistance"
op = "raster.calc"
inputs = { q = "quality.map" }
params = { expression = "1 + 99 * (1 - q) ** 3" }
note = "The resistance to movement: 1 in the best habitat, up to 100 where there is none."
data = { what = "a resistance: the cost of crossing a cell", kind = "values", expect = "1 or more" }

[[nodes]]
id = "cores"
op = "raster.calc"
inputs = { q = "quality.map" }
params = { expression = "q > 0.8" }
note = "The cores: the best habitat (1), the rest 0."

[[nodes]]
id = "parcels"
op = "landscape.parcellation"
inputs = { map = "cores.map" }
note = "Every patch numbered, of the cores and of the rest."

[[nodes]]
id = "patches"
op = "raster.calc"
inputs = { c = "cores.map", p = "parcels.parcels" }
params = { expression = "p if c == 1 else 0" }
note = "The cores' patches by number: Circuitscape's focal regions (0 elsewhere)."

[[nodes]]
id = "pairwise"
op = "circuit.pairwise"
inputs = { habitat = "resistance.map", points = "patches.map" }
params = { cum_map = true, max_map = true }
note = "Circuitscape's pairwise mode: the effective resistance between every pair of patches (resistances), the current of all pairs summed (cum): where animals moving between them would pass."

[[nodes]]
id = "omniscape"
op = "circuit.omniscape"
inputs = { resistance = "resistance.map" }
params = { radius = 12, block_size = 5, flow_potential = true, normalized_current = true }
note = "Omniscape: currents from every cell to those within 12 cells (a target every 5 cells), the sources from the resistance. normalized: the current over what it would be without resistances (above 1, channelled; below, impeded)."

[[nodes]]
id = "plot_resistance"
op = "view.plot"
inputs = { value = "resistance.map" }
params = { title = "Resistance" }

[[nodes]]
id = "plot_current"
op = "view.plot"
inputs = { value = "pairwise.cum" }
params = { title = "Current between the cores (Circuitscape)" }

[[nodes]]
id = "plot_omniscape"
op = "view.plot"
inputs = { value = "omniscape.normalized" }
params = { title = "Normalised current (Omniscape)" }
"#,
    },
    Template {
        name: "Least costs and random walks",
        group: CONNECTIVITY,
        build: None,
        about: "a resistance and two places: the accumulated cost of the cheapest routes, randomised \
                shortest paths and a random walk between them (gdistance)",
        toml: r#"
format = "terraflux/1"
name = "Least costs and random walks"
description = "gdistance's ways across a landscape between two places: the accumulated cost of the cheapest routes, and the passages of walkers that know the way less and less well (randomised shortest paths, then a random walk)."

[[nodes]]
id = "roughness"
op = "nlm.fbm"
params = { rows = 100, columns = 100, dimension = 1.5, seed = 91 }
note = "How hard each cell is to cross, 0 to 1 (in its place, a raster.read of a resistance)."
data = { what = "a resistance: how hard each cell is to cross", kind = "values", expect = "above 0" }

[[nodes]]
id = "conductance"
op = "raster.calc"
inputs = { r = "roughness.map" }
params = { expression = "1 / (1 + 20 * r ** 2)" }
note = "How easily it is crossed: 1 over the resistance."

[[nodes]]
id = "transition"
op = "cost.transition"
inputs = { map = "conductance.map" }
params = { function = "mean", directions = 8 }
note = "The connection of every cell with its 8 neighbours: the mean of their conductances."

[[nodes]]
id = "corrected"
op = "cost.geo_correction"
inputs = { transition = "transition.transition" }
params = { type = "c" }
note = "Corrected for the distances between the cells' centres (the diagonals longer)."

[[nodes]]
id = "near_a"
op = "nlm.distance_gradient"
params = { rows = 100, columns = 100, origin = [3, 8, 45, 55], nlmr_compat = false, rescale = false }

[[nodes]]
id = "origin"
op = "raster.calc"
inputs = { d = "near_a.map" }
params = { expression = "d == 0" }
note = "Where the walkers start: a strip in the west (in its place, a raster of a reserve, or vector.read of points)."
data = { what = "where the walkers start: 1 on the origin", kind = "mask" }

[[nodes]]
id = "near_b"
op = "nlm.distance_gradient"
params = { rows = 100, columns = 100, origin = [93, 98, 45, 55], nlmr_compat = false, rescale = false }

[[nodes]]
id = "goal"
op = "raster.calc"
inputs = { d = "near_b.map" }
params = { expression = "d == 0" }
note = "Where they go: a strip in the east."
data = { what = "where the walkers go: 1 on the goal", kind = "mask" }

[[nodes]]
id = "cost"
op = "cost.acc_cost"
inputs = { transition = "corrected.transition", origins = "origin.map" }
note = "The cost of the cheapest route from the origin to every cell."

[[nodes]]
id = "rsp"
op = "cost.passage"
inputs = { transition = "corrected.transition", origins = "origin.map", goals = "goal.map" }
params = { theta = 0.05 }
note = "Randomised shortest paths (theta 0.05): walkers that mostly take the cheap ways, spread over several. Try theta = 1 (near the least-cost path) or 0.001 (near a random walk)."

[[nodes]]
id = "random_walk"
op = "cost.passage"
inputs = { transition = "transition.transition", origins = "origin.map", goals = "goal.map" }
note = "Without theta, a random walk: the current from the origin to the goal (Circuitscape's), over the transition before the correction."

[[nodes]]
id = "plot_cost"
op = "view.plot"
inputs = { value = "cost.map" }
params = { title = "Accumulated cost from the origin" }

[[nodes]]
id = "plot_rsp"
op = "view.plot"
inputs = { value = "rsp.map" }
params = { title = "Randomised shortest paths" }

[[nodes]]
id = "plot_walk"
op = "view.plot"
inputs = { value = "random_walk.map" }
params = { title = "Random walk" }
"#,
    },
    Template {
        name: "Restoring a corridor",
        group: CONNECTIVITY,
        build: None,
        about: "two areas of habitat: the least-cost path between them across a resistance and what \
                restoring it gains in connectivity (GuidosToolbox's Restoration Planner)",
        toml: r#"
format = "terraflux/1"
name = "Restoring a corridor"
description = "GuidosToolbox's Restoration Planner: a resistance from the distance to the habitat, the least-cost path between two areas, and what restoring it gains."

[[nodes]]
id = "landscape"
op = "nlm.random_cluster"
params = { rows = 150, columns = 150, p = 0.4, weights = [0.8, 0.2], seed = 17 }
note = "Two classes drawn by chance: 1 the background, 2 the habitat (GuidosToolbox's coding already)."
data = { what = "a map of the habitat and the background", kind = "classes", classes = { 1 = "background", 2 = "habitat" } }

[[nodes]]
id = "near_a"
op = "nlm.distance_gradient"
params = { rows = 150, columns = 150, origin = [5, 20, 5, 20], nlmr_compat = false, rescale = false }
note = "The distance to a square in the north-west: 0 inside it."

[[nodes]]
id = "near_b"
op = "nlm.distance_gradient"
params = { rows = 150, columns = 150, origin = [130, 145, 130, 145], nlmr_compat = false, rescale = false }
note = "And to one in the south-east."

[[nodes]]
id = "a"
op = "raster.calc"
inputs = { d = "near_a.map" }
params = { expression = "d == 0" }
note = "Area A (in its place, a raster of a reserve)."
data = { what = "area A, a reserve: 1 inside it", kind = "mask" }

[[nodes]]
id = "b"
op = "raster.calc"
inputs = { d = "near_b.map" }
params = { expression = "d == 0" }
note = "Area B."
data = { what = "area B, a reserve: 1 inside it", kind = "mask" }

[[nodes]]
id = "habitat"
op = "raster.calc"
inputs = { h = "landscape.map", a = "a.map", b = "b.map" }
params = { expression = "2 if a or b else h" }
note = "The two areas are habitat too."

[[nodes]]
id = "resistance"
op = "restoration.resistance"
inputs = { map = "habitat.map" }
note = "GTB's resistance: 2 in the habitat, growing with the distance from it up to 100."

[[nodes]]
id = "path"
op = "restoration.least_cost_path"
inputs = { resistance = "resistance.map", a = "a.map", b = "b.map" }
note = "The cheapest way from A to B: cost is the cost from A plus that from B (its least along the path); stats has its length and expense."

[[nodes]]
id = "corridor"
op = "restoration.corridor"
inputs = { resistance = "resistance.map", a = "a.map", b = "b.map", habitat = "habitat.map" }
note = "The path's background restored: stats has the cells restored and the ECA and coherence gained by it."

[[nodes]]
id = "plot_cost"
op = "view.plot"
inputs = { value = "path.cost" }
params = { title = "Cost from A plus from B" }

[[nodes]]
id = "shown"
op = "raster.calc"
inputs = { h = "habitat.map", p = "corridor.path" }
params = { expression = "3 if p == 1 else h" }
note = "The corridor (3) over the background (1) and the habitat (2); corridor.restored is the habitat once its background is restored."

[[nodes]]
id = "plot_corridor"
op = "view.plot"
inputs = { value = "shown.map" }
params = { title = "The corridor (3)" }
"#,
    },
    Template {
        name: "Linkage Mapper: corridors, pinch points and barriers",
        group: CONNECTIVITY,
        build: None,
        about: "the large patches of a habitat and a resistance: the least-cost corridors between them, where \
                the current pinches and where restoring a circle would shorten a link most (Linkage Mapper)",
        toml: r#"
format = "terraflux/1"
name = "Linkage Mapper: corridors, pinch points and barriers"
description = "Linkage Mapper's three tools over one habitat: the least-cost corridors between its cores (Linkage Pathways), the pinch points in them (Pinchpoint Mapper, with Circuitscape) and the barriers where restoring a circle would shorten the links most (Barrier Mapper)."

[[nodes]]
id = "quality"
op = "nlm.gaussian_field"
params = { rows = 80, columns = 80, range = 12, seed = 31 }
note = "A habitat's quality from 0 to 1 drawn by chance (in its place, a raster.read of a suitability map)."
data = { what = "a habitat's quality, or a suitability map", kind = "values", expect = "0 to 1" }

[[nodes]]
id = "habitat"
op = "raster.calc"
inputs = { q = "quality.map" }
params = { expression = "q > 0.7" }
note = "The habitat: the quality above 0.7 (1), the rest 0."

[[nodes]]
id = "large"
op = "raster.sieve"
inputs = { map = "habitat.map" }
params = { threshold = 25, neighbours = 8 }
note = "The patches of fewer than 25 cells merged into what is around them: only the large ones are cores."

[[nodes]]
id = "cores"
op = "raster.label"
inputs = { map = "large.map" }
params = { neighbours = 8, classes = [1] }
note = "Every core numbered (1, 2, ...), no value elsewhere: Linkage Mapper's core areas (in their place, a raster of the reserves with their ids)."
data = { what = "the core areas: a raster of the reserves, an id each (1, 2, ...) and no value elsewhere", kind = "values", expect = "a whole id per core" }

[[nodes]]
id = "resistance"
op = "raster.calc"
inputs = { q = "quality.map" }
params = { expression = "1 + 99 * (1 - q) ** 3" }
note = "The resistance to movement: 1 in the best habitat, up to 100 where there is none (in its place, a raster.read of a resistance from the literature)."
data = { what = "a resistance: the cost of crossing a cell", kind = "values", expect = "1 or more" }

[[nodes]]
id = "pathways"
op = "linkage.pathways"
inputs = { cores = "cores.map", resistance = "resistance.map" }
params = { adjacency = "cost", width = 60 }
note = "Linkage Pathways: the cores whose cost allocation zones touch are linked (links), by the least-cost path (paths) and the corridors around it, cut where a cell is 60 cost units dearer than the path (corridors)."

[[nodes]]
id = "pinchpoints"
op = "linkage.pinchpoints"
inputs = { cores = "cores.map", resistance = "resistance.map", links = "pathways.links" }
params = { width = 60, mode = "pairwise" }
note = "Pinchpoint Mapper: Circuitscape between the two cores of each link, inside its corridor only; current is the largest of the links': it concentrates where the corridor narrows. links has each effective resistance."

[[nodes]]
id = "barriers"
op = "linkage.barriers"
inputs = { cores = "cores.map", resistance = "resistance.map", links = "pathways.links" }
params = { min_radius = 2, max_radius = 6, step = 2 }
note = "Barrier Mapper: for each link and each radius (2, 4 and 6 cells), what restoring a circle there would gain: the shortening of the least-cost distance per unit of restored distance. circles is the map, radius the size that gave it."

[[nodes]]
id = "plot_cores"
op = "view.plot"
inputs = { value = "cores.map" }
params = { title = "The cores" }

[[nodes]]
id = "plot_corridors"
op = "view.plot"
inputs = { value = "pathways.corridors" }
params = { title = "Corridors (normalized cost distance)" }

[[nodes]]
id = "plot_current"
op = "view.plot"
inputs = { value = "pinchpoints.current" }
params = { title = "Current in the corridors (pinch points)" }

[[nodes]]
id = "plot_improvement"
op = "view.plot"
inputs = { value = "barriers.circles" }
params = { title = "Improvement of restoring a circle (barriers)" }
"#,
    },
    Template {
        name: "Irreplaceability of a plan",
        group: CONSERVATION,
        build: None,
        about: "an optimal plan for four species evaluated (what it holds, Ferrier's irreplaceability, the \
                rarity of its cells) and a portfolio of ten near-optimal plans",
        toml: r#"
format = "terraflux/1"
name = "Irreplaceability of a plan"
description = "An optimal plan for four species and its evaluation: how much of each it holds, Ferrier's irreplaceability and the rarity of the cells chosen, and a portfolio of near-optimal plans."

[[nodes]]
id = "bird"
op = "nlm.gaussian_field"
params = { rows = 30, columns = 40, range = 8, seed = 1 }
note = "Species' amounts drawn by chance (in their place, raster.read of distribution maps)."
data = { what = "a species' distribution map (the bird's): its suitability or amount in each cell", kind = "values", expect = "0 or more" }

[[nodes]]
id = "frog"
op = "nlm.gaussian_field"
params = { rows = 30, columns = 40, range = 4, seed = 2 }
data = { what = "a species' distribution map (the frog's): its suitability or amount in each cell", kind = "values", expect = "0 or more" }

[[nodes]]
id = "orchid"
op = "nlm.gaussian_field"
params = { rows = 30, columns = 40, range = 12, seed = 3 }
data = { what = "a species' distribution map (the orchid's): its suitability or amount in each cell", kind = "values", expect = "0 or more" }

[[nodes]]
id = "wetland"
op = "nlm.fbm"
params = { rows = 30, columns = 40, dimension = 1.4, seed = 4 }
data = { what = "a feature's map (the wetland's): its amount in each cell", kind = "values", expect = "0 or more" }

[[nodes]]
id = "features"
op = "stack.of"
inputs = { bird = "bird.map", frog = "frog.map", orchid = "orchid.map", wetland = "wetland.map" }

[[nodes]]
id = "cost"
op = "nlm.gaussian_field"
params = { rows = 30, columns = 40, range = 15, seed = 9 }
note = "What each cell costs."
data = { what = "what each cell costs (land prices, say)", kind = "values", expect = "above 0" }

[[nodes]]
id = "plan"
op = "prioritize.optimize"
inputs = { features = "features.stack", cost = "cost.map" }
params = { objective = "min_set", relative_targets = 0.3, gap = 0.01 }
note = "The cheapest cells that hold 30 % of every species (within 1 % of the optimum)."

[[nodes]]
id = "evaluation"
op = "prioritize.evaluate"
inputs = { features = "features.stack", cost = "cost.map", solution = "plan.solution" }
params = { relative_targets = 0.3 }
note = "The plan evaluated: summary (the cost, the cells, the boundary), representation and coverage of every species, and per cell chosen its rarity-weighted richness and Ferrier's irreplaceability (ferrier: a layer per species and the total)."

[[nodes]]
id = "portfolio"
op = "prioritize.optimize"
inputs = { features = "features.stack", cost = "cost.map" }
params = { objective = "min_set", relative_targets = 0.3, portfolio = "gap", pool_gap = 0.1, number_solutions = 10 }
note = "Ten plans within 10 % of the best: frequency, how often each cell is chosen; the cells always chosen are hard to replace."

[[nodes]]
id = "plot_solution"
op = "view.plot"
inputs = { value = "plan.solution" }
params = { title = "The plan" }

[[nodes]]
id = "plot_ferrier"
op = "view.plot"
inputs = { value = "evaluation.ferrier" }
params = { title = "Ferrier's irreplaceability" }

[[nodes]]
id = "plot_frequency"
op = "view.plot"
inputs = { value = "portfolio.frequency" }
params = { title = "Chosen in how many of ten plans" }
"#,
    },
    Template {
        name: "A species in a warmer climate",
        group: SPECIES,
        build: None,
        about: "a MaxNet model explained (response curves, the importance of the predictors) and projected \
                to a climate 2 °C warmer: the gains, the losses and where it extrapolates (MESS)",
        toml: r#"
format = "terraflux/1"
name = "A species in a warmer climate"
description = "A species distribution model explained and projected: its response curves and the importance of its predictors, its suitability today and in a climate 2 degrees warmer, and where that projection extrapolates (MESS)."

[[nodes]]
id = "field_t"
op = "nlm.gaussian_field"
params = { rows = 60, columns = 80, range = 14, seed = 21 }

[[nodes]]
id = "temperature"
op = "raster.calc"
inputs = { f = "field_t.map" }
params = { expression = "14 + f * 12" }
note = "The mean temperature, from 14 to 26 °C (in its place, raster.read of a climate layer)."
data = { what = "the mean temperature today", kind = "values", expect = "degrees C" }

[[nodes]]
id = "field_r"
op = "nlm.gaussian_field"
params = { rows = 60, columns = 80, range = 22, seed = 22 }

[[nodes]]
id = "rainfall"
op = "raster.calc"
inputs = { f = "field_r.map" }
params = { expression = "800 + f * 1600" }
note = "The rainfall, from 800 to 2400 mm."
data = { what = "the rainfall today", kind = "values", expect = "mm" }

[[nodes]]
id = "predictors"
op = "stack.of"
inputs = { temperature = "temperature.map", rainfall = "rainfall.map" }

[[nodes]]
id = "suitability"
op = "raster.calc"
inputs = { t = "temperature.map", r = "rainfall.map" }
params = { expression = "exp(-((t - 19) ** 2) / 3 - ((r - 1600) ** 2) / 150000)" }
note = "Where the species finds its conditions: about 19 °C and 1600 mm."

[[nodes]]
id = "presences"
op = "sdm.background"
inputs = { env = "predictors.stack", bias = "suitability.map" }
params = { n = 150, seed = 3 }
note = "Presences drawn by the suitability (in their place, vector.read of the records)."

[[nodes]]
id = "data"
op = "sdm.prepare"
inputs = { occurrences = "presences.points", env = "predictors.stack" }
params = { clean = false, background = 1000, folds = 4, seed = 1 }
note = "A record per cell, 1000 background points, 4 folds of spatial blocks."

[[nodes]]
id = "fit"
op = "sdm.fit"
inputs = { records = "data.records" }
note = "MaxNet fitted over the folds (stats: AUC, TSS, Boyce...)."

[[nodes]]
id = "curves"
op = "sdm.response_curves"
inputs = { model = "fit.model", records = "data.records" }
note = "How the suitability answers each predictor, the other at its mean: a table of a column of values and one of suitability per predictor."

[[nodes]]
id = "importance"
op = "sdm.importance"
inputs = { model = "fit.model", records = "data.records" }
note = "How much the AUC falls when each predictor is shuffled: the most important first."

[[nodes]]
id = "today"
op = "sdm.maxnet_predict"
inputs = { model = "fit.model", env = "data.env" }
note = "The suitability today."

[[nodes]]
id = "warmer"
op = "raster.calc"
inputs = { t = "temperature.map" }
params = { expression = "t + 2" }
note = "The temperature in a climate 2 °C warmer (in its place, raster.read of a scenario's layer)."
data = { what = "the temperature in the future climate (a scenario's layer)", kind = "values", expect = "degrees C" }

[[nodes]]
id = "future"
op = "stack.set"
inputs = { stack = "data.env", map = "warmer.map" }
params = { name = "temperature" }
note = "The predictors of the future: the warmer temperature in place of today's."

[[nodes]]
id = "tomorrow"
op = "sdm.maxnet_predict"
inputs = { model = "fit.model", env = "future.stack" }
note = "The suitability in the warmer climate."

[[nodes]]
id = "shift"
op = "raster.calc"
inputs = { a = "today.prediction", b = "tomorrow.prediction" }
params = { expression = "b - a" }
note = "What changes: gains (positive) and losses (negative)."

[[nodes]]
id = "mess"
op = "sdm.mess"
inputs = { reference = "data.env", env = "future.stack" }
note = "Elith's MESS: how like the conditions of today the future's are; negative where the model extrapolates, and is not to be trusted."

[[nodes]]
id = "plot_today"
op = "view.plot"
inputs = { value = "today.prediction" }
params = { title = "Suitability today" }

[[nodes]]
id = "plot_tomorrow"
op = "view.plot"
inputs = { value = "tomorrow.prediction" }
params = { title = "Suitability 2 °C warmer" }

[[nodes]]
id = "plot_shift"
op = "view.plot"
inputs = { value = "shift.map" }
params = { title = "Gains and losses" }

[[nodes]]
id = "plot_mess"
op = "view.plot"
inputs = { value = "mess.result" }
params = { title = "MESS (negative: extrapolation)" }
"#,
    },
    Template {
        name: "Drivers of land change",
        group: LAND_CHANGE,
        build: None,
        about: "two dates of land use and their drivers: the Markov chain, Dinamica EGO's weights of \
                evidence and lulcc's logistic regressions, each with its map of probability",
        toml: r#"
format = "terraflux/1"
name = "Drivers of land change"
description = "What explains a change: the Markov chain of two dates, the weights of evidence of the change on its drivers (Dinamica EGO) and the logistic regressions of every class (lulcc), each as maps of probability."

[[nodes]]
id = "land_2000"
op = "nlm.random_cluster"
params = { rows = 160, columns = 160, p = 0.5, weights = [0.55, 0.35, 0.10], seed = 11 }
note = "Land use drawn by chance: 1 forest, 2 farming, 3 water. In its place, a raster.read of a map of yours."
data = { what = "a land use map, the first date", kind = "classes", classes = { 1 = "forest", 2 = "farming", 3 = "water" } }

[[nodes]]
id = "aptitude"
op = "nlm.gaussian_field"
params = { rows = 160, columns = 160, range = 25, seed = 12 }
note = "A driver: how suited each cell is to farming (0 to 1)."

[[nodes]]
id = "roughness"
op = "nlm.fbm"
params = { rows = 160, columns = 160, dimension = 1.4, seed = 13 }
note = "A driver of no effect on the change, as real models have some."

[[nodes]]
id = "land_2010"
op = "raster.calc"
inputs = { t0 = "land_2000.map", suit = "aptitude.map" }
params = { expression = "2 if t0 == 1 and nbsum(t0 == 2, 1) >= 3 and suit > 0.6 else t0" }
note = "Ten years later: farming grown into the forest beside it, where it is suited."
data = { what = "a land use map, the second date", kind = "classes", classes = { 1 = "forest", 2 = "farming", 3 = "water" } }

[[nodes]]
id = "to_farming"
op = "dynamic.distance_to"
inputs = { landscape = "land_2000.map" }
params = { classes = 2, unit = "cells" }
note = "A driver of the map itself: the distance to farming."

[[nodes]]
id = "drivers"
op = "stack.of"
inputs = { aptitude = "aptitude.map", roughness = "roughness.map", to_farming = "to_farming.distance" }
note = "The drivers, a layer each."
data = { what = "the drivers of the change, a layer each (here a suitability, a roughness and the distance to farming)", kind = "stack" }

[[nodes]]
id = "markov"
op = "markov.fit"
inputs = { t0 = "land_2000.map", t1 = "land_2010.map", steps = 1 }
note = "The transition probabilities of the period: which share of each class went to each other."

[[nodes]]
id = "weights"
op = "woe.fit"
inputs = { t0 = "land_2000.map", t1 = "land_2010.map", drivers = "drivers.stack", markov = "markov.markov" }
params = { ranges = { aptitude = 0.05, roughness = 0.05, to_farming = 1 }, fix_abnormal = true }
note = "Dinamica EGO's weights of evidence of every transition (here forest to farming) on the ranges of every driver: table is Dinamica's report, a positive weight favouring the change, a negative one keeping it away."

[[nodes]]
id = "woe_probability"
op = "woe.probability"
inputs = { model = "weights.model", landscape = "land_2010.map", drivers = "drivers.stack" }
note = "The probability of the change in every cell of forest, from the weights."

[[nodes]]
id = "regressions"
op = "glm.fit"
inputs = { landscape = "land_2010.map", drivers = "drivers.stack" }
note = "A logistic regression of the presence of every class on the drivers (R's glm, as lulcc)."

[[nodes]]
id = "glm_probability"
op = "glm.predict"
inputs = { model = "regressions.model", drivers = "drivers.stack", landscape = "land_2010.map" }
note = "The probability of every class in every cell, a layer each."

[[nodes]]
id = "plot_woe"
op = "view.plot"
inputs = { value = "woe_probability.probability" }
params = { title = "Forest to farming (weights of evidence)" }

[[nodes]]
id = "plot_glm"
op = "view.plot"
inputs = { value = "glm_probability.suitability" }
params = { title = "Every class (logistic regression)" }
"#,
    },
    Template {
        name: "A city that grows (a loop)",
        group: LAND_CHANGE,
        build: None,
        about: "a cellular automaton written in map algebra: a city grown over 25 steps of a loop into its \
                best suited neighbours, never into water or the mountains",
        toml: r#"
format = "terraflux/1"
name = "A city that grows (a loop)"
description = "A cellular automaton written by hand: a loop of 25 steps where a city grows into the cells next to it, the best suited first, never into water or the mountains."

[[nodes]]
id = "relief"
op = "nlm.gaussian_field"
params = { rows = 150, columns = 150, range = 30, nugget = 0, seed = 41 }
note = "A relief from 0 to 1 (in its place, a raster.read of an elevation model)."
data = { what = "a relief or elevation model: the lowest land is water, the highest mountains", kind = "values", expect = "0 to 1 here" }

[[nodes]]
id = "attraction"
op = "nlm.gaussian_field"
params = { rows = 150, columns = 150, range = 20, seed = 42 }
note = "How attractive each place is (the jobs, the roads...): 0 to 1."
data = { what = "how attractive each place is to the city (jobs, roads...)", kind = "values", expect = "0 to 1" }

[[nodes]]
id = "centre"
op = "nlm.distance_gradient"
params = { rows = 150, columns = 150, origin = [72, 78, 72, 78], nlmr_compat = false, rescale = false }
note = "The distance to the old town in the middle."

[[nodes]]
id = "suitability"
op = "raster.calc"
inputs = { a = "attraction.map", d = "centre.map", r = "relief.map" }
params = { expression = "None if r < 0.3 or r > 0.75 else 0.6 * a + 0.4 * (1 - d / 106)" }
note = "Where a city would rather grow: attractive and near the centre; no value (never) in the water (the lowest land) and on the mountains (the highest)."

[[nodes]]
id = "town"
op = "raster.calc"
inputs = { d = "centre.map", suit = "suitability.map" }
params = { expression = "None if isnull(suit) else d == 0" }
note = "The city at the start: the old town (1), the rest 0."
data = { what = "the city at the start: 1 on the built-up cells", kind = "mask" }

[[nodes]]
id = "growth"
op = "control.repeat"
inputs = { times = 25, city = "town.map" }
note = "The loop: every step its body runs with @city, the city of the step before, and @step; collect keeps every step's map (growth.maps), carry passes the city on (growth.city is the last)."

[nodes.body]
carry = { city = "grow.map" }
collect = { maps = "grow.map" }

[[nodes.body.nodes]]
id = "grow"
op = "raster.calc"
inputs = { c = "@city", suit = "suitability.map", step = "@step" }
params = { expression = "1 if c == 0 and nbsum(c == 1, 1) >= 3 and suit > 0.6 - step * 0.008 else c" }
note = "A cell becomes city when three of its neighbours are, if it is suited enough; the bar falls a little every step, so the best land goes first."

[[nodes]]
id = "plot_growth"
op = "view.plot"
inputs = { value = "growth.maps" }
params = { title = "The city, step by step" }

[[nodes]]
id = "plot_city"
op = "view.plot"
inputs = { value = "growth.city" }
params = { title = "The city after 25 steps" }

[[nodes]]
id = "plot_suitability"
op = "view.plot"
inputs = { value = "suitability.map" }
params = { title = "Suitability" }
"#,
    },
    Template {
        name: "Farmers on their parcels (agents)",
        group: LAND_CHANGE,
        build: None,
        about: "a farmer per parcel in a loop of 20 years: rules for the wealth, from the fertility and a \
                falling price; the parcels left as farmers go broke",
        toml: r#"
format = "terraflux/1"
name = "Farmers on their parcels (agents)"
description = "Agents in a loop: a farmer per parcel, whose wealth follows the fertility of the parcel and a price that falls, who gives up farming when broke."

[[nodes]]
id = "mosaic"
op = "nlm.mosaic_tess"
params = { rows = 120, columns = 120, germs = 60, seed = 51 }
note = "A Voronoi mosaic: 60 cells of a random value each."

[[nodes]]
id = "parcels"
op = "raster.calc"
inputs = { m = "mosaic.map" }
params = { expression = "1 + floor(m * 100000)" }
note = "Each Voronoi cell a parcel with an id (in its place, a raster.read of a cadastre's parcel ids)."
data = { what = "a map of parcel ids (a cadastre's): an id per parcel", kind = "values", expect = "a whole id per parcel" }

[[nodes]]
id = "fertility"
op = "nlm.gaussian_field"
params = { rows = 120, columns = 120, range = 25, seed = 52 }
note = "The soil's fertility, 0 to 1."
data = { what = "the soil's fertility", kind = "values", expect = "0 to 1" }

[[nodes]]
id = "farms"
op = "agents.parcels"
inputs = { parcels = "parcels.map", fertility = "fertility.map" }
note = "An agent per parcel: its id, its cells and the mean fertility of its land."

[[nodes]]
id = "start"
op = "agents.update"
inputs = { agents = "farms.agents" }
params = { rules = "wealth = 20\nfarming = 1" }
note = "Every farmer starts with a wealth of 20, farming."

[[nodes]]
id = "years"
op = "control.repeat"
inputs = { times = 20, farmers = "start.agents" }
note = "20 years: @farmers are the farmers of the year before; summary collects the totals of every year, maps the parcels still farmed."

[nodes.body]
carry = { farmers = "year.agents" }
collect = { summary = "totals.table", maps = "farmed.map" }

[[nodes.body.nodes]]
id = "year"
op = "agents.update"
inputs = { agents = "@farmers", step = "@step" }
params = { rules = "price = 10 - step * 0.25\nincome = fertility * price * 2 if farming == 1 else 1\nwealth = wealth + income - 7\nfarming = farming if wealth > 0 else 0" }
note = "The rules of a year, in order: the price falls, the farmers earn by the fertility of their parcel, pay 7, and those broke stop farming (and earn 1 elsewhere)."

[[nodes.body.nodes]]
id = "totals"
op = "agents.summary"
inputs = { agents = "year.agents" }
note = "How many farmers and the sum and mean of every attribute."

[[nodes.body.nodes]]
id = "farmed"
op = "agents.map"
inputs = { agents = "year.agents", parcels = "parcels.map" }
params = { column = "farming" }
note = "Whether each parcel is still farmed, painted on it."

[[nodes]]
id = "wealth"
op = "agents.map"
inputs = { agents = "years.farmers", parcels = "parcels.map" }
params = { column = "wealth" }
note = "The farmers after 20 years (years.farmers): their wealth painted on their parcels."

[[nodes]]
id = "plot_wealth"
op = "view.plot"
inputs = { value = "wealth.map" }
params = { title = "Wealth after 20 years" }

[[nodes]]
id = "plot_fertility"
op = "view.plot"
inputs = { value = "fertility.map" }
params = { title = "Fertility" }

[[nodes]]
id = "plot_farming"
op = "view.plot"
inputs = { value = "years.maps" }
params = { title = "Parcels still farmed, year by year" }
"#,
    },
    Template {
        name: "CRAFTY: farmers and foresters",
        group: LAND_CHANGE,
        build: None,
        about: "CRAFTY's competition for land: farmers and foresters taking the cells where their food or \
                timber is wanted, as the demand for food grows over 20 steps",
        toml: r#"
format = "terraflux/1"
name = "CRAFTY: farmers and foresters"
description = "CRAFTY's competition for land: farmers and foresters produce food and timber from the capitals of the cells, and over 20 steps take the land where their services are wanted most, as the demand for food grows."

[[nodes]]
id = "fertility"
op = "nlm.gaussian_field"
params = { rows = 100, columns = 100, range = 15, seed = 101 }
note = "A capital, 0 to 1: the soil's fertility (in its place, raster.read of a capital's map)."
data = { what = "a capital: the soil's fertility", kind = "values", expect = "0 to 1" }

[[nodes]]
id = "wood"
op = "nlm.gaussian_field"
params = { rows = 100, columns = 100, range = 25, seed = 102 }
note = "Another: the forest's growing stock."
data = { what = "a capital: the forest's growing stock", kind = "values", expect = "0 to 1" }

[[nodes]]
id = "capitals"
op = "stack.of"
inputs = { fertility = "fertility.map", wood = "wood.map" }
note = "The capitals, a layer each by name."

[[nodes]]
id = "start"
op = "nlm.random_cluster"
params = { rows = 100, columns = 100, p = 0.5, weights = [0.5, 0.5], seed = 103 }
note = "Who manages each cell at the start: 1 farmers, 2 foresters, drawn by chance."
data = { what = "who manages each cell at the start", kind = "classes", classes = { 1 = "farmers", 2 = "foresters" } }

[[nodes]]
id = "demand"
op = "table.of"
params = { columns = ["food", "timber"], rows = [[1500, 1500], [1600, 1500], [1700, 1500], [1800, 1500], [1900, 1500], [2000, 1500], [2100, 1500], [2200, 1500], [2300, 1500], [2400, 1500], [2500, 1500], [2600, 1500], [2700, 1500], [2800, 1500], [2900, 1500], [3000, 1500], [3100, 1500], [3200, 1500], [3300, 1500], [3400, 1500]] }
note = "The demand for food and timber at every step (a row each): food grows, timber stays."

[[nodes]]
id = "simulation"
op = "control.repeat"
inputs = { times = 20, land = "start.map" }
note = "20 steps: @land, who manages each cell, passed on; cover (the land cover of every step) and info (the supply and demand) collected."

[nodes.body]
carry = { land = "compete.land" }
collect = { cover = "compete.cover", info = "compete.info" }

[[nodes.body.nodes]]
id = "compete"
op = "crafty.allocate"
inputs = { land = "@land", capitals = "capitals.stack", demand = "demand.table", step = "@step", seed = 7 }
params = { search = 0.2, giving_up_rate = 0.2, afts = [
  { name = "farmer", class = 1, production = { food = 1.0 }, sensitivity = { fertility = 2.0 }, giving_up = 0.05, giving_in = 0.1 },
  { name = "forester", class = 2, production = { timber = 1.0 }, sensitivity = { wood = 1.0 }, giving_up = 0.05, giving_in = 0.1 },
] }
note = "A step of CRAFTY: what each AFT would produce from the capitals (production x capital^sensitivity), the value of the demand left unmet, managers giving up below their threshold, and a share of the cells (search) offered to the most competitive."

[[nodes]]
id = "plot_cover"
op = "view.plot"
inputs = { value = "simulation.cover" }
params = { title = "Farmers (1), foresters (2), unmanaged (0), step by step" }
"#,
    },
    Template {
        name: "Land change analysis",
        group: LAND_CHANGE,
        build: None,
        about: "three dates of land use compared: the transitions, the components of the change (Pontius) \
                and the intensity analysis (as OpenLand)",
        toml: r#"
format = "terraflux/1"
name = "Land change analysis"
description = "Three dates of land use compared: the transitions, the components of the change (Pontius) and the intensity analysis (Aldwaik & Pontius, as OpenLand)."

[[nodes]]
id = "land_2000"
op = "nlm.random_cluster"
params = { rows = 160, columns = 160, resolution = 100, p = 0.5, weights = [0.55, 0.35, 0.10], seed = 11 }
note = "Land use drawn by chance on cells of 100 m: 1 forest, 2 farming, 3 water. In its place, a raster.read of a map of yours."
data = { what = "a land use map, the first date", kind = "classes", classes = { 1 = "forest", 2 = "farming", 3 = "water" } }

[[nodes]]
id = "aptitude"
op = "nlm.gaussian_field"
params = { rows = 160, columns = 160, resolution = 100, range = 25, seed = 12 }
note = "How suited each cell is to farming."

[[nodes]]
id = "land_2010"
op = "raster.calc"
inputs = { t0 = "land_2000.map", suit = "aptitude.map" }
params = { expression = "2 if t0 == 1 and nbsum(t0 == 2, 1) >= 3 and suit > 0.65 else t0" }
note = "Ten years later: farming grown into the forest beside it, where it is suited."
data = { what = "a land use map, the second date", kind = "classes", classes = { 1 = "forest", 2 = "farming", 3 = "water" } }

[[nodes]]
id = "land_2020"
op = "raster.calc"
inputs = { t1 = "land_2010.map", suit = "aptitude.map" }
params = { expression = "2 if t1 == 1 and nbsum(t1 == 2, 1) >= 3 and suit > 0.5 else 1 if t1 == 2 and suit < 0.2 else t1" }
note = "Ten more: farming grows faster, and the least suited farms go back to forest."
data = { what = "a land use map, the third date", kind = "classes", classes = { 1 = "forest", 2 = "farming", 3 = "water" } }

[[nodes]]
id = "series"
op = "stack.of"
inputs = { y2000 = "land_2000.map", y2010 = "land_2010.map", y2020 = "land_2020.map" }
note = "The maps, a layer per date."

[[nodes]]
id = "crosstab"
op = "change.crosstab"
inputs = { from = "land_2000.map", to = "land_2020.map" }
note = "The transition matrix of 2000 to 2020: the cells of each class (rows) in each class twenty years later (columns)."

[[nodes]]
id = "transitions"
op = "change.transitions"
inputs = { maps = "series.stack" }
params = { times = [2000, 2010, 2020] }
note = "Every transition of every period in km² (areas: each class at each date), as OpenLand's contingency table."

[[nodes]]
id = "components"
op = "change.components"
inputs = { from = "land_2000.map", to = "land_2020.map" }
note = "Pontius's components: of each class its gain, loss and persistence; overall the change made of quantity, exchange and shift."

[[nodes]]
id = "intensity"
op = "change.intensity"
inputs = { maps = "series.stack" }
params = { times = [2000, 2010, 2020], gain = 2, loss = 1 }
note = "The intensity analysis: whether each period changed faster than the uniform rate (interval), which classes gained and lost actively (category), and which classes farming took (transition_gain) and forest went to (transition_loss)."

[[nodes]]
id = "plot_series"
op = "view.plot"
inputs = { value = "series.stack" }
params = { title = "Land use in 2000, 2010 and 2020" }
"#,
    },
    Template {
        name: "Validating a simulation",
        group: LAND_CHANGE,
        build: None,
        about: "a simulated map against the observed one: agreement and figure of merit, Dinamica EGO's \
                fuzzy similarity, the three-map comparison at coarser resolutions and MOLUSCE's kappas",
        toml: r#"
format = "terraflux/1"
name = "Validating a simulation"
description = "A simulated map compared with the observed one, both from the same start: agreement and figure of merit, Dinamica EGO's fuzzy similarity, the three-map comparison at coarser resolutions (Pontius) and MOLUSCE's kappas."

[[nodes]]
id = "start"
op = "nlm.random_cluster"
params = { rows = 160, columns = 160, p = 0.5, weights = [0.55, 0.35, 0.10], seed = 11 }
note = "The map both start from: 1 forest, 2 farming, 3 water (in its place, a raster.read of the map of the first date)."
data = { what = "the land use map both start from, the first date", kind = "classes", classes = { 1 = "forest", 2 = "farming", 3 = "water" } }

[[nodes]]
id = "aptitude"
op = "nlm.gaussian_field"
params = { rows = 160, columns = 160, range = 25, seed = 12 }

[[nodes]]
id = "observed"
op = "raster.calc"
inputs = { t0 = "start.map", suit = "aptitude.map" }
params = { expression = "2 if t0 == 1 and nbsum(t0 == 2, 1) >= 3 and suit > 0.6 else t0" }
note = "What happened: farming grew into the forest beside it, where it was suited (in its place, the map of the second date)."
data = { what = "the land use map observed at the second date", kind = "classes", classes = { 1 = "forest", 2 = "farming", 3 = "water" } }

[[nodes]]
id = "simulated"
op = "raster.calc"
inputs = { t0 = "start.map", suit = "aptitude.map" }
params = { expression = "2 if t0 == 1 and nbsum(t0 == 2, 1) >= 3 and nbmean(suit, 4) > 0.6 else t0" }
note = "What a model said: much the same, but from a blurred suitability (its mean over 9 x 9 cells) (in its place, a model's simulated map)."
data = { what = "a model's simulated land use map for the second date", kind = "classes", classes = { 1 = "forest", 2 = "farming", 3 = "water" } }

[[nodes]]
id = "agreement"
op = "validate.agreement"
inputs = { start = "start.map", observed = "observed.map", simulated = "simulated.map" }
note = "The share of cells right, and the figure of merit of the change: hits over hits, misses, false alarms and wrong hits."

[[nodes]]
id = "similarity"
op = "validate.similarity"
inputs = { start = "start.map", observed = "observed.map", simulated = "simulated.map" }
params = { windows = [1, 3, 5, 9, 15] }
note = "Dinamica EGO's fuzzy similarity of the changes in windows of growing size: a model right near the place scores better in the wider windows."

[[nodes]]
id = "three_map"
op = "validate.three_map"
inputs = { t0 = "start.map", t1 = "observed.map", simulated = "simulated.map" }
params = { factors = [1, 2, 4, 8, 16] }
note = "The three-map comparison (lulcc): misses, hits, wrong hits and false alarms (budget) and the figure of merit at blocks of 1 to 16 cells."

[[nodes]]
id = "kappa"
op = "validate.kappa"
inputs = { reference = "observed.map", simulated = "simulated.map" }
note = "MOLUSCE's kappa, kappa location and kappa histogram of the two maps."

[[nodes]]
id = "errors"
op = "raster.calc"
inputs = { t0 = "start.map", o = "observed.map", s = "simulated.map" }
params = { expression = "1 if o != t0 and s == o else 2 if o != t0 else 3 if s != t0 else 0" }
note = "Where the model was right: 1 a hit, 2 a miss (changed, not simulated), 3 a false alarm (simulated, not changed), 0 no change."

[[nodes]]
id = "plot_errors"
op = "view.plot"
inputs = { value = "errors.map" }
params = { title = "Hits (1), misses (2), false alarms (3)" }
"#,
    },
    Template {
        name: "Accuracy and area of a map",
        group: LAND_CHANGE,
        build: None,
        about: "the good practices of Olofsson et al. (2014): the size of a stratified sample, the points drawn, \
                their reference classes, then the accuracies and each class's area with confidence intervals",
        toml: r#"
format = "terraflux/1"
name = "Accuracy and area of a map"
description = "The accuracy and the area of a classified map from a stratified random sample (Olofsson et al. 2014): the sample's size and its share of each class, the points drawn, their reference classes, and from them the error matrix, the user's, producer's and overall accuracies and the area of each class, with 95% confidence intervals."

[[nodes]]
id = "truth"
op = "nlm.random_cluster"
params = { rows = 200, columns = 200, p = 0.5, weights = [0.6, 0.3, 0.1], seed = 21 }
note = "What is on the ground: 1 forest, 2 farming, 3 water. Known here only to label the points; in a real study, imagery or field visits tell each point's class."

[[nodes]]
id = "noise"
op = "nlm.random"
params = { rows = 200, columns = 200, seed = 22 }

[[nodes]]
id = "classified"
op = "raster.calc"
inputs = { t = "truth.map", e = "noise.map" }
params = { expression = "(1 if t == 3 else t + 1) if e > 0.9 else t" }
note = "The map to assess: the truth with a tenth of its cells in another class (in its place, a raster.read of your map)."
data = { what = "the classified map to assess", kind = "classes", classes = { 1 = "forest", 2 = "farming", 3 = "water" } }

[[nodes]]
id = "size"
op = "accuracy.sample_size"
inputs = { map = "classified.map" }
params = { user_accuracy = { "3" = 0.7 }, standard_error = 0.015, minimum = 50 }
note = "How many points for a standard error of 0.015 in the overall accuracy, water (rare, expected harder) with a user's accuracy of 0.7 and at least 50 points in every class."

[[nodes]]
id = "sample"
op = "accuracy.sample"
inputs = { map = "classified.map", sizes = "size.table" }
params = { seed = 1 }
note = "The points, drawn at random in each class of the map. In a real study: vector.write them, give each its class in the column reference (from imagery or the field), and vector.read them back."

[[nodes]]
id = "labelled"
op = "vector.extract"
inputs = { vector = "sample.points", map = "truth.map" }
note = "Here the reference class is read from the truth (the column value)."

[[nodes]]
id = "assess"
op = "accuracy.assess"
inputs = { map = "classified.map", points = "labelled.vector" }
params = { reference = "value" }
note = "The accuracies and each class's area estimated from the sample, with 95% confidence intervals: the area counted on a map with errors is biased, the estimate is not."

[[nodes]]
id = "plot"
op = "view.plot"
inputs = { value = "sample.cells" }
params = { title = "The points drawn, by class" }
"#,
    },
    Template {
        name: "REDD+ baseline (VT0007)",
        group: LAND_CHANGE,
        build: None,
        about: "Verra's VT0007 as UDef-ARP: the negligible risk threshold, the vulnerability classes from the \
                distance to the forest's edge, their deforestation density, and the expected deforestation \
                allocated to the forest",
        toml: r#"
format = "terraflux/1"
name = "REDD+ baseline (VT0007)"
description = "Verra's VT0007 (the unplanned deforestation allocation, as Clark Labs' UDef-ARP): the distance to the forest's edge below which 99.5% of a period's deforestation happened, the forest in vulnerability classes by that distance, the relative frequency of deforestation in each class, and the jurisdiction's expected deforestation allocated to its cells by it, never more than a cell."

[[nodes]]
id = "land"
op = "nlm.random_cluster"
params = { rows = 200, columns = 200, p = 0.5, weights = [0.7, 0.3], seed = 31, resolution = 30 }
note = "The jurisdiction at the start of the historical reference period: 1 forest, 2 other land, cells of 30 m (in its place, a raster.read of a forest map in an equal-area projection)."
data = { what = "a forest map at the start of the historical reference period, in an equal-area projection", kind = "classes", classes = { 1 = "forest", 2 = "other land" } }

[[nodes]]
id = "forest"
op = "raster.calc"
inputs = { l = "land.map" }
params = { expression = "l == 1" }

[[nodes]]
id = "edge"
op = "dynamic.distance_to"
inputs = { landscape = "land.map" }
params = { classes = 2 }
note = "Every forest cell's distance to the edge (the nearest other land), metres."

[[nodes]]
id = "noise"
op = "nlm.random"
params = { rows = 200, columns = 200, seed = 32, resolution = 30 }

[[nodes]]
id = "deforestation"
op = "raster.calc"
inputs = { f = "forest.map", d = "edge.distance", e = "noise.map" }
params = { expression = "1 if f == 1 and e < 0.3 * exp(0 - d / 90) else 0" }
note = "The deforestation of the period, near the edge (in its place, the period's deforestation map: 1 deforested)."
data = { what = "the period's deforestation: 1 where the forest was cleared", kind = "mask" }

[[nodes]]
id = "nrt"
op = "redd.nrt"
inputs = { distance = "edge.distance", deforestation = "deforestation.map" }
note = "The negligible risk threshold: the distance to the edge below which 99.5% of the deforestation happened."

[[nodes]]
id = "vulnerability"
op = "redd.vulnerability"
inputs = { distance = "edge.distance", nrt = "nrt.nrt" }
note = "The benchmark vulnerability: 29 classes of geometric widths from the edge to the threshold, 1 beyond it (in a real study, from the distance at the start of the period predicted)."

[[nodes]]
id = "fit"
op = "redd.fit"
inputs = { classes = "vulnerability.classes", deforestation = "deforestation.map" }
note = "The relative frequency of deforestation in every class (and subdivision, when given)."

[[nodes]]
id = "allocate"
op = "redd.allocate"
inputs = { classes = "vulnerability.classes", table = "fit.table", expected = 60 }
note = "The jurisdiction's expected deforestation (its activity data, 60 ha) allocated to the cells by the fitted density, never more than a cell's area."

[[nodes]]
id = "plot"
op = "view.plot"
inputs = { value = "allocate.density" }
params = { title = "Expected deforestation (ha per cell)" }
"#,
    },
    Template {
        name: "Forest Code: APP and Reserva Legal",
        group: CONSERVATION,
        build: None,
        about: "Brazil's Forest Code on a drawn landscape: the APPs of its rivers (traced from the relief), slopes and \
                heights, narrower in the farmland consolidated by 2008, and every property's Reserva Legal balance",
        toml: r#"
format = "terraflux/1"
name = "Forest Code: APP and Reserva Legal"
description = "Brazil's Forest Code (Lei 12.651/2012) on a drawn landscape of cells of 30 m: the rivers traced from the relief and their margins by width (art. 4), the slopes above 45° and the land above 1,800 m; in the farmland consolidated by 22 July 2008, art. 61-A's narrower margins by each property's fiscal modules; and every property's Reserva Legal: its 20% of native vegetation against what it has, the APPs counted (art. 15)."

[[nodes]]
id = "relief"
op = "nlm.fbm"
params = { rows = 150, columns = 150, dimension = 1.3, seed = 51, resolution = 30 }

[[nodes]]
id = "dem"
op = "raster.calc"
inputs = { f = "relief.map" }
params = { expression = "400 + 1600 * f" }
note = "The elevation, 400 to 2,000 m (in its place, a DEM: raster.read, or data.download's Copernicus DEM)."
data = { what = "an elevation model (DEM)", kind = "values", expect = "metres" }

[[nodes]]
id = "filled"
op = "hydro.fill"
inputs = { dem = "dem.map" }

[[nodes]]
id = "accumulation"
op = "hydro.flow_accumulation"
inputs = { dem = "filled.map" }

[[nodes]]
id = "rivers"
op = "hydro.streams"
inputs = { accumulation = "accumulation.map" }
params = { threshold = 150 }
note = "The rivers: the cells draining 150 cells or more (in their place, a map of the rivers; their width as river_width)."

[[nodes]]
id = "slope"
op = "raster.terrain"
inputs = { raster = "dem.map" }
params = { measure = "slope" }

[[nodes]]
id = "cover"
op = "nlm.random_cluster"
params = { rows = 150, columns = 150, p = 0.5, weights = [0.55, 0.45], seed = 52, resolution = 30 }
note = "The land cover: 1 native vegetation, 2 farmland (in its place, a land cover map such as MapBiomas')."
data = { what = "a land cover map", kind = "classes", classes = { 1 = "native vegetation", 2 = "farmland" } }

[[nodes]]
id = "vegetation"
op = "raster.calc"
inputs = { c = "cover.map" }
params = { expression = "c == 1" }

[[nodes]]
id = "farmland"
op = "raster.calc"
inputs = { c = "cover.map" }
params = { expression = "c == 2" }
note = "The farmland, taken as consolidated by 22 July 2008 (in its place, the land use of 2008)."
data = { what = "the farmland consolidated by 22 July 2008 (the land use of 2008): 1 on it", kind = "mask" }

[[nodes]]
id = "strips"
op = "nlm.planar_gradient"
params = { rows = 150, columns = 150, direction = 90, seed = 53, resolution = 30 }

[[nodes]]
id = "properties"
op = "raster.calc"
inputs = { g = "strips.map" }
params = { expression = "min(floor(g * 6) + 1, 6)" }
note = "Six rural properties of 337.5 ha (in their place, the CAR's properties: vector.rasterize of SICAR's polygons)."
data = { what = "the rural properties (the CAR's): an id per property", kind = "values", expect = "a whole id per property" }

[[nodes]]
id = "app"
op = "forest_code.app"
inputs = { rivers = "rivers.map", slope = "slope.raster", elevation = "dem.map", properties = "properties.map", consolidated = "farmland.map" }
params = { fiscal_module = 50 }
note = "The APPs: river margins of 30 m (the rivers taken as under 10 m wide), narrower in the consolidated farmland (properties of 6.75 fiscal modules of 50 ha: 20 m), slopes above 45° and land above 1,800 m."

[[nodes]]
id = "reserve"
op = "forest_code.reserve"
inputs = { properties = "properties.map", vegetation = "vegetation.map", app = "app.app" }
params = { share = 20, fiscal_module = 50 }
note = "Every property's Reserva Legal: 20% of its area in native vegetation (outside the Amazon), its vegetation in the APPs counted."

[[nodes]]
id = "plot_app"
op = "view.plot"
inputs = { value = "app.app" }
params = { title = "APP: 1 river, 5 slope, 7 altitude" }

[[nodes]]
id = "plot_balance"
op = "view.plot"
inputs = { value = "reserve.balance" }
params = { title = "Reserva Legal: surplus (+) or deficit (-), ha" }
"#,
    },
    Template {
        name: "Rivers: the land upstream and the gauges' catchments",
        group: CONSERVATION,
        build: None,
        about: "Rivers as a network on a drawn landscape: the share of forest and the mean slope upstream of every link, gauges moved onto the rivers and the catchment of each",
        toml: r#"
format = "terraflux/1"
name = "Rivers: the land upstream and the gauges' catchments"
description = "Rivers as a network, as hydrographr does on the Hydrography90m, on a drawn landscape of cells of 30 m: the rivers traced from the relief and cut into links; the land upstream of every link summarised, its mean slope and the share of each land cover (how much forest drains into each stretch); five gauges moved onto the nearest large river and the catchment of each, a polygon per gauge and the catchments nested between one gauge and the next."

[[nodes]]
id = "relief"
op = "nlm.fbm"
params = { rows = 150, columns = 150, dimension = 1.3, seed = 61, resolution = 30 }

[[nodes]]
id = "dem"
op = "raster.calc"
inputs = { f = "relief.map" }
params = { expression = "300 + 900 * f" }
note = "The elevation, 300 to 1,200 m (in its place, a DEM: raster.read, or data.download's Copernicus DEM)."
data = { what = "an elevation model (DEM)", kind = "values", expect = "metres" }

[[nodes]]
id = "filled"
op = "hydro.fill"
inputs = { dem = "dem.map" }

[[nodes]]
id = "accumulation"
op = "hydro.flow_accumulation"
inputs = { dem = "filled.map" }

[[nodes]]
id = "rivers"
op = "hydro.streams"
inputs = { accumulation = "accumulation.map" }
params = { threshold = 150 }
note = "The rivers: the cells draining 150 cells or more."

[[nodes]]
id = "links"
op = "hydro.stream_links"
inputs = { dem = "filled.map", streams = "rivers.map" }
note = "The rivers cut into links, between sources, junctions and outlets."

[[nodes]]
id = "cover"
op = "nlm.random_cluster"
params = { rows = 150, columns = 150, p = 0.5, weights = [0.5, 0.3, 0.2], seed = 62, resolution = 30 }
note = "The land cover: 1 forest, 2 pasture, 3 cropland (in its place, a land cover map such as MapBiomas')."
data = { what = "a land cover map", kind = "classes", classes = { 1 = "forest", 2 = "pasture", 3 = "cropland" } }

[[nodes]]
id = "slope"
op = "raster.terrain"
inputs = { raster = "dem.map" }
params = { measure = "slope" }

[[nodes]]
id = "upstream"
op = "hydro.upstream"
inputs = { dem = "filled.map", links = "links.map", values = "slope.raster", classes = "cover.map" }
params = { stats = ["mean", "max"], to_map = "share_1" }
note = "The land upstream of every link: its mean and largest slope, the share of each land cover; the map, the share of forest on each link."

[[nodes]]
id = "area"
op = "raster.calc"
inputs = { d = "dem.map" }
params = { expression = "d * 0 + 1" }

[[nodes]]
id = "outline"
op = "vector.polygonize"
inputs = { map = "area.map" }

[[nodes]]
id = "sites"
op = "vector.points"
inputs = { vector = "outline.vector" }
params = { n = 5, seed = 63 }
note = "Five gauges drawn at random (in their place, vector.read of the gauges, dams or records of a species)."

[[nodes]]
id = "gauges"
op = "hydro.snap"
inputs = { points = "sites.vector", streams = "rivers.map", accumulation = "accumulation.map" }
params = { distance = 3000, min_accumulation = 300 }
note = "Each gauge moved onto the nearest river draining 300 cells or more, within 3 km."

[[nodes]]
id = "catchments"
op = "hydro.upstream_catchment"
inputs = { dem = "filled.map", points = "gauges.vector" }
note = "The catchment of each gauge: a polygon each, and the map of the catchments between one gauge and the next."

[[nodes]]
id = "plot_forest"
op = "view.plot"
inputs = { value = "upstream.map" }
params = { title = "Share of forest upstream of each link" }

[[nodes]]
id = "plot_catchments"
op = "view.plot"
inputs = { value = "catchments.map", features = "catchments.vector" }
params = { title = "The gauges' catchments, nested" }
"#,
    },
    Template {
        name: "Rivers: dams and their connectivity (DCI)",
        group: CONNECTIVITY,
        build: None,
        about: "Dams on the rivers of a drawn landscape: the dendritic connectivity index they leave, which to remove \
                first, the stretches above and below each, the distances between them and the centrality of the links",
        toml: r#"
format = "terraflux/1"
name = "Rivers: dams and their connectivity (DCI)"
description = "Dams on the rivers of a drawn landscape of cells of 30 m, as R's riverconn and hydrographr: the river network split into segments at the dams and its dendritic connectivity index (Cote et al. 2009: the chance that a fish can swim between two points of the river), the gain of removing each dam (which first), the share of the network a fish can reach from every stretch, the 3 km above and below each dam, the distances between the dams along the river and the centrality of the links."

[[nodes]]
id = "relief"
op = "nlm.fbm"
params = { rows = 150, columns = 150, dimension = 1.3, seed = 71, resolution = 30 }

[[nodes]]
id = "dem"
op = "raster.calc"
inputs = { f = "relief.map" }
params = { expression = "200 + 800 * f" }
note = "The elevation, 200 to 1,000 m (in its place, a DEM: raster.read, or data.download's Copernicus DEM)."
data = { what = "an elevation model (DEM)", kind = "values", expect = "metres" }

[[nodes]]
id = "filled"
op = "hydro.fill"
inputs = { dem = "dem.map" }

[[nodes]]
id = "accumulation"
op = "hydro.flow_accumulation"
inputs = { dem = "filled.map" }

[[nodes]]
id = "rivers"
op = "hydro.streams"
inputs = { accumulation = "accumulation.map" }
params = { threshold = 150 }
note = "The rivers: the cells draining 150 cells or more."

[[nodes]]
id = "links"
op = "hydro.stream_links"
inputs = { dem = "filled.map", streams = "rivers.map" }

[[nodes]]
id = "area"
op = "raster.calc"
inputs = { d = "dem.map" }
params = { expression = "d * 0 + 1" }

[[nodes]]
id = "outline"
op = "vector.polygonize"
inputs = { map = "area.map" }

[[nodes]]
id = "sites"
op = "vector.points"
inputs = { vector = "outline.vector" }
params = { n = 6, seed = 72 }
note = "Six dams drawn at random (in their place, vector.read of the dams, weirs and culverts, with their passability as an attribute)."

[[nodes]]
id = "dams"
op = "hydro.snap"
inputs = { points = "sites.vector", streams = "rivers.map", accumulation = "accumulation.map" }
params = { distance = 3000, min_accumulation = 200 }
note = "Each dam moved onto the nearest river draining 200 cells or more, within 3 km."

[[nodes]]
id = "dci"
op = "hydro.dci"
inputs = { dem = "filled.map", streams = "rivers.map", barriers = "dams.vector" }
params = { passability = 0.1 }
note = "The network split at the dams, each letting 10% of the fish through: the DCI of each network, and the gain of removing each dam (rank 1 first)."

[[nodes]]
id = "stretches"
op = "hydro.along_network"
inputs = { dem = "filled.map", points = "dams.vector", streams = "rivers.map" }
params = { upstream = 3000, downstream = 3000 }
note = "The 3 km of river above each dam (its reservoir) and below it."

[[nodes]]
id = "distances"
op = "hydro.network_distance"
inputs = { dem = "filled.map", points = "dams.vector" }
note = "The distance along the river between every two dams."

[[nodes]]
id = "centrality"
op = "hydro.centrality"
inputs = { links = "links.table" }
note = "The place of each link in its network: its magnitude, the river above it, its betweenness."

[[nodes]]
id = "plot_reach"
op = "view.plot"
inputs = { value = "dci.reach", features = "dci.barriers" }
params = { title = "The share of its network a fish can reach (%)" }

[[nodes]]
id = "plot_stretches"
op = "view.plot"
inputs = { value = "stretches.map", features = "dams.vector" }
params = { title = "3 km above and below each dam" }
"#,
    },
    Template {
        name: "For each year: one analysis per map (control.each)",
        group: LAND_CHANGE,
        build: None,
        about: "A body of nodes run once for each map of a stack (the years of a land cover series): the forest of \
                each year, its share by a threshold read from the model's inputs, gathered as a stack and a table",
        toml: r#"
format = "terraflux/1"
name = "For each year: one analysis per map (control.each)"
description = "One analysis repeated for each map of a series, as control.each does: three land covers drawn for 2000, 2010 and 2020 (in their place, a folder of MapBiomas years read by raster.read), and for each of them the forest, its patches above a size and their statistics; what each year gives is gathered as a stack of the years' names and one table with the year first. The smallest patch kept is an input the body's setting reads."

[inputs.min_patch]
kind = "value"
value = 20
note = "The smallest patch of forest kept, in cells: a setting of the body reads it."

[[nodes]]
id = "cover_2000"
op = "nlm.random_cluster"
params = { rows = 80, columns = 80, p = 0.55, weights = [0.65, 0.35], seed = 81, resolution = 30 }
data = { what = "the land cover of 2000", kind = "classes", classes = { 1 = "forest", 2 = "farmland" } }

[[nodes]]
id = "cover_2010"
op = "nlm.random_cluster"
params = { rows = 80, columns = 80, p = 0.55, weights = [0.55, 0.45], seed = 82, resolution = 30 }
data = { what = "the land cover of 2010", kind = "classes", classes = { 1 = "forest", 2 = "farmland" } }

[[nodes]]
id = "cover_2020"
op = "nlm.random_cluster"
params = { rows = 80, columns = 80, p = 0.55, weights = [0.45, 0.55], seed = 83, resolution = 30 }
data = { what = "the land cover of 2020", kind = "classes", classes = { 1 = "forest", 2 = "farmland" } }

[[nodes]]
id = "years"
op = "stack.of"
inputs = { forest_2000 = "cover_2000.map", forest_2010 = "cover_2010.map", forest_2020 = "cover_2020.map" }
note = "The land cover of each year as one stack: 1 forest, 2 farmland (in its place, raster.read of a folder of years)."

[[nodes]]
id = "by_year"
op = "control.each"
inputs = { over = "years.stack" }
note = "The body below once for each year: @item is the year's map, @name its name."
[nodes.body]
collect = { forest = "kept.map", stats = "stats.table" }
[[nodes.body.nodes]]
id = "forest"
op = "raster.calc"
inputs = { cover = "@item" }
params = { expression = "cover == 1" }
[[nodes.body.nodes]]
id = "kept"
op = "raster.sieve"
inputs = { map = "forest.map" }
params = { threshold = "$min_patch" }
note = "The forest without its patches smaller than the input min_patch (the setting reads it)."
[[nodes.body.nodes]]
id = "stats"
op = "raster.stats"
inputs = { map = "kept.map" }

[[nodes]]
id = "plot_forest"
op = "view.plot"
inputs = { value = "by_year.forest" }
params = { title = "The forest of each year (patches of 20 cells or more)" }

[[nodes]]
id = "plot_stats"
op = "view.plot"
inputs = { table = "by_year.stats" }
params = { title = "Each year's statistics" }
"#,
    },
    Template {
        name: "Choose the method: a switch and its scenarios",
        group: LAND_CHANGE,
        build: None,
        about: "Two ways to the same result in one model, the one run chosen by a parameter (control.switch): the \
                suitability for farming as distance or as neighbourhood, a scenario for each, the name of the result \
                made from the method (value.calc)",
        toml: r#"
format = "terraflux/1"
name = "Choose the method: a switch and its scenarios"
description = "Two methods for one result in one model, as control.switch does: the suitability of the land for new farming either by the distance to the farmland there is or by the share of farmland around each cell, only the branch chosen run. The method is a parameter of the model, and each choice a scenario to compare side by side; value.calc makes the title of the result from it, and its weight is a setting that reads an input."

[inputs.method]
kind = "value"
value = "distance"
note = "How the suitability is found: distance or neighbourhood (the switch's case)."

[inputs.weight]
kind = "value"
value = 0.8
note = "How much the suitability counts against the slope (0 to 1): a setting reads it."

[[parameters]]
key = "$method"
label = "Method"
choices = ["distance", "neighbourhood"]
help = "Which branch of the switch runs."

[[parameters]]
key = "$weight"
label = "Weight of the suitability"
min = 0
max = 1
step = 0.1

[[scenarios]]
name = "by distance"
values = { "$method" = "distance" }

[[scenarios]]
name = "by neighbourhood"
values = { "$method" = "neighbourhood" }

[[nodes]]
id = "land"
op = "nlm.random_cluster"
params = { rows = 100, columns = 100, p = 0.5, weights = [0.6, 0.4], seed = 91, resolution = 30 }
note = "The land: 1 forest, 2 farmland (in its place, a land cover map)."
data = { what = "a land cover map", kind = "classes", classes = { 1 = "forest", 2 = "farmland" } }

[[nodes]]
id = "relief"
op = "nlm.fbm"
params = { rows = 100, columns = 100, dimension = 1.4, seed = 92, resolution = 30 }
data = { what = "the relief, the slope's stand-in", kind = "values", expect = "0 to 1" }

[[nodes]]
id = "farmland"
op = "raster.calc"
inputs = { l = "land.map" }
params = { expression = "l == 2" }

[[nodes]]
id = "suitability"
op = "control.switch"
inputs = { case = "$method", farm = "farmland.map" }
note = "Only the branch the method names runs: both give a map `map`, the suitability from 0 to 1."
[[nodes.body.nodes]]
id = "distance"
op = "control.group"
inputs = { f = "@farm" }
note = "Near the farmland there is: 1 on it, falling with the distance."
[nodes.body.nodes.body]
outputs = { map = "near.map" }
[[nodes.body.nodes.body.nodes]]
id = "away"
op = "dynamic.distance_to"
inputs = { landscape = "@f" }
params = { classes = 1 }
[[nodes.body.nodes.body.nodes]]
id = "near"
op = "raster.calc"
inputs = { d = "away.distance" }
params = { expression = "1 / (1 + d / 300)" }
[[nodes.body.nodes]]
id = "neighbourhood"
op = "control.group"
inputs = { f = "@farm" }
note = "The share of farmland in the 7 x 7 cells around each cell."
[nodes.body.nodes.body]
outputs = { map = "share.map" }
[[nodes.body.nodes.body.nodes]]
id = "share"
op = "raster.calc"
inputs = { f = "@f" }
params = { expression = "nbmean(f, 3)" }

[[nodes]]
id = "combined"
op = "raster.calc"
inputs = { s = "suitability.map", r = "relief.map", w = "$weight" }
params = { expression = "w * s + (1 - w) * (1 - r)" }
note = "The suitability weighed against the slope (the relief as its stand-in)."

[[nodes]]
id = "title"
op = "value.calc"
inputs = { method = "suitability.branch", weight = "$weight" }
params = { text = "Suitability by {method} (weight {weight})" }

[[nodes]]
id = "plot"
op = "view.plot"
inputs = { value = "combined.map" }
params = { title = { from = "title.text" } }
"#,
    },
    Template {
        name: "Terrain of a surface",
        group: SURFACES,
        build: None,
        about: "a relief drawn by midpoint displacement and its slope, aspect, heat load, topographic \
                position, ruggedness, curvature and surface metrics (ISO 25178)",
        toml: r#"
format = "terraflux/1"
name = "Terrain of a surface"
description = "The measures of a relief: slope, aspect, heat load, topographic position, ruggedness, curvature and ISO 25178's surface metrics."

[[nodes]]
id = "relief"
op = "nlm.mpd"
params = { rows = 200, columns = 200, resolution = 30, roughness = 0.55, nlmr_compat = false, seed = 6 }
note = "A fractal surface by midpoint displacement, from 0 to 1, of cells of 30 m (in its place, a raster.read of an elevation model)."

[[nodes]]
id = "elevation"
op = "raster.calc"
inputs = { r = "relief.map" }
params = { expression = "200 + r * 800" }
note = "Metres: from 200 to 1000."
data = { what = "an elevation model (DEM)", kind = "values", expect = "metres" }

[[nodes]]
id = "slope"
op = "surface.terrain"
inputs = { map = "elevation.map" }
params = { v = "slope" }
note = "Degrees, by Horn's 8 neighbours (terra's terrain)."

[[nodes]]
id = "aspect"
op = "surface.terrain"
inputs = { map = "elevation.map" }
params = { v = "aspect" }

[[nodes]]
id = "heat_load"
op = "surface.hli"
inputs = { aspect = "aspect.map", slope = "slope.map" }
params = { latitude = -20, hemisphere = "southern", scaled = true }
note = "McCune & Keon's heat load: the slopes facing the sun warmer (a latitude of 20° south, here)."

[[nodes]]
id = "position"
op = "surface.tpi"
inputs = { map = "elevation.map" }
params = { scale = 9 }
note = "Each cell above (ridges) or below (valleys) the mean of the 9 x 9 cells around it."

[[nodes]]
id = "ruggedness"
op = "surface.vrm"
inputs = { map = "elevation.map" }
params = { s = 5 }
note = "The vector ruggedness measure: 0 flat or an even slope, up to 1 broken."

[[nodes]]
id = "curvature"
op = "surface.curvature"
inputs = { map = "elevation.map" }
params = { type = "profile" }

[[nodes]]
id = "surface"
op = "surface.metrics"
inputs = { map = "elevation.map" }
params = { metrics = ["sa", "sq", "ssk", "sku", "sdr"] }
note = "The surface's roughness (sa, sq), skewness, kurtosis and developed area ratio, as geodiv computes them."

[[nodes]]
id = "plot_elevation"
op = "view.plot"
inputs = { value = "elevation.map" }
params = { title = "Elevation" }

[[nodes]]
id = "plot_heat"
op = "view.plot"
inputs = { value = "heat_load.map" }
params = { title = "Heat load" }

[[nodes]]
id = "plot_position"
op = "view.plot"
inputs = { value = "position.map" }
params = { title = "Topographic position" }
"#,
    },
    Template {
        name: "Carbon and its uncertainty",
        group: SURFACES,
        build: None,
        about: "the carbon of a forest from its cover, its parameters drawn 500 times (Monte Carlo): the \
                mean, the deviation, the quantiles and an interval of every cell",
        toml: r#"
format = "terraflux/1"
name = "Carbon and its uncertainty"
description = "The carbon of a forest and how sure one can be of it: its parameters drawn many times (Monte Carlo), the mean, the deviation and an interval of 95 % of every cell."

[[nodes]]
id = "canopy"
op = "nlm.gaussian_field"
params = { rows = 100, columns = 100, range = 15, seed = 61 }
note = "The canopy's cover, 0 to 1 (in its place, a raster.read of a tree cover map)."
data = { what = "the tree canopy cover", kind = "values", expect = "0 to 1" }

[[nodes]]
id = "carbon"
op = "uncertainty.monte_carlo"
inputs = { canopy = "canopy.map" }
params = { expression = "a * canopy ** b * fraction", parameters = [["a", 250, 25], ["b", 1.5, 0.15], ["fraction", 0.47, 0.02]], n = 500, seed = 1 }
note = "The carbon, t/ha: the biomass from the cover by an allometry a cover^b (a 250 ± 25, b 1.5 ± 0.15), times its carbon fraction (0.47 ± 0.02); the three drawn 500 times from normal distributions. mean, sd and quantiles (2.5 % and 97.5 %) of every cell."

[[nodes]]
id = "interval"
op = "uncertainty.interval"
inputs = { value = "carbon.mean", sd = "carbon.sd" }
params = { level = 0.9 }
note = "A normal interval of 90 % from the mean and the deviation."

[[nodes]]
id = "relative"
op = "raster.calc"
inputs = { m = "carbon.mean", s = "carbon.sd" }
params = { expression = "100 * s / m if m > 0 else None" }
note = "The deviation as a share of the mean (%)."

[[nodes]]
id = "plot_mean"
op = "view.plot"
inputs = { value = "carbon.mean" }
params = { title = "Carbon (t/ha), the mean" }

[[nodes]]
id = "plot_sd"
op = "view.plot"
inputs = { value = "carbon.sd" }
params = { title = "Its standard deviation" }

[[nodes]]
id = "plot_quantiles"
op = "view.plot"
inputs = { value = "carbon.quantiles" }
params = { title = "2.5 % and 97.5 %" }
"#,
    },
    Template {
        name: "More neutral landscapes",
        group: SURFACES,
        build: None,
        about: "NLMR's other models side by side (midpoint displacement, a gradient with noise, curds, \
                growth by neighbourhood, a Voronoi mosaic, rectangles, percolation), a surface cut into \
                habitat or classes",
        toml: r#"
format = "terraflux/1"
name = "More neutral landscapes"
description = "NLMR's other models: midpoint displacement, a gradient with noise, curds, growth by neighbourhood, a Voronoi mosaic, rectangles, percolation, and a surface cut into habitat or classes."

[[nodes]]
id = "mpd"
op = "nlm.mpd"
params = { rows = 129, columns = 129, roughness = 0.6, seed = 5 }
note = "A fractal surface by midpoint displacement: roughness from 0 (smooth) to 1."

[[nodes]]
id = "gradient"
op = "nlm.planar_gradient"
params = { rows = 129, columns = 129, direction = 45, seed = 5 }
note = "A gradient across the map, from the north-east."

[[nodes]]
id = "noise"
op = "nlm.gaussian_field"
params = { rows = 129, columns = 129, range = 5, seed = 5 }

[[nodes]]
id = "gradient_noise"
op = "nlm.merge"
inputs = { primary = "gradient.map", secondary = "noise.map" }
params = { scaling = 0.5 }
note = "The gradient with noise on it (util_merge): a climate, say."

[[nodes]]
id = "curds"
op = "nlm.curds"
params = { curds = [0.8, 0.7, 0.6], steps = [8, 4, 4], seed = 5 }
note = "Hierarchical curdling: at each level, every block split and a share of it kept as habitat (1)."

[[nodes]]
id = "neigh"
op = "nlm.neigh"
params = { rows = 129, columns = 129, p_neigh = 0.8, p_empty = 0.2, categories = 3, rescale = false, seed = 5 }
note = "Categories grown from their neighbours."

[[nodes]]
id = "mosaic"
op = "nlm.mosaic_tess"
params = { rows = 129, columns = 129, germs = 40, seed = 5 }
note = "A Voronoi mosaic of 40 germs."

[[nodes]]
id = "rectangles"
op = "nlm.random_rectangular_cluster"
params = { rows = 129, columns = 129, minl = 5, maxl = 30, seed = 5 }
note = "Overlapping rectangles of random values."

[[nodes]]
id = "percolation"
op = "nlm.percolation"
params = { rows = 129, columns = 129, prob = 0.59, seed = 5 }
note = "Every cell 1 with a probability of 0.59, near the threshold of percolation."

[[nodes]]
id = "habitat"
op = "nlm.binarize"
inputs = { map = "mpd.map" }
params = { breaks = 0.3 }
note = "The surface cut into 30 % habitat (2), the highest cells, and the matrix (1)."

[[nodes]]
id = "classes"
op = "nlm.classify"
inputs = { map = "gradient_noise.map" }
params = { weights = [0.5, 0.3, 0.2] }
note = "Or into classes of 50, 30 and 20 % of the cells."

[[nodes]]
id = "plot_mpd"
op = "view.plot"
inputs = { value = "mpd.map" }
params = { title = "Midpoint displacement" }

[[nodes]]
id = "plot_gradient"
op = "view.plot"
inputs = { value = "gradient_noise.map" }
params = { title = "A gradient with noise" }

[[nodes]]
id = "plot_curds"
op = "view.plot"
inputs = { value = "curds.map" }
params = { title = "Curds" }

[[nodes]]
id = "plot_neigh"
op = "view.plot"
inputs = { value = "neigh.map" }
params = { title = "Grown by neighbourhood" }

[[nodes]]
id = "plot_mosaic"
op = "view.plot"
inputs = { value = "mosaic.map" }
params = { title = "Voronoi mosaic" }

[[nodes]]
id = "plot_rectangles"
op = "view.plot"
inputs = { value = "rectangles.map" }
params = { title = "Rectangles" }

[[nodes]]
id = "plot_percolation"
op = "view.plot"
inputs = { value = "percolation.map" }
params = { title = "Percolation" }

[[nodes]]
id = "plot_habitat"
op = "view.plot"
inputs = { value = "habitat.map" }
params = { title = "30 % habitat" }

[[nodes]]
id = "plot_classes"
op = "view.plot"
inputs = { value = "classes.classes" }
params = { title = "Three classes" }
"#,
    },
    Template {
        name: "Trend and hot spots",
        group: SURFACES,
        build: None,
        about: "The trend of every cell along five years of a drawn NDVI (Mann-Kendall and Sen's slope), how much \
                neighbouring cells change alike (Moran's I), where (LISA's clusters), and the hot and cold spots \
                of the last year (Getis-Ord's Gi*)",
        toml: r#"
format = "terraflux/1"
name = "Trend and hot spots"
description = "Five years of a drawn NDVI: the monotonic trend of every cell (the Mann-Kendall test and Sen's slope, as pymannkendall), the global Moran's I of the slopes, LISA's clusters of greening and browning, and the hot and cold spots of the last year by Getis and Ord's Gi* (as PySAL's esda)."

[[nodes]]
id = "base"
op = "nlm.gaussian_field"
params = { rows = 100, columns = 100, range = 8, seed = 70 }

[[nodes]]
id = "change"
op = "nlm.gaussian_field"
params = { rows = 100, columns = 100, range = 20, seed = 76 }
note = "Where the vegetation greens (high) or browns (low) over the years."

[[nodes]]
id = "noise1"
op = "nlm.random"
params = { rows = 100, columns = 100, seed = 71 }

[[nodes]]
id = "ndvi1"
op = "raster.calc"
inputs = { b = "base.map", c = "change.map", e = "noise1.map" }
params = { expression = "0.3 + 0.4 * b + 0.00 * (c - 0.5) + 0.08 * (e - 0.5)" }
note = "The NDVI of a year: a level that stays, a change that grows year by year, noise (in their place, a series of NDVI maps: raster.read)."
data = { what = "the NDVI of year 1 of the series", kind = "values", expect = "-1 to 1" }

[[nodes]]
id = "noise2"
op = "nlm.random"
params = { rows = 100, columns = 100, seed = 72 }

[[nodes]]
id = "ndvi2"
op = "raster.calc"
inputs = { b = "base.map", c = "change.map", e = "noise2.map" }
params = { expression = "0.3 + 0.4 * b + 0.06 * (c - 0.5) + 0.08 * (e - 0.5)" }
data = { what = "the NDVI of year 2 of the series", kind = "values", expect = "-1 to 1" }

[[nodes]]
id = "noise3"
op = "nlm.random"
params = { rows = 100, columns = 100, seed = 73 }

[[nodes]]
id = "ndvi3"
op = "raster.calc"
inputs = { b = "base.map", c = "change.map", e = "noise3.map" }
params = { expression = "0.3 + 0.4 * b + 0.12 * (c - 0.5) + 0.08 * (e - 0.5)" }
data = { what = "the NDVI of year 3 of the series", kind = "values", expect = "-1 to 1" }

[[nodes]]
id = "noise4"
op = "nlm.random"
params = { rows = 100, columns = 100, seed = 74 }

[[nodes]]
id = "ndvi4"
op = "raster.calc"
inputs = { b = "base.map", c = "change.map", e = "noise4.map" }
params = { expression = "0.3 + 0.4 * b + 0.18 * (c - 0.5) + 0.08 * (e - 0.5)" }
data = { what = "the NDVI of year 4 of the series", kind = "values", expect = "-1 to 1" }

[[nodes]]
id = "noise5"
op = "nlm.random"
params = { rows = 100, columns = 100, seed = 75 }

[[nodes]]
id = "ndvi5"
op = "raster.calc"
inputs = { b = "base.map", c = "change.map", e = "noise5.map" }
params = { expression = "0.3 + 0.4 * b + 0.24 * (c - 0.5) + 0.08 * (e - 0.5)" }
data = { what = "the NDVI of year 5 of the series", kind = "values", expect = "-1 to 1" }

[[nodes]]
id = "series"
op = "stack.of"
inputs = { y1 = "ndvi1.map", y2 = "ndvi2.map", y3 = "ndvi3.map", y4 = "ndvi4.map", y5 = "ndvi5.map" }
note = "The years, a layer each."

[[nodes]]
id = "trend"
op = "stats.trend"
inputs = { maps = "series.stack" }
params = { alpha = 0.1 }
note = "Every cell's Mann-Kendall test and Sen's slope; trend 1 greening, -1 browning at 10%."

[[nodes]]
id = "moran"
op = "stats.moran"
inputs = { map = "trend.slope" }
note = "How much neighbouring cells' slopes are alike: I near 0 random, near 1 clustered."

[[nodes]]
id = "lisa"
op = "stats.lisa"
inputs = { map = "trend.slope" }
note = "The clusters: 1 greening among greening, 3 browning among browning, 2 and 4 outliers."

[[nodes]]
id = "spots"
op = "stats.getis_ord"
inputs = { map = "ndvi5.map" }
note = "The hot and cold spots of the last year's NDVI, at 90, 95 and 99%."

[[nodes]]
id = "plot_slope"
op = "view.plot"
inputs = { value = "trend.slope" }
params = { title = "Sen's slope" }

[[nodes]]
id = "plot_clusters"
op = "view.plot"
inputs = { value = "lisa.clusters" }
params = { title = "LISA's clusters" }

[[nodes]]
id = "plot_spots"
op = "view.plot"
inputs = { value = "spots.spots" }
params = { title = "Hot and cold spots" }
"#,
    },
    Template {
        name: "Interpolation: IDW and kriging",
        group: SURFACES,
        build: None,
        about: "120 points sampled from a drawn soil property, their variogram and a spherical model fitted to it, \
                ordinary kriging with its variance and inverse distance weighting, and the errors of both",
        toml: r#"
format = "terraflux/1"
name = "Interpolation: IDW and kriging"
description = "Points of a soil property interpolated as R's gstat does: 120 points sampled from a drawn surface, their sample variogram and a spherical model fitted to it, ordinary kriging (the prediction and its variance) and inverse distance weighting, and how far each is from the surface."

[[nodes]]
id = "field"
op = "nlm.gaussian_field"
params = { rows = 100, columns = 100, range = 12, seed = 81 }

[[nodes]]
id = "soil"
op = "raster.calc"
inputs = { f = "field.map" }
params = { expression = "20 + 30 * f" }
note = "The true surface, a soil's carbon in g/kg say (in its place, the area to interpolate: any map of it)."
data = { what = "the surface to interpolate (a soil's carbon): the values the sample points read", kind = "values", expect = "g/kg here" }

[[nodes]]
id = "area"
op = "raster.calc"
inputs = { f = "soil.map" }
params = { expression = "f * 0 + 1" }

[[nodes]]
id = "sample"
op = "accuracy.sample"
inputs = { map = "area.map" }
params = { n = 120, seed = 82 }
note = "120 random points (in their place, the points measured: vector.read)."

[[nodes]]
id = "points"
op = "vector.extract"
inputs = { vector = "sample.points", map = "soil.map" }
note = "The value at every point, its attribute value."

[[nodes]]
id = "variogram"
op = "stats.variogram"
inputs = { points = "points.vector" }
params = { field = "value", model = "spherical" }
note = "The sample variogram and a spherical model fitted to it (its table: np, dist, gamma and the model)."

[[nodes]]
id = "kriging"
op = "stats.kriging"
inputs = { points = "points.vector", like = "soil.map", model = "variogram.model" }
params = { field = "value" }

[[nodes]]
id = "idw"
op = "stats.idw"
inputs = { points = "points.vector", like = "soil.map" }
params = { field = "value", power = 2 }

[[nodes]]
id = "kriging_error"
op = "raster.calc"
inputs = { k = "kriging.prediction", s = "soil.map" }
params = { expression = "k - s" }

[[nodes]]
id = "idw_error"
op = "raster.calc"
inputs = { i = "idw.map", s = "soil.map" }
params = { expression = "i - s" }

[[nodes]]
id = "plot_kriging"
op = "view.plot"
inputs = { value = "kriging.prediction" }
params = { title = "Ordinary kriging" }

[[nodes]]
id = "plot_variance"
op = "view.plot"
inputs = { value = "kriging.variance" }
params = { title = "Kriging variance" }

[[nodes]]
id = "plot_idw"
op = "view.plot"
inputs = { value = "idw.map" }
params = { title = "Inverse distance weighting" }
"#,
    },
    Template {
        name: "Species migration (MigClim)",
        group: SPECIES,
        build: None,
        about: "A species following its climate across three periods (MigClim's cellular automaton): dispersal \
                within a kernel's reach and by long jumps, a river it hardly crosses, the cells it reaches, those \
                it loses and those suitable it never reaches",
        toml: r#"
format = "terraflux/1"
name = "Species migration (MigClim)"
description = "A species' dispersal as MigClim simulates it: three periods of habitat suitability whose optimum moves across a drawn landscape, a kernel of dispersal of four cells, long-distance dispersal, propagule production that grows over the first years of a population, and a river as a barrier. The final map tells the cells colonised (by step), lost, and suitable but out of reach."

[[nodes]]
id = "gradient"
op = "nlm.planar_gradient"
params = { rows = 80, columns = 100, direction = 90, seed = 91 }
note = "A gradient across the map (a temperature, say)."

[[nodes]]
id = "noise"
op = "nlm.gaussian_field"
params = { rows = 80, columns = 100, range = 6, seed = 92 }

[[nodes]]
id = "period1"
op = "raster.calc"
inputs = { g = "gradient.map", n = "noise.map" }
params = { expression = "max(0, min(1000, round(1000 - abs(g - 0.25) * 3500 + (n - 0.5) * 500)))" }
note = "The suitability of a period, 0 to 1000: its optimum moving up the gradient (in their place, a species distribution model's predictions for each period, times 1000)."
data = { what = "the habitat suitability of the first period (a species distribution model's prediction)", kind = "values", expect = "0 to 1000" }

[[nodes]]
id = "period2"
op = "raster.calc"
inputs = { g = "gradient.map", n = "noise.map" }
params = { expression = "max(0, min(1000, round(1000 - abs(g - 0.45) * 3500 + (n - 0.5) * 500)))" }
data = { what = "the habitat suitability of the second period", kind = "values", expect = "0 to 1000" }

[[nodes]]
id = "period3"
op = "raster.calc"
inputs = { g = "gradient.map", n = "noise.map" }
params = { expression = "max(0, min(1000, round(1000 - abs(g - 0.65) * 3500 + (n - 0.5) * 500)))" }
data = { what = "the habitat suitability of the third period", kind = "values", expect = "0 to 1000" }

[[nodes]]
id = "suitability"
op = "stack.of"
inputs = { p1 = "period1.map", p2 = "period2.map", p3 = "period3.map" }

[[nodes]]
id = "initial"
op = "raster.calc"
inputs = { s = "period1.map", g = "gradient.map" }
params = { expression = "1 if s > 600 and g < 0.3 else 0" }
note = "Where the species is now: suitable cells at the lower end of the gradient."
data = { what = "where the species is now: 1 on the occupied cells", kind = "mask" }

[[nodes]]
id = "river"
op = "raster.calc"
inputs = { g = "gradient.map" }
params = { expression = "1 if abs(g - 0.4) < 0.006 else 0" }
note = "A river across the gradient, a barrier."
data = { what = "a barrier to dispersal: 1 on it", kind = "mask" }

[[nodes]]
id = "migrate"
op = "sdm.migrate"
inputs = { initial = "initial.map", suitability = "suitability.stack", barriers = "river.map" }
params = { steps = 5, kernel = [1.0, 0.4, 0.16, 0.06], ini_maturity = 1, full_maturity = 3, propagules = [0.4, 0.8], ldd_frequency = 0.05, ldd_min = 6, ldd_max = 15, barrier_type = "weak", seed = 93 }
note = "Five dispersal steps per period; table: the cells occupied, colonised and lost at every step."

[[nodes]]
id = "plot_final"
op = "view.plot"
inputs = { value = "migrate.occupied" }
params = { title = "Occupied at the end" }

[[nodes]]
id = "plot_states"
op = "view.plot"
inputs = { value = "migrate.map" }
params = { title = "MigClim's final state" }
"#,
    },
    Template {
        name: "Metapopulation (incidence function model)",
        group: SPECIES,
        build: None,
        about: "Hanski's incidence function model on 60 drawn patches: a century of colonisations and extinctions, \
                the patches occupied every year and each patch's incidence, how often it holds the species",
        toml: r#"
format = "terraflux/1"
name = "Metapopulation (incidence function model)"
description = "A metapopulation in 60 habitat patches as R's MetaLandSim simulates it (spom, Hanski's incidence function model): every year an empty patch is colonised with a probability that grows with its connectivity to the occupied patches, and an occupied one goes extinct with a probability that falls with its area. A century from 40% of the patches occupied: the patches occupied every year and each patch's incidence."

[[nodes]]
id = "area_field"
op = "nlm.gaussian_field"
params = { rows = 100, columns = 100, range = 10, seed = 101 }

[[nodes]]
id = "patch_area"
op = "raster.calc"
inputs = { f = "area_field.map" }
params = { expression = "0.2 + 3 * f" }
note = "The area of a patch found at a place, in hectares (in its place, a table of the patches: x, y, area, occupied)."
data = { what = "the area of the patch found at each place", kind = "values", expect = "hectares" }

[[nodes]]
id = "draw"
op = "nlm.random"
params = { rows = 100, columns = 100, seed = 102 }

[[nodes]]
id = "start"
op = "raster.calc"
inputs = { r = "draw.map" }
params = { expression = "1 if r < 0.4 else 0" }
data = { what = "whether the patch holds the species at the start: 1 occupied", kind = "mask" }

[[nodes]]
id = "attributes"
op = "stack.of"
inputs = { area = "patch_area.map", occupied = "start.map" }

[[nodes]]
id = "everywhere"
op = "raster.calc"
inputs = { f = "area_field.map" }
params = { expression = "f * 0 + 1" }

[[nodes]]
id = "places"
op = "accuracy.sample"
inputs = { map = "everywhere.map" }
params = { n = 60, seed = 103 }
note = "60 patches at random places."

[[nodes]]
id = "patches"
op = "vector.extract"
inputs = { vector = "places.points", map = "attributes.stack" }
note = "Each patch's area and whether it holds the species at the start."

[[nodes]]
id = "metapopulation"
op = "conn.metapopulation"
inputs = { patches = "patches.vector" }
params = { steps = 100, alpha = 0.08, x = 0.5, y = 4, e = 0.2, seed = 104 }
note = "alpha: 1 / the mean dispersal distance (12.5 cells); e and x: the extinction of a patch of 1 ha and how it falls with area; y: the connectivity at which colonisation is even. Its tables: the patches occupied every year, and each patch's incidence."
"#,
    },
    Template {
        name: "SWAT+ from rasters",
        group: SURFACES,
        about: "a SWAT+ project made from a terrain, a land use and a soil (QSWAT+'s subbasins, channels and HRUs, the \
                SWAT+ Editor's files) and run: the soil lost and the runoff of each HRU, the flow and discharge of each \
                channel on the map; the curve number and the water the soil holds are inputs a batch can vary",
        toml: "",
        build: Some(swat_from_rasters),
    },
    Template {
        name: "Classify an image by clicking samples",
        group: OBJECTS,
        toml: "",
        build: Some(obia_samples),
        about: "an image cut into objects, a few objects of each class picked by their samples, and a random forest \
                that gives every object a class: the polygons, and the map of the classes (the image is drawn, in \
                its place yours; the samples are points to be replaced by yours, clicked in the Viewer)",
    },
    Template {
        name: "Classify an image by rules",
        group: OBJECTS,
        toml: "",
        build: Some(obia_rules),
        about: "objects classed by a hierarchy of fuzzy rules, as eCognition's class hierarchy: water by its NDWI, \
                vegetation by its NDVI (forest and grassland by the short-wave infrared), the rest by its NDVI \
                (cropland and urban)",
    },
    Template {
        name: "Each class its own way",
        group: OBJECTS,
        toml: "",
        build: Some(obia_tree),
        about: "a class tree where each class is found its own way, as GeoAIDA and InterIMAGE do: water by its NDWI, \
                vegetation by its NDVI, built land by samples, and inside the vegetation forest and grassland by \
                samples; the classes of one mother are arbitrated cell by cell, and the map of the leaf classes with \
                the table of conflicts are the outputs (Add a class... adds one more)",
    },
    Template {
        name: "Segment an image into polygons",
        group: OBJECTS,
        build: None,
        about: "a multiresolution segmentation (Baatz and Schape) at the scale the image suggests, the objects' \
                attributes and the polygons written to a file, smoothed",
        toml: r#"
format = "terraflux/1"
name = "Segment an image into polygons"
description = "An image cut into objects of alike cells (multiresolution segmentation, as eCognition's) and the objects as polygons with their attributes: the mean and the standard deviation of each band, NDVI, area, compactness and texture. The scale is the one the image suggests (ESP, Dragut et al. 2010); give `scale` to the segmentation to choose it."

[[nodes]]
id = "land"
op = "nlm.mosaic_gibbs"
params = { rows = 120, columns = 120, germs = 30, r = 12, patch_classes = 5, nlmr_compat = false, rescale = false, seed = 4 }
note = "A map of five classes drawn by chance (1 water, 2 forest, 3 grassland, 4 cropland, 5 urban): only the ground truth of the image below."

[[nodes]]
id = "image"
op = "nlm.image"
inputs = { classes = "land.map" }
params = { seed = 1 }
note = "A synthetic image of six bands (blue, green, red, nir, swir1, swir2) of the classes, with noise and a blur at the edges. In its place, a raster.read of your image as a stack of its bands, each named for the band it is (blue, green, red, nir...): the attributes read the names."
data = { what = "a multispectral image, a layer per band (named blue, green, red, nir, swir1, swir2 where there are such bands)", kind = "stack" }

[[nodes]]
id = "segments"
op = "segment.multiresolution"
inputs = { image = "image.image" }
note = "Objects by multiresolution segmentation. Without `scale` it takes the first scale segment.scale suggests for the image (the `scale` output says which). Raise the scale for larger objects, lower it for smaller."
why = "shape 0.1 and compactness 0.5 are eCognition's usual start; the scale is in the image's units, so the one found for the image is the safest."

[[nodes]]
id = "features"
op = "objects.features"
inputs = { objects = "segments.objects", image = "image.image" }
note = "The attributes of each object: the mean and the standard deviation of each band, NDVI, the area, the compactness and the homogeneity of the texture."
why = "The basic set is enough for most classifications; ask for `all` to see what else there is."

[[nodes]]
id = "polygons"
op = "objects.to_vector"
inputs = { objects = "features.objects" }
params = { smooth = 1 }
note = "A polygon for each object with its attributes in the table; smooth cuts the corners once so they do not look like stairs (keep 0 when the polygons are counted or measured)."

[[nodes]]
id = "write"
op = "vector.write"
inputs = { vector = "polygons.vector" }
params = { path = "objects.gpkg" }
note = "The polygons, in a file beside the model."

[[nodes]]
id = "plot"
op = "view.plot"
inputs = { value = "segments.objects" }
params = { title = "The objects" }
"#,
    },
    Template {
        name: "Land cover change by objects",
        group: OBJECTS,
        toml: "",
        build: Some(obia_change),
        about: "two dates of an image, each cut in objects and classed by rules, and the classes of the second date \
                bound by a transition matrix written by hand (a forest does not become water in ten years): the \
                table of what changed to what",
    },
    Template {
        name: "Trees and buildings from a height model",
        group: OBJECTS,
        build: None,
        about: "objects of an image classed with a height model (nDSM): trees are tall and green, buildings tall, \
                not green and rectangular, the ground low",
        toml: r##"
format = "terraflux/1"
name = "Trees and buildings from a height model"
description = "Objects of an image classed with a height model, the way a lidar nDSM and an image tell trees from buildings: both are taller than 2 m, trees are green (NDVI) and buildings are not, and a building's outline is close to a rectangle (rectangular fit). The nDSM goes in `more` of objects.features, which names its column mean_more."

[[nodes]]
id = "ground"
op = "nlm.mosaic_gibbs"
params = { rows = 120, columns = 120, germs = 24, r = 12, patch_classes = 4, nlmr_compat = false, rescale = false, seed = 6 }
note = "The ground cover drawn by chance: 1 water, 2 forest, 3 grassland, 4 cropland."

[[nodes]]
id = "footprints"
op = "nlm.random_rectangular_cluster"
params = { rows = 120, columns = 120, minl = 5, maxl = 12, seed = 7 }
note = "Rectangles of random values: the tallest of them are the buildings' footprints."

[[nodes]]
id = "scene"
op = "raster.calc"
inputs = { g = "ground.map", r = "footprints.map" }
params = { expression = "5 if r > 0.93 and g != 1 else g" }
note = "The scene's classes: the buildings (5, urban) on the ground, never on the water."

[[nodes]]
id = "relief"
op = "nlm.gaussian_field"
params = { rows = 120, columns = 120, range = 6, seed = 8 }
note = "A smooth field that makes the heights vary."

[[nodes]]
id = "image"
op = "nlm.image"
inputs = { classes = "scene.map" }
params = { seed = 2 }
note = "A synthetic image of the scene, six bands. In its place, a raster.read of your image as a stack of named bands."
data = { what = "a multispectral image, a layer per band (named blue, green, red, nir, swir1, swir2 where there are such bands)", kind = "stack" }

[[nodes]]
id = "ndsm"
op = "raster.calc"
inputs = { s = "scene.map", f = "relief.map" }
params = { expression = "(12 + 6 * f) if s == 2 else ((9 + 4 * f) if s == 5 else 0.2 * f)" }
note = "The height above the ground, metres: trees 12 to 18, buildings 9 to 13, the rest near 0. In its place, a normalised digital surface model (DSM minus DTM) on the image's grid."
data = { what = "a height model, the height above the ground on the image's grid", kind = "values", expect = "metres above the ground" }

[[nodes]]
id = "segments"
op = "segment.multiresolution"
inputs = { image = "image.image" }
note = "Objects from the image alone, at the scale ESP suggests."

[[nodes]]
id = "features"
op = "objects.features"
inputs = { objects = "segments.objects", image = "image.image", more = "ndsm.map" }
params = { features = ["mean", "ndvi", "ndwi", "area", "compactness", "rectangular_fit"] }
note = "The attributes the rules read: the bands' means, NDVI, the shape, and the nDSM's mean as mean_more."

[[nodes]]
id = "rules"
op = "objects.rules"
inputs = { objects = "features.objects" }
note = "Trees: tall and green. Buildings: tall, not green and close to a rectangle. Ground: low. Raise the heights for a taller canopy; the rectangular fit is what tells a roof from a clump of bare ground that is tall."
why = "A height model alone does not tell trees from buildings, nor the image alone a roof from bare ground: together they do."
refs = ["Blaschke 2010, Object based image analysis for remote sensing"]

[nodes.params]
classes = [
  { name = "tall", rule = "above(mean_more, 2, 4)" },
  { name = "trees", rule = "above(ndvi, 0.45, 0.55)", parent = "tall", colour = "#2f7d32" },
  { name = "buildings", rule = "below(ndvi, 0.35, 0.45) and above(rectangular_fit, 0.3, 0.5)", parent = "tall", colour = "#b33a3a" },
  { name = "ground", rule = "below(mean_more, 1, 2)", colour = "#c8b88a" },
]

[[nodes]]
id = "plot"
op = "view.plot"
inputs = { value = "rules.map" }
params = { title = "Trees, buildings and ground" }
"##,
    },
    Template {
        name: "Classes by parcel",
        group: OBJECTS,
        build: None,
        about: "objects that never cross a parcel's boundary: a multiresolution segmentation held to a vector of \
                parcels (bounds), each object told its parcel (thematic) and classed by rules",
        toml: r##"
format = "terraflux/1"
name = "Classes by parcel"
description = "Objects held to the parcels: the segmentation takes the parcels as `bounds` (a vector, each feature a zone; the cells in none are one zone), so no object is made across a parcel's boundary, the way eCognition's thematic layer does in a segmentation. objects.features reads the same parcels as `thematic` and gives each object th_mode, the parcel it lies in (the feature's number from 1, or the attribute `field` names), and th_share, the share of its cells in it (1 when the object is wholly inside). The objects are then classed by rules on the spectra. Replace the parcels with a vector.read of yours (a cadastre, fields, management units) and the image with yours (Use my data)."

[[nodes]]
id = "land"
op = "nlm.mosaic_gibbs"
params = { rows = 120, columns = 120, germs = 30, r = 12, patch_classes = 5, nlmr_compat = false, rescale = false, seed = 4 }
note = "A map of five classes drawn by chance (1 water, 2 forest, 3 grassland, 4 cropland, 5 urban): only the ground truth of the image below."

[[nodes]]
id = "image"
op = "nlm.image"
inputs = { classes = "land.map" }
params = { seed = 1 }
note = "A synthetic image of six bands (blue, green, red, nir, swir1, swir2) of the classes. In its place, a raster.read of your image as a stack of its bands, each named for the band it is."
data = { what = "a multispectral image, a layer per band (named blue, green, red, nir, swir1, swir2 where there are such bands)", kind = "stack" }

[[nodes]]
id = "parcels"
op = "vector.grid_zones"
inputs = { map = "land.map" }
params = { size = 40 }
note = "Nine square parcels over the image, numbered from 1 (in their place, a vector.read of your parcels: a file of polygons, in the image's coordinates)."

[[nodes]]
id = "segments"
op = "segment.multiresolution"
inputs = { image = "image.image", bounds = "parcels.zones" }
note = "Objects by multiresolution segmentation, kept inside the parcels: an object that would grow over a boundary stops at it."
why = "A parcel is a unit of management: an object that spans two of them has the spectra of both and belongs to neither."

[[nodes]]
id = "features"
op = "objects.features"
inputs = { objects = "segments.objects", image = "image.image", thematic = "parcels.zones" }
params = { features = ["mean", "ndvi", "ndwi", "area"] }
note = "The bands' means and the indices the rules read, and th_mode, the parcel of each object, with th_share, 1 where the object is wholly inside it. A th_share below 1 would mean an object crossed a boundary."

[[nodes]]
id = "rules"
op = "objects.rules"
inputs = { objects = "features.objects" }
note = "The class of each object by its own spectra: water by its NDWI, vegetation by its NDVI (forest and grassland by the short-wave infrared), the rest by its NDVI (cropland and urban). Several objects of a parcel may differ: they are not forced to one class."
why = "The parcels hold the objects' outlines; the classes still come from what the objects are."
refs = ["Baatz and Schape 2000", "Definiens eCognition Developer, Reference Book"]

[nodes.params]
classes = [
  { name = "wet", rule = "above(ndwi, 0, 0.2)" },
  { name = "water", parent = "wet", colour = "#2b6cb0" },
  { name = "vegetation", rule = "above(ndvi, 0.45, 0.55)" },
  { name = "forest", rule = "below(mean_swir1, 0.18, 0.22)", parent = "vegetation", colour = "#2f7d32" },
  { name = "grassland", rule = "above(mean_swir1, 0.18, 0.22)", parent = "vegetation", colour = "#9bc53d" },
  { name = "other", rule = "below(ndwi, 0, 0.2) and below(ndvi, 0.45, 0.55)" },
  { name = "cropland", rule = "between(ndvi, 0.2, 0.28, 0.45, 0.5)", parent = "other", colour = "#e8c547" },
  { name = "urban", rule = "below(ndvi, 0.2, 0.28)", parent = "other", colour = "#8d8d8d" },
]

[[nodes]]
id = "plot"
op = "view.plot"
inputs = { value = "rules.map" }
params = { title = "The classes inside the parcels" }
"##,
    },
    Template {
        name: "Separate tree crowns",
        group: OBJECTS,
        build: None,
        about: "the crowns of a canopy height model: the canopy cut at a height, the clumps of touching crowns \
                split at the saddles of the surface (a watershed), over-split pieces joined back by compactness, \
                and the tallest crown of each neighbourhood marked",
        toml: r##"
format = "terraflux/1"
name = "Separate tree crowns"
description = "Single tree crowns from a canopy height model (nDSM): the canopy is what stands above a height (segment.threshold), where crowns touch it is one object, and objects.split cuts it by the surface (`by` surface: a watershed on the heights, a centre for each peak that stands `h` metres over its saddle). A small h over-splits, so objects.fuse joins back the neighbouring pieces whose union stays compact (a round crown) and objects.extrema marks the tallest crown among its neighbours, the tops of the stand. Replace the height model with yours (Use my data): a lidar nDSM, DSM minus DTM, in metres."

[[nodes]]
id = "relief"
op = "nlm.gaussian_field"
params = { rows = 120, columns = 120, range = 9, nugget = 0, seed = 12 }
note = "A smooth field of bumps: where it is high there are crowns."

[[nodes]]
id = "ndsm"
op = "raster.calc"
inputs = { f = "relief.map" }
params = { expression = "30 * (f - 0.3) / 0.7 if f > 0.3 else 0" }
note = "The height above the ground, metres: 0 on the open ground, up to 30 m on the crowns. In its place, a normalised digital surface model on your grid."
data = { what = "a canopy height model, the height above the ground on the grid", kind = "values", expect = "metres above the ground" }

[[nodes]]
id = "canopy"
op = "segment.threshold"
inputs = { image = "ndsm.map" }
params = { thresholds = [3] }
note = "The cells above 3 m are canopy and the rest ground: the objects are the connected pieces, so touching crowns are one."
why = "A height threshold keeps the shrubs and the grass out; 2 to 5 m is the usual range."

[[nodes]]
id = "classes"
op = "objects.rules"
inputs = { objects = "canopy.objects" }
note = "The high class is canopy, the low one ground."

[nodes.params]
classes = [
  { name = "canopy", rule = "above(class, 1, 2)", colour = "#2f7d32" },
  { name = "ground", rule = "below(class, 1, 2)", colour = "#c8b88a" },
]

[[nodes]]
id = "crowns"
op = "objects.split"
inputs = { objects = "classes.objects", surface = "ndsm.map" }
params = { by = "surface", classes = ["canopy"], h = 0.3, sigma = 1 }
note = "Each clump of canopy cut at the saddles of the height model: every peak that stands 0.3 m above its saddle makes a crown. `sigma` smooths the surface first so a rough crown has one peak."
why = "The height model, not the shape, tells where one crown ends: two crowns joined by a wide neck have a saddle between their tops."
refs = ["Vincent and Soille 1991, Watersheds in digital spaces"]

[[nodes]]
id = "joined"
op = "objects.fuse"
inputs = { objects = "crowns.objects" }
params = { seeds = ["canopy"], candidates = ["canopy"], fitting = "compactness", goal = "min", threshold = 1.5, improve = false }
note = "The pieces that the small h over-split are joined back while the union stays compact (at most 1.5; `improve` off, so the threshold alone decides) (compactness, the bounding rectangle's area over the object's: 1 for a rectangle, about 1.27 for a disc, more for a ragged outline)."
why = "Fusion by the result, as eCognition's: the shape of the whole decides, not the pieces."

[[nodes]]
id = "features"
op = "objects.features"
inputs = { objects = "joined.objects", image = "ndsm.map" }
params = { features = ["mean", "max", "area"] }
note = "The attributes of each crown: its mean and tallest height and its area."

[[nodes]]
id = "tops"
op = "objects.extrema"
inputs = { objects = "features.objects" }
params = { attribute = "max_image", kind = "max", among = ["canopy"], steps = 1, class = "top" }
note = "The crowns taller than every neighbouring crown, marked as `top`. Use `radius` (map units, between centroids) instead of `steps` to compare within a distance."
why = "The tallest crown of a neighbourhood is the tree that sets its height; steps 1 compares each crown with the ones it touches."

[[nodes]]
id = "plot"
op = "view.plot"
inputs = { value = "tops.objects" }
params = { title = "The crowns and their tops" }
"##,
    },
    Template {
        name: "Roads and rivers by the skeleton",
        group: OBJECTS,
        build: None,
        about: "long thin objects told from compact ones by their skeleton: the main line's length over the width,                 so a river is told from a lake and a road from a building though both are water or built land",
        toml: r##"
format = "terraflux/1"
name = "Roads and rivers by the skeleton"
description = "Water that is a river or a lake, built land that is a road or a building: the class is the same, the shape is not. The objects are the connected pieces of each class of a land cover map (segment.threshold); objects.features' skeleton group thins each to its skeleton (Zhang and Suen; the raster's, so the values differ a little from eCognition's polygon skeleton) and gives skel_main_length, the length of the main line, and skel_width, the area over that length. A river or a road has a high skel_length_width and a low skel_width, a lake or a building the opposite; objects.rules says which. Replace the land cover with yours (Use my data): water as 1, vegetation as 2 and built land as 3, or move the codes in the rules."

[[nodes]]
id = "relief"
op = "nlm.gaussian_field"
params = { rows = 120, columns = 120, range = 20, nugget = 0, seed = 3 }
note = "A smooth field that bends the river."

[[nodes]]
id = "slope"
op = "nlm.planar_gradient"
params = { rows = 120, columns = 120, direction = 90 }
note = "A ramp across the map: where the bent ramp is 0.5 there is the river."

[[nodes]]
id = "slope2"
op = "nlm.planar_gradient"
params = { rows = 120, columns = 120, direction = 0 }
note = "Another ramp, at a right angle: a straight road where it is 0.25."

[[nodes]]
id = "blobs"
op = "nlm.gaussian_field"
params = { rows = 120, columns = 120, range = 6, nugget = 0, seed = 9 }
note = "Smooth bumps: the lakes where high, the buildings where low."

[[nodes]]
id = "cover"
op = "raster.calc"
inputs = { s = "slope.map", t = "slope2.map", f = "relief.map", b = "blobs.map" }
params = { expression = "1 if abs(s + 0.3 * (f - 0.5) - 0.5) < 0.03 else (3 if abs(t - 0.25) < 0.025 else (1 if b > 0.7 else (3 if b < 0.2 else 2)))" }
note = "A land cover: a river and lakes (1, water), a road and buildings (3, built), the rest vegetation (2). Water is both thin and compact, and so is built land. In its place, a land cover map of yours."
data = { what = "a land cover map with water and built land (the other classes are whatever else)", kind = "classes", classes = { 1 = "water", 2 = "vegetation", 3 = "built" } }

[[nodes]]
id = "pieces"
op = "segment.threshold"
inputs = { image = "cover.map" }
params = { thresholds = [1.5, 2.5], neighbours = 8 }
note = "The connected pieces of each class are the objects: a river is one object, each lake and each building another."
why = "The class alone does not tell a river from a lake: both are water. The shape of the pieces does."

[[nodes]]
id = "features"
op = "objects.features"
inputs = { objects = "pieces.objects", image = "cover.map" }
params = { features = ["mean", "area", "skeleton"] }
note = "The class of each object (mean_image, the code of the cover), the area and the skeleton group: skel_main_length, skel_width, skel_length_width and the branches."

[[nodes]]
id = "rules"
op = "objects.rules"
inputs = { objects = "features.objects" }
note = "Water or built land by the class, then by the shape: long and thin (skel_length_width high, skel_width low) a river or a road, otherwise a lake or a building. Move the limits for wider rivers."
why = "The skeleton's length over its width does not depend on the size of the object, so one limit tells a long thin object from a compact one whatever its area."
refs = ["Zhang and Suen 1984, A fast parallel algorithm for thinning digital patterns"]

[nodes.params]
classes = [
  { name = "river", rule = "below(mean_image, 1.2, 1.8) and above(skel_length_width, 5, 8) and below(skel_width, 14, 18)", colour = "#2b6cb0" },
  { name = "lake", rule = "below(mean_image, 1.2, 1.8) and below(skel_length_width, 5, 8)", colour = "#7fb2e5" },
  { name = "road", rule = "above(mean_image, 2.2, 2.8) and above(skel_length_width, 5, 8) and below(skel_width, 14, 18)", colour = "#6b6b6b" },
  { name = "building", rule = "above(mean_image, 2.2, 2.8) and below(skel_length_width, 5, 8)", colour = "#b33a3a" },
  { name = "vegetation", rule = "between(mean_image, 1.2, 1.8, 2.2, 2.8)", colour = "#9bc53d" },
]

[[nodes]]
id = "plot"
op = "view.plot"
inputs = { value = "rules.map" }
params = { title = "Rivers, lakes, roads and buildings" }
"##,
    },
    Template {
        name: "Unsupervised classes of objects",
        group: OBJECTS,
        toml: "",
        build: Some(obia_clusters),
        about: "an image cut into objects and the objects grouped by k-means on their attributes, without samples: \
                the number of clusters chosen by the silhouette, each object given its cluster (the polygons, and \
                the map of the clusters)",
    },
    Template {
        name: "Water by Otsu on the NDWI",
        group: OBJECTS,
        build: None,
        about: "the water of an image found with no samples and no thresholds to choose: the NDWI of its green and                 near-infrared bands cut in two classes at the threshold multi-level Otsu finds, the water class kept                 as objects and given as polygons",
        toml: r##"
format = "terraflux/1"
name = "Water by Otsu on the NDWI"
description = "Water without samples: the NDWI, (green - nir) / (green + nir), is high over water and low over the rest, so its histogram has two modes, and Otsu's method puts the threshold in the gap between them (segment.threshold with two classes; the threshold it found is its `thresholds` output). Each connected piece of a class is an object; objects.rules keeps the high class as `water` and the other as `land`, and objects.to_vector gives the polygons. The bands must be named green and nir. Replace the image with yours (Use my data); for a scene with shadows or clouds, an NDWI that is bimodal needs them masked first."

[[nodes]]
id = "land"
op = "nlm.mosaic_gibbs"
params = { rows = 120, columns = 120, germs = 30, r = 12, patch_classes = 5, nlmr_compat = false, rescale = false, seed = 4 }
note = "A map of five classes drawn by chance (1 water, 2 forest, 3 grassland, 4 cropland, 5 urban): only the ground truth of the image below."

[[nodes]]
id = "image"
op = "nlm.image"
inputs = { classes = "land.map" }
params = { seed = 1 }
note = "A synthetic image of six bands (blue, green, red, nir, swir1, swir2) of the classes, with noise and a blur at the edges. In its place, a raster.read of your image as a stack of its bands, with at least green and nir named so."
data = { what = "a multispectral image with a green and a near-infrared band, a layer per band (named green and nir)", kind = "stack" }

[[nodes]]
id = "ndwi"
op = "raster.calc"
inputs = { img = "image.image" }
params = { expression = "(img.green - img.nir) / (img.green + img.nir)" }
note = "McFeeters' NDWI: above 0 where the green reflects more than the near-infrared, as water does."
refs = ["McFeeters 1996, The use of the Normalized Difference Water Index (NDWI)"]

[[nodes]]
id = "split"
op = "segment.threshold"
inputs = { image = "ndwi.map" }
params = { classes = 2 }
note = "Two classes by Otsu: the objects are the connected pieces of the low NDWI (class 1) and of the high (class 2). The threshold is in the `thresholds` output."
why = "Otsu needs no threshold from you: it takes the one that separates the two modes best, so it follows the image (a turbid lake, another sensor)."
refs = ["Otsu 1979"]

[[nodes]]
id = "water"
op = "objects.rules"
inputs = { objects = "split.objects" }
note = "The objects of the high class are water, the others land (the `class` column of the threshold's table is 1 or 2)."

[nodes.params]
classes = [
  { name = "water", rule = "above(class, 1, 2)", colour = "#2b6cb0" },
  { name = "land", rule = "below(class, 1, 2)", colour = "#c8b88a" },
]

[[nodes]]
id = "polygons"
op = "objects.to_vector"
inputs = { objects = "water.objects" }
params = { smooth = 1 }
note = "A polygon for each object with its class."

[[nodes]]
id = "plot"
op = "view.plot"
inputs = { value = "water.map" }
params = { title = "Water (1) and land (2)" }
"##,
    },
    Template {
        name: "The scale chosen by itself",
        group: OBJECTS,
        build: None,
        about: "the scale of a multiresolution segmentation chosen by the data: the segmentation run at a range of                 scales and each scored by the variance inside the objects and Moran's I between them (USPO); the                 scores open as a chart and `value` is the best scale (Apply in the Viewer writes it into a segmenter)",
        toml: r##"
format = "terraflux/1"
name = "The scale chosen by itself"
description = "The scale of the segmentation chosen by the data (unsupervised parameter optimisation, Espindola et al. 2006): segment.optimize runs multiresolution segmentation at each scale from 2 to 20, and scores each run by the variance inside its objects (low: the objects are homogeneous) and Moran's I between neighbouring objects (low: they differ); both are scaled over the runs and summed, and the scale of the lowest sum is the best. The `scores` output opens as the chart of the scores (select it in the Viewer): the objective as a curve with its best marked, and the variance and Moran's I beside it; `value` is the best scale and `objects` the objects of that run. Apply to... above the chart writes the best scale into a segmenter of the model (a new one, or the one you choose), as one step you can undo. It is a guide: look at the objects of the scales around it. Replace the image with yours (Use my data)."

[[nodes]]
id = "land"
op = "nlm.mosaic_gibbs"
params = { rows = 120, columns = 120, germs = 30, r = 12, patch_classes = 5, nlmr_compat = false, rescale = false, seed = 4 }
note = "A map of five classes drawn by chance: only the ground truth of the image below."

[[nodes]]
id = "image"
op = "nlm.image"
inputs = { classes = "land.map" }
params = { seed = 1 }
note = "A synthetic image of six bands of the classes, with noise and a blur at the edges. In its place, a raster.read of your image as a stack of its bands."
data = { what = "a multispectral image, a layer per band", kind = "stack" }

[[nodes]]
id = "best"
op = "segment.optimize"
inputs = { image = "image.image" }
params = { segmenter = "segment.multiresolution", parameter = "scale", from = 2, to = 20, step = 2, params = { shape = 0.1, compactness = 0.5 } }
note = "Multiresolution segmentation at the scales 2, 4... 20, each scored. The range is in the image's units: widen it, or take segment.scale's suggestion as the middle of it. `params` holds the settings that stay fixed."
why = "Too small a scale gives objects that are all alike (the variance is low, but Moran's I is high: the neighbours are alike too), too large mixes the classes (the variance rises): each score is scaled to 1 at its best, and the run with the highest sum of the two is kept."
refs = ["Espindola et al. 2006", "Johnson and Xie 2011"]

[[nodes]]
id = "plot"
op = "view.plot"
inputs = { table = "best.scores" }
params = { title = "The scores of each scale" }
"##,
    },
    Template {
        name: "Principal components of an image",
        group: SURFACES,
        build: None,
        about: "the principal components of a multispectral image (PCA, from the correlation of its bands): the \
                scores as a map of three layers shown as a colour composite, the loadings (what each component \
                reads) and the variance each explains",
        toml: r#"
format = "terraflux/1"
name = "Principal components of an image"
description = "The principal components of an image: the bands of a multispectral image are strongly correlated, and PCA turns them into a few uncorrelated layers (the components), the first holding the most of the variance (the brightness, usually), the next ones what differs between the bands. The bands are standardised first (each divided by its standard deviation), so a band of large values does not count more. The scores are a map of three layers (pc1, pc2, pc3) that the Viewer shows as an RGB composite (the Layers box, principal components); `loadings` says which bands each component is made of and `explained` how much of the variance each holds, with the elbow chart. Replace the image with yours (Use my data)."

[[nodes]]
id = "land"
op = "nlm.mosaic_gibbs"
params = { rows = 120, columns = 120, germs = 30, r = 12, patch_classes = 5, nlmr_compat = false, rescale = false, seed = 4 }
note = "A map of five classes drawn by chance: only the ground truth of the image below."

[[nodes]]
id = "image"
op = "nlm.image"
inputs = { classes = "land.map" }
params = { seed = 1 }
note = "A synthetic image of six bands (blue, green, red, nir, swir1, swir2) of the classes, with noise and a blur at the edges. In its place, a raster.read of your image as a stack of its bands."
data = { what = "a multispectral image, a layer per band (any number of bands: the components are as many)", kind = "stack" }

[[nodes]]
id = "pca"
op = "ml.pca"
inputs = { predictors = "image.image" }
params = { components = 3, standardise = true }
note = "The first three components of the bands, from their correlations (standardise = true). Their scores are the layers pc1, pc2 and pc3; the model keeps the means and scales of this fit, so ml.predict gives another image of the same bands its scores."
why = "Standardising is the right default when the bands are in different units; for bands of one unit (reflectance) both are common. Ask for a share of the variance (components = 0.95) instead of a number to keep as many components as it takes."
refs = ["Pearson 1901", "Hotelling 1933"]

[[nodes]]
id = "scores"
op = "view.plot"
inputs = { value = "pca.scores" }
params = { title = "Principal components (RGB: pc1 pc2 pc3)" }

[[nodes]]
id = "loadings"
op = "view.plot"
inputs = { table = "pca.loadings" }
params = { title = "What each component reads (loadings)" }

[[nodes]]
id = "elbow"
op = "view.plot"
inputs = { table = "pca.explained" }
params = { title = "The variance each component explains" }
"#,
    },
    Template {
        name: "Unsupervised classification of an image (k-means)",
        group: SURFACES,
        build: None,
        about: "the cells of a multispectral image grouped by k-means in as many clusters as the silhouette says (2 \
                to 10), without a single sample: the map of the clusters and the chart that chose their number",
        toml: r#"
format = "terraflux/1"
name = "Unsupervised classification of an image (k-means)"
description = "An image classified without samples: k-means (Lloyd 1982, from the k-means++ start of Arthur and Vassilvitskii 2007) groups the cells whose bands are alike. The number of clusters is not given: with k = auto the best of 2 to 10 by the silhouette (Rousseeuw 1987) is kept, and `choice` and its chart show the others. The clusters are numbered by the first principal component of their centres, so the same numbers come out of every run; they are groups of alike cells, not classes: look at what each is (the centres table, the spectrum of each) and name them. Replace the image with yours (Use my data)."

[[nodes]]
id = "land"
op = "nlm.mosaic_gibbs"
params = { rows = 120, columns = 120, germs = 30, r = 12, patch_classes = 5, nlmr_compat = false, rescale = false, seed = 4 }
note = "A map of five classes drawn by chance: only the ground truth of the image below."

[[nodes]]
id = "image"
op = "nlm.image"
inputs = { classes = "land.map" }
params = { seed = 1 }
note = "A synthetic image of six bands (blue, green, red, nir, swir1, swir2) of the classes, with noise and a blur at the edges. In its place, a raster.read of your image as a stack of its bands."
data = { what = "a multispectral image, a layer per band", kind = "stack" }

[[nodes]]
id = "kmeans"
op = "ml.kmeans"
inputs = { predictors = "image.image" }
params = { k = "auto", sample = 4000, seed = 1 }
note = "k-means fitted on 4000 cells drawn at random (a sample is enough for the centres and keeps a large image quick; leave out `sample` to fit on all), every cell then given its nearest centre; k is chosen from 2 to 10 by the best silhouette (the node's warning says which). Give k = 6 to choose it yourself, or a list (k = [4, 5, 6, 7]) to choose among them."
why = "The silhouette says how well separated the clusters are, not how many classes you want: a number that suits the question is often better than the silhouette's."
refs = ["Lloyd 1982", "Arthur and Vassilvitskii 2007", "Rousseeuw 1987"]

[[nodes]]
id = "clusters"
op = "view.plot"
inputs = { value = "kmeans.clusters" }
params = { title = "The clusters" }

[[nodes]]
id = "choice"
op = "view.plot"
inputs = { table = "kmeans.choice" }
params = { title = "The choice of k (inertia and silhouette)" }

[[nodes]]
id = "centres"
op = "view.plot"
inputs = { table = "kmeans.centres" }
params = { title = "The centres of the clusters" }
"#,
    },
    Template {
        name: "Types of landscape by a self-organizing map",
        group: SURFACES,
        build: None,
        about: "a stack of environmental variables (elevation, rain, temperature, wetness) taught to a \
                self-organizing map whose nodes are grouped in five landscape types: the map of the types, and the \
                map's own views (U-matrix, component planes, hits) linked with it",
        toml: r#"
format = "terraflux/1"
name = "Types of landscape by a self-organizing map"
description = "Types of landscape from some environmental variables, by a self-organizing map (Kohonen 1982): a grid of nodes, each with a typical value of every variable, taught so that cells alike fall on nodes next to each other. The nodes are then grouped in five groups by Ward's method (the two levels of Vesanto and Alhoniemi 2000), and each cell takes its node's group: the map `groups` is the landscape types. The codebook table opens as the map's own chart (U-matrix, a plane of each variable, the cells each node holds, the groups), and a click on a node lights its cells on the map. The variables are standardised first. Replace the four variables with yours (Use my data): any number of them, on one grid."

[[nodes]]
id = "relief"
op = "nlm.gaussian_field"
params = { rows = 120, columns = 120, range = 25, seed = 31 }
note = "A smooth field that is the relief and, through it, the climate."

[[nodes]]
id = "noise"
op = "nlm.gaussian_field"
params = { rows = 120, columns = 120, range = 10, seed = 32 }
note = "A second field, finer, that the rain and the temperature do not take from the relief."

[[nodes]]
id = "elevation"
op = "raster.calc"
inputs = { e = "relief.map" }
params = { expression = "100 + 1400 * e" }
note = "The elevation, metres. In its place, a raster.read of your own."
data = { what = "an elevation map", kind = "values", expect = "metres above the sea" }

[[nodes]]
id = "rain"
op = "raster.calc"
inputs = { e = "relief.map", n = "noise.map" }
params = { expression = "600 + 700 * e + 300 * n" }
note = "The annual rain, mm: more on the heights. In its place, your own."
data = { what = "the annual rain map", kind = "values", expect = "mm a year" }

[[nodes]]
id = "temperature"
op = "raster.calc"
inputs = { e = "relief.map", n = "noise.map" }
params = { expression = "26 - 12 * e + 2 * n" }
note = "The annual mean temperature, degrees: colder on the heights. In its place, your own."
data = { what = "the annual mean temperature map", kind = "values", expect = "degrees" }

[[nodes]]
id = "wetness"
op = "nlm.gaussian_field"
params = { rows = 120, columns = 120, range = 15, seed = 33 }
note = "How wet the soil is, from 0 to 1, unrelated to the others. In its place, your own (a topographic wetness index, say)."
data = { what = "a soil wetness map (or any fourth variable)", kind = "values" }

[[nodes]]
id = "variables"
op = "stack.of"
inputs = { elevation = "elevation.map", rain = "rain.map", temperature = "temperature.map", wetness = "wetness.map" }
note = "The variables as a stack, a layer each, named as the maps."

[[nodes]]
id = "som"
op = "ml.som"
inputs = { predictors = "variables.stack" }
params = { rows = 8, cols = 8, clusters = 5, seed = 1 }
note = "An 8 by 8 map of the cells, standardised, taught in batch; its 64 nodes grouped in 5 types by Ward's method. Leave out rows and cols for the size Vesanto's rule gives (5 times the square root of the cells, a node each for a few cells), and give clusters = 0 to see the nodes alone."
why = "Few nodes read like a classification, many like a map of the data's shape: the groups are what the first is, the U-matrix what the second shows."
refs = ["Kohonen 1982", "Kohonen 2001", "Vesanto and Alhoniemi 2000"]

[[nodes]]
id = "types"
op = "view.plot"
inputs = { value = "som.groups" }
params = { title = "The types of landscape" }

[[nodes]]
id = "codebook"
op = "view.plot"
inputs = { table = "som.codebook" }
params = { title = "The map's nodes (codebook)" }
"#,
    },
    Template {
        name: "Change between two dates by principal components",
        group: LAND_CHANGE,
        build: None,
        about: "the red and near infrared bands of two dates of an image analysed together by PCA: what stays the \
                same is in the first components, what changed in the last, shown as a map of change",
        toml: r#"
format = "terraflux/1"
name = "Change between two dates by principal components"
description = "Change detection by principal components (Byrne et al. 1980; Richards 1984): the same bands of two dates are taken as one set of variables and rotated by PCA. What the dates share (the brightness, the vegetation) is in the first components; what changed between them is in a minor one, which holds the little that the first do not. Here the red and near infrared bands of two dates are four variables: the first two components are what the dates share and the third, `minor` (its `layer`), is the change; the fourth is only noise. The map `change` is that component's absolute value (a cell that changed is far from zero, in either direction), and `loadings` says which bands of which date make it up (the change component has the two dates' loadings of opposite sign). Which component is the change depends on the image and the bands taken: look at the loadings and at the maps of every component. Replace the two images with yours (Use my data): the same bands, on one grid."

[[nodes]]
id = "land_2000"
op = "nlm.mosaic_gibbs"
params = { rows = 120, columns = 120, germs = 30, r = 12, patch_classes = 5, nlmr_compat = false, rescale = false, seed = 4 }
note = "The first date's classes drawn by chance (1 water, 2 forest, 3 grassland, 4 cropland, 5 urban): only the truth of the image below."

[[nodes]]
id = "patches"
op = "nlm.gaussian_field"
params = { rows = 120, columns = 120, range = 25, seed = 9 }
note = "Where the land changes: the highest of the field."

[[nodes]]
id = "land_2020"
op = "raster.calc"
inputs = { t = "land_2000.map", f = "patches.map" }
params = { expression = "5 if t == 2 and f > 0.5 else (4 if t == 3 and f > 0.5 else t)" }
note = "Twenty years later: forest cleared to urban land and grassland ploughed to cropland where the field is high."

[[nodes]]
id = "image_2000"
op = "nlm.image"
inputs = { classes = "land_2000.map" }
params = { seed = 1 }
note = "The first date's image, six bands. In its place, a raster.read of your image as a stack of named bands (blue, green, red, nir, swir1, swir2)."
data = { what = "the first date's multispectral image, a layer per band (named blue, green, red, nir...)", kind = "stack" }

[[nodes]]
id = "image_2020"
op = "nlm.image"
inputs = { classes = "land_2020.map" }
params = { seed = 2 }
note = "The second date's image, on the same grid and with the same bands."
data = { what = "the second date's multispectral image, on the first's grid, the same bands", kind = "stack" }

[[nodes]]
id = "red_2000"
op = "stack.extract_layer"
inputs = { stack = "image_2000.image" }
params = { layer = "red" }

[[nodes]]
id = "nir_2000"
op = "stack.extract_layer"
inputs = { stack = "image_2000.image" }
params = { layer = "nir" }

[[nodes]]
id = "red_2020"
op = "stack.extract_layer"
inputs = { stack = "image_2020.image" }
params = { layer = "red" }

[[nodes]]
id = "nir_2020"
op = "stack.extract_layer"
inputs = { stack = "image_2020.image" }
params = { layer = "nir" }

[[nodes]]
id = "dates"
op = "stack.of"
inputs = { red_2000 = "red_2000.map", nir_2000 = "nir_2000.map", red_2020 = "red_2020.map", nir_2020 = "nir_2020.map" }
note = "Two bands of each date as the variables of one analysis (add the others: the more bands, the more components). The same cells on both dates must be the same place: register the images first."

[[nodes]]
id = "pca"
op = "ml.pca"
inputs = { predictors = "dates.stack" }
params = { components = 4, standardise = true }
note = "The four components of the two dates' bands. The first two hold what the dates share, the third where they differ, the fourth the noise."
why = "No change would make the second date a copy of the first, and the bands of the two dates one set of correlated pairs: the first components take the pairs, the change is what is left."
refs = ["Byrne et al. 1980", "Richards 1984"]

[[nodes]]
id = "minor"
op = "stack.extract_layer"
inputs = { stack = "pca.scores" }
params = { layer = "pc3" }
note = "The minor component that carries the change: pc3 of these four variables (the two dates' red and nir). Which one it is depends on the images and the bands: look at the loadings, whose change component has the dates' signs opposed, and at the maps of the components."

[[nodes]]
id = "change"
op = "raster.calc"
inputs = { c = "minor.map" }
params = { expression = "abs(c)" }
note = "The size of the change: a cell far from the component's zero, in either direction."

[[nodes]]
id = "change_map"
op = "view.plot"
inputs = { value = "change.map" }
params = { title = "Change (the minor component, pc3)" }

[[nodes]]
id = "scores"
op = "view.plot"
inputs = { value = "pca.scores" }
params = { title = "The components of the two dates" }

[[nodes]]
id = "loadings"
op = "view.plot"
inputs = { table = "pca.loadings" }
params = { title = "What each component reads (loadings)" }
"#,
    },
];

/// The scene of the object templates: a map of five classes drawn by chance (1 water, 2 forest, 3 grassland,
/// 4 cropland, 5 urban) and the image of it, for each of the seeds.
const OBIA_SCENE: &str = r#"
[[nodes]]
id = "land"
op = "nlm.mosaic_gibbs"
params = { rows = 120, columns = 120, germs = 30, r = 12, patch_classes = 5, nlmr_compat = false, rescale = false, seed = 4 }
note = "A map of five classes drawn by chance (1 water, 2 forest, 3 grassland, 4 cropland, 5 urban): only the ground truth of the image below."

[[nodes]]
id = "image"
op = "nlm.image"
inputs = { classes = "land.map" }
params = { seed = 1 }
note = "A synthetic image of six bands (blue, green, red, nir, swir1, swir2) of the classes, with noise and a blur at the edges. In its place, a raster.read of your image as a stack of its bands, each named for the band it is (blue, green, red, nir...): the attributes read the names."
data = { what = "a multispectral image, a layer per band (named blue, green, red, nir, swir1, swir2 where there are such bands)", kind = "stack" }

[[nodes]]
id = "segments"
op = "segment.multiresolution"
inputs = { image = "image.image" }
note = "Objects by multiresolution segmentation. Without `scale` it takes the first scale segment.scale suggests for the image (the `scale` output says which); raise it for larger objects, lower it for smaller ones."
why = "shape 0.1 and compactness 0.5 are eCognition's usual start; the scale is in the image's units, so the one found for the image is the safest."
"#;

/// The points of the samples' table, `x`, `y` and the class of each (SAMPLE_ROWS).
const OBIA_SAMPLE_ROWS: &str = r#"[[9.5, 104.5, "water"], [85.5, 46.5, "water"], [5.5, 38.5, "water"], [9.5, 86.5, "water"], [68.5, 112.5, "water"], [84.5, 78.5, "water"], [10.5, 18.5, "water"], [83.5, 62.5, "water"], [10.5, 3.5, "forest"], [98.5, 30.5, "forest"], [29.5, 97.5, "forest"], [100.5, 19.5, "forest"], [89.5, 22.5, "forest"], [23.5, 110.5, "forest"], [39.5, 91.5, "forest"], [38.5, 103.5, "forest"], [65.5, 90.5, "grassland"], [92.5, 100.5, "grassland"], [87.5, 114.5, "grassland"], [29.5, 20.5, "grassland"], [106.5, 111.5, "grassland"], [94.5, 7.5, "grassland"], [115.5, 69.5, "cropland"], [96.5, 55.5, "cropland"], [114.5, 8.5, "cropland"], [114.5, 85.5, "cropland"], [104.5, 75.5, "cropland"], [74.5, 7.5, "cropland"], [76.5, 16.5, "cropland"], [49.5, 110.5, "cropland"], [71.5, 60.5, "urban"], [60.5, 70.5, "urban"], [26.5, 43.5, "urban"], [59.5, 44.5, "urban"], [55.5, 58.5, "urban"], [20.5, 35.5, "urban"], [64.5, 61.5, "urban"], [50.5, 31.5, "urban"]]"#;

/// "Classify an image by clicking samples".
fn obia_samples() -> Model {
    let mut text = String::from(
        r#"
format = "terraflux/1"
name = "Classify an image by clicking samples"
description = "An image classified by examples, the way a user would in eCognition or QGIS: the image is cut into objects (multiresolution segmentation at the scale ESP suggests), their attributes are measured, a few objects of each class are named by points (their cell picks the object), a random forest learns the classes from them and gives every object a class and the probability of each class. The objects as polygons and as a map. Replace the image with yours (Use my data) and the points with yours: in the Viewer, click objects with the class's name."
"#,
    );
    text.push_str(OBIA_SCENE);
    text.push_str(
        r#"
[[nodes]]
id = "features"
op = "objects.features"
inputs = { objects = "segments.objects", image = "image.image" }
note = "The attributes of each object (the basic set): the mean and the standard deviation of each band, NDVI, the area, the compactness and the homogeneity of the texture."
why = "These are the columns the forest learns from; the classification needs the same ones for every object."

[[nodes]]
id = "sample_table"
op = "table.of"
note = "The samples: a point (x and y in the image's coordinates) on an object of each class, with the class's name. Eight per class, over different objects. In its place, the points you clicked in the Viewer (or a vector.read of points with a `class` column)."
why = "A point picks the object under it: the more different the objects of a class you name, the better the forest knows the class."

[nodes.params]
columns = ["x", "y", "class"]
rows = SAMPLE_ROWS

[[nodes]]
id = "samples"
op = "vector.from_table"
inputs = { table = "sample_table.table" }
note = "The points as features, with the class as an attribute."

[[nodes]]
id = "training"
op = "objects.samples"
inputs = { objects = "features.objects", samples = "samples.vector" }
note = "The objects under the points, with their attributes: the table the forest learns from. A warning names an object that holds two classes and a point outside the image."
why = "Classes are numbered by their names in alphabetical order, so the model's codes follow that order."

[[nodes]]
id = "model"
op = "ml.fit"
inputs = { table = "training.table" }
params = { column = "class", method = "random_forest", seed = 1 }
note = "A random forest on the samples' table (the target is the column class, every other column a predictor)."
why = "The forest works without tuning and says which attributes mattered (the importance output)."

[[nodes]]
id = "classify"
op = "objects.classify"
inputs = { objects = "features.objects", model = "model.model", samples = "training.table" }
note = "Every object takes the class of the largest probability; the table has the probabilities and the margin (low where the forest hesitates: look there for more samples). Check the result against samples kept apart, not those it learned from."

[[nodes]]
id = "polygons"
op = "objects.to_vector"
inputs = { objects = "classify.objects" }
params = { smooth = 1 }
note = "A polygon for each object with its class and its attributes."

[[nodes]]
id = "write"
op = "vector.write"
inputs = { vector = "polygons.vector" }
params = { path = "objects_classified.gpkg" }
note = "The classified polygons, in a file beside the model."

[[nodes]]
id = "plot"
op = "view.plot"
inputs = { value = "classify.map" }
params = { title = "The classes of the objects" }
"#,
    );
    Model::from_toml(&text.replace("SAMPLE_ROWS", OBIA_SAMPLE_ROWS)).expect("a template is a model")
}

/// "Unsupervised classes of objects".
fn obia_clusters() -> Model {
    let mut text = String::from(
        r#"
format = "terraflux/1"
name = "Unsupervised classes of objects"
description = "Objects classified without samples: the image is cut into objects (multiresolution segmentation at the scale ESP suggests), their attributes are measured, and k-means groups the objects whose attributes are alike (the table of the objects is the table k-means reads; the number of clusters is the best silhouette from 2 to 10). objects.classify gives every object its cluster, 'cluster 1' and on, numbered by the first principal component of their centres, the same from one run to the next. The clusters are groups of alike objects, not classes: look at their centres and at the map, and name them. Replace the image with yours (Use my data); a self-organizing map (ml.som with clusters) takes the place of k-means as it is."
"#,
    );
    text.push_str(OBIA_SCENE);
    text.push_str(
        r#"
[[nodes]]
id = "features"
op = "objects.features"
inputs = { objects = "segments.objects", image = "image.image" }
params = { features = ["mean", "ndvi"] }
note = "The attributes the clusters are made of: the mean of each band and NDVI (found from the bands' names)."
why = "Few attributes that say different things cluster better than many that repeat each other; ml.pca on this table would reduce them first."

[[nodes]]
id = "kmeans"
op = "ml.kmeans"
inputs = { table = "features.table" }
params = { columns = ["mean_blue", "mean_green", "mean_red", "mean_nir", "mean_swir1", "mean_swir2", "ndvi"], k = "auto", seed = 1 }
note = "k-means on the objects' attributes (`columns` names them: the table also has the id, the cells and the area, which are not attributes of the ground). k is the best silhouette from 2 to 10; give a number to choose it. For an image whose bands are not named so, name the columns of yours."
why = "Each attribute is divided by its standard deviation first, so the NDVI, which is small, counts as much as the bands."
refs = ["Lloyd 1982", "Arthur and Vassilvitskii 2007", "Rousseeuw 1987"]

[[nodes]]
id = "classify"
op = "objects.classify"
inputs = { objects = "features.objects", model = "kmeans.model" }
note = "Every object takes the cluster of its attributes: the model from ml.kmeans needs no samples. The classes are named cluster 1 and on; the memberships are 1 for the cluster and 0 for the others."

[[nodes]]
id = "polygons"
op = "objects.to_vector"
inputs = { objects = "classify.objects" }
params = { smooth = 1 }
note = "A polygon for each object with its cluster and its attributes."

[[nodes]]
id = "write"
op = "vector.write"
inputs = { vector = "polygons.vector" }
params = { path = "objects_clusters.gpkg" }
note = "The polygons, in a file beside the model."

[[nodes]]
id = "plot"
op = "view.plot"
inputs = { value = "classify.map" }
params = { title = "The clusters of the objects" }

[[nodes]]
id = "choice"
op = "view.plot"
inputs = { table = "kmeans.choice" }
params = { title = "The choice of k (inertia and silhouette)" }
"#,
    );
    Model::from_toml(&text).expect("a template is a model")
}

/// The class hierarchy of the rules template: water, vegetation (forest, grassland) and the rest (cropland, urban),
/// from the signatures of nlm.image's classes.
const OBIA_HIERARCHY: &str = r##"[
  { name = "wet", rule = "above(ndwi, 0, 0.2)" },
  { name = "water", parent = "wet", colour = "#2b6cb0" },
  { name = "vegetation", rule = "above(ndvi, 0.45, 0.55)" },
  { name = "forest", rule = "below(mean_swir1, 0.18, 0.22)", parent = "vegetation", colour = "#2f7d32" },
  { name = "grassland", rule = "above(mean_swir1, 0.18, 0.22)", parent = "vegetation", colour = "#9bc53d" },
  { name = "other", rule = "below(ndwi, 0, 0.2) and below(ndvi, 0.45, 0.55)" },
  { name = "cropland", rule = "between(ndvi, 0.2, 0.28, 0.45, 0.5)", parent = "other", colour = "#e8c547" },
  { name = "urban", rule = "below(ndvi, 0.2, 0.28)", parent = "other", colour = "#8d8d8d" },
]"##;

/// "Classify an image by rules".
fn obia_rules() -> Model {
    let mut text = String::from(
        r#"
format = "terraflux/1"
name = "Classify an image by rules"
description = "An image classified by rules, the way eCognition's class hierarchy does: each class is a fuzzy rule on the objects' attributes (a membership from 0 to 1), a class inherits its parent's rule, and an object becomes the leaf class of the largest membership. Water has a high NDWI, vegetation a high NDVI (forest and grassland tell apart by the short-wave infrared, swir1), cropland a middle NDVI and urban a low one. The thresholds come from the spectra of the synthetic image: look at your image's objects (attributes table) and move them."
"#,
    );
    text.push_str(OBIA_SCENE);
    text.push_str(
        r#"
[[nodes]]
id = "features"
op = "objects.features"
inputs = { objects = "segments.objects", image = "image.image" }
params = { features = ["mean", "ndvi", "ndwi"] }
note = "What the rules read: the mean of each band, NDVI and NDWI (found from the bands' names)."

[[nodes]]
id = "rules"
op = "objects.rules"
inputs = { objects = "features.objects" }
note = "The class hierarchy. wet, vegetation and other only narrow what their daughters may take; an object becomes water, forest, grassland, cropland or urban. A ramp, above(x, a, b), is 0 up to a and 1 from b."
why = "Fuzzy ramps rather than sharp thresholds: an object near a limit has a small margin, and the margin shows where the rules are unsure."
refs = ["Baatz and Schape 2000", "Definiens eCognition Developer, Reference Book"]

[nodes.params]
classes = HIERARCHY

[[nodes]]
id = "polygons"
op = "objects.to_vector"
inputs = { objects = "rules.objects" }
params = { smooth = 1 }
note = "A polygon for each object with its class, its attributes and the margin."

[[nodes]]
id = "write"
op = "vector.write"
inputs = { vector = "polygons.vector" }
params = { path = "objects_by_rules.gpkg" }

[[nodes]]
id = "plot"
op = "view.plot"
inputs = { value = "rules.map" }
params = { title = "The classes by the rules" }
"#,
    );
    Model::from_toml(&text.replace("HIERARCHY", OBIA_HIERARCHY)).expect("a template is a model")
}

/// "Each class its own way": the class tree of `class_tree::add_class` (what *Add a class...* writes) on the scene of
/// the object templates. Water and Vegetation by their indices, Built (cropland and urban) by samples, Vegetation's
/// Forest and Grassland by samples; the classes of a mother are arbitrated by objects.resolve.
fn obia_tree() -> Model {
    use crate::class_tree::{NewClass, Recipe, add_class};
    let mut text = String::from(
        r#"
format = "terraflux/1"
name = "Each class its own way"
description = "A class tree where each class is found by the method that suits it, as GeoAIDA and InterIMAGE do: every class is a branch (a group) that reads the image and gives a map of membership, 0 to 1; the sibling classes are arbitrated cell by cell (objects.resolve: the greatest membership wins), and the children of a class are looked for only inside the region where their mother won. Water is found by its NDWI, vegetation by its NDVI, built land (cropland and urban) by samples, and inside the vegetation forest and grassland by samples. The map `leaves` has the class of each cell (1 built, 2 forest, 3 grassland, 4 water) and `conflicts` the cells of one class's hypotheses that another class won. Replace the image with yours (Use my data) and the samples with yours; Add a class... in the class tree adds a branch."
request = "Classify the image with a class tree: water by its NDWI, vegetation by its NDVI with forest and grassland inside it, and built land by samples."
summary = "A class tree of five classes (Water, Vegetation, Built; Forest and Grassland inside Vegetation), each with its own method, the siblings of each mother arbitrated cell by cell, and the map of the leaf classes."
"#,
    );
    text.push_str(OBIA_SCENE);
    // (the samples of the classes the forest tells apart: Built holds cropland and urban, the rest is Other)
    let rows = OBIA_SAMPLE_ROWS
        .replace("\"water\"", "\"Other\"")
        .replace("\"forest\"", "\"Forest\"")
        .replace("\"grassland\"", "\"Grassland\"")
        .replace("\"cropland\"", "\"Built\"")
        .replace("\"urban\"", "\"Built\"");
    text.push_str(
        r#"
[[nodes]]
id = "sample_table"
op = "table.of"
note = "The samples of the classes found by samples: a point (x and y in the image's coordinates) on an object, with the class's name (Forest, Grassland, Built; Other for what is none of them). In its place, the points you clicked in the Viewer."
why = "One table serves every class that learns from samples: each class's forest takes its own name as the class and every other value as what it is not."

[nodes.params]
columns = ["x", "y", "class"]
rows = SAMPLE_ROWS

[[nodes]]
id = "samples"
op = "vector.from_table"
inputs = { table = "sample_table.table" }
note = "The points as features, with the class as an attribute."
"#,
    );
    let mut m = Model::from_toml(&text.replace("SAMPLE_ROWS", &rows)).expect("a template is a model");
    let r = terraflux_ops::Registry::standard();
    let class = |name: &str, colour: [u8; 3], parent: Option<&str>, recipe| NewClass {
        name: name.into(),
        colour,
        parent: parent.map(String::from),
        recipe,
    };
    for c in [
        class("Water", [0x2b, 0x6c, 0xb0], None, Recipe::Water),
        class("Vegetation", [0x5f, 0xa8, 0x3a], None, Recipe::Vegetation),
        class("Built", [0x8d, 0x8d, 0x8d], None, Recipe::Samples),
        class("Forest", [0x2f, 0x7d, 0x32], Some("Vegetation"), Recipe::Samples),
        class("Grassland", [0x9b, 0xc5, 0x3d], Some("Vegetation"), Recipe::Samples),
    ] {
        add_class(&mut m, &[], &r, &c).unwrap_or_else(|e| panic!("{}: {e}", c.name));
    }
    let why = |m: &mut Model, id: &str, note: &str, why: &str| {
        let n = m.nodes.iter_mut().find(|n| n.id == id).expect("a class of the tree");
        n.note = Some(note.into());
        n.why = Some(why.into());
    };
    why(
        &mut m,
        "class_Water",
        "Water by NDWI: the membership rises from 0 at an NDWI of 0 to 1 at 0.3 (the bands named green and nir).",
        "Water is the one class a single index tells apart, so it needs no samples.",
    );
    why(
        &mut m,
        "class_Vegetation",
        "Vegetation by NDVI: the membership rises from 0 at an NDVI of 0.2 to 1 at 0.5 (the bands named red and nir). Forest and grassland are looked for only inside the cells it wins.",
        "A middle NDVI (cropland) gets a membership below 1, so Built, which learns from samples, wins it.",
    );
    why(
        &mut m,
        "class_Built",
        "Built land by samples: cropland and urban objects are the samples of Built, a random forest learns them from the objects' attributes; the membership is 1 for the objects it gives the class.",
        "Cropland and urban are told apart from vegetation and water by their spectra, not by one index; samples say it best.",
    );
    why(
        &mut m,
        "class_Forest",
        "Forest by samples, inside the vegetation: a random forest learns forest from the objects the samples point to.",
        "Forest and grassland have alike NDVI and differ in the short-wave infrared: the forest of trees finds it from the objects' attributes.",
    );
    why(
        &mut m,
        "class_Grassland",
        "Grassland by samples, inside the vegetation: the same, for the grassland.",
        "As forest: the attributes of the objects tell it, the region keeps it to the vegetation.",
    );
    let extra = r#"
format = "terraflux/1"

[[nodes]]
id = "leaves"
op = "raster.calc"
inputs = { a = "resolve.class", b = "resolve_of_Vegetation.class" }
params = { expression = "4 if a == 3 else (1 if a == 1 else (2 if b == 1 else (3 if b == 2 else None)))" }
note = "The leaf classes in one map: 1 built, 2 forest, 3 grassland, 4 water (no value where nothing won). The roots' decision numbers its classes by their names (Built 1, Vegetation 2, Water 3), Vegetation's children's (Forest 1, Grassland 2): the map joins them."
why = "A leaf is the class a cell ends in: Water and Built are decided among the roots, Forest and Grassland inside the cells Vegetation won."

[[nodes]]
id = "plot"
op = "view.plot"
inputs = { value = "leaves.map" }
params = { title = "The leaf classes (1 built, 2 forest, 3 grassland, 4 water)" }

[[nodes]]
id = "conflicts"
op = "view.plot"
inputs = { table = "resolve.conflicts" }
params = { title = "Hypotheses lost between the roots" }
"#;
    m.nodes.extend(Model::from_toml(extra).expect("a template is a model").nodes);
    m
}

/// "Land cover change by objects".
fn obia_change() -> Model {
    let flat = r##"[
  { name = "water", rule = "above(ndwi, 0, 0.2)", colour = "#2b6cb0" },
  { name = "forest", rule = "above(ndvi, 0.45, 0.55) and below(mean_swir1, 0.18, 0.22)", colour = "#2f7d32" },
  { name = "grassland", rule = "above(ndvi, 0.45, 0.55) and above(mean_swir1, 0.18, 0.22)", colour = "#9bc53d" },
  { name = "cropland", rule = "between(ndvi, 0.2, 0.28, 0.45, 0.5)", colour = "#e8c547" },
  { name = "urban", rule = "below(ndvi, 0.2, 0.28) and below(ndwi, 0, 0.2)", colour = "#8d8d8d" },
]"##;
    let text = r#"
format = "terraflux/1"
name = "Land cover change by objects"
description = "Land cover change from two images, by objects: each date is cut in objects and classed by rules, then the classes of the second date are bound by a fuzzy transition matrix written by hand (Feitosa et al. 2009, as InterIMAGE's multi-temporal analysis): a transition the matrix calls impossible, water becoming forest in a few years, cannot be what an object is, whatever its spectra say. The table `change` has what became what, with the objects, the cells and the area."

[[nodes]]
id = "land_2000"
op = "nlm.mosaic_gibbs"
params = { rows = 120, columns = 120, germs = 30, r = 12, patch_classes = 5, nlmr_compat = false, rescale = false, seed = 4 }
note = "The first date's classes drawn by chance (1 water, 2 forest, 3 grassland, 4 cropland, 5 urban): only the truth of the image below."

[[nodes]]
id = "patches"
op = "nlm.gaussian_field"
params = { rows = 120, columns = 120, range = 25, seed = 9 }
note = "Where the land changes: the highest of the field."

[[nodes]]
id = "land_2020"
op = "raster.calc"
inputs = { t = "land_2000.map", f = "patches.map" }
params = { expression = "5 if t == 2 and f > 0.5 else (4 if t == 3 and f > 0.5 else t)" }
note = "Twenty years later: forest cleared to urban land and grassland ploughed to cropland where the field is high."

[[nodes]]
id = "image_2000"
op = "nlm.image"
inputs = { classes = "land_2000.map" }
params = { seed = 1 }
note = "The first date's image, six bands. In its place, a raster.read of your image as a stack of named bands (blue, green, red, nir, swir1, swir2)."
data = { what = "the first date's multispectral image, a layer per band (named blue, green, red, nir, swir1, swir2)", kind = "stack" }

[[nodes]]
id = "image_2020"
op = "nlm.image"
inputs = { classes = "land_2020.map" }
params = { seed = 2 }
note = "The second date's image, on the same grid and with the same bands."
data = { what = "the second date's multispectral image, on the first's grid, the same bands", kind = "stack" }

[[nodes]]
id = "segments_2000"
op = "segment.multiresolution"
inputs = { image = "image_2000.image" }
note = "Objects of the first date at the scale ESP suggests."

[[nodes]]
id = "segments_2020"
op = "segment.multiresolution"
inputs = { image = "image_2020.image" }
note = "Objects of the second date: its own segmentation (the two may differ; objects.temporal links them cell by cell)."

[[nodes]]
id = "features_2000"
op = "objects.features"
inputs = { objects = "segments_2000.objects", image = "image_2000.image" }
params = { features = ["mean", "ndvi", "ndwi"] }

[[nodes]]
id = "features_2020"
op = "objects.features"
inputs = { objects = "segments_2020.objects", image = "image_2020.image" }
params = { features = ["mean", "ndvi", "ndwi"] }

[[nodes]]
id = "classes_2000"
op = "objects.rules"
inputs = { objects = "features_2000.objects" }
note = "The first date's classes by fuzzy rules on NDWI, NDVI and the short-wave infrared."

[nodes.params]
classes = CLASSES

[[nodes]]
id = "classes_2020"
op = "objects.rules"
inputs = { objects = "features_2020.objects" }
note = "The same rules on the second date: the dates must be classed alike, and the classes carry the same names."

[nodes.params]
classes = CLASSES

[[nodes]]
id = "matrix"
op = "table.of"
note = "How possible each transition is, from 0 (impossible) to 1 (common): a row for each class of origin (the column class, text), a column for each destination. Water stays water; a forest may be cleared to grassland, cropland or urban land; urban land rarely goes back. Change the numbers for your area."
why = "The matrix is the knowledge of the analyst about what can happen in the period, written down: it removes the changes a noisy spectrum suggests and nothing could explain."
refs = ["Feitosa et al. 2009", "Costa et al. 2010, InterIMAGE"]

[nodes.params]
columns = ["class", "water", "forest", "grassland", "cropland", "urban"]
rows = [
  ["water", 1, 0, 0, 0, 0],
  ["forest", 0, 1, 0.5, 0.3, 1],
  ["grassland", 0, 0.5, 1, 1, 1],
  ["cropland", 0, 0.3, 0.5, 1, 1],
  ["urban", 0, 0, 0, 0.1, 1],
]

[[nodes]]
id = "temporal"
op = "objects.temporal"
inputs = { before = "classes_2000.objects", after = "classes_2020.objects", matrix = "matrix.table" }
note = "The second date's classes adjusted by the matrix: min(membership, the possibility of the transition from what the place was). The table change has the transitions found."

[[nodes]]
id = "change_map"
op = "view.plot"
inputs = { value = "temporal.map" }
params = { title = "The classes of the second date" }

[[nodes]]
id = "change_table"
op = "view.plot"
inputs = { table = "temporal.change" }
params = { title = "What changed to what" }
"#;
    Model::from_toml(&text.replace("CLASSES", flat)).expect("a template is a model")
}

/// Where the user's own templates are kept (the user's application data): a model file each, named as shown.
pub fn user_dir() -> Option<std::path::PathBuf> {
    crate::app::layout_file().and_then(|f| f.parent().map(|d| d.join("templates")))
}

/// The user's own templates: (name, file), by name.
pub fn user_templates() -> Vec<(String, std::path::PathBuf)> {
    let Some(dir) = user_dir() else { return Vec::new() };
    let Ok(entries) = std::fs::read_dir(&dir) else { return Vec::new() };
    let mut out: Vec<(String, std::path::PathBuf)> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("toml")))
        .map(|p| (p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(), p))
        .collect();
    out.sort_by_key(|a| a.0.to_lowercase());
    out
}

/// The paths of a model's files (of the nodes that read and write them, and of its inputs) made absolute from the
/// folder `base` they were relative to, so that the model opens the same files from anywhere.
pub fn absolute_paths(m: &mut Model, base: &std::path::Path) {
    fn nodes(ns: &mut [Node], base: &std::path::Path) {
        for n in ns {
            if terraflux_graph::FILE_OPS.contains(&n.op.as_str())
                && let Some(serde_json::Value::String(p)) = n.params.get_mut("path")
                && !terraflux_graph::is_sent(p)
                && !p.is_empty()
                && std::path::Path::new(p.as_str()).is_relative()
            {
                *p = base.join(p.as_str()).to_string_lossy().into_owned();
            }
            if let Some(b) = &mut n.body {
                nodes(&mut b.nodes, base);
            }
        }
    }
    nodes(&mut m.nodes, base);
    for input in m.inputs.values_mut() {
        if let terraflux_graph::Input::Map { path, .. } | terraflux_graph::Input::Stack { path, .. } = input
            && std::path::Path::new(path.as_str()).is_relative()
        {
            *path = base.join(path.as_str()).to_string_lossy().into_owned();
        }
    }
}

/// A model saved as a template of the user's, named `name` (characters a file name cannot hold replaced); its
/// files' paths made absolute from `base`. The file written.
pub fn save_user_template(
    model: &Model,
    base: Option<&std::path::Path>,
    name: &str,
) -> Result<std::path::PathBuf, String> {
    let dir = user_dir().ok_or("no folder for the application's data")?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let clean: String = name.trim().chars().map(|c| if "\\/:*?\"<>|".contains(c) { '_' } else { c }).collect();
    if clean.is_empty() {
        return Err("a template needs a name".into());
    }
    let mut m = model.clone();
    m.name = Some(clean.clone());
    if let Some(b) = base {
        absolute_paths(&mut m, b);
    }
    let path = dir.join(format!("{clean}.toml"));
    m.save(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The object templates run, quickly, and do what they say: the samples' classification and the rules' agree
    /// with the truth the image was drawn from, the polygons are the objects, the change table has the changes drawn
    /// and none the matrix forbids, the trees and the buildings are found.
    #[test]
    fn the_object_templates_run() {
        use terraflux_ops::Value;
        struct Quiet;
        impl terraflux_exec::Observer for Quiet {}
        // (the file a template writes is left out: a test writes nothing)
        let run = |name: &str| {
            let mut m = TEMPLATES.iter().find(|t| t.name == name).unwrap().model();
            m.nodes.retain(|n| n.op != "vector.write");
            let t0 = std::time::Instant::now();
            let out = terraflux_exec::Engine::default()
                .run(&m, &Default::default(), &mut Quiet)
                .unwrap_or_else(|e| panic!("{name}: {e}"));
            println!("{name}: {} nodes, {:.2?}", m.nodes.len(), t0.elapsed());
            out
        };
        let map = |out: &terraflux_exec::Results, port: &str| match out.get(port) {
            Some(Value::Map(m)) => m.clone(),
            _ => panic!("no map {port}"),
        };
        let objects = |out: &terraflux_exec::Results, port: &str| match out.get(port) {
            Some(Value::Objects(o)) => o.clone(),
            _ => panic!("no objects {port}"),
        };
        // the share of the truth's cells whose object has the class of the same name (the codes 1.. of `names`)
        let agree = |m: &terraflux_ops::Map,
                     o: &terraflux_ops::objects::Objects,
                     truth: &terraflux_ops::Map,
                     names: &[&str]| {
            let hit = m
                .data
                .iter()
                .zip(truth.data.iter())
                .filter(|(p, t)| {
                    !p.is_nan() && !t.is_nan() && o.classes[**p as usize - 1].name == names[**t as usize - 1]
                })
                .count();
            hit as f64 / truth.data.len() as f64
        };
        let names = ["water", "forest", "grassland", "cropland", "urban"];

        // by samples
        let out = run("Classify an image by clicking samples");
        let (m, o, truth) = (map(&out, "classify.map"), objects(&out, "classify.objects"), map(&out, "land.map"));
        let acc = agree(&m, &o, &truth, &names);
        println!("samples: {} objects, accuracy {acc:.4}", o.count);
        assert!(acc >= 0.85, "accuracy {acc}");
        assert_eq!(o.classes.len(), 5);
        let Some(Value::Table(c)) = out.get("training.counts") else { panic!("no counts") };
        assert!(c.rows.iter().all(|r| r[1] >= 5.0), "{:?}", c.rows);

        // by rules
        let out = run("Classify an image by rules");
        let (m, o, truth) = (map(&out, "rules.map"), objects(&out, "rules.objects"), map(&out, "land.map"));
        let acc = agree(&m, &o, &truth, &names);
        println!("rules: {} objects, accuracy {acc:.4}", o.count);
        assert!(acc >= 0.8, "accuracy {acc}");
        // (the parents are not what an object becomes)
        let parents: Vec<usize> =
            ["wet", "vegetation", "other"].iter().filter_map(|p| o.classes.iter().position(|c| c.name == *p)).collect();
        assert_eq!(parents.len(), 3);
        assert!(o.class_of.iter().all(|&k| k == 0 || !parents.contains(&(k as usize - 1))));

        // unsupervised: k-means on the objects' attributes gives the classes of the drawn image, named cluster 1...
        let out = run("Unsupervised classes of objects");
        let (m, o, truth) = (map(&out, "classify.map"), objects(&out, "classify.objects"), map(&out, "land.map"));
        let mut count = std::collections::BTreeMap::<(u32, u32), usize>::new();
        for (p, t) in m.data.iter().zip(truth.data.iter()) {
            if !p.is_nan() && !t.is_nan() {
                *count.entry((*p as u32, *t as u32)).or_default() += 1;
            }
        }
        let mut best = std::collections::BTreeMap::<u32, usize>::new();
        for (&(p, _), &n) in &count {
            let b = best.entry(p).or_default();
            *b = (*b).max(n);
        }
        let purity = best.values().sum::<usize>() as f64 / truth.data.len() as f64;
        println!("clusters: {} objects, {} clusters, purity {purity:.4}", o.count, o.classes.len());
        assert!((4..=10).contains(&o.classes.len()), "{} clusters", o.classes.len());
        assert!(o.classes.iter().enumerate().all(|(k, c)| c.name == format!("cluster {}", k + 1)));
        assert!(o.class_of.iter().all(|&k| k > 0));
        assert!(purity >= 0.85, "purity {purity}");

        // water by Otsu on the NDWI: the water class (1) is the drawn water (1)
        let out = run("Water by Otsu on the NDWI");
        let (m, o, truth) = (map(&out, "water.map"), objects(&out, "water.objects"), map(&out, "land.map"));
        assert_eq!((o.classes[0].name.as_str(), o.classes[1].name.as_str()), ("water", "land"));
        let hit = m.data.iter().zip(truth.data.iter()).filter(|(p, t)| (**p == 1.0) == (**t == 1.0)).count();
        let acc = hit as f64 / truth.data.len() as f64;
        println!("water: {} objects, agreement {acc:.4}", o.count);
        assert!(acc >= 0.9, "agreement {acc}");
        let Some(Value::Vector(v)) = out.get("polygons.vector") else { panic!("no polygons") };
        assert_eq!(v.len(), o.count);

        // the scale chosen by itself: a best value among the scales, and the scores' table
        let out = run("The scale chosen by itself");
        let Some(Value::Number(best)) = out.get("best.value") else { panic!("no value") };
        let Some(Value::Table(sc)) = out.get("best.scores") else { panic!("no scores") };
        println!("optimize: best {best}, {} scales", sc.rows.len());
        assert!((2.0..=20.0).contains(best) && sc.rows.len() == 10, "{best}");

        // segmentation: a polygon per object
        let out = run("Segment an image into polygons");
        let o = objects(&out, "features.objects");
        let Some(Value::Vector(v)) = out.get("polygons.vector") else { panic!("no polygons") };
        assert!(o.count > 10 && v.len() == o.count, "{} objects, {} polygons", o.count, v.len());
        assert!(v.attributes.iter().any(|(n, _)| n == "mean_nir"));

        // change
        let out = run("Land cover change by objects");
        let Some(Value::Table(t)) = out.get("temporal.change") else { panic!("no change table") };
        let (from, to) = (t.labels.get("from").expect("from"), t.labels.get("to").expect("to"));
        let (fi, ti) = (t.column("from").unwrap(), t.column("to").unwrap());
        let cells = t.column("cells").unwrap();
        let moved = |a: &str, b: &str| -> f64 {
            (0..t.rows.len())
                .filter(|&k| from[fi[k] as usize - 1] == a && to[ti[k] as usize - 1] == b)
                .map(|k| cells[k])
                .sum()
        };
        println!(
            "change: forest to urban {}, grassland to cropland {}",
            moved("forest", "urban"),
            moved("grassland", "cropland")
        );
        assert!(moved("forest", "urban") > 100.0 && moved("grassland", "cropland") > 100.0);
        // the matrix: the water stays water (but for the objects that straddle its edge)
        let (stay, all) = (
            moved("water", "water"),
            (0..t.rows.len()).filter(|&k| from[fi[k] as usize - 1] == "water").map(|k| cells[k]).sum::<f64>(),
        );
        assert!(stay > 0.8 * all, "{stay} of {all} cells of water");

        // trees and buildings
        let out = run("Trees and buildings from a height model");
        let (m, o, scene) = (map(&out, "rules.map"), objects(&out, "rules.objects"), map(&out, "scene.map"));
        let class = |name: &str| o.classes.iter().position(|c| c.name == name).unwrap() as f64 + 1.0;
        let recall = |truth: f64, name: &str| {
            let (mut hit, mut all) = (0usize, 0usize);
            for (p, t) in m.data.iter().zip(scene.data.iter()) {
                if *t == truth {
                    all += 1;
                    hit += (*p == class(name)) as usize;
                }
            }
            hit as f64 / all.max(1) as f64
        };
        let (trees, buildings) = (recall(2.0, "trees"), recall(5.0, "buildings"));
        println!("trees {trees:.3}, buildings {buildings:.3}");
        assert!(trees >= 0.8, "trees {trees}");
        assert!(buildings >= 0.5, "buildings {buildings}");

        // classes by parcel: no object crosses a parcel, each is told its parcel
        let out = run("Classes by parcel");
        let o = objects(&out, "rules.objects");
        let Some(Value::Table(t)) = out.get("features.table") else { panic!("no features table") };
        let (share, parcel) = (t.column("th_share").unwrap(), t.column("th_mode").unwrap());
        println!("parcels: {} objects, parcels {:?}", o.count, parcel.iter().fold(0u32, |a, p| a | 1 << *p as u32));
        assert_eq!(share.len(), o.count);
        assert!(share.iter().all(|&s| s == 1.0), "an object crosses a parcel");
        // (nine parcels of 40 cells, the objects of each take its number)
        assert!((1..=9).all(|p| parcel.contains(&(p as f64))) && parcel.iter().all(|&q| (1.0..=9.0).contains(&q)));
        assert!(o.count > 9 && o.classes.len() == 8, "{} objects", o.count);

        // separate tree crowns: the clumps split into crowns, and one maximum each
        let out = run("Separate tree crowns");
        let count = |o: &terraflux_ops::objects::Objects, name: &str| {
            o.classes
                .iter()
                .position(|c| c.name == name)
                .map_or(0, |k| o.class_of.iter().filter(|&&c| c as usize == k + 1).count())
        };
        let (clumps, crowns, joined, tops) = (
            objects(&out, "classes.objects"),
            objects(&out, "crowns.objects"),
            objects(&out, "joined.objects"),
            objects(&out, "tops.objects"),
        );
        let n = (count(&clumps, "canopy"), count(&crowns, "canopy"), count(&joined, "canopy"), count(&tops, "top"));
        println!("crowns: clumps {}, split {}, joined {}, tops {}", n.0, n.1, n.2, n.3);
        assert!(n.0 >= 3 && n.1 > 2 * n.0, "the clumps did not split: {n:?}");
        assert!(n.2 < n.1 && n.2 > n.0, "the fusion: {n:?}");
        assert!(n.3 >= 3 && n.3 < n.2, "{n:?}");
        let Some(Value::Table(e)) = out.get("tops.table") else { panic!("no table of the tops") };
        let (ext, top) = (e.column("extremum").unwrap(), e.column("max_image").unwrap());
        // (no two crowns that touch are both marked: a top is taller than its neighbours)
        assert_eq!(ext.iter().filter(|&&x| x == 1.0).count(), n.3);
        let tallest = top.iter().zip(ext.iter()).filter(|(_, x)| **x == 1.0).map(|(h, _)| *h).fold(f64::MIN, f64::max);
        assert!(top.iter().all(|&h| h.is_nan() || h <= tallest + 1e-9));

        // roads and rivers by the skeleton
        let out = run("Roads and rivers by the skeleton");
        let (m, o) = (map(&out, "rules.map"), objects(&out, "rules.objects"));
        let (s, t, f) = (map(&out, "slope.map"), map(&out, "slope2.map"), map(&out, "relief.map"));
        let class = |name: &str| o.classes.iter().position(|c| c.name == name).unwrap() as f64 + 1.0;
        let recall = |name: &str, truth: &dyn Fn(usize) -> bool| {
            let (mut hit, mut all) = (0usize, 0usize);
            for k in 0..m.data.len() {
                if truth(k) {
                    all += 1;
                    hit += (m.data[k] == class(name)) as usize;
                }
            }
            hit as f64 / all.max(1) as f64
        };
        let river = |k: usize| (s.data[k] + 0.3 * (f.data[k] - 0.5) - 0.5).abs() < 0.03;
        let road = |k: usize| !river(k) && (t.data[k] - 0.25).abs() < 0.025;
        let (r, d) = (recall("river", &river), recall("road", &road));
        println!("skeleton: river {r:.3}, road {d:.3}");
        assert!(r >= 0.7 && d >= 0.7, "river {r}, road {d}");
        // the compact objects are lakes and buildings, not rivers and roads
        assert!(
            o.classes.iter().all(|c| ["river", "lake", "road", "building", "vegetation"].contains(&c.name.as_str()))
        );
        assert!(["lake", "building"].iter().all(|c| m.data.iter().any(|&v| v == class(c))));

        // the class tree: the leaf map against the truth (water 4, forest 2, grassland 3, cropland and urban 1)
        let out = run("Each class its own way");
        let (leaves, truth) = (map(&out, "leaves.map"), map(&out, "land.map"));
        let leaf_of = [4.0, 2.0, 3.0, 1.0, 1.0];
        let hit = leaves
            .data
            .iter()
            .zip(truth.data.iter())
            .filter(|(p, t)| !p.is_nan() && !t.is_nan() && **p == leaf_of[**t as usize - 1])
            .count();
        let acc = hit as f64 / truth.data.len() as f64;
        println!("tree: accuracy {acc:.4}");
        assert!(acc >= 0.8, "accuracy {acc}");
        let Some(Value::Table(c)) = out.get("resolve.conflicts") else { panic!("no conflicts table") };
        println!("tree: {} rows of conflicts", c.rows.len());
        // (the tree is what Add a class... writes: five classes with their mothers)
        let m = TEMPLATES.iter().find(|t| t.name == "Each class its own way").unwrap().model();
        let tree = crate::class_tree::read(&m.nodes);
        let mut seen: Vec<(&str, Option<&str>)> =
            tree.iter().map(|e| (e.name.as_str(), e.parent.map(|p| tree[p].name.as_str()))).collect();
        seen.sort();
        assert_eq!(
            seen,
            [
                ("Built", None),
                ("Forest", Some("Vegetation")),
                ("Grassland", Some("Vegetation")),
                ("Vegetation", None),
                ("Water", None)
            ]
        );
    }

    /// The multivariate templates run and do what they say: the components of an image are as many as asked and the
    /// first holds most of the variance, the k-means clusters of the image are its classes, the map's groups are
    /// five and split the landscape, the minor component of two dates is large where the land changed.
    #[test]
    fn the_multivariate_templates_run() {
        use terraflux_ops::Value;
        struct Quiet;
        impl terraflux_exec::Observer for Quiet {}
        let run = |name: &str| {
            let m = TEMPLATES.iter().find(|t| t.name == name).unwrap().model();
            terraflux_exec::Engine::default()
                .run(&m, &Default::default(), &mut Quiet)
                .unwrap_or_else(|e| panic!("{name}: {e}"))
        };
        let map = |out: &terraflux_exec::Results, port: &str| match out.get(port) {
            Some(Value::Map(m)) => m.clone(),
            _ => panic!("no map {port}"),
        };
        let table = |out: &terraflux_exec::Results, port: &str| match out.get(port) {
            Some(Value::Table(t)) => t.clone(),
            _ => panic!("no table {port}"),
        };
        // the share of the cells whose cluster is the commonest class of its cells in the truth
        let purity = |m: &terraflux_ops::Map, truth: &terraflux_ops::Map| {
            let mut count = std::collections::BTreeMap::<(u32, u32), usize>::new();
            for (p, t) in m.data.iter().zip(truth.data.iter()) {
                if !p.is_nan() && !t.is_nan() {
                    *count.entry((*p as u32, *t as u32)).or_default() += 1;
                }
            }
            let mut best = std::collections::BTreeMap::<u32, usize>::new();
            for (&(p, _), &n) in &count {
                let b = best.entry(p).or_default();
                *b = (*b).max(n);
            }
            best.values().sum::<usize>() as f64 / truth.data.len() as f64
        };

        // principal components: three layers pc1..pc3, the first the largest, all variance explained in the table
        let out = run("Principal components of an image");
        let Some(Value::Stack(s)) = out.get("pca.scores") else { panic!("no scores") };
        assert_eq!(s.layers.len(), 3);
        let explained = table(&out, "pca.explained");
        let ratio = explained.column("ratio").unwrap();
        println!("pca: the explained shares {ratio:.3?}");
        assert_eq!(ratio.len(), 6);
        assert!(ratio[0] > 0.5 && ratio.windows(2).all(|w| w[0] >= w[1]));
        assert!((ratio.iter().sum::<f64>() - 1.0).abs() < 1e-9);
        assert_eq!(table(&out, "pca.loadings").rows.len(), 3);

        // k-means: the clusters of the image are its classes
        let out = run("Unsupervised classification of an image (k-means)");
        let (clusters, truth) = (map(&out, "kmeans.clusters"), map(&out, "land.map"));
        let choice = table(&out, "kmeans.choice");
        let k = choice.column("k").unwrap();
        println!("kmeans: k tried {k:?}, purity {:.4}", purity(&clusters, &truth));
        assert_eq!(k.len(), 9);
        assert!(purity(&clusters, &truth) >= 0.9);
        let top = clusters.data.iter().cloned().filter(|v| !v.is_nan()).fold(0.0, f64::max);
        assert!((4.0..=7.0).contains(&top), "{top} clusters");

        // the self-organizing map: five groups, each a share of the landscape, and the codebook of the nodes
        let out = run("Types of landscape by a self-organizing map");
        let groups = map(&out, "som.groups");
        let mut seen = [0usize; 5];
        for v in groups.data.iter().filter(|v| !v.is_nan()) {
            seen[*v as usize - 1] += 1;
        }
        println!("som: cells of each group {seen:?}");
        assert!(seen.iter().all(|&n| n > 100), "{seen:?}");
        assert_eq!(table(&out, "som.codebook").rows.len(), 64);
        assert_eq!(table(&out, "som.quality").rows.len(), 1);

        // the change: the minor component is larger where the land changed
        let out = run("Change between two dates by principal components");
        let (change, before, after) = (map(&out, "change.map"), map(&out, "land_2000.map"), map(&out, "land_2020.map"));
        let (mut moved, mut kept) = ((0.0, 0usize), (0.0, 0usize));
        for ((c, a), b) in change.data.iter().zip(before.data.iter()).zip(after.data.iter()) {
            let side = if a == b { &mut kept } else { &mut moved };
            side.0 += c;
            side.1 += 1;
        }
        let (moved, kept) = (moved.0 / moved.1.max(1) as f64, kept.0 / kept.1.max(1) as f64);
        // (the change component sets one date's bands against the other's)
        let loadings = table(&out, "pca.loadings");
        let third = loadings.rows[2].clone();
        let (a, b) = (loadings.column("nir_2000").unwrap()[2], loadings.column("nir_2020").unwrap()[2]);
        println!("change: loadings of pc3 {third:.3?}");
        assert!(a * b < 0.0, "nir_2000 {a}, nir_2020 {b}");
        println!("change: the mean of the minor component {moved:.3} where the land changed, {kept:.3} where not");
        assert!(moved > 2.0 * kept, "{moved} against {kept}");
    }

    #[test]
    fn templates_are_sound() {
        let r = terraflux_ops::Registry::standard();
        for t in &TEMPLATES {
            let m = t.model();
            let issues = terraflux_graph::check(&m, &r);
            let errors: Vec<_> = issues.iter().filter(|i| i.severity == terraflux_graph::Severity::Error).collect();
            // (the one for a file to choose excepted)
            assert!(errors.iter().all(|i| i.message.contains("path")), "{}: {errors:?}", t.name);
        }
    }

    /// Every template is in a group of the menu, under a name of its own.
    #[test]
    fn templates_are_grouped() {
        for t in &TEMPLATES {
            assert!(GROUPS.contains(&t.group), "{}", t.name);
            assert_eq!(TEMPLATES.iter().filter(|u| u.name == t.name).count(), 1, "{}", t.name);
            if t.build.is_none() {
                assert_eq!(t.model().name.as_deref(), Some(t.name));
            }
        }
    }

    /// The templates whose point is the landscapes drawn themselves: nothing in them stands for data of the user's.
    const DRAWN_ON_PURPOSE: [&str; 2] = ["Neutral landscapes", "More neutral landscapes"];

    /// A template that draws data with a neutral landscape marks the nodes that stand for the user's data (`data`),
    /// each saying what it is; one of classes names its classes; `check` has nothing to say of them.
    #[test]
    fn the_nodes_standing_for_the_users_data_are_marked() {
        fn walk<'a>(nodes: &'a [Node], out: &mut Vec<&'a Node>) {
            for n in nodes {
                out.push(n);
                if let Some(b) = &n.body {
                    walk(&b.nodes, out);
                }
            }
        }
        let r = terraflux_ops::Registry::standard();
        let (mut offenders, mut wrong, mut marked) = (Vec::new(), Vec::new(), 0);
        for t in &TEMPLATES {
            let m = t.model();
            let mut all = Vec::new();
            walk(&m.nodes, &mut all);
            let data: Vec<(&str, &terraflux_graph::Data)> =
                all.iter().filter_map(|n| n.data.as_ref().map(|d| (n.id.as_str(), d))).collect();
            if all.iter().any(|n| n.op.starts_with("nlm.")) && data.is_empty() && !DRAWN_ON_PURPOSE.contains(&t.name) {
                offenders.push(t.name);
            }
            for (id, d) in &data {
                marked += 1;
                for why in d.problems() {
                    wrong.push(format!("{} / {id}: {why}", t.name));
                }
                // (classes may be empty: "any classes", the user's are kept)
            }
            let said: Vec<String> = terraflux_graph::check(&m, &r)
                .into_iter()
                .filter(|i| i.message.contains("`data`"))
                .map(|i| format!("{}: {i}", t.name))
                .collect();
            wrong.extend(said);
        }
        assert!(offenders.is_empty(), "templates with a neutral landscape and no node marked as data: {offenders:?}");
        assert!(wrong.is_empty(), "{wrong:#?}");
        assert!(marked > 100, "{marked} nodes marked");
    }

    #[test]
    fn paths_made_absolute() {
        let mut m = Model::new("t");
        m.nodes.push(Node::new("read", "raster.read").param("path", "data/land.tif"));
        m.nodes.push(Node::new("other", "nlm.fbm").param("path", "keep"));
        m.inputs.insert(
            "x".into(),
            terraflux_graph::Input::Map { path: "x.tif".into(), band: None, nodata: None, note: None, data: None },
        );
        let base = std::env::temp_dir();
        absolute_paths(&mut m, &base);
        assert_eq!(m.nodes[0].params["path"], base.join("data/land.tif").to_string_lossy().as_ref());
        assert_eq!(m.nodes[1].params["path"], "keep");
        assert!(
            matches!(&m.inputs["x"], terraflux_graph::Input::Map { path, .. } if path == &base.join("x.tif").to_string_lossy())
        );
    }

    /// The land change templates run: three steps simulated, farming grown.
    #[test]
    fn land_change_templates_run() {
        struct Quiet;
        impl terraflux_exec::Observer for Quiet {}
        for name in ["Dinamica EGO", "CLUE-S", "PLUS (LEAS and CARS)"] {
            let m = TEMPLATES.iter().find(|t| t.name == name).unwrap().model();
            let out = terraflux_exec::Engine::default()
                .run(&m, &Default::default(), &mut Quiet)
                .unwrap_or_else(|e| panic!("{name}: {e}"));
            let Some(terraflux_ops::Value::Series(maps)) = out.get("simulation.maps") else {
                panic!("{name}: no maps")
            };
            assert_eq!(maps.len(), 3, "{name}");
            let farming = |v: &terraflux_ops::Value| match v {
                terraflux_ops::Value::Map(m) => m.data.iter().filter(|&&x| x == 2.0).count(),
                _ => 0,
            };
            let counts: Vec<usize> = maps.iter().map(farming).collect();
            assert!(counts[2] > counts[0], "{name}: farming grows ({counts:?})");
        }
    }

    /// The rivers template runs: a row per link with its shares of the land cover, every gauge on a river with a
    /// catchment.
    #[test]
    fn rivers_template_runs() {
        struct Quiet;
        impl terraflux_exec::Observer for Quiet {}
        let name = "Rivers: the land upstream and the gauges' catchments";
        let m = TEMPLATES.iter().find(|t| t.name == name).unwrap().model();
        let out = terraflux_exec::Engine::default().run(&m, &Default::default(), &mut Quiet).unwrap();
        let Some(terraflux_ops::Value::Table(t)) = out.get("upstream.table") else { panic!("no upstream") };
        assert!(t.rows.len() > 5, "{} links", t.rows.len());
        for r in &t.rows {
            let shares: f64 = ["share_1", "share_2", "share_3"]
                .iter()
                .map(|c| r[t.columns.iter().position(|x| x == c).unwrap()])
                .sum();
            assert!((shares - 1.0).abs() < 1e-9, "{r:?}");
        }
        let Some(terraflux_ops::Value::Table(g)) = out.get("gauges.table") else { panic!("no gauges") };
        assert_eq!(g.column("snapped").unwrap(), [1.0; 5]);
        let Some(terraflux_ops::Value::Table(c)) = out.get("catchments.table") else { panic!("no catchments") };
        assert!(c.column("cells").unwrap().iter().all(|n| *n >= 300.0), "{:?}", c.column("cells"));
    }

    /// The SWAT+ template runs: HRUs, channels, a soil loss and a runoff on the HRUs, a flow on the channels, two years
    /// of daily discharge; a change of the curve number moves the runoff.
    #[test]
    fn swat_template_runs() {
        struct Quiet;
        impl terraflux_exec::Observer for Quiet {}
        let m = TEMPLATES.iter().find(|t| t.name == "SWAT+ from rasters").unwrap().model();
        let given = |cn2: i64| {
            std::collections::BTreeMap::from([
                ("cn2_change".to_string(), terraflux_ops::Value::Integer(cn2)),
                ("awc_change".to_string(), terraflux_ops::Value::Integer(0)),
            ])
        };
        let run = |cn2: i64| {
            terraflux_exec::Engine::default().run(&m, &given(cn2), &mut Quiet).unwrap_or_else(|e| panic!("{e}"))
        };
        let out = run(0);
        let mean = |port: &str| match out.get(port) {
            Some(terraflux_ops::Value::Map(m)) => {
                let v: Vec<f64> = m.data.iter().copied().filter(|x| x.is_finite()).collect();
                assert!(!v.is_empty(), "{port}: no cell has a value");
                v.iter().sum::<f64>() / v.len() as f64
            }
            _ => panic!("no map {port}"),
        };
        // (the means and the sum of the script of this template in terraflux-py 1.58.1, run in Python: the same project)
        let close = |a: f64, b: f64, what: &str| assert!((a - b).abs() <= 1e-6 * b.abs(), "{what}: {a} against {b}");
        close(mean("sediment.map"), 123.2539025273669, "sediment");
        let runoff = mean("runoff.map");
        close(runoff, 192.8965964010193, "runoff");
        close(mean("flow.map"), 0.014595569413457368, "flow");
        let Some(terraflux_ops::Value::Table(q)) = out.get("discharge.table") else { panic!("no discharge") };
        // (the first of the two years is the spin-up the project skips: nyskip 1)
        assert_eq!(q.rows.len(), 365);
        assert_eq!(q.columns.len(), 2 + 9, "year, day and nine channels");
        let total: f64 = q.rows.iter().flat_map(|r| r[2..].iter()).sum();
        close(total, 49.28723263032374, "discharge");
        let Some(terraflux_ops::Value::Table(h)) = out.get("watershed.table") else { panic!("no HRUs") };
        assert!(h.rows.len() >= 3, "{} HRUs", h.rows.len());
        // a higher curve number: more runoff
        let wetter = match run(10).get("runoff.map") {
            Some(terraflux_ops::Value::Map(m)) => {
                let v: Vec<f64> = m.data.iter().copied().filter(|x| x.is_finite()).collect();
                v.iter().sum::<f64>() / v.len() as f64
            }
            _ => panic!("no runoff"),
        };
        assert!(wetter > runoff, "{wetter} against {runoff}");
    }

    /// The dams template runs: a DCI below 100 for a network with dams, the dams ranked by the gain of removing each.
    #[test]
    fn dams_template_runs() {
        struct Quiet;
        impl terraflux_exec::Observer for Quiet {}
        let name = "Rivers: dams and their connectivity (DCI)";
        let m = TEMPLATES.iter().find(|t| t.name == name).unwrap().model();
        let out = terraflux_exec::Engine::default().run(&m, &Default::default(), &mut Quiet).unwrap();
        let Some(terraflux_ops::Value::Table(t)) = out.get("dci.table") else { panic!("no dci") };
        let dci = t.column("dci_p").unwrap();
        assert!(!dci.is_empty() && dci.iter().all(|d| *d > 0.0 && *d < 100.0), "{dci:?}");
        let Some(terraflux_ops::Value::Vector(b)) = out.get("dci.barriers") else { panic!("no barriers") };
        let rank = b.attributes.iter().find(|(n, _)| n == "rank").unwrap().1.numbers();
        assert!(rank.contains(&1.0), "{rank:?}");
        let Some(terraflux_ops::Value::Table(d)) = out.get("distances.table") else { panic!("no distances") };
        assert_eq!(d.rows.len(), 15);
    }

    /// The template of control.each runs: a stack of the three years, one table with the year first.
    #[test]
    fn each_template_runs() {
        struct Quiet;
        impl terraflux_exec::Observer for Quiet {}
        let name = "For each year: one analysis per map (control.each)";
        let m = TEMPLATES.iter().find(|t| t.name == name).unwrap().model();
        let inputs = std::collections::BTreeMap::from([("min_patch".to_string(), terraflux_ops::Value::Integer(20))]);
        let out = terraflux_exec::Engine::default().run(&m, &inputs, &mut Quiet).unwrap();
        let Some(terraflux_ops::Value::Stack(s)) = out.get("by_year.forest") else { panic!("no stack") };
        assert_eq!(s.keys.len(), 3);
        let Some(terraflux_ops::Value::Table(t)) = out.get("by_year.stats") else { panic!("no table") };
        assert_eq!(t.column("item").unwrap(), [2000.0, 2010.0, 2020.0]);
    }

    /// The switch template runs each of its methods: only the branch chosen, and the title made from it.
    #[test]
    fn switch_template_runs() {
        struct Quiet;
        impl terraflux_exec::Observer for Quiet {}
        let name = "Choose the method: a switch and its scenarios";
        let m = TEMPLATES.iter().find(|t| t.name == name).unwrap().model();
        for method in ["distance", "neighbourhood"] {
            let inputs =
                std::collections::BTreeMap::from([("method".to_string(), terraflux_ops::Value::Text(method.into()))]);
            let out = terraflux_exec::Engine::default().run(&m, &inputs, &mut Quiet).unwrap();
            let Some(terraflux_ops::Value::Text(t)) = out.get("title.text") else { panic!("no title") };
            assert_eq!(t, &format!("Suitability by {method} (weight 0.8)"));
            let Some(terraflux_ops::Value::Map(c)) = out.get("combined.map") else { panic!("no map") };
            assert!(c.data.iter().all(|x| (0.0..=1.0).contains(x)), "{method}");
        }
    }

    /// The accuracy template runs: the overall accuracy near the nine tenths of cells kept, and the area of every
    /// class estimated from the sample within its confidence interval of the truth's.
    #[test]
    fn accuracy_template_runs() {
        struct Quiet;
        impl terraflux_exec::Observer for Quiet {}
        let m = TEMPLATES.iter().find(|t| t.name == "Accuracy and area of a map").unwrap().model();
        let out = terraflux_exec::Engine::default().run(&m, &Default::default(), &mut Quiet).unwrap();
        let Some(terraflux_ops::Value::Table(o)) = out.get("assess.overall") else { panic!("no overall") };
        let overall = o.column("overall_accuracy").unwrap()[0];
        assert!((0.85..0.95).contains(&overall), "{overall}");
        let Some(terraflux_ops::Value::Map(truth)) = out.get("truth.map") else { panic!("no truth") };
        let Some(terraflux_ops::Value::Table(t)) = out.get("assess.table") else { panic!("no table") };
        let (cls, area, ci) =
            (t.column("class").unwrap(), t.column("area_cells").unwrap(), t.column("area_cells_ci").unwrap());
        for k in 0..cls.len() {
            let real = truth.data.iter().filter(|&&v| v == cls[k]).count() as f64;
            assert!((area[k] - real).abs() <= ci[k], "class {}: {} ± {} for {real}", cls[k], area[k], ci[k]);
        }
    }

    /// The REDD+ template runs: the expected deforestation allocated, no cell above its area.
    #[test]
    fn redd_template_runs() {
        struct Quiet;
        impl terraflux_exec::Observer for Quiet {}
        let m = TEMPLATES.iter().find(|t| t.name == "REDD+ baseline (VT0007)").unwrap().model();
        let out = terraflux_exec::Engine::default().run(&m, &Default::default(), &mut Quiet).unwrap();
        let Some(terraflux_ops::Value::Map(d)) = out.get("allocate.density") else { panic!("no density") };
        let total: f64 = d.data.iter().sum();
        assert!((total - 60.0).abs() < 0.01, "{total}");
        assert!(d.data.iter().all(|v| (0.0..=0.09 + 1e-9).contains(v)));
    }

    /// The Forest Code template runs: river margins found, six properties balanced.
    #[test]
    fn forest_code_template_runs() {
        struct Quiet;
        impl terraflux_exec::Observer for Quiet {}
        let m = TEMPLATES.iter().find(|t| t.name == "Forest Code: APP and Reserva Legal").unwrap().model();
        let out = terraflux_exec::Engine::default().run(&m, &Default::default(), &mut Quiet).unwrap();
        let Some(terraflux_ops::Value::Map(app)) = out.get("app.app") else { panic!("no app") };
        assert!(app.data.iter().filter(|&&v| v == 1.0).count() > 100);
        let Some(terraflux_ops::Value::Table(t)) = out.get("reserve.table") else { panic!("no table") };
        assert_eq!(t.rows.len(), 6);
    }

    /// The trend template runs: cells greening and browning, clustered.
    #[test]
    fn trend_template_runs() {
        struct Quiet;
        impl terraflux_exec::Observer for Quiet {}
        let m = TEMPLATES.iter().find(|t| t.name == "Trend and hot spots").unwrap().model();
        let out = terraflux_exec::Engine::default().run(&m, &Default::default(), &mut Quiet).unwrap();
        let Some(terraflux_ops::Value::Map(t)) = out.get("trend.trend") else { panic!("no trend") };
        assert!(t.data.contains(&1.0) && t.data.contains(&-1.0));
        let Some(terraflux_ops::Value::Table(i)) = out.get("moran.table") else { panic!("no moran") };
        assert!(i.rows[0][0] > 0.3, "{:?}", i.rows[0]);
        let Some(terraflux_ops::Value::Map(c)) = out.get("lisa.clusters") else { panic!("no clusters") };
        assert!(c.data.contains(&1.0) && c.data.contains(&3.0));
    }

    /// The interpolation template runs: kriging closer to the surface than inverse distance weighting.
    #[test]
    fn interpolation_template_runs() {
        struct Quiet;
        impl terraflux_exec::Observer for Quiet {}
        let m = TEMPLATES.iter().find(|t| t.name == "Interpolation: IDW and kriging").unwrap().model();
        let out = terraflux_exec::Engine::default().run(&m, &Default::default(), &mut Quiet).unwrap();
        let mae = |port: &str| {
            let Some(terraflux_ops::Value::Map(e)) = out.get(port) else { panic!("no {port}") };
            e.data.iter().map(|v| v.abs()).sum::<f64>() / e.data.len() as f64
        };
        let (k, i) = (mae("kriging_error.map"), mae("idw_error.map"));
        assert!(k < i && k < 3.0, "{k} {i}");
        let Some(terraflux_ops::Value::Map(v)) = out.get("kriging.variance") else { panic!("no variance") };
        assert!(v.data.iter().all(|x| *x >= 0.0));
    }

    /// The migration template runs: the species spreads, and loses cells, and suitable cells stay out of reach.
    #[test]
    fn migration_template_runs() {
        struct Quiet;
        impl terraflux_exec::Observer for Quiet {}
        let m = TEMPLATES.iter().find(|t| t.name == "Species migration (MigClim)").unwrap().model();
        let out = terraflux_exec::Engine::default().run(&m, &Default::default(), &mut Quiet).unwrap();
        let Some(terraflux_ops::Value::Table(t)) = out.get("migrate.table") else { panic!("no table") };
        let occupied: Vec<f64> = t.rows.iter().map(|r| r[5]).collect();
        assert!(occupied.iter().cloned().fold(0.0, f64::max) > 2.0 * occupied[0], "{occupied:?}");
        let Some(terraflux_ops::Value::Map(f)) = out.get("migrate.map") else { panic!("no map") };
        assert!(f.data.contains(&30000.0) && f.data.iter().any(|v| *v < 0.0));
    }

    /// The metapopulation template runs: the species persists a century, patches turning over.
    #[test]
    fn metapopulation_template_runs() {
        struct Quiet;
        impl terraflux_exec::Observer for Quiet {}
        let m = TEMPLATES.iter().find(|t| t.name == "Metapopulation (incidence function model)").unwrap().model();
        let out = terraflux_exec::Engine::default().run(&m, &Default::default(), &mut Quiet).unwrap();
        let Some(terraflux_ops::Value::Table(t)) = out.get("metapopulation.table") else { panic!("no table") };
        assert_eq!(t.rows.len(), 101);
        assert!(t.rows[100][1] > 0.0 && t.rows.iter().map(|r| r[3]).sum::<f64>() > 20.0, "{:?}", t.rows[100]);
    }

    /// The species distribution model template runs: a map of suitability from 0 to 1.
    #[test]
    fn sdm_template_runs() {
        struct Quiet;
        impl terraflux_exec::Observer for Quiet {}
        let m = TEMPLATES.iter().find(|t| t.name == "Species distribution model").unwrap().model();
        let out = terraflux_exec::Engine::default().run(&m, &Default::default(), &mut Quiet).unwrap();
        let Some(terraflux_ops::Value::Map(p)) = out.get("predict.prediction") else { panic!("no prediction") };
        assert!(p.data.iter().all(|v| (0.0..=1.0).contains(v)));
        let Some(terraflux_ops::Value::Table(t)) = out.get("data.records") else { panic!("no records") };
        assert!(t.columns.contains(&"fold".to_string()) && t.rows.len() > 800);
    }

    /// The Linkage Mapper template runs: cores linked, current in the corridors and a barrier score somewhere.
    #[test]
    fn linkage_template_runs() {
        struct Quiet;
        impl terraflux_exec::Observer for Quiet {}
        let m = TEMPLATES.iter().find(|t| t.name.starts_with("Linkage Mapper")).unwrap().model();
        let out = terraflux_exec::Engine::default().run(&m, &Default::default(), &mut Quiet).unwrap();
        let Some(terraflux_ops::Value::Map(c)) = out.get("cores.map") else { panic!("no cores") };
        let n = c.data.iter().cloned().filter(|v| v.is_finite()).fold(0.0, f64::max);
        assert!(n >= 3.0, "{n} cores");
        let Some(terraflux_ops::Value::Table(l)) = out.get("pathways.links") else { panic!("no links") };
        let kept = l.columns.iter().position(|c| c == "kept").unwrap();
        assert!(l.rows.iter().filter(|r| r[kept] > 0.0).count() >= 2, "{:?}", l.rows);
        let Some(terraflux_ops::Value::Map(k)) = out.get("pathways.corridors") else { panic!("no corridors") };
        assert!(k.data.iter().any(|v| v.is_finite()));
        let Some(terraflux_ops::Value::Map(i)) = out.get("pinchpoints.current") else { panic!("no current") };
        assert!(i.data.iter().any(|v| v.is_finite() && *v > 0.0));
        let Some(terraflux_ops::Value::Map(b)) = out.get("barriers.circles") else { panic!("no circles") };
        assert!(b.data.iter().any(|v| v.is_finite() && *v > 0.0));
    }
}

#[cfg(test)]
mod table_template_tests {
    #[test]
    fn the_raster_statistics_template_runs() {
        let t = super::TEMPLATES.iter().find(|t| t.name.starts_with("Raster statistics:")).unwrap();
        let m = t.model();
        let r = match terraflux_exec::Engine::default().run(&m, &Default::default(), &mut terraflux_exec::Quiet) {
            Ok(r) => r,
            Err(e) => panic!("{e}"),
        };
        let table = |n: &str| match r.get(n) {
            Some(terraflux_ops::Value::Table(t)) => t.clone(),
            other => panic!("{n}: {other:?}"),
        };
        assert_eq!(table("summary.table").rows.len(), 1);
        assert_eq!(table("summary.histogram").rows.len(), 12);
        assert_eq!(table("by_zone.table").rows.len(), 3);
        let shares = table("hectares.table");
        assert!(shares.rows.iter().all(|r| (r[r.len() - 1] - 1.0).abs() < 1e-9), "{:?}", shares.rows);
        let matrix = table("correlation.matrix");
        assert_eq!(matrix.columns, ["layer", "elevation", "rain"]);
        assert!(matrix.rows[0][2] > 0.5, "{:?}", matrix.rows);
        assert!(matches!(r.get("scaled.map"), Some(terraflux_ops::Value::Stack(_))));
    }

    #[test]
    fn the_cleaning_template_runs() {
        let t = super::TEMPLATES.iter().find(|t| t.name.starts_with("Cleaning a map")).unwrap();
        let m = t.model();
        let r = match terraflux_exec::Engine::default().run(&m, &Default::default(), &mut terraflux_exec::Quiet) {
            Ok(r) => r,
            Err(e) => panic!("{e}"),
        };
        let table = |n: &str| match r.get(n) {
            Some(terraflux_ops::Value::Table(t)) => t.clone(),
            other => panic!("{n}: {other:?}"),
        };
        let patches = |n: &str| {
            let t = table(n);
            let np = t.columns.iter().position(|c| c == "np").unwrap();
            t.rows.iter().map(|r| r[np]).sum::<f64>()
        };
        assert!(
            patches("after.table") * 5.0 < patches("before.table"),
            "{} {}",
            patches("after.table"),
            patches("before.table")
        );
        let labels = table("patches.table");
        assert!(labels.rows.iter().all(|r| r[2] >= 20.0), "{:?}", labels.rows);
    }

    #[test]
    fn the_vector_template_runs() {
        if terraflux_gdal::gdal().is_err() {
            return;
        }
        let t = super::TEMPLATES.iter().find(|t| t.name.starts_with("Vector tools:")).unwrap();
        let m = t.model();
        let r = match terraflux_exec::Engine::default().run(&m, &Default::default(), &mut terraflux_exec::Quiet) {
            Ok(r) => r,
            Err(e) => panic!("{e}"),
        };
        let table = |n: &str| match r.get(n) {
            Some(terraflux_ops::Value::Table(t)) => t.clone(),
            other => panic!("{n}: {other:?}"),
        };
        let forest = table("shape.table");
        assert!(!forest.rows.is_empty());
        assert_eq!(table("plots.table").rows.len(), 3 * forest.rows.len());
        let counted = table("plots_per_square.table");
        let count = counted.columns.iter().position(|c| c == "count").unwrap();
        assert_eq!(counted.rows.iter().map(|r| r[count]).sum::<f64>(), (3 * forest.rows.len()) as f64);
        let area = table("forest_area.table");
        let sum = area.columns.iter().position(|c| c == "area_sum").unwrap();
        let cells = forest.columns.iter().position(|c| c == "cells").unwrap();
        let total: f64 = area.rows.iter().map(|r| r[sum]).sum();
        let want: f64 = forest.rows.iter().map(|r| r[cells]).sum();
        assert!((total - want).abs() < 1e-6, "{total} {want}");
    }

    #[test]
    fn the_spatial_analysis_template_runs() {
        let t = super::TEMPLATES.iter().find(|t| t.name.starts_with("Spatial analysis:")).unwrap();
        let m = t.model();
        let r = match terraflux_exec::Engine::default().run(&m, &Default::default(), &mut terraflux_exec::Quiet) {
            Ok(r) => r,
            Err(e) => panic!("{e}"),
        };
        let map = |n: &str| match r.get(n) {
            Some(terraflux_ops::Value::Map(m)) => m.data.to_vec(),
            other => panic!("{n}: {other:?}"),
        };
        assert_eq!(map("count.map").iter().sum::<f64>(), 60.0);
        assert!(map("density.map").iter().sum::<f64>() > 30.0);
        let seen = map("seen.map");
        assert!(seen.iter().any(|v| *v > 0.0) && seen.iter().all(|v| (0.0..=3.0).contains(v)));
        let classes = map("classes.map");
        assert!(classes.iter().all(|c| (1.0..=5.0).contains(c)));
        match r.get("contours.vector") {
            Some(terraflux_ops::Value::Vector(v)) => assert!(v.len() >= 5, "{}", v.len()),
            other => panic!("contours: {other:?}"),
        }
    }

    #[test]
    fn the_tables_template_runs() {
        let t = super::TEMPLATES.iter().find(|t| t.name.starts_with("Tables:")).unwrap();
        let m = t.model();
        let r = terraflux_exec::Engine::default().run(&m, &Default::default(), &mut terraflux_exec::Quiet);
        let r = match r {
            Ok(r) => r,
            Err(e) => panic!("{e}"),
        };
        let table = |n: &str| match r.get(&format!("{n}.table")) {
            Some(terraflux_ops::Value::Table(t)) => t.clone(),
            other => panic!("{n}: {other:?}"),
        };
        let total = table("total");
        assert!(total.columns.ends_with(&["value_per_ha".to_string(), "worth".to_string()]), "{:?}", total.columns);
        assert_eq!(total.rows.len(), 3);
        let short = table("short");
        assert_eq!(short.columns, ["class", "hectares", "edge_per_ha"]);
        assert!(short.rows.len() <= 10);
        let matrix = table("matrix");
        assert_eq!(matrix.columns[0], "from");
        assert!(matrix.columns.iter().any(|c| c == "to_2"), "{:?}", matrix.columns);
    }
}

#[cfg(test)]
mod plot_migration_tests {
    use crate::builder::{PlotLayer, plot_stack};
    use terraflux_graph::{PLOT_OP, Ref, Severity, Source};

    /// Every template's `view.plot` nodes made Plots: the same layers the Viewer drew from their `under`
    /// chains (a plot's `shown` read as what it showed), their tables kept, and nothing of them wrong.
    #[test]
    fn the_templates_plots_become_plots() {
        let r = terraflux_ops::Registry::standard();
        let mut plots = 0;
        for t in super::TEMPLATES {
            // (the model as written: read, it is made Plots already)
            let mut m: terraflux_graph::Model = match t.build {
                Some(f) => f(),
                None => toml::from_str(t.toml).expect("a template is a model"),
            };
            let ids: Vec<String> = m.nodes.iter().filter(|n| n.op == PLOT_OP).map(|n| n.id.clone()).collect();
            // (a plot's shown: what it shows, through the plots it shows)
            let shown = |m: &terraflux_graph::Model, mut r: String| {
                for _ in 0..8 {
                    let Ok(Ref::Port { node, port }) = Ref::parse(&r) else { break };
                    let Some(n) = m.nodes.iter().find(|n| n.id == node && n.op == PLOT_OP) else { break };
                    let _ = port;
                    match ["value", "features", "table"].iter().find_map(|p| n.inputs.get(*p)) {
                        Some(Source::Ref(to)) => r = to.clone(),
                        _ => break,
                    }
                }
                r
            };
            let before: Vec<(String, Vec<(String, f64)>)> = ids
                .iter()
                .map(|id| {
                    let mut layers: Vec<(String, f64)> = plot_stack(&m, id)
                        .into_iter()
                        .map(|l| match l {
                            PlotLayer::Map(n, p, o) => (shown(&m, format!("{n}.{p}")), o as f64),
                            PlotLayer::Features(n, p) => (shown(&m, format!("{n}.{p}")), 1.0),
                        })
                        .collect();
                    // (and its table, drawn as a chart, on top)
                    let n = m.nodes.iter().find(|n| &n.id == id).unwrap();
                    if let Some(Source::Ref(t)) = n.inputs.get("table") {
                        layers.push((shown(&m, t.clone()), 1.0));
                    }
                    (id.clone(), layers)
                })
                .collect();
            m.plots_from_nodes();
            assert!(m.nodes.iter().all(|n| n.op != PLOT_OP), "{}: a plot node left", t.name);
            for (id, layers) in before {
                plots += 1;
                let p = m.plot(&id).unwrap_or_else(|| panic!("{}: no Plot {id}", t.name));
                let after: Vec<(String, f64)> =
                    p.layers.iter().map(|l| (l.from.clone(), l.opacity.unwrap_or(1.0))).collect();
                assert_eq!(after, layers, "{}: the Plot {id}", t.name);
            }
            let wrong: Vec<String> = terraflux_graph::check(&m, &r)
                .into_iter()
                .filter(|i| i.severity == Severity::Error && i.at.starts_with("plots."))
                .map(|i| i.to_string())
                .collect();
            assert!(wrong.is_empty(), "{}: {wrong:?}", t.name);
        }
        assert!(plots > 90, "{plots} plots");
    }

    /// The website's models made Plots: none of their plots left a node, none of the Plots reads what is not there.
    #[test]
    fn the_websites_models_plots_become_plots() {
        let r = terraflux_ops::Registry::standard();
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../website/models");
        let mut files = Vec::new();
        let mut todo = vec![dir];
        while let Some(d) = todo.pop() {
            for e in std::fs::read_dir(&d).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    todo.push(p);
                } else if p.extension().is_some_and(|x| x == "toml") {
                    files.push(p);
                }
            }
        }
        let mut plots = 0;
        for f in &files {
            // (the model as written: read, it is made Plots already)
            let Ok(text) = std::fs::read_to_string(f) else { continue };
            let Ok(mut m) = toml::from_str::<terraflux_graph::Model>(&text) else { continue };
            let n = m.nodes.iter().filter(|n| n.op == PLOT_OP).count();
            m.plots_from_nodes();
            plots += n;
            assert_eq!(m.plots.len(), n, "{}", f.display());
            let wrong: Vec<String> = terraflux_graph::check(&m, &r)
                .into_iter()
                .filter(|i| i.severity == Severity::Error && i.at.starts_with("plots."))
                .map(|i| i.to_string())
                .collect();
            assert!(wrong.is_empty(), "{}: {wrong:?}", f.display());
        }
        assert!(plots > 40, "{plots} plots in {} models", files.len());
    }
}
