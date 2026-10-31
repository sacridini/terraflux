//! The data catalog: the datasets `data.download` fetches, as cards to choose from (what each holds, its
//! resolution, years and coverage, its licence, the paper to cite, its home page), and the choices of the one
//! chosen (its variables, layers, years, scenario...). "Add to the model" adds a `data.download` node so set,
//! on the grid of a map of the model (its `like`) or over a box of longitudes and latitudes, with the dataset's
//! reference in the node's references (the Methods and citations window cites it).

use egui::{Color32, RichText, Sense, Ui, Vec2};
use serde_json::{Value, json};
use terraflux_graph::Type;
use terraflux_ops::Registry;

use crate::app::Session;

/// What a dataset is about (the catalog's filters).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Theme {
    Climate,
    Terrain,
    Soil,
    LandCover,
    Water,
}

impl Theme {
    pub const ALL: [Theme; 5] = [Theme::Climate, Theme::Terrain, Theme::Soil, Theme::LandCover, Theme::Water];

    pub fn name(self) -> &'static str {
        match self {
            Theme::Climate => "climate",
            Theme::Terrain => "terrain",
            Theme::Soil => "soil",
            Theme::LandCover => "land cover",
            Theme::Water => "rivers",
        }
    }
}

/// A dataset of the catalog: a `source` of `data.download`.
pub struct Dataset {
    pub source: &'static str,
    pub name: &'static str,
    pub theme: Theme,
    pub what: &'static str,
    pub resolution: &'static str,
    pub period: &'static str,
    pub coverage: &'static str,
    pub licence: &'static str,
    /// The reference to cite (APA), its DOI's link last.
    pub reference: &'static str,
    pub home: &'static str,
    /// The colours of its card: a ramp, or classes side by side (`classes`).
    pub colours: &'static [[u8; 3]],
    pub classes: bool,
}

const WORLDCLIM_REF: &str = "Fick, S. E., & Hijmans, R. J. (2017). WorldClim 2: new 1-km spatial resolution climate \
     surfaces for global land areas. International Journal of Climatology, 37(12), 4302–4315. \
     https://doi.org/10.1002/joc.5086";
const WORLDCLIM_LICENCE: &str = "free for academic and other non-commercial use; redistribution or commercial use \
     only with permission (worldclim.org)";
const CHELSA_REF: &str = "Karger, D. N., Conrad, O., Böhner, J., Kawohl, T., Kreft, H., Soria-Auza, R. W., \
     Zimmermann, N. E., Linder, H. P., & Kessler, M. (2017). Climatologies at high resolution for the earth's land \
     surface areas. Scientific Data, 4, 170122. https://doi.org/10.1038/sdata.2017.122";
const TEMPERATURE: &[[u8; 3]] = &[[49, 54, 149], [116, 173, 209], [255, 255, 191], [244, 109, 67], [165, 0, 38]];
const VIRIDIS: &[[u8; 3]] = &[[68, 1, 84], [59, 82, 139], [33, 145, 140], [94, 201, 98], [253, 231, 37]];

pub const DATASETS: [Dataset; 8] = [
    Dataset {
        source: "worldclim",
        name: "WorldClim 2.1",
        theme: Theme::Climate,
        what: "The climate of 1970-2000: the 19 bioclimatic variables, monthly temperatures (mean, minimum, \
               maximum), precipitation, solar radiation, wind speed and water vapour pressure, and the elevation.",
        resolution: "10 arc-min (~18 km) to 30 arc-sec (~1 km)",
        period: "1970–2000",
        coverage: "the world's land",
        licence: WORLDCLIM_LICENCE,
        reference: WORLDCLIM_REF,
        home: "https://www.worldclim.org/data/worldclim21.html",
        colours: TEMPERATURE,
        classes: false,
    },
    Dataset {
        source: "worldclim_future",
        name: "WorldClim 2.1 · CMIP6 futures",
        theme: Theme::Climate,
        what: "The climate of the future as CMIP6's climate models project it, downscaled with WorldClim 2.1: the \
               bioclimatic variables and monthly temperatures and precipitation of a model, a scenario (SSP) and \
               a period of 20 years.",
        resolution: "10 arc-min (~18 km) to 30 arc-sec (~1 km)",
        period: "2021–2100, by 20 years",
        coverage: "the world's land",
        licence: WORLDCLIM_LICENCE,
        reference: WORLDCLIM_REF,
        home: "https://www.worldclim.org/data/cmip6/cmip6climate.html",
        colours: &[[116, 173, 209], [255, 255, 191], [253, 174, 97], [215, 48, 39], [103, 0, 13]],
        classes: false,
    },
    Dataset {
        source: "chelsa",
        name: "CHELSA 2.1",
        theme: Theme::Climate,
        what: "The 19 bioclimatic variables of 1981-2010 at 1 km, from a downscaling of the ERA5 reanalysis that \
               accounts for the wind and the terrain (a better precipitation in mountains). Values as scaled \
               integers (× 0.1; temperatures in K × 10).",
        resolution: "30 arc-sec (~1 km)",
        period: "1981–2010",
        coverage: "the world's land",
        licence: "CC0 1.0 (public domain)",
        reference: CHELSA_REF,
        home: "https://chelsa-climate.org",
        colours: VIRIDIS,
        classes: false,
    },
    Dataset {
        source: "chelsa_future",
        name: "CHELSA 2.1 · CMIP6 futures",
        theme: Theme::Climate,
        what: "The 19 bioclimatic variables of the future at 1 km: five CMIP6 models (ISIMIP's), three scenarios \
               (SSP1-2.6, SSP3-7.0, SSP5-8.5) and three periods of 30 years.",
        resolution: "30 arc-sec (~1 km)",
        period: "2011–2100, by 30 years",
        coverage: "the world's land",
        licence: "CC0 1.0 (public domain)",
        reference: CHELSA_REF,
        home: "https://chelsa-climate.org",
        colours: &[[59, 82, 139], [33, 145, 140], [94, 201, 98], [253, 231, 37], [250, 120, 30]],
        classes: false,
    },
    Dataset {
        source: "copernicus_dem",
        name: "Copernicus DEM",
        theme: Theme::Terrain,
        what: "The elevation of the surface (with its forests and buildings), from the TanDEM-X mission's radar \
               (2011-2015): GLO-30 at 30 m or GLO-90 at 90 m, read only over the tiles the grid covers.",
        resolution: "30 m or 90 m",
        period: "2011–2015",
        coverage: "the world",
        licence: "free of charge, worldwide; to be named as its source: © DLR e.V. 2010-2014 and © Airbus Defence \
                  and Space GmbH 2014-2018 provided under COPERNICUS by the European Union and ESA",
        reference: "European Space Agency. Copernicus Global Digital Elevation Model (GLO-30, GLO-90). \
                    https://doi.org/10.5270/ESA-c5d3d65",
        home: "https://dataspace.copernicus.eu/explore-data/data-collections/copernicus-contributing-missions/collections-description/COP-DEM",
        colours: &[[40, 120, 60], [140, 180, 90], [230, 210, 140], [170, 120, 80], [250, 250, 250]],
        classes: false,
    },
    Dataset {
        source: "soilgrids",
        name: "SoilGrids 2.0",
        theme: Theme::Soil,
        what: "The soil's properties predicted at six depths down to 2 m: organic carbon, pH, clay, sand and \
               silt, bulk density, cation exchange capacity, coarse fragments, nitrogen; their mean, median, 5 and \
               95% quantiles and uncertainty.",
        resolution: "250 m",
        period: "the present (profiles of 1950–2018)",
        coverage: "the world's land",
        licence: "CC BY 4.0 (ISRIC)",
        reference: "Poggio, L., de Sousa, L. M., Batjes, N. H., Heuvelink, G. B. M., Kempen, B., Ribeiro, E., & \
                    Rossiter, D. (2021). SoilGrids 2.0: producing soil information for the globe with quantified \
                    spatial uncertainty. SOIL, 7(1), 217–240. https://doi.org/10.5194/soil-7-217-2021",
        home: "https://www.isric.org/explore/soilgrids",
        colours: &[[60, 40, 25], [120, 80, 45], [180, 130, 70], [215, 185, 130], [240, 225, 195]],
        classes: false,
    },
    Dataset {
        source: "mapbiomas",
        name: "MapBiomas Brazil",
        theme: Theme::LandCover,
        what: "Brazil's land use and land cover of every year since 1985 from Landsat: forest, savanna, \
               grassland, pasture, agriculture, plantations, urban areas, water... (MapBiomas' legend: classes \
               with their names and colours).",
        resolution: "30 m",
        period: "every year since 1985",
        coverage: "Brazil",
        licence: "CC BY-SA 4.0 (MapBiomas' terms of use)",
        reference: "Souza, C. M., Jr., Shimbo, J. Z., Rosa, M. R., Parente, L. L., Alencar, A. A., Rudorff, B. F. \
                    T., et al. (2020). Reconstructing three decades of land use and land cover changes in Brazilian \
                    biomes with Landsat archive and Earth Engine. Remote Sensing, 12(17), 2735. \
                    https://doi.org/10.3390/rs12172735",
        home: "https://brasil.mapbiomas.org",
        colours: &[[31, 141, 73], [125, 201, 117], [214, 188, 116], [255, 239, 92], [228, 149, 230], [42, 98, 255]],
        classes: true,
    },
    Dataset {
        source: "hydrography90m",
        name: "Hydrography90m",
        theme: Theme::Water,
        what: "The world's river network at 90 m (from MERIT Hydro): sub-catchments, stream segments, basins, \
               flow direction and accumulation, stream orders (Strahler, Shreve...), slopes, distances to the \
               stream and the outlet, and flow indices, read over the tiles the grid covers.",
        resolution: "3 arc-sec (~90 m)",
        period: "the present",
        coverage: "the world (to 60° S)",
        licence: "CC BY-NC 4.0 (non-commercial) on the dataset's record at IGB Berlin (FRED): check it before \
                  using it otherwise",
        reference: "Amatulli, G., Garcia Marquez, J., Sethi, T., Kiesel, J., Grigoropoulou, A., Üblacker, M. M., \
                    Shen, L. Q., & Domisch, S. (2022). Hydrography90m: a new high-resolution global hydrographic \
                    dataset. Earth System Science Data, 14(10), 4525–4550. https://doi.org/10.5194/essd-14-4525-2022",
        home: "https://hydrography.org/hydrography90m/hydrography90m_layers",
        colours: &[[222, 235, 247], [158, 202, 225], [66, 146, 198], [8, 81, 156], [8, 48, 107]],
        classes: false,
    },
];

/// The bioclimatic variables, BIO1 to BIO19.
pub const BIO: [&str; 19] = [
    "annual mean temperature",
    "mean diurnal range",
    "isothermality",
    "temperature seasonality",
    "max temperature of the warmest month",
    "min temperature of the coldest month",
    "temperature annual range",
    "mean temperature of the wettest quarter",
    "mean temperature of the driest quarter",
    "mean temperature of the warmest quarter",
    "mean temperature of the coldest quarter",
    "annual precipitation",
    "precipitation of the wettest month",
    "precipitation of the driest month",
    "precipitation seasonality",
    "precipitation of the wettest quarter",
    "precipitation of the driest quarter",
    "precipitation of the warmest quarter",
    "precipitation of the coldest quarter",
];

const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

const WORLDCLIM_VARIABLES: [(&str, &str); 9] = [
    ("bio", "the 19 bioclimatic variables"),
    ("tavg", "mean temperature (°C), monthly"),
    ("tmin", "minimum temperature (°C), monthly"),
    ("tmax", "maximum temperature (°C), monthly"),
    ("prec", "precipitation (mm), monthly"),
    ("srad", "solar radiation (kJ/m²/day), monthly"),
    ("wind", "wind speed (m/s), monthly"),
    ("vapr", "water vapour pressure (kPa), monthly"),
    ("elev", "elevation (m)"),
];
const FUTURE_VARIABLES: [(&str, &str); 4] = [
    ("bioc", "the 19 bioclimatic variables"),
    ("tmin", "minimum temperature (°C), monthly"),
    ("tmax", "maximum temperature (°C), monthly"),
    ("prec", "precipitation (mm), monthly"),
];
const RESOLUTIONS: [(&str, &str); 4] = [
    ("10m", "10 arc-min (~18 km)"),
    ("5m", "5 arc-min (~9 km)"),
    ("2.5m", "2.5 arc-min (~4.5 km)"),
    ("30s", "30 arc-sec (~1 km)"),
];
const WORLDCLIM_GCMS: [&str; 13] = [
    "ACCESS-CM2",
    "BCC-CSM2-MR",
    "CMCC-ESM2",
    "EC-Earth3-Veg",
    "FIO-ESM-2-0",
    "GISS-E2-1-G",
    "HadGEM3-GC31-LL",
    "INM-CM5-0",
    "IPSL-CM6A-LR",
    "MIROC6",
    "MPI-ESM1-2-HR",
    "MRI-ESM2-0",
    "UKESM1-0-LL",
];
const CHELSA_GCMS: [&str; 5] = ["GFDL-ESM4", "IPSL-CM6A-LR", "MPI-ESM1-2-HR", "MRI-ESM2-0", "UKESM1-0-LL"];
const WORLDCLIM_SSPS: [&str; 4] = ["ssp126", "ssp245", "ssp370", "ssp585"];
const CHELSA_SSPS: [&str; 3] = ["ssp126", "ssp370", "ssp585"];
const WORLDCLIM_PERIODS: [&str; 4] = ["2021-2040", "2041-2060", "2061-2080", "2081-2100"];
const CHELSA_PERIODS: [&str; 3] = ["2011-2040", "2041-2070", "2071-2100"];
const SOIL_PROPERTIES: [(&str, &str); 10] = [
    ("soc", "soil organic carbon (dg/kg)"),
    ("phh2o", "pH in water (pH × 10)"),
    ("clay", "clay (g/kg)"),
    ("sand", "sand (g/kg)"),
    ("silt", "silt (g/kg)"),
    ("bdod", "bulk density (cg/cm³)"),
    ("cec", "cation exchange capacity (mmol(c)/kg)"),
    ("cfvo", "coarse fragments (cm³/dm³)"),
    ("nitrogen", "nitrogen (cg/kg)"),
    ("ocd", "organic carbon density (hg/m³)"),
];
const DEPTHS: [&str; 6] = ["0-5cm", "5-15cm", "15-30cm", "30-60cm", "60-100cm", "100-200cm"];
const STATS: [(&str, &str); 5] = [
    ("mean", "mean"),
    ("Q0.5", "median"),
    ("Q0.05", "5% quantile"),
    ("Q0.95", "95% quantile"),
    ("uncertainty", "uncertainty"),
];

/// The years of a MapBiomas collection.
pub fn mapbiomas_years(collection: u32) -> std::ops::RangeInclusive<u32> {
    match collection {
        9 => 1985..=2023,
        10 => 1985..=2024,
        _ => 1985..=2025,
    }
}

/// Years written as a list and ranges: "1985-2000, 2010, 2023" (each in `range`).
pub fn parse_years(text: &str, range: std::ops::RangeInclusive<u32>) -> Result<Vec<u32>, String> {
    let mut out = Vec::new();
    for part in text.split([',', ';', ' ']).map(str::trim).filter(|p| !p.is_empty()) {
        let year = |t: &str| t.trim().parse::<u32>().map_err(|_| format!("{t:?} is not a year"));
        let (a, b) = match part.split_once('-') {
            Some((a, b)) => (year(a)?, year(b)?),
            None => (year(part)?, year(part)?),
        };
        if a > b {
            return Err(format!("{part}: from the first year to the last"));
        }
        for y in [a, b] {
            if !range.contains(&y) {
                return Err(format!("{y}: the years are {} to {}", range.start(), range.end()));
            }
        }
        out.extend(a..=b);
    }
    if out.is_empty() {
        return Err("no year".into());
    }
    out.sort_unstable();
    out.dedup();
    Ok(out)
}

/// The choices of the dataset chosen, and the grid to fill.
#[derive(Clone, Debug)]
pub struct Choices {
    pub variable: String,
    pub future_variable: String,
    pub bio: [bool; 19],
    pub months: [bool; 12],
    pub resolution: String,
    pub gcm: String,
    pub chelsa_gcm: String,
    pub ssp: String,
    pub chelsa_ssp: String,
    pub period: String,
    pub chelsa_period: String,
    pub dem_resolution: u32,
    pub property: String,
    pub depths: [bool; 6],
    pub stat: String,
    pub collection: u32,
    pub years: String,
    pub hydro: String,
    /// The map whose grid to fill (`node.port`); none: the box below.
    pub like: Option<String>,
    /// [west, south, east, north], degrees.
    pub extent: [f64; 4],
    pub cellsize: f64,
}

impl Default for Choices {
    fn default() -> Self {
        let mut bio = [false; 19];
        bio[0] = true;
        bio[11] = true;
        let mut depths = [false; 6];
        depths[0] = true;
        Choices {
            variable: "bio".into(),
            future_variable: "bioc".into(),
            bio,
            months: [true; 12],
            resolution: "10m".into(),
            gcm: "MPI-ESM1-2-HR".into(),
            chelsa_gcm: "MPI-ESM1-2-HR".into(),
            ssp: "ssp245".into(),
            chelsa_ssp: "ssp370".into(),
            period: "2041-2060".into(),
            chelsa_period: "2041-2070".into(),
            dem_resolution: 90,
            property: "soc".into(),
            depths,
            stat: "mean".into(),
            collection: 10,
            years: "2024".into(),
            hydro: "sub_catchment".into(),
            like: None,
            extent: [-48.0, -16.0, -47.0, -15.0],
            cellsize: 1.0 / 120.0,
        }
    }
}

/// The numbers of the boxes ticked (from 1), none when all are (`data.download` takes them all by default).
fn ticked(on: &[bool]) -> Result<Option<Vec<u32>>, String> {
    let n: Vec<u32> = on.iter().enumerate().filter(|(_, t)| **t).map(|(k, _)| k as u32 + 1).collect();
    match n.len() {
        0 => Err("no layer ticked".into()),
        k if k == on.len() => Ok(None),
        _ => Ok(Some(n)),
    }
}

/// The settings of a `data.download` node for a dataset and its choices (the grid's apart).
pub fn params(d: &Dataset, c: &Choices) -> Result<Vec<(&'static str, Value)>, String> {
    let mut p: Vec<(&'static str, Value)> = vec![("source", json!(d.source))];
    let layers = |p: &mut Vec<(&'static str, Value)>, on: &[bool]| -> Result<(), String> {
        if let Some(l) = ticked(on)? {
            p.push(("layers", json!(l)));
        }
        Ok(())
    };
    match d.source {
        "worldclim" => {
            p.push(("variable", json!(c.variable)));
            match c.variable.as_str() {
                "bio" => layers(&mut p, &c.bio)?,
                "elev" => {}
                _ => layers(&mut p, &c.months)?,
            }
            p.push(("resolution", json!(c.resolution)));
        }
        "worldclim_future" => {
            p.push(("variable", json!(c.future_variable)));
            layers(&mut p, if c.future_variable == "bioc" { &c.bio[..] } else { &c.months[..] })?;
            p.push(("resolution", json!(c.resolution)));
            p.push(("gcm", json!(c.gcm)));
            p.push(("ssp", json!(c.ssp)));
            p.push(("period", json!(c.period)));
        }
        "chelsa" => layers(&mut p, &c.bio)?,
        "chelsa_future" => {
            layers(&mut p, &c.bio)?;
            p.push(("gcm", json!(c.chelsa_gcm)));
            p.push(("ssp", json!(c.chelsa_ssp)));
            p.push(("period", json!(c.chelsa_period)));
        }
        "copernicus_dem" => p.push(("dem_resolution", json!(c.dem_resolution))),
        "soilgrids" => {
            let depths: Vec<&str> = DEPTHS.iter().zip(c.depths).filter(|(_, on)| *on).map(|(d, _)| *d).collect();
            if depths.is_empty() {
                return Err("no depth ticked".into());
            }
            p.push(("property", json!(c.property)));
            p.push(("depth", if depths.len() == 1 { json!(depths[0]) } else { json!(depths) }));
            p.push(("stat", json!(c.stat)));
        }
        "mapbiomas" => {
            let years = parse_years(&c.years, mapbiomas_years(c.collection))?;
            p.push(("collection", json!(c.collection)));
            p.push(("years", if years.len() == 1 { json!(years[0]) } else { json!(years) }));
        }
        "hydrography90m" => p.push(("variable", json!(c.hydro))),
        s => return Err(format!("{s}: not in the catalog")),
    }
    if c.like.is_none() {
        let [w, s, e, n] = c.extent;
        if e <= w || n <= s || c.cellsize.is_nan() || c.cellsize <= 0.0 {
            return Err("the box: west below east, south below north, and cells above 0".into());
        }
        p.push(("extent", json!(c.extent)));
        p.push(("cellsize", json!(c.cellsize)));
    }
    Ok(p)
}

/// What the choices fetch, in words (the node's note).
pub fn summary(d: &Dataset, c: &Choices) -> String {
    let some = |on: &[bool], names: &dyn Fn(usize) -> String| {
        let n: Vec<String> = on.iter().enumerate().filter(|(_, t)| **t).map(|(k, _)| names(k)).collect();
        if n.len() == on.len() { "all".to_string() } else { n.join(", ") }
    };
    let bio = some(&c.bio, &|k| format!("BIO{}", k + 1));
    let months = some(&c.months, &|k| MONTHS[k].to_string());
    let what = match d.source {
        "worldclim" => match c.variable.as_str() {
            "bio" => format!("bioclimatic variables {bio}, {}", c.resolution),
            "elev" => format!("elevation, {}", c.resolution),
            v => format!("{v} of months {months}, {}", c.resolution),
        },
        "worldclim_future" => {
            let l = if c.future_variable == "bioc" { bio } else { months };
            format!("{} {l}, {} {} {}, {}", c.future_variable, c.gcm, c.ssp, c.period, c.resolution)
        }
        "chelsa" => format!("bioclimatic variables {bio}"),
        "chelsa_future" => {
            format!("bioclimatic variables {bio}, {} {} {}", c.chelsa_gcm, c.chelsa_ssp, c.chelsa_period)
        }
        "copernicus_dem" => format!("elevation, GLO-{}", c.dem_resolution),
        "soilgrids" => {
            let d: Vec<&str> = DEPTHS.iter().zip(c.depths).filter(|(_, on)| *on).map(|(d, _)| *d).collect();
            format!("{} at {} ({})", c.property, d.join(", "), c.stat)
        }
        "mapbiomas" => format!("collection {}, {}", c.collection, c.years.trim()),
        "hydrography90m" => c.hydro.clone(),
        _ => String::new(),
    };
    format!("{}: {what}", d.name)
}

/// The window's state.
#[derive(Default)]
pub struct Catalog {
    pub open: bool,
    pub query: String,
    pub theme: Option<Theme>,
    /// The dataset chosen (its index in [`DATASETS`]).
    pub chosen: usize,
    pub choices: Choices,
    pub said: Option<Result<String, String>>,
    /// Whether the grid was set to the model's first map (once).
    pub seen: bool,
}

/// The datasets a search and a theme keep.
pub fn matching(query: &str, theme: Option<Theme>) -> Vec<usize> {
    let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    DATASETS
        .iter()
        .enumerate()
        .filter(|(_, d)| theme.is_none_or(|t| d.theme == t))
        .filter(|(_, d)| {
            let all = format!("{} {} {} {} {}", d.name, d.what, d.coverage, d.theme.name(), d.source).to_lowercase();
            words.iter().all(|w| all.contains(w.as_str()))
        })
        .map(|(k, _)| k)
        .collect()
}

/// The maps of the model (of the scope edited) whose grid a download can fill: `node.port`.
pub fn grids(s: &Session, r: &Registry) -> Vec<String> {
    let path = s.editor.path.clone();
    let nodes = s.source.scope(&path).cloned().unwrap_or_default();
    let mut out = Vec::new();
    for n in &nodes {
        for (port, ty) in crate::editor::output_ports(n, r) {
            if matches!(ty, Type::Map | Type::Stack | Type::Raster) {
                out.push(format!("{}.{port}", n.id));
            }
        }
    }
    out
}

/// A `data.download` node of the dataset and its choices added to the model (in the scope edited), its `like`
/// connected, the reference in its references; its id.
pub fn add(s: &mut Session, r: &Registry, d: &Dataset, c: &Choices) -> Result<String, String> {
    let params = params(d, c)?;
    let path = s.editor.path.clone();
    let nodes = s.source.scope(&path).cloned().unwrap_or_default();
    let from = c.like.as_ref().and_then(|l| l.rsplit_once('.')).map(|(n, _)| n.to_string());
    let pos = crate::builder::beside(&nodes, from.as_deref());
    let id = crate::builder::add_at(s, r, "data.download", &[], Some(pos))?;
    if let Some(l) = &c.like {
        crate::editor::connect(&mut s.source, r, &path, l, &id, "like").map_err(|e| e.to_string())?;
    }
    if let Some(n) = s.source.scope_mut(&path).and_then(|v| v.iter_mut().find(|n| n.id == id)) {
        for (k, v) in params {
            n.params.insert(k.to_string(), v);
        }
        n.note = Some(summary(d, c));
        n.why = Some(format!(
            "From the data catalog: {} ({}, {}, {}). Licence: {}.",
            d.name, d.resolution, d.period, d.coverage, d.licence
        ));
        n.refs = vec![d.reference.to_string()];
    }
    s.edited();
    crate::builder::select(s, r, &id);
    Ok(id)
}

/// The colours of a dataset's card: a ramp, or its classes side by side.
fn swatch(ui: &mut Ui, d: &Dataset, size: Vec2) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let p = ui.painter_at(rect);
    let c = |k: usize| {
        let [r, g, b] = d.colours[k];
        Color32::from_rgb(r, g, b)
    };
    let n = d.colours.len();
    if d.classes {
        let w = rect.width() / n as f32;
        for k in 0..n {
            let r = egui::Rect::from_min_size(rect.min + Vec2::new(w * k as f32, 0.0), Vec2::new(w + 0.5, size.y));
            p.rect_filled(r, 0.0, c(k));
        }
    } else {
        let steps = 32;
        let w = rect.width() / steps as f32;
        for s in 0..steps {
            let t = s as f32 / (steps - 1) as f32 * (n - 1) as f32;
            let (k, f) = ((t.floor() as usize).min(n - 2), t - t.floor().min((n - 2) as f32));
            let r = egui::Rect::from_min_size(rect.min + Vec2::new(w * s as f32, 0.0), Vec2::new(w + 0.5, size.y));
            p.rect_filled(r, 0.0, c(k).lerp_to_gamma(c(k + 1), f));
        }
    }
    p.rect_stroke(
        rect,
        4.0,
        egui::Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color),
        egui::StrokeKind::Inside,
    );
    response
}

fn combo(ui: &mut Ui, salt: &str, value: &mut String, options: &[(&str, &str)]) {
    let shown = options.iter().find(|(k, _)| *k == value.as_str()).map_or(value.clone(), |(_, n)| n.to_string());
    egui::ComboBox::from_id_salt(salt).selected_text(shown).width(250.0).show_ui(ui, |ui| {
        for (k, name) in options {
            ui.selectable_value(value, k.to_string(), *name);
        }
    });
}

fn plain(options: &[&'static str]) -> Vec<(&'static str, &'static str)> {
    options.iter().map(|o| (*o, *o)).collect()
}

/// The boxes of the bioclimatic variables (their names on hover), with all and none.
fn bio_boxes(ui: &mut Ui, on: &mut [bool; 19]) {
    ui.horizontal(|ui| {
        ui.label("Variables");
        if ui.small_button("all").clicked() {
            *on = [true; 19];
        }
        if ui.small_button("none").clicked() {
            *on = [false; 19];
        }
        if ui.small_button("temperature").on_hover_text("BIO1 to BIO11").clicked() {
            *on = std::array::from_fn(|k| k < 11);
        }
        if ui.small_button("precipitation").on_hover_text("BIO12 to BIO19").clicked() {
            *on = std::array::from_fn(|k| k >= 11);
        }
    });
    egui::Grid::new("catalog bio").num_columns(2).spacing([12.0, 0.0]).show(ui, |ui| {
        for k in 0..10 {
            for j in [k, k + 10] {
                if j < 19 {
                    ui.checkbox(&mut on[j], format!("BIO{} {}", j + 1, BIO[j]));
                }
            }
            ui.end_row();
        }
    });
}

fn month_boxes(ui: &mut Ui, on: &mut [bool; 12]) {
    ui.horizontal_wrapped(|ui| {
        ui.label("Months");
        for (k, m) in MONTHS.iter().enumerate() {
            ui.checkbox(&mut on[k], *m);
        }
    });
}

/// The choices of a dataset.
fn form(ui: &mut Ui, d: &Dataset, c: &mut Choices) {
    let row = |ui: &mut Ui, label: &str, add: &mut dyn FnMut(&mut Ui)| {
        ui.horizontal(|ui| {
            ui.add_sized([86.0, 18.0], egui::Label::new(label));
            add(ui);
        });
    };
    match d.source {
        "worldclim" => {
            row(ui, "Variable", &mut |ui| combo(ui, "catalog variable", &mut c.variable, &WORLDCLIM_VARIABLES));
            row(ui, "Resolution", &mut |ui| combo(ui, "catalog resolution", &mut c.resolution, &RESOLUTIONS));
            match c.variable.as_str() {
                "bio" => bio_boxes(ui, &mut c.bio),
                "elev" => {}
                _ => month_boxes(ui, &mut c.months),
            }
        }
        "worldclim_future" => {
            row(ui, "Variable", &mut |ui| combo(ui, "catalog fvariable", &mut c.future_variable, &FUTURE_VARIABLES));
            row(ui, "Resolution", &mut |ui| combo(ui, "catalog resolution", &mut c.resolution, &RESOLUTIONS));
            row(ui, "Model", &mut |ui| combo(ui, "catalog gcm", &mut c.gcm, &plain(&WORLDCLIM_GCMS)));
            row(ui, "Scenario", &mut |ui| combo(ui, "catalog ssp", &mut c.ssp, &plain(&WORLDCLIM_SSPS)));
            row(ui, "Period", &mut |ui| combo(ui, "catalog period", &mut c.period, &plain(&WORLDCLIM_PERIODS)));
            if c.future_variable == "bioc" {
                bio_boxes(ui, &mut c.bio);
            } else {
                month_boxes(ui, &mut c.months);
            }
        }
        "chelsa" => bio_boxes(ui, &mut c.bio),
        "chelsa_future" => {
            row(ui, "Model", &mut |ui| combo(ui, "catalog cgcm", &mut c.chelsa_gcm, &plain(&CHELSA_GCMS)));
            row(ui, "Scenario", &mut |ui| combo(ui, "catalog cssp", &mut c.chelsa_ssp, &plain(&CHELSA_SSPS)));
            row(ui, "Period", &mut |ui| combo(ui, "catalog cperiod", &mut c.chelsa_period, &plain(&CHELSA_PERIODS)));
            bio_boxes(ui, &mut c.bio);
        }
        "copernicus_dem" => row(ui, "Resolution", &mut |ui| {
            ui.radio_value(&mut c.dem_resolution, 30, "GLO-30 (30 m)");
            ui.radio_value(&mut c.dem_resolution, 90, "GLO-90 (90 m)");
        }),
        "soilgrids" => {
            row(ui, "Property", &mut |ui| combo(ui, "catalog property", &mut c.property, &SOIL_PROPERTIES));
            row(ui, "Statistic", &mut |ui| combo(ui, "catalog stat", &mut c.stat, &STATS));
            row(ui, "Depths", &mut |ui| {
                ui.horizontal_wrapped(|ui| {
                    for (k, d) in DEPTHS.iter().enumerate() {
                        ui.checkbox(&mut c.depths[k], *d);
                    }
                });
            });
        }
        "mapbiomas" => {
            row(ui, "Collection", &mut |ui| {
                for k in [9, 10, 11] {
                    ui.radio_value(&mut c.collection, k, k.to_string());
                }
            });
            let range = mapbiomas_years(c.collection);
            row(ui, "Years", &mut |ui| {
                ui.add(egui::TextEdit::singleline(&mut c.years).hint_text("1985-2020, 2024").desired_width(150.0));
                ui.label(RichText::new(format!("{}–{}", range.start(), range.end())).weak());
            });
        }
        "hydrography90m" => {
            let shown = c.hydro.clone();
            row(ui, "Variable", &mut |ui| {
                egui::ComboBox::from_id_salt("catalog hydro").selected_text(&shown).width(250.0).show_ui(ui, |ui| {
                    for (group, vars) in terraflux_ops::data::H90M_VARIABLES {
                        ui.label(RichText::new(group).weak().size(11.0));
                        for v in vars {
                            ui.selectable_value(&mut c.hydro, v.to_string(), *v);
                        }
                    }
                });
            });
        }
        _ => {}
    }
}

/// Where the data goes: the grid of a map of the model, or a box of longitudes and latitudes.
fn grid_form(ui: &mut Ui, c: &mut Choices, maps: &[String]) {
    ui.label(RichText::new("ON THE GRID OF").size(10.5).strong().color(ui.visuals().weak_text_color()));
    let shown = c.like.clone().unwrap_or_else(|| "a box of longitudes and latitudes".into());
    egui::ComboBox::from_id_salt("catalog like").selected_text(shown).width(250.0).show_ui(ui, |ui| {
        for m in maps {
            ui.selectable_value(&mut c.like, Some(m.clone()), m);
        }
        ui.selectable_value(&mut c.like, None, "a box of longitudes and latitudes");
    });
    if c.like.is_some() {
        ui.label(
            RichText::new("its extent, cells and coordinate system: the data warped onto it, cell for cell")
                .weak()
                .size(11.0),
        );
        return;
    }
    let [w, s, e, n] = &mut c.extent;
    ui.horizontal(|ui| {
        ui.label("west");
        ui.add(egui::DragValue::new(w).range(-180.0..=180.0).speed(0.05).suffix("°"));
        ui.label("east");
        ui.add(egui::DragValue::new(e).range(-180.0..=180.0).speed(0.05).suffix("°"));
    });
    ui.horizontal(|ui| {
        ui.label("south");
        ui.add(egui::DragValue::new(s).range(-90.0..=90.0).speed(0.05).suffix("°"));
        ui.label("north");
        ui.add(egui::DragValue::new(n).range(-90.0..=90.0).speed(0.05).suffix("°"));
    });
    ui.horizontal(|ui| {
        ui.label("cells");
        egui::ComboBox::from_id_salt("catalog cells").selected_text(cell_name(c.cellsize)).show_ui(ui, |ui| {
            for (v, _) in CELLS {
                ui.selectable_value(&mut c.cellsize, v, cell_name(v));
            }
        });
        let (cols, rows) = (((*e - *w) / c.cellsize).round(), ((*n - *s) / c.cellsize).round());
        if cols > 0.0 && rows > 0.0 {
            ui.label(RichText::new(format!("{cols} × {rows} cells")).weak());
        }
    });
}

const CELLS: [(f64, &str); 5] = [
    (1.0 / 6.0, "10 arc-min (~18 km)"),
    (1.0 / 12.0, "5 arc-min (~9 km)"),
    (1.0 / 24.0, "2.5 arc-min (~4.5 km)"),
    (1.0 / 120.0, "30 arc-sec (~1 km)"),
    (1.0 / 1200.0, "3 arc-sec (~90 m)"),
];

fn cell_name(v: f64) -> String {
    CELLS.iter().find(|(c, _)| (c - v).abs() < 1e-12).map_or(format!("{v}°"), |(_, n)| n.to_string())
}

/// The catalog's window.
pub fn window(ctx: &egui::Context, s: &mut Session, r: &Registry) {
    if !s.catalog.open {
        return;
    }
    let maps = grids(s, r);
    // a map gone: the box; the grid of the model's first map by default (once)
    if s.catalog.choices.like.as_ref().is_some_and(|l| !maps.contains(l)) {
        s.catalog.choices.like = None;
    }
    if !s.catalog.seen && !maps.is_empty() {
        s.catalog.seen = true;
        s.catalog.choices.like = maps.first().cloned();
    }
    let mut open = true;
    let mut add_it = false;
    egui::Window::new("Data catalog")
        .open(&mut open)
        .default_size([820.0, 600.0])
        .resizable(true)
        .collapsible(false)
        .show(ctx, |ui| {
            let cat = &mut s.catalog;
            egui::Panel::left("catalog list").resizable(true).default_size(330.0).show(ui, |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut cat.query)
                        .hint_text("search: rain, soil, Brazil, future...")
                        .desired_width(f32::INFINITY),
                );
                ui.horizontal_wrapped(|ui| {
                    if ui.selectable_label(cat.theme.is_none(), "all").clicked() {
                        cat.theme = None;
                    }
                    for t in Theme::ALL {
                        if ui.selectable_label(cat.theme == Some(t), t.name()).clicked() {
                            cat.theme = if cat.theme == Some(t) { None } else { Some(t) };
                        }
                    }
                });
                ui.add_space(4.0);
                let shown = matching(&cat.query, cat.theme);
                if shown.is_empty() {
                    ui.label(RichText::new("no dataset matches").weak());
                }
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    for k in shown {
                        let d = &DATASETS[k];
                        let chosen = cat.chosen == k;
                        let frame = egui::Frame::new()
                            .fill(if chosen {
                                ui.visuals().selection.bg_fill.gamma_multiply(0.35)
                            } else {
                                ui.visuals().faint_bg_color
                            })
                            .stroke(egui::Stroke::new(
                                1.0,
                                if chosen {
                                    ui.visuals().selection.stroke.color
                                } else {
                                    ui.visuals().widgets.noninteractive.bg_stroke.color
                                },
                            ))
                            .corner_radius(8)
                            .inner_margin(egui::Margin::same(8));
                        let r = frame
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.horizontal(|ui| {
                                    swatch(ui, d, Vec2::new(54.0, 40.0));
                                    ui.vertical(|ui| {
                                        ui.label(RichText::new(d.name).strong());
                                        ui.label(
                                            RichText::new(format!("{} · {} · {}", d.resolution, d.period, d.coverage))
                                                .size(11.0)
                                                .weak(),
                                        );
                                        ui.label(
                                            RichText::new(d.theme.name())
                                                .size(10.5)
                                                .color(ui.visuals().hyperlink_color),
                                        );
                                    });
                                });
                            })
                            .response
                            .interact(Sense::click());
                        if r.clicked() {
                            cat.chosen = k;
                            cat.said = None;
                        }
                        ui.add_space(4.0);
                    }
                });
            });
            let d = &DATASETS[cat.chosen.min(DATASETS.len() - 1)];
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                ui.horizontal(|ui| {
                    swatch(ui, d, Vec2::new(40.0, 28.0));
                    ui.heading(d.name);
                });
                ui.label(d.what);
                ui.add_space(4.0);
                egui::Grid::new("catalog facts").num_columns(2).spacing([10.0, 3.0]).show(ui, |ui| {
                    let weak = ui.visuals().weak_text_color();
                    for (k, v) in [
                        ("resolution", d.resolution),
                        ("years", d.period),
                        ("coverage", d.coverage),
                        ("licence", d.licence),
                    ] {
                        ui.label(RichText::new(k).color(weak));
                        ui.add(egui::Label::new(v).wrap());
                        ui.end_row();
                    }
                    ui.label(RichText::new("cite").color(weak));
                    ui.vertical(|ui| {
                        ui.add(egui::Label::new(RichText::new(d.reference).size(11.5)).wrap());
                        ui.horizontal(|ui| {
                            if ui.small_button("copy").on_hover_text("the reference, to the clipboard").clicked() {
                                ui.ctx().copy_text(d.reference.to_string());
                            }
                            ui.hyperlink_to("home page", d.home);
                        });
                    });
                    ui.end_row();
                });
                ui.separator();
                ui.label(RichText::new("WHAT TO FETCH").size(10.5).strong().color(ui.visuals().weak_text_color()));
                form(ui, d, &mut cat.choices);
                ui.add_space(6.0);
                grid_form(ui, &mut cat.choices, &maps);
                ui.add_space(8.0);
                let ready = params(d, &cat.choices);
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(ready.is_ok(), egui::Button::new(RichText::new("Add to the model").strong()))
                        .on_hover_text(
                            "a data.download node so set: it fetches the data when it runs (kept in the cache)",
                        )
                        .clicked()
                    {
                        add_it = true;
                    }
                    if let Err(e) = &ready {
                        ui.colored_label(ui.visuals().warn_fg_color, e);
                    }
                });
                match &cat.said {
                    Some(Ok(m)) => {
                        ui.label(RichText::new(m).size(11.5));
                    }
                    Some(Err(e)) => {
                        ui.colored_label(ui.visuals().error_fg_color, e);
                    }
                    None => {}
                }
                ui.label(
                    RichText::new(
                        "Only the part the grid covers is read from the servers, once: the files stay in Terraflux's \
                         cache. The reference goes with the node (Methods and citations cites it).",
                    )
                    .weak()
                    .size(11.0),
                );
            });
        });
    if add_it {
        let d = &DATASETS[s.catalog.chosen.min(DATASETS.len() - 1)];
        let c = s.catalog.choices.clone();
        s.catalog.said = Some(add(s, r, d, &c).map(|id| format!("added: {id} ({})", summary(d, &c))));
    }
    if !open {
        s.catalog.open = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_dataset_is_a_source_of_the_download() {
        let r = Registry::standard();
        let sig = r.sig("data.download").unwrap();
        let source = sig.params.iter().find(|p| p.name == "source").unwrap();
        for d in &DATASETS {
            assert!(source.choices.iter().any(|c| c == d.source), "{}", d.source);
            assert!(d.reference.contains("https://doi.org/10."), "{}", d.name);
            assert!(d.home.starts_with("https://") && d.colours.len() >= 2);
        }
        // every source of the download in the catalog
        assert_eq!(source.choices.len(), DATASETS.len());
    }

    #[test]
    fn the_choices_as_settings() {
        let c = Choices::default();
        let p = params(&DATASETS[0], &c).unwrap();
        assert_eq!(
            p,
            vec![
                ("source", json!("worldclim")),
                ("variable", json!("bio")),
                ("layers", json!([1, 12])),
                ("resolution", json!("10m")),
                ("extent", json!([-48.0, -16.0, -47.0, -15.0])),
                ("cellsize", json!(1.0 / 120.0)),
            ]
        );
        // all the layers: none named (the default); on a map's grid: no box
        let mut c = Choices { bio: [true; 19], like: Some("dem.raster".into()), ..Default::default() };
        let p = params(&DATASETS[2], &c).unwrap();
        assert_eq!(p, vec![("source", json!("chelsa"))]);
        c.bio = [false; 19];
        assert!(params(&DATASETS[2], &c).is_err());
        // a scenario as a text (not read as a number)
        c.bio[4] = true;
        let p = params(&DATASETS[1], &c).unwrap();
        assert!(p.contains(&("ssp", json!("ssp245"))) && p.contains(&("layers", json!([5]))));
        // depths, years
        let mut c = Choices { like: Some("x.map".into()), ..Default::default() };
        c.depths[2] = true;
        assert!(params(&DATASETS[5], &c).unwrap().contains(&("depth", json!(["0-5cm", "15-30cm"]))));
        c.years = "2000-2002, 2024".into();
        assert!(params(&DATASETS[6], &c).unwrap().contains(&("years", json!([2000, 2001, 2002, 2024]))));
        c.years = "1970".into();
        assert!(params(&DATASETS[6], &c).is_err());
        assert_eq!(summary(&DATASETS[6], &Choices::default()), "MapBiomas Brazil: collection 10, 2024");
    }

    #[test]
    fn years_and_searches() {
        assert_eq!(parse_years("2024, 1985-1987;1986", 1985..=2024).unwrap(), vec![1985, 1986, 1987, 2024]);
        assert!(parse_years("2001-2000", 1985..=2024).is_err());
        assert!(parse_years("", 1985..=2024).is_err());
        assert!(parse_years("year", 1985..=2024).is_err());
        let names = |q: &str, t| matching(q, t).into_iter().map(|k| DATASETS[k].source).collect::<Vec<_>>();
        assert_eq!(names("brazil", None), ["mapbiomas"]);
        assert_eq!(names("", Some(Theme::Soil)), ["soilgrids"]);
        assert_eq!(names("CMIP6", Some(Theme::Climate)), ["worldclim_future", "chelsa_future"]);
        assert_eq!(names("precipitation", None), ["worldclim", "worldclim_future", "chelsa"]);
    }
}
