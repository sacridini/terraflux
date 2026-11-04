//! The maps and vector layers of a run as a QGIS project: each written to a folder (GeoTIFF, GeoPackage; rasters
//! read from a file referred to where they are), styled as the Viewer shows them (the classes in their colours
//! and with their names, as a paletted map; values on the ramp from its minimum to its maximum), with a `.qml`
//! style beside each map written (QGIS takes it when the map is added by hand), the vector layers in the Viewer's
//! colours (one, or by an attribute's classes or range; as QGIS's rules when asked, to edit there), labelled by an
//! attribute and their points drawn as a marker chosen, and the project (`.qgz`, QGIS's own, or `.qgs`) that opens
//! them all in QGIS, the vector layers on top.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};

use egui::{Color32, RichText};
use terraflux_ops::legend::Class;
use terraflux_ops::{Map, Value};
use terraflux_raster::vector::Geometry;

use crate::app::{Look, Session};
use crate::builder::Palette;
use crate::map_view::Source;
use crate::palette::Coloring;

/// The QGIS version the project and the styles are written as (QGIS reads those of older versions).
const QGIS_VERSION: &str = "3.34.0-Prizren";
/// The stops of a ramp written in a style (QGIS interpolates between them).
const RAMP_STOPS: usize = 11;

/// A map or vector layer to put in the project, as chosen in the application: its name, its value, its palette,
/// the legend of its classes, the layer of it shown (of a stack, a series or rasters).
pub struct Pick {
    pub name: String,
    pub value: Value,
    pub palette: Palette,
    /// The range of its colours chosen by hand (least, most), when it is of values.
    pub range: Option<(f64, f64)>,
    pub legend: Option<Vec<Class>>,
    pub band: usize,
    /// A vector layer's style in the Viewer (when it is drawn over the map there).
    pub style: Option<crate::vector_tiles::Style>,
    /// A vector layer's options in QGIS: its labels, its points' marker, its style as rules.
    pub options: VectorOptions,
}

/// The markers a vector layer's points are drawn as in QGIS (its names of them).
pub const MARKERS: [&str; 8] = ["circle", "square", "triangle", "diamond", "star", "cross", "pentagon", "hexagon"];

/// How a vector layer goes to QGIS beyond its colours: labelled by an attribute, its points as a marker of
/// [`MARKERS`], its style as rules (a rule each class or range, and one for the features of none: QGIS's rule-based
/// renderer, to change there as rules).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct VectorOptions {
    pub label: Option<String>,
    pub marker: usize,
    pub rules: bool,
}

/// Whether a value is a map or vector layer QGIS opens.
pub fn exportable(v: &Value) -> bool {
    match v {
        Value::Map(_) | Value::Stack(_) | Value::Raster(_) | Value::Vector(_) => true,
        Value::Series(items) => !items.is_empty() && items.iter().all(|v| matches!(v, Value::Map(_))),
        _ => false,
    }
}

/// The layers of a value (the maps of a stack or a series, the bands of rasters).
fn layers(v: &Value) -> usize {
    match v {
        Value::Stack(s) => s.layers.len(),
        Value::Series(items) => items.len(),
        Value::Raster(r) => r.layers.len(),
        _ => 1,
    }
}

/// The layer `band` of a map value, as the viewer draws it.
fn band_source(v: &Value, band: usize) -> Option<Source> {
    let b = band.min(layers(v).saturating_sub(1));
    match v {
        Value::Map(m) => Some(Source::Map(m.clone())),
        Value::Stack(s) => Some(Source::Map(Map { grid: s.grid.clone(), data: s.layers.get(b)?.clone() })),
        Value::Series(items) => match items.get(b)? {
            Value::Map(m) => Some(Source::Map(m.clone())),
            _ => None,
        },
        Value::Raster(r) => Some(Source::Rasters(r.clone(), b)),
        _ => None,
    }
}

/// Text in XML.
fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&apos;"),
            c => o.push(c),
        }
    }
    o
}

fn hex(c: Color32) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b())
}

/// A number as QGIS reads it (as short as it stays exact).
fn num(v: f64) -> String {
    if v == v.trunc() && v.abs() < 1e15 { format!("{}", v as i64) } else { format!("{v}") }
}

/// A number to 12 significant digits (a stop of a ramp without the noise of its arithmetic: 1000, not
/// 999.9999999999998).
fn tidy(v: f64) -> f64 {
    if v == 0.0 || !v.is_finite() {
        return v;
    }
    let p = 10f64.powi(12 - v.abs().log10().ceil() as i32);
    (v * p).round() / p
}

/// The renderer of a map's band (1-based) in its look's colours: a paletted map of its classes (named by the
/// legend, else as the viewer names them), or a pseudocolour ramp from the minimum to the maximum (stops spaced
/// evenly, or by their logarithm). The colours as they are, not as a colour-blind reader sees them.
pub fn renderer(look: &Look, legend: Option<&[Class]>, band: usize) -> String {
    match &look.coloring {
        Coloring::Classes(classes) => {
            let mut x = format!(
                "<rasterrenderer type=\"paletted\" band=\"{band}\" opacity=\"1\" alphaBand=\"-1\" nodataColor=\"\">\
                 <rasterTransparency/><minMaxOrigin><limits>None</limits><extent>WholeRaster</extent>\
                 <statAccuracy>Estimated</statAccuracy><cumulativeCutLower>0.02</cumulativeCutLower>\
                 <cumulativeCutUpper>0.98</cumulativeCutUpper><stdDevFactor>2</stdDevFactor></minMaxOrigin>\
                 <colorPalette>"
            );
            for (v, c) in classes {
                let label = crate::legend_view::name(look, legend, *v);
                x.push_str(&format!(
                    "<paletteEntry value=\"{}\" color=\"{}\" alpha=\"{}\" label=\"{}\"/>",
                    num(*v),
                    hex(*c),
                    c.a(),
                    esc(&label)
                ));
            }
            x.push_str("</colorPalette><colorramp type=\"randomcolors\" name=\"[source]\"/></rasterrenderer>");
            x
        }
        Coloring::Continuous { min, max, ramp, log } => {
            let (lo, hi) = (*min, if max > min { *max } else { min + 1.0 });
            let mut items = String::new();
            for k in 0..RAMP_STOPS {
                let t = k as f64 / (RAMP_STOPS - 1) as f64;
                let v = tidy(if *log && lo > 0.0 { lo * (hi / lo).powf(t) } else { lo + (hi - lo) * t });
                let c = ramp.colour(t as f32);
                items.push_str(&format!(
                    "<item value=\"{}\" color=\"{}\" alpha=\"255\" label=\"{}\"/>",
                    num(v),
                    hex(c),
                    esc(&crate::animation::short(v))
                ));
            }
            format!(
                "<rasterrenderer type=\"singlebandpseudocolor\" band=\"{band}\" opacity=\"1\" alphaBand=\"-1\" \
                 classificationMin=\"{lo}\" classificationMax=\"{hi}\" nodataColor=\"\"><rasterTransparency/>\
                 <minMaxOrigin><limits>None</limits><extent>WholeRaster</extent><statAccuracy>Estimated</statAccuracy>\
                 <cumulativeCutLower>0.02</cumulativeCutLower><cumulativeCutUpper>0.98</cumulativeCutUpper>\
                 <stdDevFactor>2</stdDevFactor></minMaxOrigin><rastershader><colorrampshader \
                 colorRampType=\"INTERPOLATED\" classificationMode=\"1\" clip=\"0\" minimumValue=\"{lo}\" \
                 maximumValue=\"{hi}\" labelPrecision=\"4\">{items}</colorrampshader></rastershader></rasterrenderer>"
            )
        }
    }
}

/// A raster's style file (`.qml`): its renderer.
pub fn qml(renderer: &str) -> String {
    format!(
        "<!DOCTYPE qgis PUBLIC 'http://mrcc.com/qgis.dtd' 'SYSTEM'>\n<qgis version=\"{QGIS_VERSION}\" \
         styleCategories=\"Symbology\"><pipe>{renderer}</pipe><blendMode>0</blendMode></qgis>\n"
    )
}

/// The kind of geometry of a vector layer as QGIS names it: of its first feature with one.
fn geometry_of(v: &terraflux_raster::vector::Vector) -> &'static str {
    match v.geometries.iter().find(|g| !matches!(g, Geometry::Empty)) {
        Some(Geometry::Point(_) | Geometry::MultiPoint(_)) => "Point",
        Some(Geometry::LineString(_) | Geometry::MultiLineString(_)) => "Line",
        _ => "Polygon",
    }
}

/// How a vector layer is drawn: its colour, the polygons' fill (0 to 1), the lines' width (millimetres), the
/// points' marker.
#[derive(Clone, Copy)]
struct VectorLook {
    colour: Color32,
    fill: f32,
    width_mm: f32,
    marker: &'static str,
}

/// A symbol of a vector layer, named `name`.
fn symbol(name: &str, geometry: &str, l: VectorLook) -> String {
    let c = l.colour;
    let rgb = format!("{},{},{}", c.r(), c.g(), c.b());
    let opt = |k: &str, v: &str| format!("<Option name=\"{k}\" type=\"QString\" value=\"{v}\"/>");
    let w = format!("{:.2}", l.width_mm);
    let (kind, class, options) = match geometry {
        "Point" => (
            "marker",
            "SimpleMarker",
            [
                opt("name", l.marker),
                opt("color", &format!("{rgb},255")),
                opt("outline_color", "35,35,35,255"),
                opt("size", &format!("{:.2}", 1.6 + l.width_mm * 2.0)),
            ]
            .concat(),
        ),
        "Line" => ("line", "SimpleLine", [opt("line_color", &format!("{rgb},255")), opt("line_width", &w)].concat()),
        _ => {
            let fill = (l.fill.clamp(0.0, 1.0) * 255.0).round() as u8;
            let outline = if l.width_mm > 0.0 { "solid" } else { "no" };
            (
                "fill",
                "SimpleFill",
                [
                    opt("color", &format!("{rgb},{fill}")),
                    opt("outline_color", &format!("{rgb},255")),
                    opt("outline_width", &w),
                    opt("outline_style", outline),
                    opt("style", if fill == 0 { "no" } else { "solid" }),
                ]
                .concat(),
            )
        }
    };
    format!(
        "<symbol type=\"{kind}\" name=\"{}\" alpha=\"1\" clip_to_extent=\"1\" force_rhr=\"0\"><layer class=\"{class}\" \
         enabled=\"1\" locked=\"0\" pass=\"0\"><Option type=\"Map\">{options}</Option></layer></symbol>",
        esc(name)
    )
}

/// The marker of an option's (a circle for one unknown).
fn marker_of(o: &VectorOptions) -> &'static str {
    MARKERS.get(o.marker).copied().unwrap_or(MARKERS[0])
}

/// A vector layer's renderer: one symbol, of the colour given (polygons filled half transparent).
fn vector_renderer(geometry: &str, c: Color32, o: &VectorOptions) -> String {
    let l = VectorLook { colour: c, fill: 90.0 / 255.0, width_mm: 0.4, marker: marker_of(o) };
    if o.rules { rules(geometry, &[], l) } else { single(geometry, l) }
}

fn single(geometry: &str, l: VectorLook) -> String {
    format!(
        "<renderer-v2 type=\"singleSymbol\" symbollevels=\"0\" enableorderby=\"0\" forceraster=\"0\"><symbols>{}\
         </symbols></renderer-v2>",
        symbol("0", geometry, l)
    )
}

/// A rule of a rule-based renderer: its filter (an expression of QGIS's), its label, how its features are drawn.
struct Rule {
    filter: String,
    label: String,
    look: VectorLook,
}

/// QGIS's rule-based renderer of rules: theirs, then one for the features of none (`ELSE`, drawn as `other`); of
/// no rules, one that draws every feature.
fn rules(geometry: &str, given: &[Rule], other: VectorLook) -> String {
    let mut out = String::new();
    let mut symbols = String::new();
    for (i, r) in given.iter().enumerate() {
        out.push_str(&format!(
            "<rule key=\"{{terraflux-rule-{i}}}\" filter=\"{}\" label=\"{}\" symbol=\"{i}\"/>",
            esc(&r.filter),
            esc(&r.label)
        ));
        symbols.push_str(&symbol(&i.to_string(), geometry, r.look));
    }
    let k = given.len();
    let (filter, label) = if given.is_empty() { ("", "all") } else { (" filter=\"ELSE\"", "other") };
    out.push_str(&format!("<rule key=\"{{terraflux-rule-{k}}}\"{filter} label=\"{label}\" symbol=\"{k}\"/>"));
    symbols.push_str(&symbol(&k.to_string(), geometry, other));
    format!(
        "<renderer-v2 type=\"RuleRenderer\" symbollevels=\"0\" enableorderby=\"0\" forceraster=\"0\">\
         <rules key=\"{{terraflux-rules}}\">{out}</rules><symbols>{symbols}</symbols></renderer-v2>"
    )
}

/// An attribute in an expression of QGIS's.
fn field(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

/// A text in an expression of QGIS's.
fn quoted(text: &str) -> String {
    format!("'{}'", text.replace('\'', "''"))
}

/// A vector layer's labels in QGIS: the attribute's values, dark on a light halo, placed as its geometry takes
/// them (around a point, along a line, over a polygon's middle).
fn labeling(geometry: &str, attr: &str) -> String {
    let placement = match geometry {
        "Point" => 6,
        "Line" => 2,
        _ => 1,
    };
    format!(
        "<labeling type=\"simple\"><settings calloutType=\"simple\"><text-style fieldName=\"{}\" isExpression=\"0\" \
         fontSize=\"9\" fontSizeUnit=\"Point\" textColor=\"35,35,35,255\" textOpacity=\"1\" fontWeight=\"50\" \
         fontItalic=\"0\" multilineHeight=\"1\"><text-buffer bufferDraw=\"1\" bufferSize=\"0.8\" \
         bufferSizeUnits=\"MM\" bufferColor=\"255,255,255,255\" bufferOpacity=\"1\" bufferJoinStyle=\"128\"/>\
         </text-style><text-format wrapChar=\"\" multilineAlign=\"0\"/><placement placement=\"{placement}\" dist=\"1\" \
         distUnits=\"MM\" priority=\"5\"/><rendering obstacle=\"1\" drawLabels=\"1\" labelPerPart=\"0\" \
         displayAll=\"0\" limitNumLabels=\"0\" scaleVisibility=\"0\"/></settings></labeling>",
        esc(attr)
    )
}

/// The ranges of a graduated renderer of values (the Viewer's sequential ramp across them).
const RANGES: usize = 7;

/// A vector layer's renderer in its style in the Viewer: one colour, or by an attribute (its classes, a colour
/// each, as the Viewer gives them; a range of numbers in ranges along the ramp); as rules when the options say.
fn styled_renderer(
    geometry: &str,
    v: &terraflux_raster::vector::Vector,
    style: &crate::vector_tiles::Style,
    o: &VectorOptions,
) -> String {
    use terraflux_raster::vector::Column;
    // (the Viewer's widths in pixels: about a quarter of a millimetre each)
    let base =
        VectorLook { colour: style.colour, fill: style.fill, width_mm: style.width * 0.26, marker: marker_of(o) };
    let Some((attr, column)) = style.by.as_ref().and_then(|b| v.column(b).map(|c| (b, c))) else {
        return if o.rules { rules(geometry, &[], base) } else { single(geometry, base) };
    };
    let colours = crate::vector_tiles::colours(v, style);
    let head = |kind: &str, extra: &str| {
        format!(
            "<renderer-v2 type=\"{kind}\" attr=\"{}\"{extra} symbollevels=\"0\" enableorderby=\"0\" forceraster=\"0\">",
            esc(attr)
        )
    };
    // classes: each value its colour (that of its first feature)
    let categorized = |values: Vec<String>, numbers: bool| {
        if o.rules {
            let mut given: Vec<Rule> = Vec::new();
            for (k, value) in values.iter().enumerate() {
                if given.iter().any(|r| r.label == *value) {
                    continue;
                }
                let filter = if value.is_empty() {
                    format!("{} IS NULL", field(attr))
                } else if numbers {
                    format!("{} = {value}", field(attr))
                } else {
                    format!("{} = {}", field(attr), quoted(value))
                };
                given.push(Rule { filter, label: value.clone(), look: VectorLook { colour: colours[k], ..base } });
            }
            return rules(geometry, &given, base);
        }
        let mut cats = String::new();
        let mut symbols = String::new();
        let mut seen: Vec<&String> = Vec::new();
        for (k, value) in values.iter().enumerate() {
            if seen.contains(&value) {
                continue;
            }
            seen.push(value);
            let i = seen.len() - 1;
            cats.push_str(&format!(
                "<category value=\"{v}\" label=\"{v}\" symbol=\"{i}\" render=\"true\"/>",
                v = esc(value)
            ));
            symbols.push_str(&symbol(&i.to_string(), geometry, VectorLook { colour: colours[k], ..base }));
        }
        format!(
            "{}<categories>{cats}</categories><symbols>{symbols}</symbols></renderer-v2>",
            head("categorizedSymbol", "")
        )
    };
    match column {
        Column::Texts(t) => categorized(t.clone(), false),
        Column::Numbers(x) => {
            let mut distinct: Vec<f64> = x.iter().copied().filter(|v| !v.is_nan()).collect();
            distinct.sort_by(f64::total_cmp);
            distinct.dedup();
            if distinct.len() <= 24 && distinct.iter().all(|v| v.fract() == 0.0) {
                return categorized(x.iter().map(|v| if v.is_nan() { String::new() } else { num(*v) }).collect(), true);
            }
            let (lo, hi) = (distinct.first().copied().unwrap_or(0.0), distinct.last().copied().unwrap_or(1.0));
            let span = if hi > lo { hi - lo } else { 1.0 };
            if o.rules {
                // (as QGIS's ranges: above the lower bound, up to the upper; the first from its lower bound)
                let given: Vec<Rule> = (0..RANGES)
                    .map(|i| {
                        let (a, b) = (
                            tidy(lo + span * i as f64 / RANGES as f64),
                            tidy(lo + span * (i + 1) as f64 / RANGES as f64),
                        );
                        let above = if i == 0 { ">=" } else { ">" };
                        let f = field(attr);
                        Rule {
                            filter: format!("{f} {above} {} AND {f} <= {}", num(a), num(b)),
                            label: format!("{} - {}", crate::animation::short(a), crate::animation::short(b)),
                            look: VectorLook {
                                colour: crate::palette::sequential((i as f32 + 0.5) / RANGES as f32),
                                ..base
                            },
                        }
                    })
                    .collect();
                return rules(geometry, &given, base);
            }
            let mut ranges = String::new();
            let mut symbols = String::new();
            for i in 0..RANGES {
                let (a, b) =
                    (tidy(lo + span * i as f64 / RANGES as f64), tidy(lo + span * (i + 1) as f64 / RANGES as f64));
                let mid = (i as f32 + 0.5) / RANGES as f32;
                ranges.push_str(&format!(
                    "<range lower=\"{}\" upper=\"{}\" label=\"{} - {}\" symbol=\"{i}\" render=\"true\"/>",
                    num(a),
                    num(b),
                    esc(&crate::animation::short(a)),
                    esc(&crate::animation::short(b))
                ));
                let colour = crate::palette::sequential(mid);
                symbols.push_str(&symbol(&i.to_string(), geometry, VectorLook { colour, ..base }));
            }
            format!(
                "{}<ranges>{ranges}</ranges><symbols>{symbols}</symbols></renderer-v2>",
                head("graduatedSymbol", " graduatedMethod=\"GraduatedColor\"")
            )
        }
    }
}

/// Files in a zip archive (deflated), as QGIS's `.qgz` holds its project.
fn zip(files: &[(&str, &[u8])]) -> Vec<u8> {
    let (mut out, mut central) = (Vec::new(), Vec::new());
    for (name, data) in files {
        let crc = crate::figure::crc32(data);
        let packed = miniz_oxide::deflate::compress_to_vec(data, 6);
        let offset = out.len() as u32;
        // the version needed (2.0), no flags, deflated, at 1980-01-01 00:00
        let common = |v: &mut Vec<u8>| {
            v.extend(20u16.to_le_bytes());
            v.extend(0u16.to_le_bytes());
            v.extend(8u16.to_le_bytes());
            v.extend(0u16.to_le_bytes());
            v.extend(0x21u16.to_le_bytes());
            v.extend(crc.to_le_bytes());
            v.extend((packed.len() as u32).to_le_bytes());
            v.extend((data.len() as u32).to_le_bytes());
            v.extend((name.len() as u16).to_le_bytes());
            v.extend(0u16.to_le_bytes());
        };
        out.extend(0x0403_4b50u32.to_le_bytes());
        common(&mut out);
        out.extend(name.as_bytes());
        out.extend(&packed);
        central.extend(0x0201_4b50u32.to_le_bytes());
        central.extend(20u16.to_le_bytes());
        common(&mut central);
        // no comment, disk 0, no attributes (2 + 2 + 2 + 4 bytes), then the local header's place
        central.extend([0u8; 10]);
        central.extend(offset.to_le_bytes());
        central.extend(name.as_bytes());
    }
    let (at, size) = (out.len() as u32, central.len() as u32);
    out.extend(central);
    out.extend(0x0605_4b50u32.to_le_bytes());
    out.extend([0u8; 4]);
    out.extend((files.len() as u16).to_le_bytes());
    out.extend((files.len() as u16).to_le_bytes());
    out.extend(size.to_le_bytes());
    out.extend(at.to_le_bytes());
    out.extend(0u16.to_le_bytes());
    out
}

/// The colours of the vector layers, one after the other (Okabe and Ito's).
const VECTOR_COLOURS: [Color32; 6] = [
    Color32::from_rgb(0, 114, 178),
    Color32::from_rgb(213, 94, 0),
    Color32::from_rgb(0, 158, 115),
    Color32::from_rgb(204, 121, 167),
    Color32::from_rgb(230, 159, 0),
    Color32::from_rgb(86, 180, 233),
];

/// A layer of the project.
struct Written {
    name: String,
    /// What QGIS opens: a file (relative to the project's folder, or absolute), a GeoPackage's layer.
    source: String,
    crs: Option<String>,
    kind: Kind,
}

enum Kind {
    Raster {
        renderer: String,
    },
    /// (its labels: none, or QGIS's labelling of them)
    Vector {
        geometry: &'static str,
        renderer: String,
        labeling: String,
    },
}

/// The coordinate system of a layer in the project: as WKT, or its EPSG code (QGIS takes the file's own when
/// there is none).
fn srs(crs: Option<&str>) -> String {
    match crs {
        Some(c) if c.starts_with("EPSG:") => {
            format!("<srs><spatialrefsys><authid>{}</authid></spatialrefsys></srs>", esc(c))
        }
        Some(c) => format!("<srs><spatialrefsys><wkt>{}</wkt></spatialrefsys></srs>", esc(c)),
        None => String::new(),
    }
}

/// The project: the layers in the tree and in the project, the first one on top.
fn project(title: &str, layers: &[Written]) -> String {
    let ids: Vec<String> = layers
        .iter()
        .enumerate()
        .map(|(i, l)| {
            let clean: String = l.name.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
            // (QGIS keeps a layer's id only when it is longer than 10 characters, else gives it another)
            format!("{clean}_{i:02}_terraflux")
        })
        .collect();
    let mut tree = String::new();
    let mut maps = String::new();
    for (l, id) in layers.iter().zip(&ids) {
        let provider = match l.kind {
            Kind::Raster { .. } => "gdal",
            Kind::Vector { .. } => "ogr",
        };
        tree.push_str(&format!(
            "<layer-tree-layer expanded=\"1\" checked=\"Qt::Checked\" id=\"{id}\" name=\"{}\" source=\"{}\" \
             providerKey=\"{provider}\"><customproperties/></layer-tree-layer>",
            esc(&l.name),
            esc(&l.source)
        ));
        let (head, body) = match &l.kind {
            Kind::Raster { renderer } => ("<maplayer type=\"raster\">".to_string(), format!("<pipe>{renderer}</pipe>")),
            Kind::Vector { geometry, renderer, labeling } => {
                // (labels drawn only when the layer says so)
                let on = if labeling.is_empty() { "" } else { " labelsEnabled=\"1\"" };
                (format!("<maplayer type=\"vector\" geometry=\"{geometry}\"{on}>"), format!("{renderer}{labeling}"))
            }
        };
        let encoding = if provider == "ogr" { " encoding=\"UTF-8\"" } else { "" };
        maps.push_str(&format!(
            "{head}<id>{id}</id><datasource>{}</datasource><layername>{}</layername>{}\
             <provider{encoding}>{provider}</provider>{body}<blendMode>0</blendMode></maplayer>",
            esc(&l.source),
            esc(&l.name),
            srs(l.crs.as_deref())
        ));
    }
    let custom: String = ids.iter().map(|id| format!("<item>{id}</item>")).collect();
    let order: String = ids.iter().map(|id| format!("<layer id=\"{id}\"/>")).collect();
    let crs = layers.iter().find_map(|l| l.crs.as_deref()).map(|c| srs(Some(c))).unwrap_or_default();
    // (QGIS reads the project's coordinate system only when its projections are on)
    let crs = crs.replace("<srs>", "<projectCrs>").replace("</srs>", "</projectCrs>");
    format!(
        "<!DOCTYPE qgis PUBLIC 'http://mrcc.com/qgis.dtd' 'SYSTEM'>\n<qgis projectname=\"{t}\" version=\"{QGIS_VERSION}\">\
         <title>{t}</title>{crs}<layer-tree-group><customproperties/>{tree}<custom-order enabled=\"0\">{custom}\
         </custom-order></layer-tree-group><projectlayers>{maps}</projectlayers><layerorder>{order}</layerorder>\
         <properties><Paths><Absolute type=\"bool\">false</Absolute></Paths><SpatialRefSys>\
         <ProjectionsEnabled type=\"int\">1</ProjectionsEnabled></SpatialRefSys></properties></qgis>\n",
        t = esc(title)
    )
}

/// A file's name of a layer's name (letters, digits, `-` and `_`).
fn file_stem(name: &str) -> String {
    let s: String = name.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
    let s = s.trim_matches('_').to_string();
    if s.is_empty() { "layer".into() } else { s }
}

/// The layers written into `dir` and the project there that opens them (`title.qgz`, zipped as QGIS writes its
/// own, or `title.qgs` when `plain`); the project's file.
pub fn export(picks: &[Pick], dir: &Path, title: &str, plain: bool) -> Result<PathBuf, String> {
    if picks.is_empty() {
        return Err("no map chosen".into());
    }
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let mut stems = BTreeSet::new();
    let mut rasters = Vec::new();
    let mut vectors = Vec::new();
    for p in picks {
        let mut stem = file_stem(&p.name);
        let mut k = 2;
        while !stems.insert(stem.clone()) {
            stem = format!("{}_{k}", file_stem(&p.name));
            k += 1;
        }
        if let Value::Vector(v) = &p.value {
            let file = terraflux_exec::write_value(&p.value, &dir.join(format!("{stem}.gpkg")), None)
                .map_err(|e| format!("{}: {e}", p.name))?;
            let geometry = geometry_of(v);
            let o = &p.options;
            let renderer = match &p.style {
                Some(style) => styled_renderer(geometry, v, style, o),
                None => vector_renderer(geometry, VECTOR_COLOURS[vectors.len() % VECTOR_COLOURS.len()], o),
            };
            // (labelled by an attribute it has)
            let labeling = match &o.label {
                Some(attr) if v.column(attr).is_some() => labeling(geometry, attr),
                _ => String::new(),
            };
            vectors.push(Written {
                name: p.name.clone(),
                source: format!("./{}|layername={stem}", file.file_name().unwrap().to_string_lossy()),
                crs: v.crs.clone(),
                kind: Kind::Vector { geometry, renderer, labeling },
            });
            continue;
        }
        let Some(src) = band_source(&p.value, p.band) else { continue };
        let band = p.band.min(layers(&p.value).saturating_sub(1));
        // rasters in one file referred to where they are (not copied); other maps written
        let (source, band_no, own) = match &p.value {
            Value::Raster(r) if r.images.len() == 1 => {
                (r.images[0].path().to_string_lossy().into_owned(), r.layers[band].band + 1, false)
            }
            _ => {
                let file = terraflux_exec::write_value(&p.value, &dir.join(format!("{stem}.tif")), None)
                    .map_err(|e| format!("{}: {e}", p.name))?;
                (format!("./{}", file.file_name().unwrap().to_string_lossy()), band + 1, true)
            }
        };
        let look = crate::builder::ranged(crate::builder::look_of(&src, p.palette), p.range);
        let look = match &p.legend {
            Some(c) => crate::legend_view::apply(&look, c),
            None => look,
        };
        let renderer = renderer(&look, p.legend.as_deref(), band_no);
        if own {
            std::fs::write(dir.join(format!("{stem}.qml")), qml(&renderer)).map_err(|e| e.to_string())?;
        }
        let g = src.grid();
        // (an EPSG code as WKT when GDAL gives it: QGIS takes a project's coordinate system only so)
        let crs = g.geo.wkt.clone().or_else(|| {
            g.geo.epsg().map(|c| terraflux_gdal::crs_wkt(&format!("EPSG:{c}")).unwrap_or(format!("EPSG:{c}")))
        });
        rasters.push(Written { name: p.name.clone(), source, crs, kind: Kind::Raster { renderer } });
    }
    vectors.extend(rasters);
    let stem = file_stem(title);
    let xml = project(title, &vectors);
    if plain {
        let path = dir.join(format!("{stem}.qgs"));
        std::fs::write(&path, xml).map_err(|e| e.to_string())?;
        return Ok(path);
    }
    let path = dir.join(format!("{stem}.qgz"));
    let name = format!("{stem}.qgs");
    std::fs::write(&path, zip(&[(&name, xml.as_bytes())])).map_err(|e| e.to_string())?;
    Ok(path)
}

/// The window of the export to QGIS: the maps and vector layers of the run to put in the project, the folder,
/// and the project made (opened in QGIS by a click).
#[derive(Default)]
pub struct Export {
    pub open: bool,
    /// The outputs chosen (node, port); none until the window first opens (then the model's outputs, the one
    /// shown and the vector layers drawn over it).
    pub chosen: Option<BTreeSet<(String, String)>>,
    job: Option<(PathBuf, Receiver<Result<PathBuf, String>>)>,
    pub done: Option<Result<PathBuf, String>>,
    /// The project as text (`.qgs`), not zipped as QGIS keeps its own (`.qgz`).
    pub plain: bool,
    /// The options of the vector layers (node, port): their labels, marker, rules.
    pub options: BTreeMap<(String, String), VectorOptions>,
}

/// The maps and vector layers of the run, in the model's order: (node, port, their name).
pub fn candidates(s: &Session) -> Vec<(String, String, String)> {
    let mut out = Vec::new();
    for n in &s.source.nodes {
        let Some(ports) = s.outputs.get(&n.id) else { continue };
        for (port, v) in ports {
            if exportable(v) {
                out.push((n.id.clone(), port.clone(), name_of(s, &n.id, port)));
            }
        }
    }
    out
}

/// An output's name: the model's output that writes it, else as the viewer names it.
fn name_of(s: &Session, node: &str, port: &str) -> String {
    let at = format!("{node}.{port}");
    match s.source.outputs.iter().find(|(_, o)| o.from == at && o.layer.is_none()) {
        Some((name, _)) => name.clone(),
        None => crate::builder::output_name(node, port),
    }
}

/// What to export of an output: its value, palette, legend and the layer shown.
pub fn pick(s: &Session, node: &str, port: &str) -> Option<Pick> {
    let value = s.value_of(node, port)?;
    let shown = s.preview.as_ref().is_some_and(|(n, p)| n == node && p == port);
    let band = if shown {
        s.preview_band
    } else {
        match &value {
            // a series: its last step (where the simulation got to); rasters: the band chosen
            Value::Series(items) => items.len().saturating_sub(1),
            Value::Raster(r) => r.chosen,
            _ => 0,
        }
    };
    let legend = band_source(&value, band).and_then(|src| crate::legend_view::legend_of(s, node, port, &src));
    // a vector layer as the Viewer draws it over the map
    // (a layer kept, not the map shown: the shown one is drawn in its own colours)
    let shown = s.preview.as_ref().is_some_and(|(n, p)| n == node && p == port);
    let style = (!shown).then(|| s.preview_layers.iter().find(|l| l.is(node, port)).map(|l| l.style.clone())).flatten();
    Some(Pick {
        name: name_of(s, node, port),
        palette: crate::builder::palette_of(s, node, port),
        range: crate::builder::range_of(s, node, port),
        legend,
        band,
        value,
        style,
        options: s.qgis.options.get(&(node.to_string(), port.to_string())).cloned().unwrap_or_default(),
    })
}

/// The attributes of a vector output and its kind of geometry as QGIS names it.
fn vector_info(s: &Session, node: &str, port: &str) -> Option<(Vec<String>, &'static str)> {
    match s.value_of(node, port)? {
        Value::Vector(v) => Some((v.attributes.iter().map(|(k, _)| k.clone()).collect(), geometry_of(&v))),
        _ => None,
    }
}

/// The options of a vector layer chosen: the attribute it is labelled by, its points' marker, its style as rules.
fn options_ui(ui: &mut egui::Ui, id: &str, o: &mut VectorOptions, attrs: &[String], geometry: &str) {
    ui.horizontal_wrapped(|ui| {
        ui.add_space(22.0);
        ui.label(RichText::new("labels").size(11.5));
        egui::ComboBox::from_id_salt(("qgis label", id))
            .selected_text(o.label.as_deref().unwrap_or("none"))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut o.label, None, "none");
                for a in attrs {
                    ui.selectable_value(&mut o.label, Some(a.clone()), a);
                }
            })
            .response
            .on_hover_text("the attribute written by each feature in QGIS");
        if geometry == "Point" {
            ui.label(RichText::new("marker").size(11.5));
            egui::ComboBox::from_id_salt(("qgis marker", id)).selected_text(marker_of(o)).show_ui(ui, |ui| {
                for (k, m) in MARKERS.iter().enumerate() {
                    ui.selectable_value(&mut o.marker, k, *m);
                }
            });
        }
        ui.checkbox(&mut o.rules, RichText::new("as rules").size(11.5)).on_hover_text(
            "the style as QGIS's rules: a rule each class or range, and one for the features of none, to change \
             there as rules",
        );
    });
}

/// The window.
pub fn window(ctx: &egui::Context, s: &mut Session) {
    if !s.qgis.open {
        return;
    }
    let all = candidates(s);
    if s.qgis.chosen.is_none() {
        let mut c: BTreeSet<(String, String)> = all
            .iter()
            .filter(|(n, p, _)| s.source.outputs.values().any(|o| o.from == format!("{n}.{p}")))
            .map(|(n, p, _)| (n.clone(), p.clone()))
            .collect();
        if let Some(p) = &s.preview
            && all.iter().any(|(n, q, _)| (n, q) == (&p.0, &p.1))
        {
            c.insert(p.clone());
        }
        // and the layers drawn with it
        // (not a difference: no file of it)
        for l in s.preview_layers.iter().filter(|l| l.visible && l.minus.is_none()) {
            if all.iter().any(|(m, q, _)| (m, q) == (&l.node, &l.port)) {
                c.insert((l.node.clone(), l.port.clone()));
            }
        }
        s.qgis.chosen = Some(c);
    }
    // the project made
    if let Some((_, rx)) = &s.qgis.job
        && let Ok(r) = rx.try_recv()
    {
        s.qgis.done = Some(r);
        s.qgis.job = None;
    }
    let mut open = true;
    egui::Window::new("Export to QGIS")
        .open(&mut open)
        .default_size([480.0, 420.0])
        .resizable(true)
        .collapsible(false)
        .show(ctx, |ui| {
            let weak = ui.visuals().weak_text_color();
            ui.label(
                RichText::new(
                    "The maps and vector layers chosen, written to a folder with a QGIS project that opens them in the \
                     colours they have here (the classes with their names). A raster read from a file is referred to \
                     where it is, not copied.",
                )
                .size(11.5)
                .color(weak),
            );
            ui.add_space(4.0);
            if all.is_empty() {
                ui.label("Nothing to export yet: run the model (ctrl+enter runs the node shown).");
                return;
            }
            let info: BTreeMap<(String, String), (Vec<String>, &'static str)> =
                all.iter().filter_map(|(n, p, _)| Some(((n.clone(), p.clone()), vector_info(s, n, p)?))).collect();
            let Export { chosen, options, .. } = &mut s.qgis;
            let chosen = chosen.as_mut().unwrap();
            ui.horizontal(|ui| {
                if ui.small_button("all").clicked() {
                    *chosen = all.iter().map(|(n, p, _)| (n.clone(), p.clone())).collect();
                }
                if ui.small_button("none").clicked() {
                    chosen.clear();
                }
            });
            egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                for (n, p, name) in &all {
                    let key = (n.clone(), p.clone());
                    let mut on = chosen.contains(&key);
                    if ui.checkbox(&mut on, name).changed() {
                        if on {
                            chosen.insert(key.clone());
                        } else {
                            chosen.remove(&key);
                        }
                    }
                    // a vector layer's: its labels, marker and rules
                    if on && let Some((attrs, geometry)) = info.get(&key) {
                        let o = options.entry(key.clone()).or_default();
                        options_ui(ui, &format!("{n}.{p}"), o, attrs, geometry);
                    }
                }
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label("Project");
                ui.radio_value(&mut s.qgis.plain, false, ".qgz").on_hover_text("zipped, as QGIS keeps its own");
                ui.radio_value(&mut s.qgis.plain, true, ".qgs").on_hover_text("as text (XML)");
            });
            let chosen = s.qgis.chosen.as_mut().unwrap();
            let keys: Vec<(String, String)> = chosen.iter().cloned().collect();
            let busy = s.qgis.job.is_some();
            let n = keys.len();
            ui.horizontal(|ui| {
                let label = format!("Export {n} layer{}...", if n == 1 { "" } else { "s" });
                if ui.add_enabled(n > 0 && !busy, egui::Button::new(label)).clicked()
                    && let Some(dir) = rfd::FileDialog::new().set_title("The folder of the QGIS project").pick_folder()
                {
                    let order: Vec<(String, String)> =
                        all.iter().map(|(n, p, _)| (n.clone(), p.clone())).filter(|k| keys.contains(k)).collect();
                    let picks: Vec<Pick> = order.iter().filter_map(|(n, p)| pick(s, n, p)).collect();
                    let title = match s.source.name.as_deref().map(str::trim).filter(|t| !t.is_empty()) {
                        Some(t) => t.to_string(),
                        None => s
                            .path
                            .as_ref()
                            .and_then(|p| p.file_stem())
                            .map_or("terraflux".to_string(), |f| f.to_string_lossy().into_owned()),
                    };
                    let (tx, rx) = channel();
                    let d = dir.clone();
                    let c = ui.ctx().clone();
                    let plain = s.qgis.plain;
                    std::thread::spawn(move || {
                        let _ = tx.send(export(&picks, &d, &title, plain));
                        c.request_repaint();
                    });
                    s.qgis.job = Some((dir, rx));
                    s.qgis.done = None;
                }
                if busy {
                    ui.spinner();
                    ui.label(RichText::new("writing the layers...").weak());
                }
            });
            match &s.qgis.done {
                Some(Ok(p)) => {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new(format!("written: {}", p.display())).size(11.5));
                        if ui
                            .button("Open in QGIS")
                            .on_hover_text("the project, in the program that opens .qgz and .qgs files")
                            .clicked()
                        {
                            crate::batch_view::open_folder(p);
                        }
                        if ui.small_button("folder").clicked() {
                            crate::batch_view::open_folder(p.parent().unwrap_or(p));
                        }
                    });
                }
                Some(Err(e)) => {
                    ui.colored_label(ui.visuals().error_fg_color, format!("not written: {e}"));
                }
                None => {}
            }
        });
    if !open {
        s.qgis.open = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn look(coloring: Coloring) -> Look {
        Look { coloring, counts: Vec::new(), product: None }
    }

    #[test]
    fn classes_as_a_paletted_map() {
        let l =
            look(Coloring::Classes(vec![(1.0, Color32::from_rgb(0, 114, 178)), (2.0, Color32::from_rgb(230, 159, 0))]));
        let legend = vec![Class { value: 1.0, name: "forest & <wood>".into(), colour: "#0072b2".into() }];
        let r = renderer(&l, Some(&legend), 2);
        assert!(r.starts_with("<rasterrenderer type=\"paletted\" band=\"2\""), "{r}");
        assert!(r.contains(
            "<paletteEntry value=\"1\" color=\"#0072b2\" alpha=\"255\" label=\"forest &amp; &lt;wood&gt;\"/>"
        ));
        assert!(r.contains("<paletteEntry value=\"2\" color=\"#e69f00\" alpha=\"255\" label=\"2\"/>"));
    }

    #[test]
    fn values_on_a_ramp() {
        let ramp = crate::palette::Ramp::default();
        let r = renderer(&look(Coloring::Continuous { min: 0.0, max: 10.0, ramp, log: false }), None, 1);
        assert!(r.contains("type=\"singlebandpseudocolor\" band=\"1\""));
        assert_eq!(r.matches("<item ").count(), RAMP_STOPS);
        assert!(r.contains(&format!("<item value=\"0\" color=\"{}\"", hex(ramp.colour(0.0)))));
        assert!(r.contains(&format!("<item value=\"10\" color=\"{}\"", hex(ramp.colour(1.0)))));
        // by the logarithm: 1, 10, 100... between 1 and 10^10
        let r = renderer(&look(Coloring::Continuous { min: 1.0, max: 1e10, ramp, log: true }), None, 1);
        assert!(r.contains("<item value=\"10\" ") && r.contains("<item value=\"100000\" "), "{r}");
    }

    #[test]
    fn a_project_of_maps_and_features() {
        let dir = std::env::temp_dir().join(format!("terraflux-qgis-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut g = terraflux_raster::Grid::new(4, 3);
        g.transform = [500000.0, 30.0, 0.0, 9000000.0, 0.0, -30.0];
        // a UTM zone (as GDAL gives its EPSG code), written as WKT in the project and its layers
        let wkt = terraflux_gdal::crs_wkt("EPSG:32651").unwrap();
        assert!(wkt.contains("UTM zone 51N"), "{wkt}");
        g.geo.wkt = Some(wkt);
        let g = Arc::new(g);
        let m = Map::new(g.clone(), vec![1.0, 1.0, 2.0, 2.0, 1.0, f64::NAN, 2.0, 2.0, 1.0, 1.0, 1.0, 2.0]);
        let ramp = Map::new(g.clone(), (0..12).map(|i| i as f64 * 0.5).collect());
        let pts = terraflux_raster::vector::Vector {
            geometries: vec![Geometry::Point([500030.0, 8999950.0])],
            attributes: vec![("name".into(), terraflux_raster::vector::Column::Texts(vec!["plot 1".into()]))],
            crs: None,
        };
        let picks = vec![
            Pick {
                name: "land use".into(),
                value: Value::Map(m.clone()),
                palette: Palette::Auto,
                range: None,
                legend: None,
                band: 0,
                style: None,
                options: VectorOptions::default(),
            },
            Pick {
                name: "land use".into(),
                value: Value::Map(ramp),
                palette: Palette::Auto,
                range: None,
                legend: None,
                band: 0,
                style: None,
                options: VectorOptions::default(),
            },
            Pick {
                name: "plots".into(),
                value: Value::Vector(Arc::new(pts)),
                palette: Palette::Auto,
                range: None,
                legend: None,
                band: 0,
                style: None,
                options: VectorOptions { label: Some("name".into()), marker: 1, rules: false },
            },
        ];
        let qgs = export(&picks, &dir, "A <model>", true).unwrap();
        assert_eq!(qgs, dir.join("A__model.qgs"));
        for f in ["land_use.tif", "land_use.qml", "land_use_2.tif", "land_use_2.qml", "plots.gpkg"] {
            assert!(dir.join(f).exists(), "{f}");
        }
        let x = std::fs::read_to_string(&qgs).unwrap();
        assert!(x.contains("<title>A &lt;model&gt;</title>"));
        assert!(x.contains("<projectCrs><spatialrefsys><wkt>PROJCS[&quot;WGS 84 / UTM zone 51N&quot;"), "{x}");
        assert_eq!(x.matches("<srs><spatialrefsys><wkt>PROJCS").count(), 2, "both maps'");
        // (without it QGIS leaves the project's coordinate system unset)
        assert!(x.contains("<ProjectionsEnabled type=\"int\">1</ProjectionsEnabled>"));
        // the vector layer on top, then the maps in their order
        let at = |s: &str| x.find(s).unwrap_or_else(|| panic!("{s} in {x}"));
        assert!(at("name=\"plots\"") < at("name=\"land use\""));
        assert!(at("<datasource>./land_use.tif</datasource>") < at("<datasource>./land_use_2.tif</datasource>"));
        assert!(x.contains("<datasource>./plots.gpkg|layername=plots</datasource>"));
        assert!(x.contains("type=\"paletted\"") && x.contains("type=\"singlebandpseudocolor\""));
        assert!(x.contains("<layerorder><layer id=\"plots_00_terraflux\"/>"), "{x}");
        // the points labelled by their name, drawn as squares
        assert!(x.contains("<maplayer type=\"vector\" geometry=\"Point\" labelsEnabled=\"1\">"), "{x}");
        assert!(x.contains("<labeling type=\"simple\">") && x.contains("fieldName=\"name\""), "{x}");
        assert!(x.contains("placement=\"6\""));
        assert!(x.contains("<Option name=\"name\" type=\"QString\" value=\"square\"/>"), "{x}");
        let qml = std::fs::read_to_string(dir.join("land_use.qml")).unwrap();
        assert!(qml.contains("<pipe><rasterrenderer type=\"paletted\""));
        // nothing chosen: said
        assert!(export(&[], &dir, "x", true).is_err());
        // zipped (.qgz): the same project inside, as QGIS reads it
        let qgz = export(&picks, &dir, "A <model>", false).unwrap();
        assert_eq!(qgz, dir.join("A__model.qgz"));
        let bytes = std::fs::read(&qgz).unwrap();
        assert_eq!(&bytes[..4], b"PK\x03\x04");
        let (packed, size, name) = (
            u32::from_le_bytes(bytes[18..22].try_into().unwrap()) as usize,
            u32::from_le_bytes(bytes[22..26].try_into().unwrap()) as usize,
            u16::from_le_bytes(bytes[26..28].try_into().unwrap()) as usize,
        );
        assert_eq!(&bytes[30..30 + name], b"A__model.qgs");
        let inner = miniz_oxide::inflate::decompress_to_vec(&bytes[30 + name..30 + name + packed]).unwrap();
        assert_eq!(inner.len(), size);
        assert_eq!(String::from_utf8(inner).unwrap(), x);
        // the end of the central directory (the last 22 bytes): one entry, where the directory is and how long
        let end = &bytes[bytes.len() - 22..];
        assert_eq!(&end[..4], b"PK\x05\x06");
        assert_eq!(u16::from_le_bytes(end[10..12].try_into().unwrap()), 1);
        let (size_cd, at) = (
            u32::from_le_bytes(end[12..16].try_into().unwrap()) as usize,
            u32::from_le_bytes(end[16..20].try_into().unwrap()) as usize,
        );
        assert_eq!(at + size_cd, bytes.len() - 22);
        // its entry: the file's name 46 bytes in, the local header at 0
        let cd = &bytes[at..at + size_cd];
        assert_eq!(&cd[..4], b"PK\x01\x02");
        assert_eq!(u32::from_le_bytes(cd[42..46].try_into().unwrap()), 0);
        assert_eq!(&cd[46..], b"A__model.qgs");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn vectors_in_the_viewers_colours() {
        use terraflux_raster::vector::{Column, Vector};
        let v = Vector {
            geometries: vec![Geometry::Point([0.0, 0.0]), Geometry::Point([1.0, 1.0]), Geometry::Point([2.0, 2.0])],
            attributes: vec![
                ("use".into(), Column::Texts(vec!["forest".into(), "pasture".into(), "forest".into()])),
                ("area".into(), Column::Numbers(vec![0.5, 12.25, 100.0])),
            ],
            crs: None,
        };
        let one = crate::vector_tiles::Style {
            colour: Color32::from_rgb(10, 20, 30),
            by: None,
            fill: 0.5,
            width: 2.0,
            ..Default::default()
        };
        let plain = VectorOptions::default();
        let r = styled_renderer("Polygon", &v, &one, &plain);
        assert!(r.contains("type=\"singleSymbol\"") && r.contains("value=\"10,20,30,128\""), "{r}");
        assert!(r.contains("name=\"outline_width\" type=\"QString\" value=\"0.52\""), "{r}");
        // by the classes of a text: a colour each, as the Viewer gives them
        let by_use = crate::vector_tiles::Style { by: Some("use".into()), ..one.clone() };
        let r = styled_renderer("Point", &v, &by_use, &plain);
        let colours = crate::vector_tiles::colours(&v, &by_use);
        assert!(r.contains("type=\"categorizedSymbol\" attr=\"use\""), "{r}");
        assert_eq!(r.matches("<category ").count(), 2);
        let rgb = |c: Color32| format!("{},{},{},255", c.r(), c.g(), c.b());
        assert!(r.contains(&rgb(colours[0])) && r.contains(&rgb(colours[1])));
        // by a range of numbers: ranges along the ramp
        let by_area = crate::vector_tiles::Style { by: Some("area".into()), ..one.clone() };
        let r = styled_renderer("Line", &v, &by_area, &plain);
        assert!(r.contains("type=\"graduatedSymbol\" attr=\"area\""), "{r}");
        assert_eq!(r.matches("<range ").count(), RANGES);
        assert!(r.contains("lower=\"0.5\"") && r.contains("upper=\"100\""), "{r}");
        // as rules: a rule each class (a text quoted, its quote doubled), then the others'
        let as_rules = VectorOptions { rules: true, ..plain.clone() };
        let r = styled_renderer("Point", &v, &by_use, &as_rules);
        assert!(r.contains("type=\"RuleRenderer\""), "{r}");
        assert_eq!(r.matches("<rule ").count(), 3);
        assert!(r.contains("filter=\"&quot;use&quot; = &apos;forest&apos;\" label=\"forest\""), "{r}");
        assert!(r.contains("filter=\"ELSE\" label=\"other\""), "{r}");
        assert!(r.contains(&rgb(colours[1])));
        assert_eq!(quoted("farmer's"), "'farmer''s'");
        // ranges as rules: the first from its lower bound, the others above theirs
        let r = styled_renderer("Line", &v, &by_area, &as_rules);
        assert_eq!(r.matches("<rule ").count(), RANGES + 1);
        assert!(r.contains("&quot;area&quot; &gt;= 0.5 AND &quot;area&quot; &lt;="), "{r}");
        assert!(r.contains("&lt;= 100\""), "{r}");
        // one colour as rules: one rule of every feature, no ELSE
        let r = styled_renderer("Polygon", &v, &one, &as_rules);
        assert_eq!(r.matches("<rule ").count(), 1);
        assert!(!r.contains("ELSE") && r.contains("label=\"all\""), "{r}");
        // the marker chosen; a polygon's labels over its middle, a line's along it
        let star = VectorOptions { marker: 4, ..plain };
        assert!(styled_renderer("Point", &v, &one, &star).contains("value=\"star\""));
        assert!(labeling("Polygon", "use").contains("placement=\"1\""));
        assert!(labeling("Line", "use").contains("placement=\"2\""));
    }

    /// A `.qgz` of a map of classes, points labelled as squares and features styled as rules, written to the folder
    /// `TERRAFLUX_QGIS_DIR` for QGIS to read (`tools/check_qgz.py` in PyQGIS).
    #[test]
    #[ignore]
    fn a_project_for_pyqgis() {
        use terraflux_raster::vector::{Column, Vector};
        let dir = std::path::PathBuf::from(std::env::var("TERRAFLUX_QGIS_DIR").expect("TERRAFLUX_QGIS_DIR"));
        let _ = std::fs::remove_dir_all(&dir);
        let mut g = terraflux_raster::Grid::new(4, 3);
        g.transform = [500000.0, 30.0, 0.0, 9000000.0, 0.0, -30.0];
        g.geo.wkt = Some(terraflux_gdal::crs_wkt("EPSG:32651").unwrap());
        let g = Arc::new(g);
        let m = Map::new(g, vec![1.0, 1.0, 2.0, 2.0, 1.0, f64::NAN, 2.0, 2.0, 1.0, 1.0, 1.0, 3.0]);
        let pts = Vector {
            geometries: vec![Geometry::Point([500030.0, 8999950.0]), Geometry::Point([500060.0, 8999930.0])],
            attributes: vec![
                ("name".into(), Column::Texts(vec!["plot 1".into(), "farmer's".into()])),
                ("use".into(), Column::Texts(vec!["forest".into(), "pasture".into()])),
            ],
            crs: None,
        };
        let pts = Arc::new(pts);
        let by_use = crate::vector_tiles::Style {
            colour: Color32::from_rgb(10, 20, 30),
            by: Some("use".into()),
            fill: 0.5,
            width: 2.0,
            ..Default::default()
        };
        let pick = |name: &str, value: Value, style, options| Pick {
            name: name.into(),
            value,
            palette: Palette::Auto,
            range: None,
            legend: None,
            band: 0,
            style,
            options,
        };
        let picks = vec![
            pick("land use", Value::Map(m), None, VectorOptions::default()),
            pick(
                "plots",
                Value::Vector(pts.clone()),
                None,
                VectorOptions { label: Some("name".into()), marker: 1, rules: false },
            ),
            pick("uses", Value::Vector(pts), Some(by_use), VectorOptions { label: None, marker: 4, rules: true }),
        ];
        let qgz = export(&picks, &dir, "PyQGIS check", false).unwrap();
        println!("{}", qgz.display());
    }
}
