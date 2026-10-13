//! Vector features as the viewer shows them: drawn as tiles by threads ([`crate::vector_tiles`]) over a grid that
//! spans them (in their coordinate system, so a basemap can go under them), as smoothly whatever their number:
//! scroll to zoom, drag to move, double-click to fit again; the feature under the pointer named with its
//! attributes, and the style (a colour or the colours of an attribute, the fill, the lines' width) beside. The
//! attributes' table is the node's output of its own (`table`), not repeated here.

use std::sync::Arc;

use egui::Ui;
use terraflux_raster::Grid;
use terraflux_raster::vector::{Column, Geometry, Vector};

use crate::map_view::{MapView, Source};
use crate::palette::Coloring;
use crate::vector_tiles::{Overlay, Style};

/// The cells across the grid a layer is drawn on (along its longer side).
const CELLS: f64 = 65536.0;

/// A grid that spans the features (3 % around them), in their coordinate system.
pub fn grid_over(v: &Vector) -> Grid {
    let b = v.bounds().unwrap_or([0.0, 0.0, 1.0, 1.0]);
    let (w, h) = (b[2] - b[0], b[3] - b[1]);
    let side = w.max(h);
    let side = if side > 0.0 { side * 1.06 } else { 1.0 };
    let cell = side / CELLS;
    let (cx, cy) = ((b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0);
    let (nx, ny) = (((w * 1.06).max(cell) / cell).ceil() as usize, ((h * 1.06).max(cell) / cell).ceil() as usize);
    let mut g = Grid::new(nx.max(1), ny.max(1));
    g.transform = [cx - nx as f64 * cell / 2.0, cell, 0.0, cy + ny as f64 * cell / 2.0, 0.0, -cell];
    g.geo.wkt = v.crs.clone();
    g
}

/// The style of a layer's controls: a colour, the attribute that colours the features, the polygons' fill, the
/// lines' width (the points' size).
pub fn style_controls(ui: &mut Ui, v: &Vector, style: &mut Style, id: egui::Id) {
    ui.horizontal(|ui| {
        ui.label("colour");
        ui.color_edit_button_srgba(&mut style.colour);
        let name = style.by.clone().unwrap_or_else(|| "one".into());
        egui::ComboBox::from_id_salt(id.with("by")).selected_text(format!("by: {name}")).show_ui(ui, |ui| {
            ui.selectable_value(&mut style.by, None, "one colour");
            for (n, _) in &v.attributes {
                ui.selectable_value(&mut style.by, Some(n.clone()), n);
            }
        });
    });
    ui.spacing_mut().slider_width = (ui.available_width() - 90.0).clamp(40.0, 160.0);
    if v.geometries.iter().any(Geometry::is_polygonal) {
        ui.add(egui::Slider::new(&mut style.fill, 0.0..=1.0).text("fill"));
    }
    let what = if v.geometries.iter().all(Geometry::is_point) { "size" } else { "line width" };
    ui.add(egui::Slider::new(&mut style.width, 0.0..=12.0).step_by(0.1).text(what))
        .on_hover_text("pixels: the lines' and outlines' width (0: polygons without outline), the points' size");
}

/// What the panel shows of a layer, worked out once: the grid it is drawn on, its kinds of features and
/// attributes of text.
#[derive(Clone)]
struct Summary {
    key: usize,
    grid: Option<Arc<Grid>>,
    counts: String,
    texts: Vec<String>,
}

fn summary(v: &Arc<Vector>) -> Summary {
    let (mut polygons, mut lines, mut points) = (0, 0, 0);
    for g in &v.geometries {
        if g.is_polygonal() {
            polygons += 1;
        } else if g.is_point() {
            points += 1;
        } else if matches!(g, Geometry::LineString(_) | Geometry::MultiLineString(_)) {
            lines += 1;
        }
    }
    Summary {
        key: Arc::as_ptr(v) as usize,
        grid: v.bounds().map(|_| Arc::new(grid_over(v))),
        counts: format!("{} features: {polygons} polygons, {lines} lines, {points} points", v.len()),
        texts: v.attributes.iter().filter(|(_, c)| matches!(c, Column::Texts(_))).map(|(n, _)| n.clone()).collect(),
    }
}

/// The features drawn in the space left, their style in a panel at the right. What is worked out of the features
/// (their grid, counts) is kept while they are the same: nothing goes over them per frame.
pub fn show(ui: &mut Ui, v: &Arc<Vector>, view: &mut MapView, id: egui::Id) {
    let mut style: Style = ui.data(|d| d.get_temp(id.with("style"))).unwrap_or_default();
    let key = Arc::as_ptr(v) as usize;
    let sum = match ui.data(|d| d.get_temp::<Summary>(id.with("summary"))) {
        Some(s) if s.key == key => s,
        _ => {
            let s = summary(v);
            ui.data_mut(|d| d.insert_temp(id.with("summary"), s.clone()));
            s
        }
    };
    egui::Panel::right(id.with("attributes"))
        .resizable(true)
        .default_size(300.0)
        .frame(egui::Frame::new().fill(ui.visuals().panel_fill).inner_margin(egui::Margin::same(8)))
        .show(ui, |ui| {
            ui.label(&sum.counts);
            if !sum.texts.is_empty() {
                ui.label(egui::RichText::new(format!("texts: {}", sum.texts.join(", "))).weak());
            }
            ui.add_space(4.0);
            style_controls(ui, v, &mut style, id);
            if v.crs.is_some() {
                egui::ComboBox::from_id_salt(id.with("basemap"))
                    .selected_text(format!("basemap: {}", view.basemap.name()))
                    .show_ui(ui, |ui| {
                        for b in crate::map_view::Basemap::ALL {
                            ui.selectable_value(&mut view.basemap, b, b.name());
                        }
                    });
            }
        });
    ui.data_mut(|d| d.insert_temp(id.with("style"), style.clone()));
    let Some(grid) = sum.grid.clone() else {
        ui.centered_and_justified(|ui| ui.label("no geometry"));
        return;
    };
    let overlays = [Overlay { vector: v.clone(), style }];
    egui::CentralPanel::default().frame(egui::Frame::new().fill(crate::theme::WELL)).show(ui, |ui| {
        view.show_layers(ui, &Source::Blank(grid), &Coloring::Classes(Vec::new()), &overlays);
    });
}
