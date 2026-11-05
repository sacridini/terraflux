//! Vector features as the viewer shows them: drawn as tiles by threads ([`crate::vector_tiles`]) over a grid that
//! spans them (in their coordinate system, so a basemap can go under them), as smoothly whatever their number:
//! scroll to zoom, drag to move, double-click to fit again; the feature under the pointer named with its
//! attributes, and the style (a colour or the colours of an attribute, the fill, the lines' width, the attribute that
//! labels them) beside. The attributes' table is also the node's output of its own (`table`); here a button opens
//! it under the map, its rows linked to the features ([`crate::attribute_table`]).

use std::sync::Arc;

use egui::Ui;
use terraflux_raster::Grid;
use terraflux_raster::vector::{Column, Geometry, Vector};

use crate::map_view::{MapView, Source};
use crate::palette::{Coloring, Ramp};
use crate::vector_tiles::{Breaks, MAX_CLASSES, Overlay, Style, Symbol};

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

/// A ramp as a bar that a click opens the Viewer's ramps on (as the map's palette does): the one chosen marked.
fn ramp_picker(ui: &mut Ui, ramp: &mut Ramp, id: egui::Id, width: f32) {
    use egui::{Pos2, Rect, Sense, Vec2};
    fn paint(ui: &Ui, r: Rect, ramp: Ramp) {
        let k = 40;
        for j in 0..k {
            let a = r.left() + r.width() * j as f32 / k as f32;
            let b = r.left() + r.width() * (j + 1) as f32 / k as f32;
            let c = crate::palette::seen(ramp.colour(j as f32 / (k - 1) as f32));
            ui.painter().rect_filled(Rect::from_min_max(Pos2::new(a, r.top()), Pos2::new(b + 0.5, r.bottom())), 0.0, c);
        }
    }
    let (r, bar) = ui.allocate_exact_size(Vec2::new(width, 14.0), Sense::click());
    bar.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "ramp"));
    paint(ui, r, *ramp);
    let stroke = if bar.hovered() {
        ui.visuals().widgets.hovered.fg_stroke
    } else {
        egui::Stroke::new(1.0, ui.visuals().widgets.noninteractive.fg_stroke.color.gamma_multiply(0.4))
    };
    ui.painter().rect_stroke(r, 2.0, stroke, egui::StrokeKind::Outside);
    let bar = bar.on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text("a click: another ramp");
    egui::Popup::menu(&bar).id(id).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
        ui.set_min_width(250.0);
        for (name, this) in crate::builder::RAMPS {
            let on = *ramp == this;
            let clicked = ui
                .horizontal(|ui| {
                    let (r, swatch) = ui.allocate_exact_size(Vec2::new(64.0, 14.0), Sense::click());
                    paint(ui, r, this);
                    if on {
                        ui.painter().rect_stroke(
                            r.expand(1.0),
                            2.0,
                            ui.visuals().selection.stroke,
                            egui::StrokeKind::Outside,
                        );
                    }
                    let label = ui.add(egui::Button::selectable(on, name));
                    swatch.clicked() || label.clicked()
                })
                .inner;
            if clicked {
                *ramp = this;
                ui.close();
            }
        }
    });
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
    // the numbers of the attribute: the ramp they run along, how they are split into colours
    if let Some(Column::Numbers(_)) = style.by.as_ref().and_then(|b| v.column(b)) {
        ui.horizontal(|ui| {
            ui.label("ramp");
            let width = (ui.available_width() - 4.0).clamp(60.0, 200.0);
            ramp_picker(ui, &mut style.ramp, id.with("ramp"), width);
        });
        ui.horizontal(|ui| {
            ui.label("breaks");
            egui::ComboBox::from_id_salt(id.with("breaks"))
                .width(110.0)
                .selected_text(style.breaks.name())
                .show_ui(ui, |ui| {
                    for b in Breaks::ALL {
                        ui.selectable_value(&mut style.breaks, b, b.name());
                    }
                })
                .response
                .on_hover_text(
                    "how the numbers become colours: along the ramp (a few whole numbers: a colour each), or in \
                     classes of equal width, of as many features each, or where the values gather (Jenks)",
                );
            if style.breaks != Breaks::Continuous {
                ui.add(egui::DragValue::new(&mut style.classes).range(2..=MAX_CLASSES).suffix(" classes"));
            }
        });
        ui.checkbox(&mut style.categories, "each value a category")
            .on_hover_text("the numbers as categories, a colour for each distinct value (texts always are)");
    }
    ui.spacing_mut().slider_width = (ui.available_width() - 130.0).clamp(40.0, 160.0);
    if v.geometries.iter().any(Geometry::is_polygonal) {
        ui.add(egui::Slider::new(&mut style.fill, 0.0..=1.0).text("fill"));
    }
    let what = if v.geometries.iter().all(Geometry::is_point) { "size" } else { "line width" };
    ui.add(egui::Slider::new(&mut style.width, 0.0..=12.0).step_by(0.1).text(what))
        .on_hover_text("pixels: the lines' and outlines' width (0: polygons without outline), the points' size");
    if v.geometries.iter().any(Geometry::is_point) {
        ui.horizontal(|ui| {
            ui.label("symbol");
            egui::ComboBox::from_id_salt(id.with("symbol")).selected_text(style.symbol.key()).show_ui(ui, |ui| {
                for s in Symbol::ALL {
                    ui.selectable_value(&mut style.symbol, s, s.key());
                }
            });
        });
        ui.horizontal(|ui| {
            ui.label("size by");
            let name = style.size_by.clone().unwrap_or_else(|| "none".into());
            egui::ComboBox::from_id_salt(id.with("size by")).width(90.0).selected_text(name).show_ui(ui, |ui| {
                ui.selectable_value(&mut style.size_by, None, "none");
                for (n, c) in &v.attributes {
                    if matches!(c, Column::Numbers(_)) {
                        ui.selectable_value(&mut style.size_by, Some(n.clone()), n);
                    }
                }
            });
            if style.size_by.is_some() {
                let max = style.size_max;
                ui.add(egui::DragValue::new(&mut style.size_min).range(1.0..=max).speed(0.2).suffix(" px"))
                    .on_hover_text("the size of the least value, pixels across");
                ui.label("to");
                let min = style.size_min;
                ui.add(egui::DragValue::new(&mut style.size_max).range(min..=60.0).speed(0.2).suffix(" px"))
                    .on_hover_text("the size of the greatest value (the area goes as the value)");
            }
        });
    }
    ui.horizontal(|ui| {
        ui.label("labels");
        let name = style.labels.clone().unwrap_or_else(|| "none".into());
        egui::ComboBox::from_id_salt(id.with("labels")).selected_text(name).show_ui(ui, |ui| {
            ui.selectable_value(&mut style.labels, None, "none");
            for (n, _) in &v.attributes {
                ui.selectable_value(&mut style.labels, Some(n.clone()), n);
            }
        });
        if style.labels.is_some() {
            ui.add(egui::DragValue::new(&mut style.label_size).range(9.0..=20.0).suffix(" pt"))
                .on_hover_text("the labels' size, points");
        }
    });
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
    let mut open: Option<crate::attribute_table::Open> = ui.data(|d| d.get_temp(id.with("open")));
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
            let shown = open.is_some();
            if ui
                .add(egui::Button::selectable(shown, "Table"))
                .on_hover_text("the attributes of the features in a table under the map: a row selects its feature")
                .clicked()
            {
                open = (!shown).then(crate::attribute_table::Open::default);
            }
            if v.crs.is_some() {
                egui::ComboBox::from_id_salt(id.with("basemap"))
                    .selected_text(format!("basemap: {}", view.basemap.name()))
                    .show_ui(ui, |ui| crate::basemap::choices(ui, id.with("basemap"), &mut view.basemap));
                crate::basemap::custom_window(ui.ctx(), id.with("basemap"), &mut view.basemap);
            }
        });
    ui.data_mut(|d| d.insert_temp(id.with("style"), style.clone()));
    let Some(grid) = sum.grid.clone() else {
        ui.centered_and_justified(|ui| ui.label("no geometry"));
        return;
    };
    let overlays = [Overlay { vector: v.clone(), style, name: String::new() }];
    // the table under the map, the feature of its row selected on it
    if let Some(o) = &mut open {
        if crate::attribute_table::panel(ui, id.with("table"), v, "", o) {
            crate::attribute_table::link(o, view, ui.ctx(), v, Some(0));
        } else {
            open = None;
        }
    }
    if open.is_none() {
        view.selected = None;
    }
    egui::CentralPanel::default().frame(egui::Frame::new().fill(crate::theme::well())).show(ui, |ui| {
        view.show_layers(ui, &Source::Blank(grid), &Coloring::Classes(Vec::new()), &overlays);
    });
    // (a shape dragged on the map opens the table, to hold the selection)
    if open.is_none() && view.area_done.is_some() {
        open = Some(crate::attribute_table::Open::default());
    }
    match &mut open {
        Some(o) => {
            crate::attribute_table::clicked(o, view, Some(0));
            crate::attribute_table::area(o, view, ui.ctx(), v, Some(0));
        }
        None => view.feature_clicked.clear(),
    }
    ui.data_mut(|d| match open {
        Some(o) => {
            d.insert_temp(id.with("open"), o);
        }
        None => {
            d.remove::<crate::attribute_table::Open>(id.with("open"));
        }
    });
}
