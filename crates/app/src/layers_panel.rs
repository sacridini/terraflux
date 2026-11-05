//! The Layers of the Viewer, as a GIS has them: a bar of icons (add a layer of the run or a file, remove, up,
//! down, zoom to it, all shown or hidden, every legend folded) over a list of rows, the top layer first, that
//! scrolls when long; then the properties of the layer selected. A row: its arrow (its legend folded under it, as
//! in QGIS), whether it is shown (its box), its symbol, its name, what it is (how many maps, features) and its ⋯;
//! a click selects it (a map: it is shown, its statistics at the right), a double click zooms to it, a drag puts it
//! over or under the others, a right click (or its ⋯) gives what can be done with it. The properties: how opaque,
//! which map of a stack, the palette (a click on it gives the others) and the histogram, the style of features.

use super::*;
use crate::map_view::cell_text;
use egui::{Pos2, Sense};

/// What the Layers asked for in a frame, done once the rows are drawn.
#[derive(Default)]
struct Asked {
    remove: Option<usize>,
    /// A map whose legend and settings go to the right (it is the map shown).
    show: Option<(String, String)>,
    zoom: Option<usize>,
    table: Option<usize>,
    up: Option<usize>,
    down: Option<usize>,
    top: Option<usize>,
    bottom: Option<usize>,
    /// A layer dragged and let go: its index, and the place among the rows (from the top) it goes to.
    moved: Option<(usize, usize)>,
    /// Shown alone (the others hidden).
    solo: Option<usize>,
    /// All shown, or all hidden.
    all: Option<bool>,
    /// An output of the run added on top; the map shown kept as a layer.
    add: Option<(String, String)>,
    pin: bool,
    /// Files read by new nodes, their layers on top (none: chosen in a dialog).
    files: Option<Vec<PathBuf>>,
    pick_files: bool,
    /// The window that adds a layer from an address on the web.
    url: bool,
    /// The rasters dropped at once joined as one stack, or the offer let go.
    join: bool,
    no_join: bool,
    /// Every legend folded (true) or unfolded (false).
    fold: Option<bool>,
}

/// What a row is: a map (of values or classes), vector features, a difference of two maps.
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Map,
    Features,
    Difference,
}

/// A layer's key among the others (a difference by the maps it is of).
fn key_of(l: &Layer) -> String {
    match l.difference_key() {
        Some(k) => format!("{k:?}"),
        None => format!("{}.{}", l.node, l.port),
    }
}

/// A layer's map, its look and the range of its colours by the values (see [`layer_look`]).
pub(super) type LayerLook = (Source, Look, Option<(f64, f64)>);

/// A layer's map as the Viewer draws it: its source, its look (its palette, the range of its colours chosen, the
/// model's legend), and the range of its colours by the values (before the one chosen); none while it is made.
pub(super) fn layer_look(s: &mut Session, ctx: &egui::Context, l: &Layer) -> Option<LayerLook> {
    let src = s.value_of(&l.node, &l.port).and_then(|v| layer_source(&v, l.band))?;
    let key = (l.node.clone(), l.port.clone());
    let palette =
        s.palettes.get(&key).copied().unwrap_or_else(|| default_palette(&s.model.nodes, &l.node, &l.port, Some(&src)));
    let look = s.looks.get(ctx, &src, palette)?;
    let first = match look.coloring {
        Coloring::Continuous { min, max, .. } => Some((min, max)),
        Coloring::Classes(_) | Coloring::Packed | Coloring::Shade { .. } => None,
    };
    let look = ranged(look, s.ranges.get(&key).copied());
    let look = match crate::legend_view::legend_of(s, &l.node, &l.port, &src) {
        Some(c) => crate::legend_view::apply(&look, &c),
        None => look,
    };
    Some((src, look, first))
}

/// The map shown, as the Viewer draws it: its legend under its row in the Layers (as a GIS has it), with its
/// palette and the legend's editor.
pub(super) struct ShownLegend<'a> {
    pub r: &'a Registry,
    pub look: &'a Look,
    pub names: Option<&'a [terraflux_ops::legend::Class]>,
}

/// The Layers: their bar, their rows (the top layer first, in a list that scrolls when long), then the
/// properties of the layer selected (see the module).
#[allow(clippy::too_many_arguments)]
pub(super) fn layers_section(
    ui: &mut Ui,
    s: &mut Session,
    r: &Registry,
    sh: &mut Shown,
    others: &[(String, String)],
    main_name: &str,
    shown: Option<&ShownLegend<'_>>,
    salt: &str,
) {
    let (node, port) = (sh.node.clone(), sh.port.clone());
    let vectors = vector_values(s, r);
    let shown_is_layer = sh.layers.iter().any(|l| l.is(&node, &port));
    let n = sh.layers.len();
    let selected_id = egui::Id::new(("layer selected", salt));
    let selected: Option<usize> = ui
        .data(|d| d.get_temp::<String>(selected_id))
        .and_then(|k| sh.layers.iter().position(|l| key_of(l) == k))
        .or_else(|| sh.layers.iter().position(|l| l.is(&node, &port)));
    let mut asked = Asked::default();
    // the run's other maps and features, to add
    let more: Vec<(String, String)> = others
        .iter()
        .cloned()
        .chain(vectors.iter().map(|(n, p, _)| (n.clone(), p.clone())))
        .filter(|(n, p)| !sh.layers.iter().any(|l| l.is(n, p)))
        .collect();
    toolbar(ui, sh, &more, (shown_is_layer, main_name), selected, &mut asked);
    if let Some(fold) = asked.fold.take() {
        ui.data_mut(|d| {
            d.insert_temp(egui::Id::new("layers unfolded"), !fold);
            for l in &sh.layers {
                d.insert_temp(egui::Id::new(("layer row", salt, key_of(l))).with("open"), !fold);
            }
        });
    }
    ui.add_space(3.0);
    // rasters dropped at once, a layer each: the offer to join them as one stack (a time series)
    s.dropped_rasters.retain(|id| s.source.nodes.iter().any(|n| &n.id == id));
    if s.dropped_rasters.len() > 1 {
        egui::Frame::new()
            .fill(theme::accent().gamma_multiply(0.15))
            .stroke(egui::Stroke::new(1.0, theme::accent().gamma_multiply(0.6)))
            .corner_radius(4)
            .inner_margin(Margin::same(6))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(
                    RichText::new(format!("{} rasters dropped, a layer each", s.dropped_rasters.len())).size(11.5),
                );
                ui.horizontal(|ui| {
                    if ui
                        .button("As one stack (a time series)")
                        .on_hover_text(
                            "one node that reads them all, a layer per band of each file in the order of their names, \
                             played with Play: a pattern of their names when they are in one folder, else a \
                             stack.of they feed",
                        )
                        .clicked()
                    {
                        asked.join = true;
                    }
                    if ui.small_button("Keep them apart").clicked() {
                        asked.no_join = true;
                    }
                });
            });
        ui.add_space(3.0);
    }
    let dragged_id = egui::Id::new(("layer dragged", salt));
    // (the list, with the legends under the rows, takes at most three quarters of the panel: the properties of the
    // layer selected stay in sight)
    let most = (ui.available_height() * 0.75).max(140.0);
    let frame = egui::Frame::new()
        .fill(ui.visuals().extreme_bg_color)
        .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
        .corner_radius(4)
        .inner_margin(Margin::same(2))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            egui::ScrollArea::vertical()
                .id_salt(("layers list", salt))
                .max_height(most)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    // the map shown that is no layer: a row of its own, kept by the pin
                    if !shown_is_layer {
                        shown_row(ui, main_name, &mut asked);
                        if let Some(shown) = shown {
                            indented(ui, |ui| shown_legend(ui, s, (&node, &port), shown));
                        }
                    }
                    // the rows, the top layer first; where each is (its row, then its legend)
                    let mut rows: Vec<Rect> = Vec::new();
                    let mut let_go = None;
                    for i in (0..n).rev() {
                        let kind = match &sh.layers[i] {
                            l if l.minus.is_some() => Kind::Difference,
                            l if vectors.iter().any(|(vn, vp, _)| l.is(vn, vp)) => Kind::Features,
                            _ => Kind::Map,
                        };
                        let (rect, started, released) = layer_row(
                            ui,
                            s,
                            sh,
                            &vectors,
                            (i, kind),
                            (&node, &port),
                            selected == Some(i),
                            shown,
                            salt,
                            &mut asked,
                        );
                        if started {
                            ui.data_mut(|d| d.insert_temp(dragged_id, i));
                        }
                        if released {
                            let_go = Some(i);
                        }
                        rows.push(rect);
                    }
                    if n == 0 {
                        ui.add_space(6.0);
                        ui.vertical_centered(|ui| {
                            ui.label(
                                RichText::new(if shown_is_layer {
                                    "no layer"
                                } else {
                                    "+ adds the run's maps and features, or a file; files dropped here too"
                                })
                                .weak()
                                .size(11.0),
                            );
                        });
                        ui.add_space(6.0);
                    }
                    drag_feedback(ui, s, sh, &rows, let_go, dragged_id, &mut asked);
                });
        });
    // files of the Files pane let go over the Layers: read by new nodes, their layers on top
    if egui::DragAndDrop::has_payload_of_type::<DraggedFile>(ui.ctx())
        && let Some(p) = ui.ctx().pointer_latest_pos()
        && frame.response.rect.contains(p)
    {
        ui.painter().rect_stroke(frame.response.rect, 4.0, (2.0, theme::accent()), egui::StrokeKind::Inside);
        if ui.input(|i| i.pointer.any_released())
            && let Some(f) = egui::DragAndDrop::take_payload::<DraggedFile>(ui.ctx())
        {
            asked.files = Some(f.0.clone());
        }
    }
    // the properties of the layer selected
    if let Some(i) = selected.filter(|i| *i < n) {
        let kind = match &sh.layers[i] {
            l if l.minus.is_some() => Kind::Difference,
            l if vectors.iter().any(|(vn, vp, _)| l.is(vn, vp)) => Kind::Features,
            _ => Kind::Map,
        };
        ui.add_space(6.0);
        properties(ui, s, sh, &vectors, (i, kind), (&node, &port), salt, &mut asked);
    }
    act(ui, s, r, sh, asked, (&node, &port), selected_id);
}

/// The map shown that is no layer: its name in italics and the pin that keeps it as a layer.
fn shown_row(ui: &mut Ui, main_name: &str, asked: &mut Asked) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 24.0), Sense::hover());
    let pin = icon_button_at(
        ui,
        Rect::from_min_size(rect.min + Vec2::new(3.0, 2.0), Vec2::splat(20.0)),
        Icon::Pin,
        true,
    )
    .on_hover_text(
        "keep it as a layer: it stays on the map when another node is clicked (a double click on the node keeps \
             it too)",
    );
    pin.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Keep as a layer"));
    if pin.clicked() {
        asked.pin = true;
    }
    let visuals = ui.visuals();
    let mut job = egui::text::LayoutJob::default();
    job.append(
        main_name,
        0.0,
        egui::TextFormat {
            font_id: egui::FontId::proportional(13.0),
            color: visuals.text_color(),
            italics: true,
            ..Default::default()
        },
    );
    job.append(
        "   shown, not kept",
        0.0,
        egui::TextFormat {
            font_id: egui::FontId::proportional(11.0),
            color: visuals.weak_text_color(),
            ..Default::default()
        },
    );
    job.wrap = egui::text::TextWrapping {
        max_width: (rect.width() - 34.0).max(10.0),
        max_rows: 1,
        break_anywhere: true,
        overflow_character: Some('…'),
    };
    let galley = ui.fonts_mut(|f| f.layout_job(job));
    ui.painter().galley(Pos2::new(rect.left() + 30.0, rect.center().y - galley.size().y / 2.0), galley, Color32::WHITE);
    let line = ui.visuals().widgets.noninteractive.bg_stroke;
    ui.painter().hline(rect.x_range(), rect.bottom() - 0.5, line);
}

/// A row dragged: a line where it would go (between two rows), the name carried by the pointer, and there once
/// let go.
fn drag_feedback(
    ui: &mut Ui,
    s: &Session,
    sh: &Shown,
    rows: &[Rect],
    let_go: Option<usize>,
    dragged_id: egui::Id,
    asked: &mut Asked,
) {
    let n = sh.layers.len();
    let from: Option<usize> = ui.data(|d| d.get_temp(dragged_id));
    if let Some(from) = from.filter(|f| *f < n)
        && let Some(p) = ui.ctx().pointer_latest_pos()
        && !rows.is_empty()
    {
        let slot = rows.iter().filter(|r| r.center().y < p.y).count();
        let at_from = n - 1 - from;
        let moves = slot != at_from && slot != at_from + 1;
        if let_go.is_some() {
            if moves {
                asked.moved = Some((from, slot));
            }
            ui.data_mut(|d| d.remove::<usize>(dragged_id));
        } else {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
            let y = match rows.get(slot) {
                Some(r) => r.top() + 0.5,
                None => rows[rows.len() - 1].bottom() - 0.5,
            };
            if moves {
                let (l, rt) = (ui.min_rect().left(), ui.min_rect().right());
                let accent = theme::accent();
                ui.painter()
                    .line_segment([Pos2::new(l + 4.0, y), Pos2::new(rt - 2.0, y)], egui::Stroke::new(2.0, accent));
                ui.painter().circle_stroke(Pos2::new(l + 4.0, y), 3.0, egui::Stroke::new(1.5, accent));
            }
            // (the name carried by the pointer)
            let name =
                sh.layers[from].name_by(made_of(s, &sh.layers[from]).as_deref(), |n, p, b| band_name(s, n, p, b));
            let painter = ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Tooltip, dragged_id));
            let galley = painter.layout_no_wrap(name, egui::FontId::proportional(12.0), ui.visuals().text_color());
            let at = p + Vec2::new(12.0, 4.0);
            let back = Rect::from_min_size(at, galley.size()).expand(4.0);
            painter.rect(back, 3.0, ui.visuals().window_fill, ui.visuals().window_stroke, egui::StrokeKind::Outside);
            painter.galley(at, galley, ui.visuals().text_color());
        }
    }
    // (a drag that ended elsewhere: let go)
    if from.is_some() && !ui.ctx().input(|i| i.pointer.any_down()) && let_go.is_none() {
        ui.data_mut(|d| d.remove::<usize>(dragged_id));
    }
}

/// The icons of the Layers, drawn (the same size and weight in every theme and font, unlike symbols of a font).
#[derive(Clone, Copy, PartialEq)]
enum Icon {
    Add,
    Remove,
    Up,
    Down,
    Zoom,
    Eye,
    Pin,
    Fold,
    Unfold,
}

/// An icon drawn in a square, in a colour.
fn paint_icon(p: &egui::Painter, r: Rect, icon: Icon, c: Color32) {
    let s = egui::Stroke::new(1.4, c);
    let m = r.center();
    let u = r.width().min(r.height()) / 16.0;
    let at = |x: f32, y: f32| m + Vec2::new(x * u, y * u);
    match icon {
        Icon::Add => {
            p.line_segment([at(-5.0, 0.0), at(5.0, 0.0)], s);
            p.line_segment([at(0.0, -5.0), at(0.0, 5.0)], s);
        }
        Icon::Remove => {
            // a bin: its lid, its handle, its body with two lines
            p.line_segment([at(-5.5, -3.5), at(5.5, -3.5)], s);
            p.line_segment([at(-1.5, -5.5), at(1.5, -5.5)], s);
            let body = [at(-4.0, -3.5), at(-3.2, 5.5), at(3.2, 5.5), at(4.0, -3.5)];
            p.add(egui::Shape::line(body.to_vec(), s));
            p.line_segment([at(-1.2, -1.2), at(-1.0, 3.5)], s);
            p.line_segment([at(1.2, -1.2), at(1.0, 3.5)], s);
        }
        Icon::Up | Icon::Down => {
            let k = if icon == Icon::Up { 1.0 } else { -1.0 };
            p.line_segment([at(0.0, 5.0 * k), at(0.0, -5.0 * k)], s);
            p.add(egui::Shape::line(vec![at(-4.0, -k), at(0.0, -5.0 * k), at(4.0, -k)], s));
        }
        Icon::Zoom => {
            // a lens over a frame's corners: zoom to the layer
            p.circle_stroke(at(-1.0, -1.0), 3.8 * u, s);
            p.line_segment([at(1.8, 1.8), at(5.5, 5.5)], egui::Stroke::new(2.0, c));
        }
        Icon::Eye => {
            let n = 12;
            let lid = |sign: f32| -> Vec<Pos2> {
                (0..=n)
                    .map(|k| {
                        let t = k as f32 / n as f32 * std::f32::consts::PI;
                        at(-6.0 * t.cos(), sign * 3.8 * t.sin())
                    })
                    .collect()
            };
            p.add(egui::Shape::line(lid(-1.0), s));
            p.add(egui::Shape::line(lid(1.0), s));
            p.circle_filled(m, 1.9 * u, c);
        }
        Icon::Pin => {
            p.circle_stroke(at(0.0, -2.5), 2.8 * u, s);
            p.line_segment([at(0.0, 0.3), at(0.0, 6.0)], s);
        }
        Icon::Fold | Icon::Unfold => {
            // lines of a list, an arrow folding or unfolding them
            for y in [-4.0, 0.0, 4.0] {
                p.line_segment([at(-1.0, y), at(6.0, y)], s);
            }
            let k = if icon == Icon::Unfold { 1.0 } else { -1.0 };
            p.add(egui::Shape::line(vec![at(-6.0, -2.0 * k), at(-4.0, 2.0 * k), at(-2.0, -2.0 * k)], s));
        }
    }
}

/// A button of an icon in a rectangle: no frame until the pointer is over it.
fn icon_button_at(ui: &mut Ui, r: Rect, icon: Icon, enabled: bool) -> egui::Response {
    let resp = ui.interact(
        r,
        ui.id().with(("icon", r.min.x as i32, r.min.y as i32)),
        if enabled { Sense::click() } else { Sense::hover() },
    );
    let v = ui.visuals();
    if enabled && resp.hovered() {
        ui.painter().rect_filled(r, 3.0, v.widgets.hovered.weak_bg_fill);
    }
    let c = if !enabled {
        v.weak_text_color().gamma_multiply(0.5)
    } else if resp.hovered() {
        v.strong_text_color()
    } else {
        v.text_color()
    };
    paint_icon(ui.painter(), r.shrink(3.0), icon, c);
    if enabled { resp.on_hover_cursor(egui::CursorIcon::PointingHand) } else { resp }
}

/// A button of an icon in the flow of a bar, its name told to screen readers (and tests).
fn icon_button(ui: &mut Ui, icon: Icon, enabled: bool, name: &str) -> egui::Response {
    let (r, _) = ui.allocate_exact_size(Vec2::splat(22.0), Sense::hover());
    let resp = icon_button_at(ui, r, icon, enabled);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, name));
    resp
}

/// The bar over the rows: the title and how many, then (from the right) add, remove, down, up, zoom to it (the
/// layer selected), all shown or hidden, every legend folded or unfolded.
fn toolbar(
    ui: &mut Ui,
    sh: &Shown,
    more: &[(String, String)],
    (shown_is_layer, main_name): (bool, &str),
    selected: Option<usize>,
    asked: &mut Asked,
) {
    let n = sh.layers.len();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.label(RichText::new("Layers").strong());
        if n > 0 {
            ui.label(RichText::new(n.to_string()).weak().size(11.0));
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 1.0;
            let add = icon_button(ui, Icon::Add, true, "Add a layer")
                .on_hover_text("add a layer: a map or features of the run, or a file (files dropped on the list too)");
            egui::Popup::menu(&add).show(|ui| {
                ui.set_min_width(220.0);
                if !shown_is_layer
                    && ui.button(format!("{main_name}, kept")).on_hover_text("the map shown, kept as a layer").clicked()
                {
                    asked.pin = true;
                    ui.close();
                }
                ui.label(RichText::new("OF THE RUN").size(10.5).color(theme::weak()).strong());
                if more.is_empty() {
                    ui.label(RichText::new("every map and features of the run is a layer").weak().size(11.0));
                }
                for (n, p) in more {
                    if ui.button(output_name(n, p)).on_hover_text("a layer on top of the others").clicked() {
                        asked.add = Some((n.clone(), p.clone()));
                        ui.close();
                    }
                }
                ui.separator();
                if ui
                    .button("A file from disk...")
                    .on_hover_text("a raster or vector features: a node of the model reads it, its layer on top")
                    .clicked()
                {
                    asked.pick_files = true;
                    ui.close();
                }
                if ui
                    .button("A URL...")
                    .on_hover_text(
                        "a COG or any raster GDAL reads over the web (read by windows, as the map asks), or vector \
                         features: https://..., s3://..., gs://..., /vsicurl/...",
                    )
                    .clicked()
                {
                    asked.url = true;
                    ui.close();
                }
            });
            let at = selected.filter(|i| *i < n);
            if icon_button(ui, Icon::Remove, at.is_some(), "Remove the layer")
                .on_hover_text("remove the layer selected (no longer a layer; the node stays)")
                .clicked()
            {
                asked.remove = at;
            }
            ui.add_space(4.0);
            if icon_button(ui, Icon::Down, at.is_some_and(|i| i > 0), "Down")
                .on_hover_text("the layer selected under the one below (or drag its row)")
                .clicked()
            {
                asked.down = at;
            }
            if icon_button(ui, Icon::Up, at.is_some_and(|i| i + 1 < n), "Up")
                .on_hover_text("the layer selected over the one above (or drag its row)")
                .clicked()
            {
                asked.up = at;
            }
            ui.add_space(4.0);
            if icon_button(ui, Icon::Zoom, at.is_some(), "Zoom to layer")
                .on_hover_text("the view on all of the layer selected (a double click on its row too)")
                .clicked()
            {
                asked.zoom = at;
            }
            let all = sh.layers.iter().all(|l| l.visible);
            if icon_button(ui, Icon::Eye, n > 0, if all { "Hide every layer" } else { "Show every layer" })
                .on_hover_text(if all { "hide every layer" } else { "show every layer" })
                .clicked()
            {
                asked.all = Some(!all);
            }
            let unfolded = ui.data(|d| d.get_temp::<bool>(egui::Id::new("layers unfolded"))).unwrap_or(n <= 6);
            let (icon, name) =
                if unfolded { (Icon::Fold, "Fold every legend") } else { (Icon::Unfold, "Unfold every legend") };
            if icon_button(ui, icon, n > 0, name).on_hover_text(name.to_lowercase()).clicked() {
                asked.fold = Some(unfolded);
            }
        });
    });
}

/// A layer's row (its box, its arrow, its symbol, its name, what it is, its ⋯), then its legend when unfolded;
/// where it is (the row and its legend), and whether a drag of it started or ended.
#[allow(clippy::too_many_arguments)]
fn layer_row(
    ui: &mut Ui,
    s: &mut Session,
    sh: &mut Shown,
    vectors: &[(String, String, Arc<terraflux_raster::vector::Vector>)],
    (i, kind): (usize, Kind),
    (node, port): (&str, &str),
    selected: bool,
    shown: Option<&ShownLegend<'_>>,
    salt: &str,
    asked: &mut Asked,
) -> (Rect, bool, bool) {
    let n = sh.layers.len();
    let ctx = ui.ctx().clone();
    let active = sh.layers[i].is(node, port);
    let made = made_of(s, &sh.layers[i]);
    let name = sh.layers[i].name_by(made.as_deref(), |n, p, b| band_name(s, n, p, b));
    let look = if kind == Kind::Map { layer_look(s, &ctx, &sh.layers[i]) } else { None };
    let row_id = egui::Id::new(("layer row", salt, key_of(&sh.layers[i])));
    let open_id = row_id.with("open");
    // (every legend folded or unfolded from the bar; else each as it was, unfolded when there are a few layers)
    let all = ui.data(|d| d.get_temp::<bool>(egui::Id::new("layers unfolded")));
    let mut open: bool = ui.data(|d| d.get_temp(open_id)).or(all).unwrap_or(n <= 6 || active);
    let table_open = sh.attributes.as_ref().is_some_and(|o| o.is(&sh.layers[i].node, &sh.layers[i].port));
    let features = vectors.iter().find(|(vn, vp, _)| sh.layers[i].is(vn, vp)).map(|(_, _, v)| v.clone());
    // what it is, at the right of its name: how many maps, features
    let about = match kind {
        Kind::Map => {
            let count = s.value_of(&sh.layers[i].node, &sh.layers[i].port).map_or(0, |v| maps_of(&v).0.len());
            if count > 1 { format!("{count} maps") } else { String::new() }
        }
        Kind::Features => features.as_ref().map_or(String::new(), |v| format!("{} features", v.geometries.len())),
        Kind::Difference => "difference".into(),
    };
    let top = ui.cursor().top();
    let (rect, row) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 24.0), Sense::click_and_drag());
    row.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, selected, &name));
    let visuals = ui.visuals().clone();
    if selected {
        ui.painter().rect_filled(rect, 3.0, visuals.selection.bg_fill.gamma_multiply(0.45));
    } else if row.hovered() {
        ui.painter().rect_filled(rect, 3.0, visuals.widgets.hovered.weak_bg_fill.gamma_multiply(0.7));
    }
    // the map shown: a bar of the accent at its left
    if active {
        let bar = Rect::from_min_size(rect.min + Vec2::new(0.0, 4.0), Vec2::new(2.5, rect.height() - 8.0));
        ui.painter().rect_filled(bar, 1.0, theme::accent());
    }
    let mut x = rect.left() + 5.0;
    // the arrow that folds its legend
    let arrow = ui.interact(
        Rect::from_center_size(Pos2::new(x + 6.0, rect.center().y), Vec2::splat(16.0)),
        open_id,
        Sense::click(),
    );
    arrow.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "legend"));
    {
        // a small chevron: right when folded, down when unfolded
        let c = if arrow.hovered() { visuals.strong_text_color() } else { visuals.weak_text_color() };
        let m = arrow.rect.center();
        let pts = if open {
            vec![m + Vec2::new(-3.5, -1.5), m + Vec2::new(0.0, 2.0), m + Vec2::new(3.5, -1.5)]
        } else {
            vec![m + Vec2::new(-1.5, -3.5), m + Vec2::new(2.0, 0.0), m + Vec2::new(-1.5, 3.5)]
        };
        ui.painter().add(egui::Shape::line(pts, egui::Stroke::new(1.5, c)));
    }
    if arrow.on_hover_text("its legend, under it, or folded").clicked() {
        open = !open;
    }
    x += 16.0;
    let l = &mut sh.layers[i];
    ui.put(
        Rect::from_min_size(Pos2::new(x, rect.top()), Vec2::new(18.0, rect.height())),
        egui::Checkbox::without_text(&mut l.visible),
    )
    .on_hover_text("shown on the map, or hidden")
    .widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, true, l.visible, format!("show {name}")));
    x += 22.0;
    let symbol = Rect::from_min_size(Pos2::new(x, rect.center().y - 7.0), Vec2::new(18.0, 14.0));
    let packed = Coloring::Packed;
    let by = if kind == Kind::Map && l.rgb.is_some() { Some(&packed) } else { look.as_ref().map(|x| &x.1.coloring) };
    paint_symbol(ui, symbol, kind, by, made.as_deref(), features.as_deref(), &l.style);
    x += 24.0;
    // its ⋯ at the right (seen on the row hovered or selected): what can be done with it
    let dots_rect = Rect::from_center_size(Pos2::new(rect.right() - 12.0, rect.center().y), Vec2::new(20.0, 20.0));
    let dots = ui.interact(dots_rect, row_id.with("more"), Sense::click()).on_hover_text(
        "what can be done with it: zoom to it, its table, its colours, up and down, remove (a right click on the row too)",
    );
    dots.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, format!("{name}: its menu")));
    let near = row.hovered() || dots.hovered() || selected || ui.rect_contains_pointer(rect);
    if near {
        if dots.hovered() {
            ui.painter().rect_filled(dots_rect, 3.0, visuals.widgets.hovered.weak_bg_fill);
        }
        let dot = if dots.hovered() { visuals.strong_text_color() } else { visuals.weak_text_color() };
        for dx in [-4.5, 0.0, 4.5] {
            ui.painter().circle_filled(dots_rect.center() + Vec2::new(dx, 0.0), 1.5, dot);
        }
    }
    // what it is, at the right, dim
    let mut right = dots_rect.left() - 4.0;
    if !about.is_empty() {
        let g = ui.painter().layout_no_wrap(about.clone(), egui::FontId::proportional(10.5), visuals.weak_text_color());
        if right - g.size().x - x > 60.0 {
            right -= g.size().x;
            ui.painter().galley(Pos2::new(right, rect.center().y - g.size().y / 2.0), g, visuals.weak_text_color());
            right -= 6.0;
        }
    }
    // its name, cut to the room left
    let colour = if l.visible { visuals.text_color() } else { visuals.weak_text_color() };
    let text = if active { RichText::new(&name).strong().color(colour) } else { RichText::new(&name).color(colour) };
    let room = (right - x).max(10.0);
    let galley =
        egui::WidgetText::from(text).into_galley(ui, Some(egui::TextWrapMode::Truncate), room, egui::TextStyle::Body);
    ui.painter().galley(Pos2::new(x, rect.center().y - galley.size().y / 2.0), galley, colour);
    let row = row.on_hover_text(format!(
        "{name}: a click selects it (its properties under the list{}); a double click zooms to it; drag it over or \
         under the others; a right click: more",
        if kind == Kind::Map && !active { ", it is shown, its statistics at the right" } else { "" }
    ));
    let row = if row.dragged() { row } else { row.on_hover_cursor(egui::CursorIcon::Grab) };
    if row.clicked() {
        ui.data_mut(|d| d.insert_temp(egui::Id::new(("layer selected", salt)), key_of(&sh.layers[i])));
        if kind == Kind::Map && !active {
            asked.show = Some((sh.layers[i].node.clone(), sh.layers[i].port.clone()));
        }
    }
    if row.double_clicked() {
        asked.zoom = Some(i);
    }
    let coloring = look.as_ref().map(|x| x.1.coloring.clone());
    let menu = |ui: &mut Ui, s: &mut Session, asked: &mut Asked| {
        layer_menu(ui, s, &sh.layers[i], (i, n, kind, active), coloring.as_ref(), table_open, asked);
    };
    row.context_menu(|ui| menu(ui, s, asked));
    egui::Popup::menu(&dots).show(|ui| menu(ui, s, asked));
    let (started, released) = (row.drag_started(), row.drag_stopped());
    // its legend, under it (as a GIS has it): the map shown's whole, with its palette and editor
    if open {
        match shown.filter(|_| active && kind == Kind::Map) {
            // (shown as colour: the three bands and their ranges instead of the ramp)
            _ if kind == Kind::Map && sh.layers[i].rgb.is_some() => {
                indented(ui, |ui| rgb_legend(ui, s, &sh.layers[i]));
            }
            Some(shown) => indented(ui, |ui| shown_legend(ui, s, (node, port), shown)),
            None => indented(ui, |ui| {
                inline_legend(
                    ui,
                    s,
                    &mut sh.layers[i],
                    (kind, active),
                    salt,
                    look.as_ref(),
                    made.as_deref(),
                    features.as_deref(),
                );
            }),
        }
    }
    // (its relief, under its legend: where the sun is)
    if open && let Some(shade) = sh.layers[i].shade.filter(|_| kind == Kind::Map) {
        indented(ui, |ui| {
            ui.label(RichText::new(shade.label()).weak().size(11.0));
        });
    }
    ui.data_mut(|d| d.insert_temp(open_id, open));
    let whole = Rect::from_min_max(Pos2::new(rect.left(), top), Pos2::new(rect.right(), ui.cursor().top()));
    (whole, started, released)
}

/// Classes as a legend has them, a line each: a swatch and the name (the first `most`, then how many more).
fn class_lines(ui: &mut Ui, lines: &[(Color32, String, usize)], most: usize) {
    let text = ui.visuals().text_color();

    let edge = egui::Stroke::new(1.0, ui.visuals().weak_text_color().gamma_multiply(0.5));
    for (col, name, _) in lines.iter().take(most) {
        let (r, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 17.0), Sense::hover());
        let sw = Rect::from_min_size(Pos2::new(r.left(), r.center().y - 5.0), Vec2::new(14.0, 10.0));
        ui.painter().rect_filled(sw, 1.5, crate::palette::seen(*col));
        ui.painter().rect_stroke(sw, 1.5, edge, egui::StrokeKind::Inside);
        let mut job = egui::text::LayoutJob::single_section(
            name.clone(),
            egui::TextFormat { font_id: egui::FontId::proportional(11.5), color: text, ..Default::default() },
        );
        job.wrap = egui::text::TextWrapping {
            max_width: (r.width() - 22.0).max(10.0),
            max_rows: 1,
            break_anywhere: true,
            overflow_character: Some('…'),
        };
        let g = ui.fonts_mut(|f| f.layout_job(job));
        ui.painter().galley(Pos2::new(sw.right() + 7.0, r.center().y - g.size().y / 2.0), g, text);
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, name));
    }
    if lines.len() > most {
        ui.label(RichText::new(format!("+ {} more", lines.len() - most)).size(11.0).weak())
            .on_hover_text("every class under its row when it is the map shown (a click on its row)");
    }
}

/// A legend under its row, indented to its name.
fn indented(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    ui.add_space(1.0);
    ui.horizontal(|ui| {
        ui.add_space(46.0);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 1.0;
            add(ui);
        });
    });
    ui.add_space(4.0);
}

/// The legend of the map shown, under its row: every class (its palette and the legend's editor over them), or the
/// ramp of its values (a click on it gives the others).
fn shown_legend(ui: &mut Ui, s: &mut Session, (node, port): (&str, &str), shown: &ShownLegend<'_>) {
    let key = (node.to_string(), port.to_string());
    let look = shown.look;
    match &look.coloring {
        Coloring::Classes(c) => {
            if crate::legend_view::editing(ui, node, port) {
                crate::legend_view::editor(ui, s, shown.r, node, port, look, shown.names);
                return;
            }
            ui.horizontal(|ui| {
                theme::quiet(ui);
                ui.spacing_mut().item_spacing.x = 4.0;
                let colours = ui.small_button("Colours ⏷").on_hover_text("another palette");
                egui::Popup::menu(&colours)
                    .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                    .show(|ui| palette_menu(ui, s, &key, &look.coloring));
                if ui
                    .small_button("Edit legend")
                    .on_hover_text("the names and colours of the classes, kept in the model (a raster.legend node)")
                    .clicked()
                {
                    crate::legend_view::begin(ui, node, port, look, shown.names);
                }
            });
            ui.add_space(2.0);
            let lines =
                crate::map_view::legend_lines(c, &look.counts, &|v| crate::legend_view::name(look, shown.names, v));
            class_lines(ui, &lines, usize::MAX);
        }
        Coloring::Continuous { .. } => palette_legend(ui, s, &key, look, shown.names, 0),
        Coloring::Packed | Coloring::Shade { .. } => {}
    }
}

/// A layer's legend under its row: of a map, its classes (the first few) or its ramp and range; of a difference,
/// its classes; of features, what they are.
#[allow(clippy::too_many_arguments)]
fn inline_legend(
    ui: &mut Ui,
    s: &Session,
    l: &mut Layer,
    (kind, active): (Kind, bool),
    salt: &str,
    look: Option<&LayerLook>,
    made: Option<&crate::difference::Made>,
    features: Option<&terraflux_raster::vector::Vector>,
) {
    let small = |t: String| RichText::new(t).size(11.0);
    let classes_of = |ui: &mut Ui, lines: Vec<(Color32, String, usize)>| class_lines(ui, &lines, 10);
    // of a stack, a series or rasters on disk not shown: the map it shows (the map shown: its steps at the right)
    if kind == Kind::Map && !active {
        let names = s.value_of(&l.node, &l.port).map(|v| maps_of(&v).1).unwrap_or_default();
        if names.len() > 1 {
            let raster_chosen = match s.value_of(&l.node, &l.port) {
                Some(Value::Raster(r)) => r.chosen,
                _ => 0,
            };
            let at = l.band.unwrap_or(raster_chosen).min(names.len() - 1);
            egui::ComboBox::from_id_salt(("layer band", salt, &l.node, &l.port))
                .selected_text(RichText::new(format!("map: {}", names[at])).size(11.5))
                .width((ui.available_width() - 8.0).clamp(80.0, 220.0))
                .show_ui(ui, |ui| {
                    for (b, name) in names.iter().enumerate() {
                        if ui.selectable_label(b == at, name).clicked() {
                            l.band = Some(b);
                        }
                    }
                })
                .response
                .on_hover_text("the map of the stack this layer shows");
            ui.add_space(2.0);
        }
    }
    match kind {
        Kind::Map => match look {
            Some((src, lk, _)) => match &lk.coloring {
                Coloring::Classes(c) => {
                    let names = crate::legend_view::legend_of(s, &l.node, &l.port, src);
                    let lines = crate::map_view::legend_lines(c, &lk.counts, &|v| {
                        crate::legend_view::name(lk, names.as_deref(), v)
                    });
                    classes_of(ui, lines);
                }
                Coloring::Continuous { min, max, ramp, log } => {
                    let width = (ui.available_width() - 4.0).clamp(60.0, 180.0);
                    let (r, _) = ui.allocate_exact_size(Vec2::new(width, 10.0), Sense::hover());
                    let k = 40;
                    for j in 0..k {
                        let a = r.left() + r.width() * j as f32 / k as f32;
                        let b = r.left() + r.width() * (j + 1) as f32 / k as f32;
                        let c = crate::palette::seen(ramp.colour(j as f32 / (k - 1) as f32));
                        ui.painter().rect_filled(
                            Rect::from_min_max(Pos2::new(a, r.top()), Pos2::new(b + 0.5, r.bottom())),
                            0.0,
                            c,
                        );
                    }
                    ui.allocate_ui(Vec2::new(width, 14.0), |ui| {
                        ui.horizontal(|ui| {
                            ui.label(small(cell_text(*min)).weak());
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(small(cell_text(*max)).weak());
                                if *log {
                                    ui.label(small("log".into()).weak());
                                }
                            });
                        });
                    });
                }
                Coloring::Packed | Coloring::Shade { .. } => {}
            },
            None => {
                let what = if s.value_of(&l.node, &l.port).is_some() { "its colours..." } else { "not run yet" };
                ui.label(small(what.into()).weak());
            }
        },
        Kind::Difference => match made {
            Some(d) => match &d.coloring {
                Coloring::Classes(c) => classes_of(ui, crate::map_view::legend_lines(c, &d.counts, &|v| d.name(v))),
                Coloring::Continuous { .. } | Coloring::Packed | Coloring::Shade { .. } => {
                    ui.label(small(d.summary.clone()).weak());
                }
            },
            None => {
                ui.label(small("the difference...".into()).weak());
            }
        },
        Kind::Features => {
            use terraflux_raster::vector::Geometry;
            let what = match features.and_then(|v| v.geometries.iter().find(|g| !matches!(g, Geometry::Empty))) {
                Some(Geometry::Point(_) | Geometry::MultiPoint(_)) => "points",
                Some(Geometry::LineString(_) | Geometry::MultiLineString(_)) => "lines",
                Some(_) => "polygons",
                None => "no feature",
            };
            ui.label(small(what.into()).weak());
            if let Some(v) = features {
                vector_legend(ui, v, &l.style, egui::Id::new(("vector legend", &l.node, &l.port, salt)));
            }
        }
    }
}

/// A symbol of radius `r` points at `at`.
fn paint_marker(
    p: &egui::Painter,
    symbol: crate::vector_tiles::Symbol,
    at: Pos2,
    r: f32,
    fill: Color32,
    edge: Color32,
) {
    use crate::vector_tiles::Symbol;
    let stroke = egui::Stroke::new(1.0, edge);
    match symbol {
        Symbol::Circle => {
            p.circle_filled(at, r, fill);
            p.circle_stroke(at, r, stroke);
        }
        // (two bars: the egui fills convex shapes only)
        Symbol::Cross => {
            let t = r * 0.38;
            for size in [Vec2::new(2.0 * r, 2.0 * t), Vec2::new(2.0 * t, 2.0 * r)] {
                p.rect_filled(Rect::from_center_size(at, size), 0.0, fill);
            }
            p.rect_stroke(
                Rect::from_center_size(at, Vec2::new(2.0 * r, 2.0 * t)),
                0.0,
                stroke,
                egui::StrokeKind::Inside,
            );
            p.rect_stroke(
                Rect::from_center_size(at, Vec2::new(2.0 * t, 2.0 * r)),
                0.0,
                stroke,
                egui::StrokeKind::Inside,
            );
        }
        _ => {
            let points = symbol.outline(r).into_iter().map(|(dx, dy)| at + Vec2::new(dx, dy)).collect();
            p.add(egui::Shape::convex_polygon(points, fill, stroke));
        }
    }
}

/// The legend of a vector layer under its row: the attribute that colours it over a line each category or class
/// ("0.2 – 0.5": about 12, then "and k more"), or its ramp with the least and the greatest value; the attribute that
/// sizes the points over two or three of the sizes. Worked out once per style (kept while it is the same).
fn vector_legend(ui: &mut Ui, v: &terraflux_raster::vector::Vector, style: &crate::vector_tiles::Style, id: egui::Id) {
    use crate::vector_tiles::Swatches;
    const MOST: usize = 12;
    let key = (v as *const _ as usize, style.key());
    let legend = match ui.data(|d| d.get_temp::<(usize, u64, std::sync::Arc<crate::vector_tiles::Legend>)>(id)) {
        Some((p, k, l)) if (p, k) == key => l,
        _ => {
            let l = std::sync::Arc::new(crate::vector_tiles::look(v, style).legend);
            ui.data_mut(|d| d.insert_temp(id, (key.0, key.1, l.clone())));
            l
        }
    };
    let small = |t: String| RichText::new(t).size(11.0);
    if let (Some(attr), false) = (&legend.attribute, matches!(legend.swatches, Swatches::One)) {
        ui.label(small(format!("by {attr}")).weak());
        match &legend.swatches {
            Swatches::Ramp { ramp, min, max } => {
                let width = (ui.available_width() - 4.0).clamp(60.0, 180.0);
                let (r, _) = ui.allocate_exact_size(Vec2::new(width, 10.0), Sense::hover());
                let k = 40;
                for j in 0..k {
                    let a = r.left() + r.width() * j as f32 / k as f32;
                    let b = r.left() + r.width() * (j + 1) as f32 / k as f32;
                    let c = crate::palette::seen(ramp.colour(j as f32 / (k - 1) as f32));
                    ui.painter().rect_filled(
                        Rect::from_min_max(Pos2::new(a, r.top()), Pos2::new(b + 0.5, r.bottom())),
                        0.0,
                        c,
                    );
                }
                ui.allocate_ui(Vec2::new(width, 14.0), |ui| {
                    ui.horizontal(|ui| {
                        ui.label(small(cell_text(*min)).weak());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(small(cell_text(*max)).weak());
                        });
                    });
                });
            }
            _ => {
                let rows = legend.rows();
                let lines: Vec<(Color32, String, usize)> =
                    rows.iter().take(MOST).map(|(c, t)| (*c, t.clone(), 0)).collect();
                class_lines(ui, &lines, usize::MAX);
                let more = rows.len().saturating_sub(MOST) + legend.more();
                if more > 0 {
                    ui.label(small(format!("and {more} more")).weak());
                }
            }
        }
    }
    if let (Some(attr), false) = (&legend.size_by, legend.sizes.is_empty()) {
        ui.label(small(format!("size by {attr}")).weak());
        let (fill, edge) = (crate::palette::seen(style.colour), Color32::from_black_alpha(160));
        let widest = legend.sizes.iter().map(|(d, _)| *d).fold(0.0, f32::max).max(6.0);
        for (d, text) in &legend.sizes {
            let (r, resp) =
                ui.allocate_exact_size(Vec2::new(ui.available_width(), (widest + 3.0).max(17.0)), Sense::hover());
            paint_marker(
                ui.painter(),
                style.symbol,
                Pos2::new(r.left() + widest / 2.0, r.center().y),
                d / 2.0,
                fill,
                edge,
            );
            ui.painter().text(
                Pos2::new(r.left() + widest + 6.0, r.center().y),
                egui::Align2::LEFT_CENTER,
                text,
                egui::FontId::proportional(11.5),
                ui.visuals().text_color(),
            );
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, text));
        }
    }
}

/// What a right click on a row (or its ⋯) gives.
fn layer_menu(
    ui: &mut Ui,
    s: &mut Session,
    l: &Layer,
    (i, n, kind, active): (usize, usize, Kind, bool),
    coloring: Option<&Coloring>,
    table_open: bool,
    asked: &mut Asked,
) {
    ui.set_min_width(200.0);
    if ui.button("🔍 Zoom to layer").clicked() {
        asked.zoom = Some(i);
        ui.close();
    }
    if kind == Kind::Map && !active && ui.button("Its legend and settings at the right").clicked() {
        asked.show = Some((l.node.clone(), l.port.clone()));
        ui.close();
    }
    if kind == Kind::Features
        && ui
            .add(egui::Button::selectable(table_open, "Attribute table"))
            .on_hover_text("the attributes of its features in a table under the map (again: closes it)")
            .clicked()
    {
        asked.table = Some(i);
        ui.close();
    }
    if let Some(c) = coloring {
        ui.menu_button("🎨 Colours", |ui| palette_menu(ui, s, &(l.node.clone(), l.port.clone()), c));
    }
    ui.separator();
    if ui.button("Show only this layer").clicked() {
        asked.solo = Some(i);
        ui.close();
    }
    if ui.button("Show every layer").clicked() {
        asked.all = Some(true);
        ui.close();
    }
    ui.separator();
    if ui.add_enabled(i + 1 < n, egui::Button::new("To the top")).clicked() {
        asked.top = Some(i);
        ui.close();
    }
    if ui.add_enabled(i + 1 < n, egui::Button::new("Up")).clicked() {
        asked.up = Some(i);
        ui.close();
    }
    if ui.add_enabled(i > 0, egui::Button::new("Down")).clicked() {
        asked.down = Some(i);
        ui.close();
    }
    if ui.add_enabled(i > 0, egui::Button::new("To the bottom")).clicked() {
        asked.bottom = Some(i);
        ui.close();
    }
    ui.separator();
    if ui.button("🗑 Remove layer").on_hover_text("no longer a layer (the node stays)").clicked() {
        asked.remove = Some(i);
        ui.close();
    }
}

/// How opaque a layer is, as a percentage on a slider.
fn opacity_slider(ui: &mut Ui, opacity: &mut f32) {
    ui.horizontal(|ui| {
        ui.label(RichText::new("Opacity").size(11.5));
        ui.spacing_mut().slider_width = (ui.available_width() - 56.0).clamp(40.0, 220.0);
        ui.add(
            egui::Slider::new(opacity, 0.0..=1.0)
                .custom_formatter(|v, _| format!("{:.0}%", v * 100.0))
                .custom_parser(|t| t.trim().trim_end_matches('%').trim().parse::<f64>().ok().map(|v| v / 100.0)),
        )
        .on_hover_text("how opaque the layer is over the ones under it");
    });
}

/// How a layer is mixed with the ones under it (see [`crate::blend`]); the map shown is the base of the picture, so
/// it has none.
fn blend_combo(ui: &mut Ui, l: &mut Layer, salt: &str, active: bool) {
    use crate::blend::Blend;
    if active {
        return;
    }
    ui.horizontal(|ui| {
        ui.label(RichText::new("Blend").size(11.5));
        egui::ComboBox::from_id_salt(("blend", salt, &l.node, &l.port, &l.minus))
            .selected_text(RichText::new(l.blend.key()).size(11.5))
            .show_ui(ui, |ui| {
                for b in Blend::ALL {
                    ui.selectable_value(&mut l.blend, b, b.key()).on_hover_text(b.about());
                }
            })
            .response
            .on_hover_text(
                "how the layer is mixed with the ones under it: normal, multiply (darkens) or screen (lightens)",
            );
    });
}

/// The relief of a map of values drawn under its colours: a box "Shade with this elevation" (`can`: the layer is a
/// map of values; one already shaded can always be turned off), once on the sun's azimuth and altitude, the vertical
/// exaggeration and how strong the shade is.
fn shade_controls(ui: &mut Ui, l: &mut Layer, can: bool) {
    use crate::shade::{EXAGGERATIONS, Shade};
    let mut on = l.shade.is_some();
    if ui
        .add_enabled(can || on, egui::Checkbox::new(&mut on, "Shade with this elevation"))
        .on_hover_text(
            "draw the relief of its values (a hillshade made of them) darkening its colours, as the sun lights it",
        )
        .changed()
    {
        l.shade = on.then(Shade::default);
    }
    let Some(sh) = &mut l.shade else { return };
    ui.spacing_mut().slider_width = (ui.available_width() - 120.0).clamp(40.0, 160.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new("azimuth").size(11.5));
        ui.add(egui::Slider::new(&mut sh.azimuth, 0.0..=360.0).suffix("°").max_decimals(0))
            .on_hover_text("where the sun is, degrees clockwise from north (315: the north-west, as maps are shaded)");
    });
    ui.horizontal(|ui| {
        ui.label(RichText::new("sun altitude").size(11.5));
        ui.add(egui::Slider::new(&mut sh.altitude, 0.0..=90.0).suffix("°").max_decimals(0))
            .on_hover_text("how high the sun is over the horizon: low makes long shadows");
    });
    ui.horizontal(|ui| {
        ui.label(RichText::new("exaggeration").size(11.5));
        ui.add(egui::Slider::new(&mut sh.z, EXAGGERATIONS).logarithmic(true).prefix("×").max_decimals(2))
            .on_hover_text("the vertical exaggeration of the heights (a grid in degrees is taken in metres)");
    });
    ui.horizontal(|ui| {
        ui.label(RichText::new("strength").size(11.5));
        ui.add(
            egui::Slider::new(&mut sh.strength, 0.0..=1.0)
                .custom_formatter(|v, _| format!("{:.0}%", v * 100.0))
                .custom_parser(|t| t.trim().trim_end_matches('%').trim().parse::<f64>().ok().map(|v| v / 100.0)),
        )
        .on_hover_text("how much the shade darkens the colours: 0 none, 100% the shade alone");
    });
}

/// The colour composite of a layer of three maps or more (a stack, a series, a raster of bands): its box "RGB"; once
/// on, the bands the names say (a preset of the satellite's, else the first three), the band of each channel with the
/// range of its values ("auto": the 2nd to 98th percentiles of the band) and the gamma.
fn rgb_controls(ui: &mut Ui, s: &Session, l: &mut Layer, salt: &str) {
    use crate::rgb::{CHANNELS, GAMMAS, Rgb};
    let Some(v) = s.value_of(&l.node, &l.port) else { return };
    let (maps, names) = maps_of(&v);
    if !Rgb::applicable(&maps) {
        return;
    }
    let count = maps.len();
    let name = |b: usize| names.get(b).cloned().unwrap_or_else(|| format!("band {}", b + 1));
    let mut on = l.rgb.is_some();
    if ui
        .checkbox(&mut on, "RGB")
        .on_hover_text("show three of its maps as one colour image: natural or false colour, a band for each of red, green and blue")
        .changed()
    {
        l.rgb = on.then(|| Rgb::first(&names));
    }
    let Some(rgb) = &mut l.rgb else { return };
    let used = rgb.used(ui.ctx(), &s.rgb_ranges, &maps);
    // the satellites' usual composites, when the names of the bands tell them
    let presets = crate::rgb::presets(&names);
    egui::ComboBox::from_id_salt(("rgb preset", salt, &l.node, &l.port))
        .selected_text(RichText::new("composites").size(11.5))
        .width((ui.available_width() - 8.0).clamp(80.0, 220.0))
        .show_ui(ui, |ui| {
            for (label, bands) in &presets {
                if ui.selectable_label(rgb.bands == *bands, label).clicked() {
                    rgb.bands = *bands;
                    rgb.ranges = [None; 3];
                }
            }
        })
        .response
        .on_hover_text("the bands of a satellite's natural colour, infrared and short-wave infrared, by their names");
    for i in 0..3 {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.label(RichText::new(CHANNELS[i]).strong().size(11.5));
            let mut b = rgb.bands[i].min(count - 1);
            egui::ComboBox::from_id_salt(("rgb band", salt, &l.node, &l.port, i))
                .selected_text(RichText::new(name(b)).size(11.5))
                .width(96.0)
                .show_ui(ui, |ui| {
                    for k in 0..count {
                        ui.selectable_value(&mut b, k, name(k));
                    }
                });
            // (another band: its own range)
            if b != rgb.bands[i] {
                rgb.bands[i] = b;
                rgb.ranges[i] = None;
            }
            let (mut lo, mut hi) = rgb.ranges[i].or(used.map(|u| u[i])).unwrap_or((0.0, 1.0));
            let speed = ((hi - lo).abs() / 200.0).max(1e-6);
            let a = ui
                .add(egui::DragValue::new(&mut lo).speed(speed).max_decimals(4))
                .on_hover_text("the value shown black");
            let z = ui
                .add(egui::DragValue::new(&mut hi).speed(speed).max_decimals(4))
                .on_hover_text("the value shown in the full colour");
            if a.changed() || z.changed() {
                rgb.ranges[i] = Some((lo, hi));
            }
            if ui
                .add_enabled(rgb.ranges[i].is_some(), egui::Button::new(RichText::new("auto").size(10.5)).small())
                .on_hover_text("the 2nd to the 98th percentile of the band")
                .clicked()
            {
                rgb.ranges[i] = None;
            }
        });
    }
    ui.horizontal(|ui| {
        ui.label(RichText::new("gamma").size(11.5));
        ui.spacing_mut().slider_width = (ui.available_width() - 60.0).clamp(40.0, 180.0);
        ui.add(egui::Slider::new(&mut rgb.gamma, GAMMAS).logarithmic(true).max_decimals(2))
            .on_hover_text("above 1 brightens the middle tones, below 1 darkens them");
    });
}

/// The legend of a layer shown as colour, under its row: a line for each channel, its band and the range it is drawn
/// over ("R B4 0.01–0.18").
fn rgb_legend(ui: &mut Ui, s: &Session, l: &Layer) {
    let (Some(rgb), Some(v)) = (&l.rgb, s.value_of(&l.node, &l.port)) else { return };
    let (maps, names) = maps_of(&v);
    match rgb.used(ui.ctx(), &s.rgb_ranges, &maps) {
        Some(ranges) => {
            let lines: Vec<(Color32, String, usize)> = crate::rgb::legend(&rgb.names(&names, maps.len()), &ranges)
                .into_iter()
                .map(|(c, t)| (c, t, 0))
                .collect();
            class_lines(ui, &lines, 3);
        }
        None => {
            ui.label(RichText::new("its colours...").weak().size(11.0));
        }
    }
}

/// The properties of the layer selected, under the list: its name and what it is, how opaque; of a map, which of
/// its maps, its palette (a click on it gives the others) and its histogram (the range of its colours set there),
/// or where they are when it is the map shown; of features, their style and table; of a difference, what it is.
#[allow(clippy::too_many_arguments)]
fn properties(
    ui: &mut Ui,
    s: &mut Session,
    sh: &mut Shown,
    vectors: &[(String, String, Arc<terraflux_raster::vector::Vector>)],
    (i, kind): (usize, Kind),
    (node, port): (&str, &str),
    salt: &str,
    asked: &mut Asked,
) {
    let ctx = ui.ctx().clone();
    let active = sh.layers[i].is(node, port);
    let table_open = sh.attributes.as_ref().is_some_and(|o| o.is(&sh.layers[i].node, &sh.layers[i].port));
    let made = made_of(s, &sh.layers[i]);
    let name = sh.layers[i].name_by(made.as_deref(), |n, p, b| band_name(s, n, p, b));
    let look = if kind == Kind::Map { layer_look(s, &ctx, &sh.layers[i]) } else { None };
    egui::Frame::new()
        .fill(ui.visuals().faint_bg_color)
        .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
        .corner_radius(4)
        .inner_margin(Margin::symmetric(8, 6))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 5.0;
            ui.horizontal(|ui| {
                ui.label(RichText::new("PROPERTIES").size(10.0).color(theme::weak()).strong());
                ui.add(egui::Label::new(RichText::new(&name).strong()).truncate());
            });
            let l = &mut sh.layers[i];
            match kind {
                Kind::Difference => {
                    opacity_slider(ui, &mut l.opacity);
                    blend_combo(ui, l, salt, active);
                    match made.as_deref() {
                        Some(d) => {
                            ui.label(RichText::new(&d.summary).weak().size(11.0));
                        }
                        None => {
                            match s
                                .layer_differences
                                .get(&l.difference_key().unwrap_or_default())
                                .and_then(|d| d.made.as_ref())
                            {
                                Some(Err(e)) => {
                                    ui.label(RichText::new(e).color(ui.visuals().error_fg_color).size(11.0));
                                }
                                _ => {
                                    ui.horizontal(|ui| {
                                        ui.spinner();
                                        ui.label(RichText::new("the difference...").weak());
                                    });
                                }
                            }
                        }
                    }
                }
                Kind::Features => {
                    if ui
                        .add(egui::Button::selectable(table_open, "Attribute table"))
                        .on_hover_text("the attributes of its features in a table under the map (again: closes it)")
                        .clicked()
                    {
                        asked.table = Some(i);
                    }
                    if let Some((vn, vp, v)) = vectors.iter().find(|(vn, vp, _)| l.is(vn, vp)) {
                        crate::vector_view::style_controls(ui, v, &mut l.style, egui::Id::new((vn, vp, salt)));
                    }
                }
                Kind::Map => {
                    let key = (l.node.clone(), l.port.clone());
                    opacity_slider(ui, &mut l.opacity);
                    blend_combo(ui, l, salt, active);
                    rgb_controls(ui, s, l, salt);
                    // (a map of values, not of classes and not a composite: it can be shaded with its own relief)
                    let values =
                        look.as_ref().is_some_and(|(_, lk, _)| matches!(lk.coloring, Coloring::Continuous { .. }));
                    shade_controls(ui, l, values && l.rgb.is_none());
                    if active {
                        ui.label(
                            RichText::new(
                                "the map shown: its legend under its row, its statistics and steps at the right",
                            )
                            .weak()
                            .size(11.0),
                        );
                        return;
                    }
                    match &look {
                        Some((src, lk, first)) => {
                            let named = crate::legend_view::legend_of(s, &l.node, &l.port, src);
                            palette_legend(ui, s, &key, lk, named.as_deref(), 0);
                            // the values: their histogram, the range of the colours set on it
                            if matches!(lk.coloring, Coloring::Continuous { .. }) {
                                let id = egui::Id::new(("layer histogram", salt, &l.node, &l.port));
                                range_histogram(ui, s, &key, src, &lk.coloring, *first, id);
                            }
                        }
                        None => {
                            if s.value_of(&l.node, &l.port).is_some() {
                                ui.spinner();
                            } else {
                                ui.label(RichText::new("not run yet").weak().size(11.0));
                            }
                        }
                    }
                }
            }
        });
}

/// A row's symbol: a map's ramp or first classes, features' colour as points, lines or polygons, a difference's
/// colours (a ± while it is made).
fn paint_symbol(
    ui: &Ui,
    r: Rect,
    kind: Kind,
    coloring: Option<&Coloring>,
    made: Option<&crate::difference::Made>,
    features: Option<&terraflux_raster::vector::Vector>,
    style: &crate::vector_tiles::Style,
) {
    use terraflux_raster::vector::Geometry;
    let p = ui.painter();
    let edge = egui::Stroke::new(1.0, ui.visuals().widgets.noninteractive.fg_stroke.color.gamma_multiply(0.5));
    let coloring = coloring.or(made.map(|d| &d.coloring));
    match (kind, coloring, features) {
        (Kind::Features, _, Some(v)) => {
            let c = crate::palette::seen(style.colour);
            match v.geometries.iter().find(|g| !matches!(g, Geometry::Empty)) {
                Some(Geometry::Point(_) | Geometry::MultiPoint(_)) => {
                    for (dx, dy) in [(-4.0, 2.0), (1.0, -3.0), (5.0, 3.0)] {
                        if style.symbol == crate::vector_tiles::Symbol::Circle {
                            p.circle_filled(r.center() + Vec2::new(dx, dy), 2.5, c);
                        } else {
                            paint_marker(p, style.symbol, r.center() + Vec2::new(dx, dy), 3.0, c, c);
                        }
                    }
                }
                Some(Geometry::LineString(_) | Geometry::MultiLineString(_)) => {
                    let pts = [
                        r.left_bottom() + Vec2::new(1.0, -2.0),
                        Pos2::new(r.center().x - 2.0, r.top() + 3.0),
                        Pos2::new(r.center().x + 3.0, r.bottom() - 3.0),
                        r.right_top() + Vec2::new(-1.0, 2.0),
                    ];
                    p.line(pts.to_vec(), egui::Stroke::new(2.0, c));
                }
                _ => {
                    let inner = r.shrink(1.0);
                    p.rect_filled(inner, 2.0, c.gamma_multiply(style.fill.max(0.3)));
                    p.rect_stroke(inner, 2.0, egui::Stroke::new(1.5, c), egui::StrokeKind::Inside);
                }
            }
        }
        (_, Some(Coloring::Continuous { ramp, .. }), _) => {
            let k = 9;
            for j in 0..k {
                let a = r.left() + r.width() * j as f32 / k as f32;
                let b = r.left() + r.width() * (j + 1) as f32 / k as f32;
                let c = crate::palette::seen(ramp.colour(j as f32 / (k - 1) as f32));
                p.rect_filled(Rect::from_min_max(Pos2::new(a, r.top()), Pos2::new(b + 0.5, r.bottom())), 0.0, c);
            }
            p.rect_stroke(r, 1.0, edge, egui::StrokeKind::Inside);
        }
        (_, Some(Coloring::Classes(c)), _) if !c.is_empty() => {
            // (up to four classes, in the quarters)
            let quarters = [(0.0, 0.0), (0.5, 0.0), (0.0, 0.5), (0.5, 0.5)];
            for (j, (fx, fy)) in quarters.iter().enumerate() {
                let col = crate::palette::seen(c[(j * c.len() / 4) % c.len()].1);
                let q = Rect::from_min_size(r.min + Vec2::new(r.width() * fx, r.height() * fy), r.size() / 2.0);
                p.rect_filled(q, 0.0, col);
            }
            p.rect_stroke(r, 1.0, edge, egui::StrokeKind::Inside);
        }
        // a colour composite: the red, green and blue in thirds
        (_, Some(Coloring::Packed), _) => {
            let thirds =
                [Color32::from_rgb(230, 40, 40), Color32::from_rgb(40, 190, 60), Color32::from_rgb(60, 100, 240)];
            for (j, c) in thirds.into_iter().enumerate() {
                let a = r.left() + r.width() * j as f32 / 3.0;
                let b = r.left() + r.width() * (j + 1) as f32 / 3.0;
                p.rect_filled(Rect::from_min_max(Pos2::new(a, r.top()), Pos2::new(b + 0.5, r.bottom())), 0.0, c);
            }
            p.rect_stroke(r, 1.0, edge, egui::StrokeKind::Inside);
        }
        (Kind::Difference, None, _) => {
            p.text(
                r.center(),
                egui::Align2::CENTER_CENTER,
                "±",
                egui::FontId::proportional(13.0),
                ui.visuals().weak_text_color(),
            );
        }
        _ => {
            p.rect_stroke(r.shrink(1.0), 2.0, edge, egui::StrokeKind::Inside);
        }
    }
}

/// A palette drawn as a bar (a ramp, or the colours of the classes side by side) that a click opens the
/// palettes on.
fn palette_bar(ui: &mut Ui, coloring: &Coloring, width: f32) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(Vec2::new(width, 14.0), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "palette"));
    let p = ui.painter();
    match coloring {
        Coloring::Continuous { ramp, .. } => {
            let k = 48;
            for j in 0..k {
                let a = r.left() + r.width() * j as f32 / k as f32;
                let b = r.left() + r.width() * (j + 1) as f32 / k as f32;
                let c = crate::palette::seen(ramp.colour(j as f32 / (k - 1) as f32));
                p.rect_filled(Rect::from_min_max(Pos2::new(a, r.top()), Pos2::new(b + 0.5, r.bottom())), 0.0, c);
            }
        }
        Coloring::Classes(c) => {
            let k = c.len().max(1);
            for (j, (_, col)) in c.iter().enumerate() {
                let a = r.left() + r.width() * j as f32 / k as f32;
                let b = r.left() + r.width() * (j + 1) as f32 / k as f32;
                p.rect_filled(
                    Rect::from_min_max(Pos2::new(a, r.top()), Pos2::new(b + 0.5, r.bottom())),
                    0.0,
                    crate::palette::seen(*col),
                );
            }
        }
        Coloring::Packed | Coloring::Shade { .. } => {}
    }
    let stroke = if resp.hovered() {
        ui.visuals().widgets.hovered.fg_stroke
    } else {
        egui::Stroke::new(1.0, ui.visuals().widgets.noninteractive.fg_stroke.color.gamma_multiply(0.4))
    };
    p.rect_stroke(r, 2.0, stroke, egui::StrokeKind::Outside);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text("a click: another palette")
}

/// A map's legend with its palette first, a click on the palette giving the others: of values, the range of the
/// colours under the ramp; of classes, a line each (`most` of them at most, a line with how many more).
pub(super) fn palette_legend(
    ui: &mut Ui,
    s: &mut Session,
    key: &(String, String),
    look: &Look,
    names: Option<&[terraflux_ops::legend::Class]>,
    most: usize,
) {
    let width = ui.available_width().clamp(120.0, 260.0);
    let bar = palette_bar(ui, &look.coloring, width);
    egui::Popup::menu(&bar)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| palette_menu(ui, s, key, &look.coloring));
    match &look.coloring {
        Coloring::Continuous { min, max, log, .. } => {
            ui.allocate_ui(Vec2::new(width, 16.0), |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(cell_text(*min)).size(11.0));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new(cell_text(*max)).size(11.0));
                        if *log {
                            ui.with_layout(egui::Layout::centered_and_justified(egui::Direction::LeftToRight), |ui| {
                                ui.label(RichText::new("logarithmic").weak().size(10.5));
                            });
                        }
                    });
                });
            });
        }
        Coloring::Classes(c) => {
            let lines = crate::map_view::legend_lines(c, &look.counts, &|v| crate::legend_view::name(look, names, v));
            for (col, name, _) in lines.iter().take(most) {
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(Vec2::new(14.0, 10.0), Sense::hover());
                    ui.painter().rect_filled(r, 2.0, crate::palette::seen(*col));
                    ui.add(egui::Label::new(RichText::new(name).size(11.5)).truncate());
                });
            }
            if most > 0 && lines.len() > most {
                ui.label(RichText::new(format!("and {} more", lines.len() - most)).weak().size(11.0));
            }
        }
        Coloring::Packed | Coloring::Shade { .. } => {}
    }
}

/// The palettes a map can be shown in, each drawn: by what the values are, or a ramp, on a scale of logarithms
/// or not (kept for the output).
pub(super) fn palette_menu(ui: &mut Ui, s: &mut Session, key: &(String, String), now: &Coloring) {
    ui.set_min_width(250.0);
    let chosen = s.palettes.get(key).copied();
    let (ramp_now, log_now) = match now {
        Coloring::Continuous { ramp, log, .. } => (Some(*ramp), *log),
        Coloring::Classes(_) | Coloring::Packed | Coloring::Shade { .. } => (None, false),
    };
    if ui
        .add(egui::Button::selectable(chosen.is_none(), "by what they are"))
        .on_hover_text("the colours of what made the values: their classes, a product's own, a ramp of values")
        .clicked()
    {
        s.palettes.remove(key);
        ui.close();
    }
    ui.separator();
    for (name, ramp) in RAMPS {
        let on = matches!(chosen, Some(Palette::Chosen(r, _)) if r == ramp);
        let clicked = ui
            .horizontal(|ui| {
                let (r, swatch) = ui.allocate_exact_size(Vec2::new(64.0, 14.0), Sense::click());
                let k = 24;
                for j in 0..k {
                    let a = r.left() + r.width() * j as f32 / k as f32;
                    let b = r.left() + r.width() * (j + 1) as f32 / k as f32;
                    let c = crate::palette::seen(ramp.colour(j as f32 / (k - 1) as f32));
                    ui.painter().rect_filled(
                        Rect::from_min_max(Pos2::new(a, r.top()), Pos2::new(b + 0.5, r.bottom())),
                        0.0,
                        c,
                    );
                }
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
            s.palettes.insert(key.clone(), Palette::Chosen(ramp, log_now));
            ui.close();
        }
    }
    ui.separator();
    let mut log = log_now;
    if ui
        .checkbox(&mut log, "logarithmic")
        .on_hover_text("the colours on a scale of logarithms (the values at or below 0 in the first colour)")
        .changed()
    {
        let ramp = match chosen {
            Some(Palette::Chosen(r, _)) => r,
            _ => ramp_now.unwrap_or(Ramp::Viridis),
        };
        s.palettes.insert(key.clone(), Palette::Chosen(ramp, log));
        // (a range at or below 0 has no logarithm: the colours over the values again)
        if log && s.ranges.get(key).is_some_and(|(a, _)| *a <= 0.0) {
            s.ranges.remove(key);
        }
    }
}

/// The histogram of a map of values, the range of its colours set on it (see [`crate::histogram::show_ranged`]):
/// its bars span the range by the values (`first`) and the one chosen, and stay while a line is dragged.
pub(super) fn range_histogram(
    ui: &mut Ui,
    s: &mut Session,
    key: &(String, String),
    src: &Source,
    coloring: &Coloring,
    first: Option<(f64, f64)>,
    id: egui::Id,
) {
    let Coloring::Continuous { min, max, log, .. } = *coloring else { return };
    let held = id.with("span");
    let span = ui.data(|d| d.get_temp::<(f64, f64)>(held)).unwrap_or_else(|| {
        let (a, b) = first.unwrap_or((min, max));
        (a.min(min), b.max(max))
    });
    match s.histograms.get(ui.ctx(), src, span.0, span.1, log) {
        Some(h) => {
            let (asked, dragging) = crate::histogram::show_ranged(ui, &h, coloring, id);
            ui.data_mut(|d| {
                if dragging {
                    d.insert_temp(held, span);
                } else {
                    d.remove::<(f64, f64)>(held);
                }
            });
            match asked {
                Some(crate::histogram::Asked::Range(a, b)) => {
                    s.ranges.insert(key.clone(), (a, b));
                }
                Some(crate::histogram::Asked::Again) => {
                    s.ranges.remove(key);
                }
                None => {}
            }
        }
        None => {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(RichText::new("the histogram...").weak());
            });
        }
    }
}

/// A layer moved among the rows: from its index to a place among the rows from the top (0: over them all; the
/// number of rows: under them all).
pub(super) fn move_to_slot(layers: &mut Vec<Layer>, from: usize, slot: usize) {
    let n = layers.len();
    if from >= n {
        return;
    }
    let at_from = n - 1 - from;
    let mut rows: Vec<Layer> = layers.drain(..).rev().collect();
    let l = rows.remove(at_from);
    let at = if slot > at_from { slot - 1 } else { slot };
    rows.insert(at.min(rows.len()), l);
    *layers = rows.into_iter().rev().collect();
}

/// What the Layers asked for, done.
fn act(
    ui: &mut Ui,
    s: &mut Session,
    r: &Registry,
    sh: &mut Shown,
    asked: Asked,
    (node, port): (&str, &str),
    selected_id: egui::Id,
) {
    let n = sh.layers.len();
    if let Some(i) = asked.table.filter(|i| *i < n) {
        let l = &sh.layers[i];
        let was = sh.attributes.as_ref().is_some_and(|o| o.is(&l.node, &l.port));
        sh.attributes = (!was).then(|| crate::attribute_table::Open::of(&l.node, &l.port));
    }
    if let Some(i) = asked.zoom.filter(|i| *i < n) {
        let l = &sh.layers[i];
        if l.is(node, port) {
            sh.view.fit();
        } else if let Some(cells) = sh.view.shown_grid().and_then(|g| layer_cells(s, l, g)) {
            sh.view.zoom_to(cells);
        }
    }
    if let Some(i) = asked.solo.filter(|i| *i < n) {
        for (j, l) in sh.layers.iter_mut().enumerate() {
            l.visible = j == i;
        }
    }
    if let Some(on) = asked.all {
        for l in &mut sh.layers {
            l.visible = on;
        }
    }
    if let Some((from, slot)) = asked.moved {
        move_to_slot(&mut sh.layers, from, slot);
    }
    if let Some(i) = asked.up.filter(|i| i + 1 < n) {
        sh.layers.swap(i, i + 1);
    }
    if let Some(i) = asked.down.filter(|i| *i > 0 && *i < n) {
        sh.layers.swap(i, i - 1);
    }
    if let Some(i) = asked.top.filter(|i| *i < n) {
        let l = sh.layers.remove(i);
        sh.layers.push(l);
    }
    if let Some(i) = asked.bottom.filter(|i| *i < n) {
        let l = sh.layers.remove(i);
        sh.layers.insert(0, l);
    }
    if let Some(i) = asked.remove.filter(|i| *i < n) {
        sh.layers.remove(i);
    }
    // a map clicked: it is the map shown (its legend and settings at the right)
    if let Some((n, p)) = asked.show {
        let band = sh.layers.iter().find(|l| l.is(&n, &p)).and_then(|l| l.band);
        sh.node = n;
        sh.port = p;
        sh.band = band.unwrap_or(0);
        sh.chosen = band.is_some();
    }
    // the map shown kept as a layer: on top (and under what is added, so that both are seen)
    if asked.pin || asked.add.is_some() {
        pin(&mut sh.layers, node, port);
    }
    if let Some((n, p)) = asked.add {
        sh.layers.push(Layer::new(&n, &p));
        ui.data_mut(|d| d.insert_temp(selected_id, format!("{n}.{p}")));
    }
    if asked.url && s.url_layer.is_none() {
        s.url_layer = Some(crate::remote::Dialog::default());
    }
    if asked.no_join {
        s.dropped_rasters.clear();
    }
    if asked.join {
        // (joined with the Viewer's layers in the session: the stack takes the place of theirs)
        let ids = std::mem::take(&mut s.dropped_rasters);
        let was = std::mem::replace(&mut s.preview_layers, std::mem::take(&mut sh.layers));
        match crate::series_drop::join(s, r, &ids) {
            Ok(_) => {
                if let Some((n, p)) = s.preview.clone() {
                    sh.node = n;
                    sh.port = p;
                    sh.band = 0;
                    sh.chosen = false;
                }
            }
            Err(e) => s.editor.message = Some(e),
        }
        sh.layers = std::mem::replace(&mut s.preview_layers, was);
    }
    let files = match (asked.files, asked.pick_files) {
        (Some(f), _) => f,
        (None, true) => pick_layer_files().unwrap_or_default(),
        _ => Vec::new(),
    };
    if !files.is_empty() {
        // (the files' nodes added as the Viewer takes files dropped on it: their layers on top of these, the map
        // shown kept under them)
        pin(&mut sh.layers, node, port);
        let was = std::mem::replace(&mut s.preview_layers, std::mem::take(&mut sh.layers));
        let shown = s.preview.clone();
        drop_on_viewer(s, r, &files);
        sh.layers = std::mem::replace(&mut s.preview_layers, was);
        s.preview = shown;
    }
}

/// Rasters or vector features chosen in a dialog, to add as layers.
fn pick_layer_files() -> Option<Vec<PathBuf>> {
    let mut both: Vec<&str> = terraflux_ops::files::RASTER_EXTENSIONS.to_vec();
    both.extend(VECTOR_EXTENSIONS);
    rfd::FileDialog::new()
        .set_title("Rasters or vector features, as layers")
        .add_filter("Rasters and vectors", &both)
        .add_filter("Rasters", &terraflux_ops::files::RASTER_EXTENSIONS)
        .add_filter("Vectors", &VECTOR_EXTENSIONS)
        .add_filter("Every file", &["*"])
        .pick_files()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(l: &[Layer]) -> Vec<&str> {
        l.iter().map(|l| l.node.as_str()).collect()
    }

    /// The rows from the top are the layers from the last: a row moved to a place among them.
    #[test]
    fn a_row_moved_among_the_others() {
        let start = || vec![Layer::new("a", "map"), Layer::new("b", "map"), Layer::new("c", "map")];
        // rows: c, b, a; a (at the bottom) over them all
        let mut l = start();
        move_to_slot(&mut l, 0, 0);
        assert_eq!(names(&l), ["b", "c", "a"]);
        // c (on top) under them all
        let mut l = start();
        move_to_slot(&mut l, 2, 3);
        assert_eq!(names(&l), ["c", "a", "b"]);
        // c between b and a
        let mut l = start();
        move_to_slot(&mut l, 2, 2);
        assert_eq!(names(&l), ["a", "c", "b"]);
        // its own place: as it was
        let mut l = start();
        move_to_slot(&mut l, 1, 1);
        assert_eq!(names(&l), ["a", "b", "c"]);
        move_to_slot(&mut l, 1, 2);
        assert_eq!(names(&l), ["a", "b", "c"]);
    }
}
