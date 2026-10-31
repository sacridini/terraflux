//! The maps and vector layers of a run as a QGIS project: each written to a folder (GeoTIFF, GeoPackage; rasters
//! read from a file referred to where they are), styled as the Viewer shows them (the classes in their colours
//! and with their names, as a paletted map; values on the ramp from its minimum to its maximum), with a `.qml`
//! style beside each map written (QGIS takes it when the map is added by hand), and the project (`.qgs`) that
//! opens them all in QGIS, the vector layers on top.

use std::collections::BTreeSet;
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
    pub legend: Option<Vec<Class>>,
    pub band: usize,
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

/// A vector layer's renderer: one symbol, of the colour given (polygons filled half transparent).
fn vector_renderer(geometry: &str, c: Color32) -> String {
    let rgb = format!("{},{},{}", c.r(), c.g(), c.b());
    let opt = |k: &str, v: &str| format!("<Option name=\"{k}\" type=\"QString\" value=\"{v}\"/>");
    let (kind, class, options) = match geometry {
        "Point" => (
            "marker",
            "SimpleMarker",
            [
                opt("name", "circle"),
                opt("color", &format!("{rgb},255")),
                opt("outline_color", "35,35,35,255"),
                opt("size", "2.4"),
            ]
            .concat(),
        ),
        "Line" => ("line", "SimpleLine", [opt("line_color", &format!("{rgb},255")), opt("line_width", "0.6")].concat()),
        _ => (
            "fill",
            "SimpleFill",
            [
                opt("color", &format!("{rgb},90")),
                opt("outline_color", &format!("{rgb},255")),
                opt("outline_width", "0.4"),
                opt("style", "solid"),
            ]
            .concat(),
        ),
    };
    format!(
        "<renderer-v2 type=\"singleSymbol\" symbollevels=\"0\" enableorderby=\"0\" forceraster=\"0\"><symbols>\
         <symbol type=\"{kind}\" name=\"0\" alpha=\"1\" clip_to_extent=\"1\" force_rhr=\"0\"><layer class=\"{class}\" \
         enabled=\"1\" locked=\"0\" pass=\"0\"><Option type=\"Map\">{options}</Option></layer></symbol></symbols>\
         </renderer-v2>"
    )
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
    Raster { renderer: String },
    Vector { geometry: &'static str, renderer: String },
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
            Kind::Vector { geometry, renderer } => {
                (format!("<maplayer type=\"vector\" geometry=\"{geometry}\">"), renderer.clone())
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

/// The layers written into `dir` and the project `title.qgs` there that opens them; the project's file.
pub fn export(picks: &[Pick], dir: &Path, title: &str) -> Result<PathBuf, String> {
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
            let colour = VECTOR_COLOURS[vectors.len() % VECTOR_COLOURS.len()];
            vectors.push(Written {
                name: p.name.clone(),
                source: format!("./{}|layername={stem}", file.file_name().unwrap().to_string_lossy()),
                crs: v.crs.clone(),
                kind: Kind::Vector { geometry, renderer: vector_renderer(geometry, colour) },
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
        let look = crate::builder::look_of(&src, p.palette);
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
    let path = dir.join(format!("{}.qgs", file_stem(title)));
    std::fs::write(&path, project(title, &vectors)).map_err(|e| e.to_string())?;
    Ok(path)
}

/// The window of the export to QGIS: the maps and vector layers of the run to put in the project, the folder,
/// and the project made (opened in QGIS by a click).
#[derive(Default)]
pub struct Export {
    pub open: bool,
    /// The outputs chosen (node, port); none until the window first opens (then the model's outputs and the one
    /// shown).
    pub chosen: Option<BTreeSet<(String, String)>>,
    job: Option<(PathBuf, Receiver<Result<PathBuf, String>>)>,
    pub done: Option<Result<PathBuf, String>>,
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
    Some(Pick { name: name_of(s, node, port), palette: crate::builder::palette_of(s, node, port), legend, band, value })
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
            let chosen = s.qgis.chosen.as_mut().unwrap();
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
                            chosen.insert(key);
                        } else {
                            chosen.remove(&key);
                        }
                    }
                }
            });
            ui.add_space(6.0);
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
                    std::thread::spawn(move || {
                        let _ = tx.send(export(&picks, &d, &title));
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
                            .on_hover_text("the project, in the program that opens .qgs files")
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
            attributes: vec![],
            crs: None,
        };
        let picks = vec![
            Pick {
                name: "land use".into(),
                value: Value::Map(m.clone()),
                palette: Palette::Auto,
                legend: None,
                band: 0,
            },
            Pick { name: "land use".into(), value: Value::Map(ramp), palette: Palette::Auto, legend: None, band: 0 },
            Pick {
                name: "plots".into(),
                value: Value::Vector(Arc::new(pts)),
                palette: Palette::Auto,
                legend: None,
                band: 0,
            },
        ];
        let qgs = export(&picks, &dir, "A <model>").unwrap();
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
        let qml = std::fs::read_to_string(dir.join("land_use.qml")).unwrap();
        assert!(qml.contains("<pipe><rasterrenderer type=\"paletted\""));
        // nothing chosen: said
        assert!(export(&[], &dir, "x").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
