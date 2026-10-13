//! Landscape patterns from landschaft's port of GuidosToolbox: the Morphological Spatial Pattern Analysis
//! (`landscape.mspa`, Soille & Vogt 2009), the same bits as GTB's `mspa` executable; the simplified pattern
//! analysis (`landscape.spa`) and the objects by size (`landscape.accounting`), as GTB's; the parcels of every
//! class (`landscape.parcellation`), the contortion of the patches (`landscape.contortion`) and the Euclidean
//! distance to the foreground's edge (`distance.euclidean`).
//!
//! GTB reads maps coded 0 missing, 1 background, 2 foreground (and 3, 4 special backgrounds where it takes them):
//! the operators take such a map, or any map with the foreground's classes given ([`gtb_coded`]). What they give
//! is in GTB's codes, named and coloured by [`Product`].

use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_ls_sys::ffi;

use crate::gtb_palettes;
use crate::{Args, Ctx, Error, Map, Operator, Outputs, Registry, Result, Table, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Mspa));
    r.add(Arc::new(Spa));
    r.add(Arc::new(Accounting));
    r.add(Arc::new(Parcellation));
    r.add(Arc::new(Contortion));
    r.add(Arc::new(EuclideanDistance));
    r.add(Arc::new(Influence { proximity: false }));
    r.add(Arc::new(Influence { proximity: true }));
}

/// The classes of an MSPA map (internal features, inside core-openings, carry 100 more).
pub const MSPA_CLASSES: [(u8, &str); 15] = [
    (0, "Background"),
    (1, "Branch"),
    (3, "Edge"),
    (5, "Perforation"),
    (9, "Islet"),
    (17, "Core"),
    (33, "Bridge"),
    (35, "Bridge in Edge"),
    (37, "Bridge in Perforation"),
    (65, "Loop"),
    (67, "Loop in Edge"),
    (69, "Loop in Perforation"),
    (100, "Core-Opening"),
    (129, "Missing"),
    (220, "Border-Opening"),
];

/// The classes of an SPA map.
pub const SPA_CLASSES: [(u8, &str); 8] = [
    (0, "Other background"),
    (1, "Margin (SPA2: small linear features)"),
    (3, "Edge"),
    (5, "Perforation"),
    (9, "Islet"),
    (17, "Core (SPA2: contiguous)"),
    (100, "Core-Opening"),
    (129, "Missing"),
];

/// The codes of Accounting's map: its size classes 1 to 6 in GTB's colours, the three largest objects, the
/// backgrounds and the missing cells.
pub const ACCOUNTING_CLASSES: [(u8, &str); 11] = [
    (0, "Background"),
    (103, "Size class 1"),
    (33, "Size class 2"),
    (65, "Size class 3"),
    (1, "Size class 4"),
    (9, "Size class 5"),
    (17, "Size class 6"),
    (80, "Three largest objects"),
    (105, "Special background 1"),
    (176, "Special background 2"),
    (129, "Missing"),
];

/// The classes of FAD's class map (`classes`) and their colours.
pub const FAD_CLASSES: [(u8, &str, [u8; 3]); 10] = [
    (1, "Rare (<10%)", [215, 50, 40]),
    (2, "Patchy (10-40%)", [250, 140, 90]),
    (3, "Transitional (40-60%)", [255, 200, 0]),
    (4, "Dominant (60-90%)", [140, 200, 100]),
    (5, "Interior (90-100%)", [0, 175, 0]),
    (6, "Intact (100%)", [0, 120, 0]),
    (101, "Background", [175, 175, 175]),
    (102, "Missing", [255, 255, 255]),
    (105, "Special background", [0, 100, 255]),
    (106, "Special background, non-fragmenting", [0, 200, 255]),
];

/// The name of an MSPA class.
pub fn mspa_class(v: f64) -> Option<String> {
    let v = byte(v)?;
    if let Some((_, n)) = MSPA_CLASSES.iter().find(|(k, _)| *k == v) {
        return Some(n.to_string());
    }
    let (_, n) =
        MSPA_CLASSES.iter().find(|(k, _)| u16::from(*k) + 100 == u16::from(v) && !matches!(k, 0 | 100 | 129 | 220))?;
    Some(format!("{n} (internal)"))
}

/// GuidosToolbox's colour of an MSPA class (bridges and loops across edges and perforations as transitions), from
/// its table `mspacolorston`, which SPA and Accounting are shown in too.
pub fn mspa_colour(v: u8) -> [u8; 3] {
    let base = |v: u8| -> Option<[u8; 3]> {
        Some(match v {
            0 => [220, 220, 220],
            1 => [255, 140, 0],
            3 => [0, 0, 0],
            5 => [0, 0, 255],
            9 => [160, 60, 0],
            17 => [0, 200, 0],
            33 | 35 | 37 => [255, 0, 0],
            65 | 67 | 69 => [255, 255, 0],
            80 => [255, 0, 255],
            100 => [136, 136, 136],
            129 => [255, 255, 255],
            176 => [191, 191, 255],
            220 => [194, 194, 194],
            _ => return None,
        })
    };
    base(v).or_else(|| if v > 100 && !matches!(v, 129 | 220) { base(v - 100) } else { None }).unwrap_or([0, 0, 0])
}

/// What GTB's operators give, for the names and colours of their values (as landschaft's `landschaft_product`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Product {
    Mspa,
    Spa,
    Accounting,
    /// A density or connectivity in % (FAD, FOS): 0-100 on the foreground, GTB's codes 101-106 elsewhere; in 5
    /// classes (interior and intact together) or 6.
    Density {
        five: bool,
    },
    /// FAD's classes 1 (rare) to 6 (intact).
    FadClasses,
    /// Contagion and clustering in % (0-100), 101 background, 102 missing.
    Connectivity,
    /// The landscape mosaic, in 19 classes or 103.
    Mosaic {
        classes: u8,
    },
    /// Simple and morphological change: stable background, gain, loss, stable foreground, spurious, missing.
    Change,
    /// The change of FOS: the change + 100, foreground gained and lost, stable background, missing.
    FosChange,
    /// The Euclidean distance's byte map: 130 + the distance in cells inside the foreground, 130 - it outside.
    Distance,
    /// Contortion on GTB's display scale: 0 missing, 1 background, 2 neglected, 3-255 the contortion.
    Contortion,
    /// GTB's Influence Zones and Proximity in its viewport codes.
    Influence,
    /// The components of a habitat network (GTN): networks in GTB's colours, isolated cores, missing.
    GtnComponents,
    /// The importance of a habitat network's links and nodes: their top 1, 5 and 10 %.
    GtnImportance,
    /// An isochrone map: the cost scaled 0-100, markers, missing, the least-cost path, unreachable.
    Isochrone,
    /// A resistance (GTB's Restoration Planner): missing 0, habitat 2, the background 3-100.
    Resistance,
}

/// landschaft's names of its products (`landschaft_product`, written in its GeoTIFFs).
const TAGS: [(&str, Product); 21] = [
    ("MSPA", Product::Mspa),
    ("SPA2", Product::Spa),
    ("SPA3", Product::Spa),
    ("SPA5", Product::Spa),
    ("SPA6", Product::Spa),
    ("ACCOUNTING", Product::Accounting),
    ("FAD", Product::Density { five: false }),
    ("FAD_MULTISCALE", Product::Density { five: false }),
    ("FOS", Product::Density { five: false }),
    ("FAD_CLASSES", Product::FadClasses),
    ("CONTAGION", Product::Connectivity),
    ("CLUSTERING", Product::Connectivity),
    ("LANDSCAPE_MOSAIC", Product::Mosaic { classes: 19 }),
    ("SIMPLE_CHANGE", Product::Change),
    ("MORPHOLOGICAL_CHANGE", Product::Change),
    ("FOS_CHANGE", Product::FosChange),
    ("EUCLIDEAN_DISTANCE", Product::Distance),
    ("INFLUENCE_ZONES", Product::Influence),
    ("GTN_COMPONENTS", Product::GtnComponents),
    ("GTN_IMPORTANCE", Product::GtnImportance),
    ("RESISTANCE", Product::Resistance),
];

impl Product {
    /// The product of a tag of landschaft's (`landschaft_product`).
    pub fn from_tag(tag: &str) -> Option<Product> {
        let t = tag.trim().to_uppercase();
        if t.starts_with("ISOCHRONE") && !t.ends_with("COST") && !t.ends_with("LCP") {
            return Some(Product::Isochrone);
        }
        TAGS.iter().find(|(k, _)| *k == t).map(|(_, p)| *p)
    }

    /// landschaft's tag of a product (the first of its names), for the GeoTIFFs written.
    pub fn tag(self) -> Option<&'static str> {
        match self {
            Product::Isochrone => Some("ISOCHRONE_A"),
            Product::Contortion => None,
            p => TAGS.iter().find(|(_, q)| *q == p).map(|(k, _)| *k),
        }
    }

    /// The product of what an operator gives at a port, with the node's settings.
    pub fn of(op: &str, port: &str, params: &serde_json::Map<String, serde_json::Value>) -> Option<Product> {
        let int = |k: &str| params.get(k).and_then(|c| c.as_i64());
        Some(match (op, port) {
            ("landscape.mspa" | "landscape.mspa_tiled", _) => Product::Mspa,
            ("landscape.spa", "map") => Product::Spa,
            ("landscape.accounting", "map") => Product::Accounting,
            ("fragmentation.fad" | "fragmentation.fad_multiscale", "classes") => Product::FadClasses,
            ("fragmentation.fad" | "fragmentation.fad_multiscale", "map" | "scales") => {
                Product::Density { five: false }
            }
            ("fragmentation.fos", "map") => Product::Density { five: int("classes") == Some(5) },
            ("fragmentation.contagion" | "fragmentation.clustering", "map") => Product::Connectivity,
            ("fragmentation.landscape_mosaic", "map") => {
                Product::Mosaic { classes: if int("classes") == Some(103) { 103 } else { 19 } }
            }
            ("change.simple" | "change.morphological", "map") => Product::Change,
            ("change.fos", "map") => Product::FosChange,
            ("distance.euclidean", "display") => Product::Distance,
            ("landscape.contortion", "display") => Product::Contortion,
            ("distance.influence_zones" | "distance.objects_proximity", "map") => Product::Influence,
            ("networks.gtn_components", "map") => Product::GtnComponents,
            ("networks.gtn_importance", "map") => Product::GtnImportance,
            ("restoration.isochrone", "map") => Product::Isochrone,
            ("restoration.resistance", "map") => Product::Resistance,
            _ => return None,
        })
    }

    /// The name of a value (values of the same name are one line of a legend).
    pub fn name(self, v: f64) -> Option<String> {
        let b = byte(v)?;
        let find = |t: &[(u8, &str)]| t.iter().find(|(k, _)| *k == b).map(|(_, n)| n.to_string());
        match self {
            Product::Mspa => mspa_class(v),
            Product::Spa => find(&SPA_CLASSES),
            Product::Accounting => find(&ACCOUNTING_CLASSES),
            Product::FadClasses => FAD_CLASSES.iter().find(|(k, ..)| *k == b).map(|(_, n, _)| n.to_string()),
            Product::Density { five } => Some(
                match b {
                    0..=9 => "Rare (0-9%)",
                    10..=39 => "Patchy (10-39%)",
                    40..=59 => "Transitional (40-59%)",
                    60..=89 => "Dominant (60-89%)",
                    90..=99 if five => "Interior (90-100%)",
                    100 if five => "Interior (90-100%)",
                    90..=99 => "Interior (90-99%)",
                    100 => "Intact (100%)",
                    101 => "Background",
                    102 => "Missing",
                    105 => "Special background",
                    106 => "Special background, non-fragmenting",
                    _ => return None,
                }
                .to_string(),
            ),
            Product::Connectivity => Some(match b {
                0..=99 => format!("{}-{}%", b / 10 * 10, b / 10 * 10 + 9),
                100 => "100%".into(),
                101 => "Background".into(),
                102 => "Missing".into(),
                _ => return None,
            }),
            Product::Mosaic { classes: 19 } => find(&gtb_palettes::LM19_CLASSES),
            Product::Mosaic { .. } => match gtb_palettes::RECODE_103_TO_19[b as usize] {
                // the codes of 103 that are none of its classes keep their value
                r if r == b && b > 19 => None,
                r => gtb_palettes::LM19_CLASSES.iter().find(|(k, _)| *k == r).map(|(_, n)| n.to_string()),
            },
            Product::Change => Some(
                match b {
                    11 => "Stable background",
                    12 => "Gain",
                    21 => "Loss",
                    22 => "Stable foreground",
                    176 => "Spurious change",
                    255 => "Missing",
                    _ => return None,
                }
                .to_string(),
            ),
            Product::FosChange => find(&gtb_palettes::FOS_CHANGE_NAMES),
            Product::Distance => Some(match b {
                0 => "Missing".into(),
                130 => "Edge".into(),
                // the runs of GTB's colours, as distances in cells
                10..=129 => {
                    let (lo, hi) = run(&gtb_palettes::DIST, b, 10, 129);
                    format!("Background, {}-{} cells", 130 - hi, 130 - lo)
                }
                131..=250 => {
                    let (lo, hi) = run(&gtb_palettes::DIST, b, 131, 250);
                    format!("Foreground, {}-{} cells", lo - 130, hi - 130)
                }
                _ => return None,
            }),
            Product::GtnComponents => Some(
                match b {
                    0 => "Background",
                    103 => "Isolated core",
                    129 => "Missing",
                    1 | 5 | 9 | 17 | 33 | 65 | 80 | 150 | 176 => "Network",
                    _ => return None,
                }
                .to_string(),
            ),
            Product::GtnImportance => Some(
                match b {
                    0 => "Background",
                    103 => "Bridge",
                    176 => "Core",
                    129 => "Missing",
                    192 => "Link: top 1%",
                    194 => "Link: top 5%",
                    195 => "Link: top 10%",
                    208 => "Node: top 1%",
                    210 => "Node: top 5%",
                    211 => "Node: top 10%",
                    _ => return None,
                }
                .to_string(),
            ),
            Product::Isochrone => Some(match b {
                0..=99 => format!("Cost {}-{}", b / 10 * 10, b / 10 * 10 + 9),
                100 => "Cost 100".into(),
                101 => "Marker objects".into(),
                102 => "Missing".into(),
                103 => "Least-cost path".into(),
                104 => "Unreachable".into(),
                _ => return None,
            }),
            Product::Resistance => Some(match b {
                0 => "Missing".into(),
                2 => "Habitat".into(),
                3..=100 => format!("Resistance {}-{}", (b - 3) / 10 * 10 + 3, ((b - 3) / 10 * 10 + 12).min(100)),
                _ => return None,
            }),
            Product::Influence => Some(
                match b {
                    0 => "Zone (background) / hole in an object",
                    4 => "Watershed line (zone boundary)",
                    12 | 59 | 5 | 9 | 17 | 33 | 65 | 18 | 56 | 55 | 57 | 58 | 101 => "Object",
                    80 => "Buffer boundary",
                    160 => "Buffer",
                    129 => "Missing",
                    165 => "Watershed line with proximity <= prox_max",
                    176 => "Foreground not part of an object",
                    193 => "Watershed line at the minimum proximity",
                    _ => return None,
                }
                .to_string(),
            ),
            Product::Contortion => Some(match b {
                0 => "Missing".into(),
                1 => "Background".into(),
                2 => "Neglected (small or without core)".into(),
                // the map's contortion from its least (3) to its most (255), in fifths
                _ => ["Least contorted", "Little contorted", "Contorted", "Much contorted", "Most contorted"]
                    [((b - 3) as usize * 5 / 253).min(4)]
                .into(),
            }),
        }
    }

    /// GTB's colour of a value (`None` for a value the product has not).
    pub fn colour(self, v: f64) -> Option<[u8; 3]> {
        self.name(v)?;
        let b = byte(v)? as usize;
        Some(match self {
            Product::Mspa | Product::Spa | Product::Accounting | Product::Influence => mspa_colour(b as u8),
            Product::GtnComponents => gtb_palettes::GTN_COMPONENTS[b],
            Product::GtnImportance => gtb_palettes::GTN_IMPORTANCE[b],
            Product::Isochrone => gtb_palettes::ENTROPY[b],
            Product::Resistance => gtb_palettes::RESISTANCE[b],
            Product::FadClasses => FAD_CLASSES.iter().find(|(k, ..)| *k as usize == b)?.2,
            Product::Density { five: false } => gtb_palettes::FAD[b],
            Product::Density { five: true } => gtb_palettes::FAD5[b],
            Product::Connectivity => gtb_palettes::ENTROPY[b],
            Product::Mosaic { classes: 19 } => gtb_palettes::LM[b],
            Product::Mosaic { .. } => gtb_palettes::LM[gtb_palettes::RECODE_103_TO_19[b] as usize],
            Product::Change => gtb_palettes::CHANGE[b],
            Product::FosChange => gtb_palettes::FOS_CHANGE[b],
            Product::Distance => gtb_palettes::DIST[b],
            Product::Contortion => gtb_palettes::CONTORTION[b],
        })
    }
}

/// The run of equal colours of a table around `b`, within `lo..=hi`.
fn run(table: &[[u8; 3]; 256], b: u8, lo: u8, hi: u8) -> (u8, u8) {
    let c = table[b as usize];
    let (mut a, mut z) = (b, b);
    while a > lo && table[a as usize - 1] == c {
        a -= 1;
    }
    while z < hi && table[z as usize + 1] == c {
        z += 1;
    }
    (a, z)
}

/// A whole value 0-255 as a byte.
fn byte(v: f64) -> Option<u8> {
    (v.fract() == 0.0 && (0.0..256.0).contains(&v)).then_some(v as u8)
}

/// The settings of the foreground every GTB operator takes.
pub(crate) fn foreground_param(sig: OpSignature) -> OpSignature {
    sig.param(PortSpec::optional("foreground", Type::Any, "the foreground's classes (a class or a list)"))
}

/// A map in GTB's coding: with the setting `foreground` (classes), those cells 2, the others 1 and those without a
/// value 0; without it, the map's own values, which must be GTB's codes: 0 missing, 1 background, 2 foreground,
/// and the special backgrounds 3 and 4 when `specials`. Cells without a value are missing either way.
pub(crate) fn gtb_coded(args: &Args, map: &Map, specials: bool) -> Result<Vec<u8>> {
    gtb_code(args, &foreground_of(args)?, &map.data, specials)
}

/// The foreground's classes of a node (the setting `foreground`), if given.
pub(crate) fn foreground_of(args: &Args) -> Result<Option<Vec<f64>>> {
    match args.params.get("foreground") {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::Number(n)) => Ok(Some(vec![n.as_f64().unwrap()])),
        Some(v) => serde_json::from_value(v.clone())
            .map(Some)
            .map_err(|e| Error::msg(format!("node {:?}: foreground: {e}", args.node))),
    }
}

/// Values in GTB's coding, the foreground's classes given or not (see [`gtb_coded`]).
pub(crate) fn gtb_code(args: &Args, foreground: &Option<Vec<f64>>, data: &[f64], specials: bool) -> Result<Vec<u8>> {
    match foreground {
        Some(classes) => Ok(data
            .iter()
            .map(|v| {
                if v.is_nan() {
                    0
                } else if classes.contains(v) {
                    2
                } else {
                    1
                }
            })
            .collect()),
        None => {
            let top = if specials { 4.0 } else { 2.0 };
            data.iter()
                .map(|&v| match v {
                    v if v.is_nan() => Ok(0),
                    v if v.fract() == 0.0 && (0.0..=top).contains(&v) => Ok(v as u8),
                    v => Err(Error::msg(format!(
                        "node {:?}: the map has a {v}: give the foreground's classes, or code it 0 missing, 1 \
                         background, 2 foreground{}",
                        args.node,
                        if specials { " (3 and 4 special backgrounds)" } else { "" }
                    ))),
                })
                .collect()
        }
    }
}

/// A map of the grid of `like` from bytes.
pub(crate) fn map_of<T: Copy + Into<f64>>(like: &Map, values: impl IntoIterator<Item = T>) -> Value {
    Value::Map(Map::new(like.grid.clone(), values.into_iter().map(Into::into).collect()))
}

struct Mspa;

impl Operator for Mspa {
    fn signature(&self) -> OpSignature {
        foreground_param(
            OpSignature::new(
                "landscape.mspa",
                "Morphological Spatial Pattern Analysis (Soille & Vogt 2009), as GuidosToolbox computes it: the \
                 foreground split into core, islet, perforation, edge, loop, bridge and branch (internal features, \
                 inside core-openings, 100 more). The foreground is the classes given; without them the map is coded \
                 as GTB's: 0 missing, 1 background, 2 foreground. Cells without a value are missing.",
                Access::Global,
            )
            .input(PortSpec::new("map", Type::Map, "the map"))
            .output(PortSpec::new("map", Type::Map, "the MSPA classes")),
        )
        .param(PortSpec::optional("connectivity", Type::Integer, "of the foreground: 8 (default) or 4").choices([8, 4]))
        .param(PortSpec::optional("edge_width", Type::Integer, "the edge's width in cells, 1-255 (default 1)"))
        .param(PortSpec::optional("internal", Type::Bool, "tell internal features apart (default true)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let map = args.map("map")?;
        let connectivity: i32 = args.param("connectivity", 8)?;
        let edge_width: i32 = args.param("edge_width", 1)?;
        let internal: bool = args.param("internal", true)?;
        let input = gtb_coded(args, map, false)?;
        let (ny, nx) = map.shape();
        let out = ffi::mspa(&input, ny as i64, nx as i64, connectivity, edge_width, internal)?;
        Ok(outputs([("map", map_of(map, out))]))
    }
}

/// The rows of SPA's table by variant: the classes it reports, by code.
fn spa_rows(variant: i32) -> &'static [u8] {
    match variant {
        2 => &[17, 1],
        3 => &[17, 1],
        5 => &[17, 3, 5, 1],
        _ => &[17, 9, 3, 5, 1],
    }
}

struct Spa;

impl Operator for Spa {
    fn signature(&self) -> OpSignature {
        foreground_param(
            OpSignature::new(
                "landscape.spa",
                "Simplified Pattern Analysis, as GuidosToolbox computes it: a coarser, faster MSPA of the \
                 8-connected foreground (edges of one cell). SPA2: contiguous foreground (17) and small linear \
                 features (1); SPA3: core, core-opening (100) and margin (1); SPA5: core, edge (3), perforation (5) \
                 and margin; SPA6: SPA5 and islets (9). The foreground is the classes given, or the map is coded \
                 0 missing, 1 background, 2 foreground.",
                Access::Global,
            )
            .input(PortSpec::new("map", Type::Map, "the map"))
            .output(PortSpec::new("map", Type::Map, "the SPA classes"))
            .output(PortSpec::new(
                "stats",
                Type::Table,
                "per class reported: its code, cells, % of the data and % of the foreground",
            ))
            .output(PortSpec::new(
                "objects",
                Type::Table,
                "the objects of the foreground and of each class, as GTB counts them",
            ))
            .output(PortSpec::new("porosity", Type::Number, "100 - contiguous / (contiguous + core-openings) x 100"))
            .output(PortSpec::new("report", Type::Text, "GTB's report (spaN.txt)")),
        )
        .param(PortSpec::optional("variant", Type::Integer, "2, 3, 5 (default) or 6 classes").choices([5, 2, 3, 6]))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let map = args.map("map")?;
        let variant: i32 = args.param("variant", 5)?;
        let input = gtb_coded(args, map, false)?;
        let (ny, nx) = map.shape();
        let out = ffi::spa(&input, ny as i64, nx as i64, variant)?;
        let mut h = [0i64; 256];
        for &v in &out.map {
            h[v as usize] += 1;
        }
        let missing = input.iter().filter(|&&v| v == 0).count() as i64;
        let data = input.len() as i64 - missing;
        let (holes, other) = (h[100], h[0]);
        let foreground = data - holes - other;
        let contiguous = h[17] + h[3] + h[5];
        let pct = |n: i64, of: i64| if of > 0 { n as f64 * 100.0 / of as f64 } else { f64::NAN };
        let mut stats =
            Table::new(["code", "cells", "percent_of_data", "percent_of_foreground"].map(String::from).to_vec());
        for &code in spa_rows(variant) {
            let n = h[code as usize];
            stats.rows.push(vec![f64::from(code), n as f64, pct(n, data), pct(n, foreground)]);
        }
        // the counts GTB reports, those of the variant only (-1 for the others)
        let names = ["foreground", "core", "margin", "core_opening", "edge", "perforation", "islet", "contiguous"];
        let names: Vec<&str> =
            if variant == 2 { vec!["foreground", "contiguous", "small_linear_features"] } else { names.to_vec() };
        let counts: Vec<i64> = if variant == 2 { out.counts[..3].to_vec() } else { out.counts.clone() };
        let (columns, row): (Vec<String>, Vec<f64>) =
            names.iter().zip(&counts).filter(|(_, n)| **n >= 0).map(|(k, n)| (k.to_string(), *n as f64)).unzip();
        let objects = Table { columns, rows: vec![row] };
        let internal = contiguous + holes;
        let porosity = if internal > 0 { 100.0 - contiguous as f64 / internal as f64 * 100.0 } else { f64::NAN };
        let names: &[&str] = match variant {
            2 => &["Contiguous", "SLF"],
            3 => &["Core", "Margin"],
            5 => &["Core", "Edge", "Perforation", "Margin"],
            _ => &["Core", "Islet", "Edge", "Perforation", "Margin"],
        };
        let rows: Vec<(&str, i64)> = names.iter().zip(spa_rows(variant)).map(|(n, &c)| (*n, h[c as usize])).collect();
        let areas = [
            ("image", input.len() as i64),
            ("missing", missing),
            ("data", data),
            ("foreground", foreground),
            ("background", other + holes),
            ("core_opening", holes),
            ("other_background", other),
            ("contiguous", contiguous),
        ];
        let named: Vec<(String, i64)> = objects
            .columns
            .iter()
            .zip(&objects.rows[0])
            .map(|(k, &v)| (if k == "small_linear_features" { "slf".to_string() } else { k.clone() }, v as i64))
            .collect();
        let report = spa_report(variant, &rows, &areas, &named, porosity);
        Ok(outputs([
            ("map", map_of(map, out.map)),
            ("stats", Value::Table(stats)),
            ("objects", Value::Table(objects)),
            ("porosity", Value::Number(porosity)),
            ("report", Value::Text(report)),
        ]))
    }
}

/// GTB's batch Accounting's size classes.
const ACCOUNTING_THRESHOLDS: [i64; 5] = [50, 250, 1000, 5000, 20000];

struct Accounting;

impl Operator for Accounting {
    fn signature(&self) -> OpSignature {
        foreground_param(
            OpSignature::new(
                "landscape.accounting",
                "The foreground's objects in size classes, as GuidosToolbox's Accounting: every object labelled, \
                 measured and put in one of up to six classes (class 1 up to the first threshold, class k above \
                 threshold k-1 up to threshold k, the last above the last threshold), the map in GTB's colour codes. \
                 The foreground is the classes given, or the map is coded 0 missing, 1 background, 2 foreground, 3 \
                 and 4 special backgrounds.",
                Access::Global,
            )
            .input(PortSpec::new("map", Type::Map, "the map"))
            .output(PortSpec::new("map", Type::Map, "the size classes in GTB's codes"))
            .output(PortSpec::new(
                "ids",
                Type::Map,
                "each object's number, 1.. (0 background, -1 missing, -3 and -4 special backgrounds)",
            ))
            .output(PortSpec::new("area", Type::Map, "the cells of each cell's object (codes as ids)"))
            .output(PortSpec::new(
                "stats",
                Type::Table,
                "per size class: its bounds in cells, objects, cells, % of the objects and of the foreground",
            ))
            .output(PortSpec::new("objects", Type::Table, "per object: its number and cells"))
            .output(PortSpec::new("report", Type::Text, "GTB's report (acc.txt)")),
        )
        .param(PortSpec::optional("connectivity", Type::Integer, "of the foreground: 8 (default) or 4").choices([8, 4]))
        .param(PortSpec::optional(
            "thresholds",
            Type::Any,
            "the size classes' upper bounds in cells, 1 to 5 increasing (default 50 250 1000 5000 20000, GTB's \
             batch), or \"auto\" (0.1, 1, 10, 50 and 90% of the range of the objects' areas, as GTB proposes)",
        ))
        .param(PortSpec::optional("highlight_largest", Type::Bool, "paint the three largest objects (code 80)"))
        .param(PortSpec::optional("pixel_resolution", Type::Number, "the report's cell size in metres (25)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let map = args.map("map")?;
        let connectivity: i32 = args.param("connectivity", 8)?;
        let highlight: bool = args.param("highlight_largest", false)?;
        let thresholds: Vec<i64> = match args.params.get("thresholds") {
            None | Some(serde_json::Value::Null) => ACCOUNTING_THRESHOLDS.to_vec(),
            Some(serde_json::Value::String(s)) if s == "auto" => Vec::new(),
            Some(serde_json::Value::Number(n)) => vec![n.as_i64().unwrap_or(0)],
            Some(v) => {
                let t: Vec<i64> = serde_json::from_value(v.clone()).map_err(|e| {
                    Error::msg(format!("node {:?}: thresholds: {e} (a list of cells, or \"auto\")", args.node))
                })?;
                if t.is_empty() || t.len() > 5 || t.iter().any(|&x| x < 1) || t.windows(2).any(|w| w[1] < w[0]) {
                    return Err(Error::msg(format!(
                        "node {:?}: thresholds: 1 to 5 sizes of 1 cell or more, increasing (got {t:?})",
                        args.node
                    )));
                }
                // repeats dropped, as in GTB
                let mut t = t;
                t.dedup();
                t
            }
        };
        let input = gtb_coded(args, map, true)?;
        let (ny, nx) = map.shape();
        let out = ffi::accounting(&input, ny as i64, nx as i64, connectivity, &thresholds, highlight)?;
        let used = &out.thresholds;
        let n = out.object_area.len();
        let total: i64 = out.object_area.iter().sum();
        let mut counts = vec![0i64; used.len() + 1];
        let mut areas = vec![0i64; used.len() + 1];
        for &a in &out.object_area {
            let k = used.partition_point(|&t| t < a);
            counts[k] += 1;
            areas[k] += a;
        }
        let mut stats = Table::new(
            ["size_class", "from", "to", "objects", "cells", "percent_of_objects", "percent_of_area"]
                .map(String::from)
                .to_vec(),
        );
        for k in 0..=used.len() {
            let from = if k == 0 { 1 } else { used[k - 1] + 1 };
            let to = used.get(k).map_or(f64::NAN, |&t| t as f64);
            stats.rows.push(vec![
                (k + 1) as f64,
                from as f64,
                to,
                counts[k] as f64,
                areas[k] as f64,
                counts[k] as f64 * 100.0 / n as f64,
                areas[k] as f64 * 100.0 / total as f64,
            ]);
        }
        let mut objects = Table::new(vec!["id".into(), "cells".into()]);
        objects.rows = out.object_area.iter().enumerate().map(|(i, &a)| vec![(i + 1) as f64, a as f64]).collect();
        let mut largest = Vec::new();
        let mut at = 0;
        while at < out.largest.len() {
            let k = out.largest[at] as usize;
            largest.push(out.largest[at + 1..at + 1 + k].to_vec());
            at += 1 + k;
        }
        let mut o = outputs([
            ("map", map_of(map, out.classes)),
            ("ids", map_of(map, out.ids)),
            ("area", map_of(map, out.area)),
            ("stats", Value::Table(stats)),
            ("objects", Value::Table(objects)),
        ]);
        if n > 0 {
            let resolution: f64 = args.param("pixel_resolution", 25.0)?;
            let report =
                accounting_report(connectivity, resolution, used, &out.object_area, &largest, highlight && n >= 3);
            o.insert("report".into(), Value::Text(report));
        }
        Ok(o)
    }
}

struct Parcellation;

impl Operator for Parcellation {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "landscape.parcellation",
            "Parcellation, as GuidosToolbox: the patches (parcels) of every class of a map, and per class and for \
             the whole map their number, the average patch size (APS), the area-weighted one (AWAPS, the \
             effective mesh size), the division index and the parcellation index (PARC). The classes are whole \
             values 1-255; 0 and cells without a value are missing.",
            Access::Global,
        )
        .input(PortSpec::new("map", Type::Map, "the map of classes"))
        .output(PortSpec::new("parcels", Type::Map, "each cell's parcel, numbered (0 missing)"))
        .output(PortSpec::new(
            "stats",
            Type::Table,
            "per class: its parcels, cells, APS, AWAPS, AWAPS over the data, division and PARC (%)",
        ))
        .output(PortSpec::new(
            "summary",
            Type::Table,
            "the whole map: its parcels, cells with data, APS, AWAPS, division and PARC (%)",
        ))
        .param(PortSpec::optional("connectivity", Type::Integer, "of the parcels: 8 (default) or 4").choices([8, 4]))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let map = args.map("map")?;
        let connectivity: i32 = args.param("connectivity", 8)?;
        let input: Vec<u8> = map
            .data
            .iter()
            .map(|&v| match v {
                v if v.is_nan() => Ok(0),
                v if v.fract() == 0.0 && (0.0..=255.0).contains(&v) => Ok(v as u8),
                v => Err(Error::msg(format!(
                    "node {:?}: parcellation reads classes 1-255 (0 missing), not {v}: reclassify the map first",
                    args.node
                ))),
            })
            .collect::<Result<_>>()?;
        let (ny, nx) = map.shape();
        let out = ffi::parcellation(&input, ny as i64, nx as i64, connectivity)?;
        let mut stats = Table::new(
            ["class", "parcels", "cells", "aps", "awaps", "awaps_data", "division", "parc"].map(String::from).to_vec(),
        );
        stats.rows = out.classes.chunks(8).map(|r| r.to_vec()).collect();
        let mut summary =
            Table::new(["parcels", "cells", "aps", "awaps", "division", "parc"].map(String::from).to_vec());
        summary.rows.push(out.global.clone());
        Ok(outputs([
            ("parcels", map_of(map, out.labels)),
            ("stats", Value::Table(stats)),
            ("summary", Value::Table(summary)),
        ]))
    }
}

struct Contortion;

impl Operator for Contortion {
    fn signature(&self) -> OpSignature {
        foreground_param(
            OpSignature::new(
                "landscape.contortion",
                "Contortion, as GuidosToolbox: how contorted the boundary of each patch of foreground is (the turns \
                 of its outline), for the patches with a core and of the size given or more. The map gives GTB's \
                 values: 0 missing, 1 background, 2 neglected foreground, from 3 the contortion + 1; the display, \
                 the contortion scaled to 3-255 over the map's range. The foreground is the classes given, or the \
                 map is coded 0 missing, 1 background, 2 foreground.",
                Access::Global,
            )
            .input(PortSpec::new("map", Type::Map, "the map"))
            .output(PortSpec::new("map", Type::Map, "GTB's contortion map"))
            .output(PortSpec::new("display", Type::Map, "the contortion on GTB's display scale (3-255)")),
        )
        .param(PortSpec::optional("min_area", Type::Integer, "the patches' least size in cells (default 1)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let map = args.map("map")?;
        let min_area: i64 = args.param("min_area", 1)?;
        let input = gtb_coded(args, map, false)?;
        let (ny, nx) = map.shape();
        let out = ffi::contortion(&input, ny as i64, nx as i64, min_area)?;
        Ok(outputs([("display", map_of(map, contortion_display(&out))), ("map", map_of(map, out))]))
    }
}

/// GTB's display of a contortion map: from 3 up scaled to 3-255 over the map's range (as numpy rounds), the
/// codes below 3 as they are.
pub fn contortion_display(c: &[u32]) -> Vec<u8> {
    let (lo, hi) = c.iter().filter(|&&v| v >= 3).fold((u32::MAX, 0), |(a, b), &v| (a.min(v), b.max(v)));
    c.iter()
        .map(|&v| match v {
            0..=2 => v as u8,
            v if hi > lo => 3 + (f64::from(v - lo) / f64::from(hi - lo) * 252.0).round_ties_even() as u8,
            _ => 3,
        })
        .collect()
}

struct EuclideanDistance;

impl Operator for EuclideanDistance {
    fn signature(&self) -> OpSignature {
        foreground_param(
            OpSignature::new(
                "distance.euclidean",
                "The Euclidean distance to the foreground's edge, as GuidosToolbox: in cells, positive inside the \
                 foreground and negative outside it, with GTB's byte map (130 + the distance inside, 130 - it \
                 outside, up to 120 cells) and the mean and largest distance on each side. The foreground is the \
                 classes given, or the map is coded 0 missing, 1 background, 2 foreground.",
                Access::Global,
            )
            .input(PortSpec::new("map", Type::Map, "the map"))
            .output(PortSpec::new("map", Type::Map, "the signed distance in cells"))
            .output(PortSpec::new("display", Type::Map, "GTB's byte map of the distance"))
            .output(PortSpec::new(
                "stats",
                Type::Table,
                "the foreground (1) and the background (0): their cells, mean and largest distance",
            )),
        )
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let map = args.map("map")?;
        let input = gtb_coded(args, map, false)?;
        let (ny, nx) = map.shape();
        let out = ffi::euclidean_distance(&input, ny as i64, nx as i64)?;
        let s = &out.stats;
        let mut stats = Table::new(["foreground", "cells", "mean", "max"].map(String::from).to_vec());
        stats.rows = vec![vec![1.0, s[0], s[1], s[2]], vec![0.0, s[3], s[4], s[5]]];
        // the missing cells have no distance
        let distance: Vec<f64> =
            out.distance.iter().zip(&input).map(|(&d, &c)| if c == 0 { f64::NAN } else { f64::from(d) }).collect();
        Ok(outputs([
            ("map", Value::Map(Map::new(map.grid.clone(), distance))),
            ("display", map_of(map, out.bytes)),
            ("stats", Value::Table(stats)),
        ]))
    }
}

struct Influence {
    proximity: bool,
}

impl Operator for Influence {
    fn signature(&self) -> OpSignature {
        let sig = if self.proximity {
            OpSignature::new(
                "distance.objects_proximity",
                "The proximity between neighbouring foreground objects, as GuidosToolbox's Distance -> Proximity \
                 (landschaft's proximity): the objects (8-connected, holes filled, of min_area cells at least) \
                 split the map into zones of influence (the watershed of the distance to them); along the zones' \
                 lines the gap between the objects and their connected area gain. Experimental in landschaft: \
                 IDL's distance transform is not yet reproduced exactly.",
                Access::Global,
            )
            .output(PortSpec::new(
                "distance",
                Type::Map,
                "round(2 x the distance to the nearest object), the lines' updated",
            ))
            .output(PortSpec::new("cag", Type::Map, "the connected area gain on the lines"))
            .output(PortSpec::new(
                "lines",
                Type::Table,
                "per line cell: its column and row (1-based), the zones it touches, the connector length, the CAG, \
                 the first two objects and their areas",
            ))
            .output(PortSpec::new("csv", Type::Text, "GTB's proximity CSV"))
            .param(PortSpec::optional(
                "prox_max",
                Type::Integer,
                "lines up to this proximity marked 165 (0)",
            ))
        } else {
            OpSignature::new(
                "distance.influence_zones",
                "The zones of influence of the foreground objects, as GuidosToolbox's Distance -> Influence Zones \
                 (landschaft's influence_zones): the objects (8-connected, holes filled, of min_area cells at least, \
                 or their cores) and the watershed of the distance to them; a background buffer around them. \
                 Experimental in landschaft: IDL's distance transform is not yet reproduced exactly.",
                Access::Global,
            )
            .param(PortSpec::optional(
                "buffer",
                Type::Integer,
                "a background buffer this wide around the objects (0)",
            ))
        };
        foreground_param(
            sig.input(PortSpec::new("map", Type::Map, "the map"))
                .output(PortSpec::new("map", Type::Map, "GTB's codes: objects, zones, lines, buffer, missing"))
                .output(PortSpec::new(
                    "ids",
                    Type::Map,
                    "the object of every cell (holes included); 0 zones, -1 holes, -2 foreground of no object, -3 \
                     lines, -4 missing, -5 the buffer's edge, -6 buffer",
                ))
                .output(PortSpec::new("zones", Type::Map, "the zone (watershed basin) of every cell, 0 on the lines"))
                .output(PortSpec::new(
                    "objects",
                    Type::Table,
                    "per object (1..): its area in cells (holes not counted)",
                ))
                .param(PortSpec::optional("min_area", Type::Integer, "the least object, in cells (GTB's 5000)"))
                .param(PortSpec::optional(
                    "core_buffer",
                    Type::Integer,
                    "the objects reduced to their core first: this many cells of their edge dropped (0)",
                )),
        )
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let map = args.map("map")?;
        let input = gtb_coded(args, map, false)?;
        let (ny, nx) = map.shape();
        let min_area: i64 = args.param("min_area", 5000)?;
        let core: i32 = args.param("core_buffer", 0)?;
        let extra: i32 = if self.proximity { args.param("prox_max", 0)? } else { args.param("buffer", 0)? };
        let out = ffi::influence(&input, ny as i64, nx as i64, min_area, core, extra, self.proximity)?;
        let ints =
            |v: &[i32]| -> Value { Value::Map(Map::new(map.grid.clone(), v.iter().map(|&x| f64::from(x)).collect())) };
        let mut objects = Table::new(["object", "area"].map(String::from).to_vec());
        objects.rows =
            out.object_area.iter().skip(1).enumerate().map(|(k, &a)| vec![(k + 1) as f64, a as f64]).collect();
        let mut o = outputs([
            ("map", map_of(map, out.classes)),
            ("ids", ints(&out.ids)),
            ("zones", ints(&out.zones)),
            ("objects", Value::Table(objects)),
        ]);
        if self.proximity {
            o.insert("distance".into(), ints(&out.distance));
            let cag = if out.cag.is_empty() { vec![0.0; ny * nx] } else { out.cag.iter().map(|&v| v as f64).collect() };
            o.insert("cag".into(), Value::Map(Map::new(map.grid.clone(), cag)));
            let mut lines = Table::new(
                ["x", "y", "nbr", "clength", "cag", "id_1", "area_1", "id_2", "area_2"].map(String::from).to_vec(),
            );
            let mut text_rows = Vec::new();
            let mut at = 0;
            while at < out.lines.len() {
                let l = &out.lines[at..];
                let k = l[5] as usize;
                let (ids, areas) = (&l[6..6 + k], &l[6 + k..6 + 2 * k]);
                let get = |v: &[i64], i: usize| v.get(i).map_or(f64::NAN, |&x| x as f64);
                lines.rows.push(vec![
                    l[0] as f64,
                    l[1] as f64,
                    l[2] as f64,
                    l[3] as f64,
                    l[4] as f64,
                    get(ids, 0),
                    get(areas, 0),
                    get(ids, 1),
                    get(areas, 1),
                ]);
                let pairs: Vec<String> = ids.iter().zip(areas).map(|(i, a)| format!("{i}-{a}")).collect();
                text_rows.push(format!("{},{},{},{},{},{}", l[0], l[1], l[2], l[3], l[4], pairs.join("  ")));
                at += 6 + 2 * k;
            }
            // GTB's proximity CSV
            let targets = if core > 0 { " Core-obj." } else { " FG-obj." };
            let prox_min = if out.prox_min < 0 { "undef.".to_string() } else { out.prox_min.to_string() };
            let mut title = format!(
                "Min.{targets} size: {min_area} - {} objects - prox_min: {prox_min}",
                out.object_area.len() - 1
            );
            if core > 0 {
                title += &format!(" - core-buffer: {core} pixels");
            }
            let source: String = args.param("source", String::new())?;
            let mut csv = vec![format!("{source} - {title}"), "WS-X,WS-Y, #nbr, clength, CAG, IDs(area),".to_string()];
            csv.extend(text_rows);
            o.insert("lines".into(), Value::Table(lines));
            o.insert("csv".into(), Value::Text(csv.join("\n") + "\n"));
        }
        Ok(o)
    }
}

/// A number as Python's `{v:.6g}` (C's %g) writes it.
pub(crate) fn g6(x: f64) -> String {
    if x.is_nan() {
        return "nan".into();
    }
    if x.is_infinite() {
        return if x > 0.0 { "inf".into() } else { "-inf".into() };
    }
    if x == 0.0 {
        return "0".into();
    }
    let trim = |s: String| if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s };
    let s = format!("{x:.5e}");
    let (mant, exp) = s.split_once('e').unwrap();
    let exp: i32 = exp.parse().unwrap();
    if !(-4..6).contains(&exp) {
        format!("{}e{}{:02}", trim(mant.to_string()), if exp < 0 { '-' } else { '+' }, exp.abs())
    } else {
        trim(format!("{:.*}", (5 - exp) as usize, x))
    }
}

/// GTB's SPA report (spaN.txt): the areas and the proportions of the classes, the objects, the porosity.
#[allow(clippy::too_many_arguments)]
fn spa_report(
    variant: i32,
    rows: &[(&str, i64)],
    areas: &[(&str, i64)],
    objects: &[(String, i64)],
    porosity: f64,
) -> String {
    let a = |k: &str| areas.iter().find(|(n, _)| *n == k).unwrap().1;
    let (line, rule) = ("=".repeat(80), "-".repeat(80));
    let mut out = vec![
        format!("SPA{variant}: 8-connected Foreground, summary analysis"),
        String::new(),
        line.clone(),
        "           Category              Area [pixels]: ".to_string(),
        line.clone(),
    ];
    for (i, (name, area)) in rows.iter().enumerate() {
        let label = format!("{}{name}: ", if i == 0 { "  " } else { "+ " });
        out.push(format!("{label:>20}{area:>22}"));
    }
    if variant == 5 || variant == 6 {
        out.push(format!("(Contiguous = Core + Edge + Perforation: {})", a("contiguous")));
    }
    out.extend([rule.clone(), format!("{:>20}{:>22}", "= Foreground Total: ", a("foreground")), String::new()]);
    if variant != 2 {
        out.extend([
            format!("{:>20}{:>22}", "Core-Opening: ", a("core_opening")),
            format!("{:>20}{:>22}", "+ Other BG: ", a("other_background")),
            rule.clone(),
            format!("{:>20}{:>22}", "= Background Total: ", a("background")),
            String::new(),
        ]);
    }
    out.extend([
        format!("{:>20}{:>22}", "Data Area: ", a("data")),
        format!("{:>20}{:>22}", "+ Missing: ", a("missing")),
        rule.clone(),
        format!("{:>20}{:>22}", "= Image Area Total: ", a("image")),
        String::new(),
        line.clone(),
        "           Category    Proportion [%]: ".to_string(),
        line,
    ]);
    let (data, fg) = (a("data"), a("foreground"));
    for (name, area) in rows {
        out.push(format!("{:>20}{:11.4}", format!("{name}/Data: "), *area as f64 * 100.0 / data as f64));
    }
    out.push(rule);
    for (name, area) in rows {
        let p = if fg > 0 { *area as f64 * 100.0 / fg as f64 } else { f64::NAN };
        out.push(format!(
            "{:>20}{:>11}",
            format!("{name}/FG: "),
            if p.is_nan() { "nan".into() } else { format!("{p:.4}") }
        ));
    }
    out.push(String::new());
    out.push(format!("Objects: {}", objects.iter().map(|(k, v)| format!("{k} {v}")).collect::<Vec<_>>().join(", ")));
    if variant != 2 {
        out.push(format!("Porosity [%]: {porosity:.4}"));
    }
    out.join("\n")
}

/// GTB's colour names of the Accounting's size classes.
const ACCOUNTING_COLOURS: [&str; 6] = ["black", "red", "yellow", "orange", "brown", "green"];

/// GTB's Accounting report (acc.txt).
fn accounting_report(
    connectivity: i32,
    resolution: f64,
    used: &[i64],
    object_area: &[i64],
    largest: &[Vec<i64>],
    highlighted: bool,
) -> String {
    let n = object_area.len();
    let total: i64 = object_area.iter().sum();
    let mut counts = vec![0i64; used.len() + 1];
    let mut areas = vec![0i64; used.len() + 1];
    for &a in object_area {
        let k = used.partition_point(|&t| t < a);
        counts[k] += 1;
        areas[k] += a;
    }
    let rule = format!("{} ", "-".repeat(93));
    let header = "          # Objects      Area[pixels]     % of all objects  % of total FGarea";
    let ha = resolution * resolution / 10000.0;
    let mut lines = vec![
        "Accounting size classes result using: landschaft".to_string(),
        format!("Base settings: {connectivity}-connectivity, pixel resolution: {} [m]", g6(resolution)),
        format!("Conversion factor: pixel_to_hectare: {}, pixel_to_acres: {}", g6(ha), g6(ha * 2.47105)),
        rule.clone(),
    ];
    for k in 0..=used.len() {
        if k == used.len() && counts[k] == 0 {
            continue;
        }
        let range = match k {
            0 => format!("1 - {}", used[0]),
            k if k < used.len() => format!("{} - {}", used[k - 1] + 1, used[k]),
            _ => format!("{} -> ", used[used.len() - 1] + 1),
        };
        lines.push(format!(" Size class {}: [{range}] pixels; color: {}", k + 1, ACCOUNTING_COLOURS[k]));
        lines.push(header.to_string());
        let po = g6(counts[k] as f64 * 100.0 / n as f64);
        let pa = g6(areas[k] as f64 * 100.0 / total as f64);
        lines.push(format!("{:>18}{:>18}{po:>18}{pa:>18}", counts[k], areas[k]));
        lines.push(rule.clone());
    }
    let mut sorted = object_area.to_vec();
    sorted.sort_unstable();
    let mean = total as f64 / n as f64;
    let std = if n > 1 {
        (object_area.iter().map(|&a| (a as f64 - mean).powi(2)).sum::<f64>() / (n - 1) as f64).sqrt()
    } else {
        f64::NAN
    };
    lines.extend([
        rule,
        "Sum of all classes:".to_string(),
        header.to_string(),
        format!("{n:>18}{total:>18}{:>18}{:>18}", "100", "100"),
        " ".to_string(),
        format!("{:>20}{:>16}", "Median Patch Size: ", sorted[n / 2]),
        format!("{:>20}{:>16}", "Average Patch Size: ", g6(mean)),
        format!("{:>20}{:>16}", "Standard Deviation: ", g6(std)),
        " ".to_string(),
    ]);
    if highlighted {
        lines.push("Three largest object IDs and area[pixels]; color: pink".into());
        lines.push("These 3 objects overlay objects listed above".into());
        for (rank, ids) in largest.iter().enumerate() {
            if let Some(&first) = ids.first() {
                let joined = ids.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(",");
                lines.push(format!("{}) {joined:>18}{:>18}", rank + 1, object_area[first as usize - 1]));
            }
        }
    } else {
        lines.push(format!("Largest object:     {}", object_area.iter().max().unwrap()));
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_colours() {
        assert_eq!(mspa_class(17.0).as_deref(), Some("Core"));
        assert_eq!(mspa_class(117.0).as_deref(), Some("Core (internal)"));
        assert_eq!(mspa_class(229.0), None);
        assert_eq!(mspa_colour(133), [255, 0, 0]);
        assert_eq!(mspa_colour(220), [194, 194, 194]);
        assert_eq!(Product::Spa.name(17.0).as_deref(), Some("Core (SPA2: contiguous)"));
        assert_eq!(Product::Spa.name(33.0), None, "not SPA's");
        assert_eq!(Product::Accounting.colour(80.0), Some([255, 0, 255]));
        assert_eq!(Product::Accounting.colour(103.0), Some([0, 0, 0]));
        let six = Product::Density { five: false };
        assert_eq!(six.name(95.0).as_deref(), Some("Interior (90-99%)"));
        assert_eq!(six.colour(100.0), Some([0, 120, 0]));
        assert_eq!(six.name(103.0), None);
        let five = Product::Density { five: true };
        assert_eq!(five.name(100.0).as_deref(), Some("Interior (90-100%)"));
        assert_eq!(five.colour(100.0), five.colour(95.0));
        assert_eq!(Product::FadClasses.colour(6.0), Some([0, 120, 0]));
        let none = serde_json::Map::new();
        assert_eq!(Product::of("fragmentation.fad", "classes", &none), Some(Product::FadClasses));
        assert_eq!(Product::of("landscape.accounting", "ids", &none), None);
        let five = serde_json::json!({"classes": 5});
        assert_eq!(
            Product::of("fragmentation.fos", "map", five.as_object().unwrap()),
            Some(Product::Density { five: true })
        );
    }
}
