//! What a raster or vector features are, as a list of named facts for the viewer's Metadata window: the files
//! and their format, the coordinate system, the cells (their number, size and extent), the bands, their type and
//! no-data value; of features their number, kinds, attributes and bounds.

use terraflux_ops::Value;
use terraflux_raster::Grid;
use terraflux_raster::vector::{Column, Geometry};

/// A fact: its name and value (the coordinate system's WKT is one, long).
pub type Fact = (String, String);

fn number(v: f64) -> String {
    if v.is_nan() {
        "none".into()
    } else if v == v.trunc() && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

/// The facts of a grid: its coordinate system, size, cells and extent.
fn grid_facts(g: &Grid, out: &mut Vec<Fact>) {
    let t = g.transform;
    let crs = crate::prepare::crs_of(g);
    out.push(("coordinate system".into(), crate::prepare::crs_name(crs.as_deref())));
    out.push(("size".into(), format!("{} x {} cells ({} cells)", g.width, g.height, g.width * g.height)));
    out.push(("cell size".into(), format!("{} x {}", number(t[1].abs()), number(t[5].abs()))));
    let (x0, y0) = (t[0], t[3]);
    let (x1, y1) = (t[0] + g.width as f64 * t[1], t[3] + g.height as f64 * t[5]);
    out.push((
        "extent".into(),
        format!(
            "xmin {}, ymin {}, xmax {}, ymax {}",
            number(x0.min(x1)),
            number(y0.min(y1)),
            number(x0.max(x1)),
            number(y0.max(y1))
        ),
    ));
    out.push(("origin".into(), format!("{}, {} (top left)", number(t[0]), number(t[3]))));
    if t[2] != 0.0 || t[4] != 0.0 {
        out.push(("rotation".into(), format!("{}, {}", number(t[2]), number(t[4]))));
    }
    for (k, v) in &g.geo.metadata {
        out.push((k.clone(), v.clone()));
    }
    if let Some(w) = crs {
        out.push(("WKT".into(), w));
    }
}

/// The facts of a value (rasters, a map, a stack, a series of maps, vector features); none for others.
pub fn facts(v: &Value) -> Vec<Fact> {
    let mut out = Vec::new();
    match v {
        Value::Raster(r) => {
            let files: Vec<String> = r.images.iter().map(|i| i.path().display().to_string()).collect();
            out.push((if files.len() == 1 { "file" } else { "files" }.into(), files.join("\n")));
            out.push(("read by".into(), r.images[0].reader()));
            out.push(("layers".into(), r.layers.iter().map(|l| l.name.clone()).collect::<Vec<_>>().join(", ")));
            out.push(("data type".into(), format!("{:?}", r.dtype()).to_lowercase()));
            out.push(("no data".into(), r.layers[r.chosen].nodata.map_or("none".into(), number)));
            if r.mixed {
                out.push(("grids".into(), "the files are of different grids (the first file's below)".into()));
            }
            grid_facts(r.grid_of(r.chosen), &mut out);
        }
        Value::Map(m) => {
            out.push(("in memory".into(), "a map".into()));
            let values = m.data.iter().filter(|v| !v.is_nan()).count();
            out.push(("cells with a value".into(), format!("{values}")));
            grid_facts(&m.grid, &mut out);
        }
        Value::Stack(s) => {
            out.push(("in memory".into(), format!("a stack of {} layers", s.layers.len())));
            out.push(("layers".into(), s.keys.iter().map(|k| k.to_string()).collect::<Vec<_>>().join(", ")));
            grid_facts(&s.grid, &mut out);
        }
        Value::Series(items) => {
            out.push(("in memory".into(), format!("a series of {} values", items.len())));
            if let Some(Value::Map(m)) = items.first() {
                grid_facts(&m.grid, &mut out);
            }
        }
        Value::Vector(v) => {
            let (mut polygons, mut lines, mut points, mut empty) = (0, 0, 0, 0);
            for g in &v.geometries {
                match g {
                    Geometry::Empty => empty += 1,
                    g if g.is_polygonal() => polygons += 1,
                    g if g.is_point() => points += 1,
                    _ => lines += 1,
                }
            }
            out.push(("features".into(), format!("{}", v.len())));
            out.push((
                "geometries".into(),
                format!("{polygons} polygons, {lines} lines, {points} points, {empty} empty"),
            ));
            let attrs: Vec<String> = v
                .attributes
                .iter()
                .map(|(n, c)| format!("{n} ({})", if matches!(c, Column::Numbers(_)) { "number" } else { "text" }))
                .collect();
            out.push(("attributes".into(), if attrs.is_empty() { "none".into() } else { attrs.join(", ") }));
            out.push(("coordinate system".into(), crate::prepare::crs_name(v.crs.as_deref())));
            if let Some(b) = v.bounds() {
                out.push((
                    "extent".into(),
                    format!(
                        "xmin {}, ymin {}, xmax {}, ymax {}",
                        number(b[0]),
                        number(b[1]),
                        number(b[2]),
                        number(b[3])
                    ),
                ));
            }
            if let Some(w) = &v.crs {
                out.push(("WKT".into(), w.clone()));
            }
        }
        _ => {}
    }
    out
}

/// A grid in a line or two: its size and bands, its cells and its system (a map's range of values besides, when it
/// is in memory and not too large to be walked at once).
fn grid_brief(g: &Grid, bands: usize, nodata: Option<f64>, data: Option<&[f64]>) -> String {
    let t = g.transform;
    let crs = crate::prepare::crs_name(crate::prepare::crs_of(g).as_deref());
    let band = if bands == 1 { "band".to_string() } else { "bands".to_string() };
    let mut out = format!("{} × {} cells, {bands} {band}", g.width, g.height);
    out.push_str(&format!("\ncell size {} × {}, {crs}", number(t[1].abs()), number(t[5].abs())));
    if let Some(d) = data.filter(|d| d.len() <= RANGE_CELLS) {
        let (mut least, mut most, mut empty) = (f64::INFINITY, f64::NEG_INFINITY, 0usize);
        for &v in d {
            if v.is_nan() {
                empty += 1;
            } else {
                least = least.min(v);
                most = most.max(v);
            }
        }
        if least <= most {
            out.push_str(&format!("\nrange {} to {}", number(least), number(most)));
        } else {
            out.push_str("\nno values");
        }
        out.push_str(&if empty == 0 { ", no nodata".to_string() } else { format!(", {empty} cells nodata") });
    } else if let Some(n) = nodata {
        out.push_str(&format!("\nnodata {}", number(n)));
    }
    out
}

/// The most cells of a map whose range of values a tooltip works out.
const RANGE_CELLS: usize = 20_000_000;

/// What a value is, shortly (the tooltip of a connection): a map's size, cells, system and range of values; a number
/// itself; a table's rows and columns; features' count and kind; for others the type.
pub fn brief(v: &Value) -> String {
    let cut =
        |t: String| if t.chars().count() > 80 { format!("{}...", t.chars().take(80).collect::<String>()) } else { t };
    match v {
        Value::Map(m) => grid_brief(&m.grid, 1, None, Some(&m.data)),
        Value::Stack(s) => grid_brief(&s.grid, s.layers.len(), None, s.layers.first().map(|l| l.as_slice())),
        Value::Raster(r) => grid_brief(r.grid_of(r.chosen), r.layers.len(), r.layers[r.chosen].nodata, None),
        Value::Series(items) => match items.first() {
            Some(Value::Map(m)) => format!("{} maps\n{}", items.len(), grid_brief(&m.grid, 1, None, None)),
            _ => format!("{} values", items.len()),
        },
        Value::Table(t) => format!("{} rows × {} columns", t.rows.len(), t.columns.len()),
        Value::Vector(f) => {
            let (mut polygons, mut lines, mut points) = (0, 0, 0);
            for g in &f.geometries {
                match g {
                    Geometry::Empty => {}
                    g if g.is_polygonal() => polygons += 1,
                    g if g.is_point() => points += 1,
                    _ => lines += 1,
                }
            }
            let kind = match (polygons > 0, lines > 0, points > 0) {
                (true, false, false) => "polygons",
                (false, true, false) => "lines",
                (false, false, true) => "points",
                (false, false, false) => "no geometry",
                _ => "mixed geometries",
            };
            format!("{} features, {kind}", f.len())
        }
        Value::Text(t) => cut(format!("{t:?}")),
        Value::Json(j) => cut(j.to_string()),
        other => crate::tools::summarise(other),
    }
}

/// The facts in a window of their own.
pub fn window(ctx: &egui::Context, title: &str, facts: &[Fact], open: &mut bool) {
    let at = ctx.content_rect().center() - egui::vec2(230.0, 180.0);
    egui::Window::new(format!("Metadata · {title}")).open(open).default_width(460.0).default_pos(at).show(ctx, |ui| {
        egui::ScrollArea::vertical().max_height(520.0).show(ui, |ui| {
            egui::Grid::new(("metadata", title)).num_columns(2).striped(true).spacing([14.0, 6.0]).show(ui, |ui| {
                for (k, v) in facts {
                    ui.label(egui::RichText::new(k).color(ui.visuals().weak_text_color()));
                    if k == "WKT" {
                        egui::CollapsingHeader::new("the coordinate system in full").show(ui, |ui| {
                            ui.label(egui::RichText::new(v).monospace().size(10.5));
                        });
                    } else {
                        ui.label(v);
                    }
                    ui.end_row();
                }
            });
        });
        if ui.button("Copy").on_hover_text("the facts as text").clicked() {
            let text: String = facts.iter().map(|(k, v)| format!("{k}: {v}\n")).collect();
            ui.ctx().copy_text(text);
        }
    });
}
