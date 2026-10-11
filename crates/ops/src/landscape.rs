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
}

impl Product {
    /// The product of what an operator gives at a port, with the node's settings.
    pub fn of(op: &str, port: &str, params: &serde_json::Map<String, serde_json::Value>) -> Option<Product> {
        let int = |k: &str| params.get(k).and_then(|c| c.as_i64());
        Some(match (op, port) {
            ("landscape.mspa", _) => Product::Mspa,
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
            Product::Mspa | Product::Spa | Product::Accounting => mspa_colour(b as u8),
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
    let foreground: Option<Vec<f64>> = match args.params.get("foreground") {
        None | Some(serde_json::Value::Null) => None,
        Some(serde_json::Value::Number(n)) => Some(vec![n.as_f64().unwrap()]),
        Some(v) => Some(
            serde_json::from_value(v.clone())
                .map_err(|e| Error::msg(format!("node {:?}: foreground: {e}", args.node)))?,
        ),
    };
    match &foreground {
        Some(classes) => Ok(map
            .data
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
            map.data
                .iter()
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
        .param(PortSpec::optional("connectivity", Type::Integer, "of the foreground: 8 (default) or 4"))
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
            .output(PortSpec::new(
                "porosity",
                Type::Number,
                "100 - contiguous / (contiguous + core-openings) x 100",
            )),
        )
        .param(PortSpec::optional("variant", Type::Integer, "2, 3, 5 (default) or 6 classes"))
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
        Ok(outputs([
            ("map", map_of(map, out.map)),
            ("stats", Value::Table(stats)),
            ("objects", Value::Table(objects)),
            ("porosity", Value::Number(porosity)),
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
            .output(PortSpec::new("objects", Type::Table, "per object: its number and cells")),
        )
        .param(PortSpec::optional("connectivity", Type::Integer, "of the foreground: 8 (default) or 4"))
        .param(PortSpec::optional(
            "thresholds",
            Type::Any,
            "the size classes' upper bounds in cells, 1 to 5 increasing (default 50 250 1000 5000 20000, GTB's \
             batch), or \"auto\" (0.1, 1, 10, 50 and 90% of the range of the objects' areas, as GTB proposes)",
        ))
        .param(PortSpec::optional(
            "highlight_largest",
            Type::Bool,
            "paint the three largest objects (code 80)",
        ))
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
        Ok(outputs([
            ("map", map_of(map, out.classes)),
            ("ids", map_of(map, out.ids)),
            ("area", map_of(map, out.area)),
            ("stats", Value::Table(stats)),
            ("objects", Value::Table(objects)),
        ]))
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
        .param(PortSpec::optional("connectivity", Type::Integer, "of the parcels: 8 (default) or 4"))
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
