//! The Layers of the Viewer, as a GIS has them: a bar (add a layer of the run or a file, remove, up, down, zoom
//! to it, all shown or hidden) over a list of rows, the top layer first. A row: whether it is shown (a click on its
//! box), its legend and settings folded under it (its arrow), its symbol, its name; a click selects it (a map:
//! its legend and settings at the right), a double click zooms to it, a drag puts it over or under the others, a
//! right click (or its ⋯) gives what can be done with it. A palette is changed by a click on it.

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
        Coloring::Classes(_) => None,
    };
    let look = ranged(look, s.ranges.get(&key).copied());
    let look = match crate::legend_view::legend_of(s, &l.node, &l.port, &src) {
        Some(c) => crate::legend_view::apply(&look, &c),
        None => look,
    };
    Some((src, look, first))
}

/// The Layers: their bar, then their rows (see the module).
pub(super) fn layers_section(
    ui: &mut Ui,
    s: &mut Session,
    r: &Registry,
    sh: &mut Shown,
    others: &[(String, String)],
    main_name: &str,
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
    ui.add_space(2.0);
    let dragged_id = egui::Id::new(("layer dragged", salt));
    let frame = egui::Frame::new()
        .fill(ui.visuals().extreme_bg_color)
        .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
        .corner_radius(4)
        .inner_margin(Margin::same(3))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 1.0;
            // the map shown that is no layer: a row of its own, kept by the pin
            if !shown_is_layer {
                ui.horizontal(|ui| {
                    if ui
                        .small_button("📌")
                        .on_hover_text(
                            "keep it as a layer: it stays on the map when another node is clicked (a double click on \
                             the node keeps it too)",
                        )
                        .clicked()
                    {
                        asked.pin = true;
                    }
                    let mut job = egui::text::LayoutJob::default();
                    let font = egui::TextStyle::Body.resolve(ui.style());
                    let text = ui.visuals().text_color();
                    let weak = ui.visuals().weak_text_color();
                    job.append(
                        main_name,
                        0.0,
                        egui::TextFormat { font_id: font, color: text, italics: true, ..Default::default() },
                    );
                    let small = egui::FontId::proportional(11.0);
                    job.append("  shown", 0.0, egui::TextFormat { font_id: small, color: weak, ..Default::default() });
                    ui.add(egui::Label::new(job).truncate());
                });
                if n > 0 {
                    ui.separator();
                }
            }
            // the rows, the top layer first; where each is (its row, then its legend and settings)
            let mut rows: Vec<Rect> = Vec::new();
            let mut let_go = None;
            for i in (0..n).rev() {
                let kind = match &sh.layers[i] {
                    l if l.minus.is_some() => Kind::Difference,
                    l if vectors.iter().any(|(vn, vp, _)| l.is(vn, vp)) => Kind::Features,
                    _ => Kind::Map,
                };
                let (rect, started, released) =
                    layer_row(ui, s, sh, &vectors, (i, kind), (&node, &port), selected == Some(i), salt, &mut asked);
                if started {
                    ui.data_mut(|d| d.insert_temp(dragged_id, i));
                }
                if released {
                    let_go = Some(i);
                }
                rows.push(rect);
            }
            if n == 0 && shown_is_layer {
                ui.label(RichText::new("no layer").weak());
            }
            if n == 0 && !shown_is_layer {
                ui.label(
                    RichText::new("➕ adds the run's maps and features, or a file, over the map shown")
                        .weak()
                        .size(11.0),
                );
            }
            // a row dragged: a line where it would go (between two rows), and there once let go
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
                        Some(r) => r.top() - 0.5,
                        None => rows[rows.len() - 1].bottom() + 0.5,
                    };
                    if moves {
                        let (l, rt) = (ui.min_rect().left(), ui.min_rect().right());
                        let accent = theme::accent();
                        ui.painter().line_segment([Pos2::new(l, y), Pos2::new(rt, y)], egui::Stroke::new(2.0, accent));
                        ui.painter().circle_filled(Pos2::new(l + 2.0, y), 3.0, accent);
                    }
                    // (the name carried by the pointer)
                    let name = sh.layers[from]
                        .name_by(made_of(s, &sh.layers[from]).as_deref(), |n, p, b| band_name(s, n, p, b));
                    let painter = ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Tooltip, dragged_id));
                    let galley =
                        painter.layout_no_wrap(name, egui::FontId::proportional(12.0), ui.visuals().text_color());
                    let at = p + Vec2::new(12.0, 4.0);
                    let back = Rect::from_min_size(at, galley.size()).expand(4.0);
                    painter.rect(
                        back,
                        3.0,
                        ui.visuals().window_fill,
                        ui.visuals().window_stroke,
                        egui::StrokeKind::Outside,
                    );
                    painter.galley(at, galley, ui.visuals().text_color());
                }
            }
            // (a drag that ended elsewhere: let go)
            if from.is_some() && !ui.ctx().input(|i| i.pointer.any_down()) && let_go.is_none() {
                ui.data_mut(|d| d.remove::<usize>(dragged_id));
            }
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
    ui.label(
        RichText::new("drag a row to put it over or under the others; a right click on it: more").weak().size(10.5),
    );
    act(ui, s, r, sh, asked, (&node, &port), selected_id);
}

/// The bar over the rows: the title, then (from the right) add, remove, down, up, zoom to it (the layer
/// selected), all shown or hidden.
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
        ui.label(RichText::new("LAYERS").size(10.5).color(theme::weak()).strong());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            ui.menu_button("➕", |ui| {
                ui.set_min_width(220.0);
                if !shown_is_layer
                    && ui.button(format!("📌 {main_name}")).on_hover_text("the map shown, kept as a layer").clicked()
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
            })
            .response
            .on_hover_text("Add a layer: a map or features of the run, or a file");
            let at = selected.filter(|i| *i < n);
            if ui
                .add_enabled(at.is_some(), egui::Button::new("🗑").small())
                .on_hover_text("remove the layer selected (no longer a layer; the node stays)")
                .clicked()
            {
                asked.remove = at;
            }
            if ui
                .add_enabled(at.is_some_and(|i| i > 0), egui::Button::new("⏷").small())
                .on_hover_text("down: the layer selected under the one below")
                .clicked()
            {
                asked.down = at;
            }
            if ui
                .add_enabled(at.is_some_and(|i| i + 1 < n), egui::Button::new("⏶").small())
                .on_hover_text("up: the layer selected over the one above")
                .clicked()
            {
                asked.up = at;
            }
            if ui
                .add_enabled(at.is_some(), egui::Button::new("🔍").small())
                .on_hover_text("zoom to layer: the view on all of the layer selected (a double click on a row too)")
                .clicked()
            {
                asked.zoom = at;
            }
            let all = sh.layers.iter().all(|l| l.visible);
            if ui
                .add_enabled(n > 0, egui::Button::new("👁").small())
                .on_hover_text(if all { "hide every layer" } else { "show every layer" })
                .clicked()
            {
                asked.all = Some(!all);
            }
        });
    });
}

/// A layer's row (its box, its arrow, its symbol, its name, its ⋯), then its legend and settings when unfolded;
/// where it is (the row and its settings), and whether a drag of it started or ended.
#[allow(clippy::too_many_arguments)]
fn layer_row(
    ui: &mut Ui,
    s: &mut Session,
    sh: &mut Shown,
    vectors: &[(String, String, Arc<terraflux_raster::vector::Vector>)],
    (i, kind): (usize, Kind),
    (node, port): (&str, &str),
    selected: bool,
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
    let mut open: bool = ui.data(|d| d.get_temp(open_id)).unwrap_or(true);
    let table_open = sh.attributes.as_ref().is_some_and(|o| o.is(&sh.layers[i].node, &sh.layers[i].port));
    let top = ui.cursor().top();
    let (rect, row) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 24.0), Sense::click_and_drag());
    row.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, selected, &name));
    let visuals = ui.visuals().clone();
    if selected {
        ui.painter().rect_filled(rect, 3.0, visuals.selection.bg_fill.gamma_multiply(0.5));
    } else if row.hovered() {
        ui.painter().rect_filled(rect, 3.0, visuals.widgets.hovered.weak_bg_fill);
    }
    // the map whose legend is at the right: a bar of the accent at its left
    if active {
        let bar = Rect::from_min_size(rect.min + Vec2::new(0.0, 3.0), Vec2::new(3.0, rect.height() - 6.0));
        ui.painter().rect_filled(bar, 1.5, theme::accent());
    }
    let mut x = rect.left() + 6.0;
    let l = &mut sh.layers[i];
    ui.put(
        Rect::from_min_size(Pos2::new(x, rect.top()), Vec2::new(18.0, rect.height())),
        egui::Checkbox::without_text(&mut l.visible),
    )
    .on_hover_text("shown on the map, or hidden")
    .widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, true, l.visible, format!("show {name}")));
    x += 22.0;
    let arrow = ui.interact(
        Rect::from_center_size(Pos2::new(x + 6.0, rect.center().y), Vec2::splat(16.0)),
        open_id,
        Sense::click(),
    );
    arrow.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "legend and settings"));
    egui::collapsing_header::paint_default_icon(ui, if open { 1.0 } else { 0.0 }, &arrow);
    if arrow.on_hover_text("its legend and settings, under it, or folded").clicked() {
        open = !open;
    }
    x += 16.0;
    let symbol = Rect::from_min_size(Pos2::new(x, rect.center().y - 7.0), Vec2::new(18.0, 14.0));
    let features = vectors.iter().find(|(vn, vp, _)| l.is(vn, vp)).map(|(_, _, v)| v.clone());
    paint_symbol(
        ui,
        symbol,
        kind,
        look.as_ref().map(|x| &x.1.coloring),
        made.as_deref(),
        features.as_deref(),
        &l.style,
    );
    x += 24.0;
    // its ⋯ at the right: what can be done with it
    let dots_rect = Rect::from_center_size(Pos2::new(rect.right() - 12.0, rect.center().y), Vec2::new(20.0, 20.0));
    let dots = ui.interact(dots_rect, row_id.with("more"), Sense::click()).on_hover_text(
        "what can be done with it: zoom to it, its table, its colours, up and down, remove (a right click on the row too)",
    );
    dots.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, format!("{name}: its menu")));
    let dot = if dots.hovered() { visuals.strong_text_color() } else { visuals.weak_text_color() };
    for dx in [-5.0, 0.0, 5.0] {
        ui.painter().circle_filled(dots_rect.center() + Vec2::new(dx, 0.0), 1.6, dot);
    }
    // its name, cut to the room left
    let colour = if l.visible { visuals.text_color() } else { visuals.weak_text_color() };
    let text = if active { RichText::new(&name).strong().color(colour) } else { RichText::new(&name).color(colour) };
    let room = (dots_rect.left() - 4.0 - x).max(10.0);
    let galley =
        egui::WidgetText::from(text).into_galley(ui, Some(egui::TextWrapMode::Truncate), room, egui::TextStyle::Body);
    ui.painter().galley(Pos2::new(x, rect.center().y - galley.size().y / 2.0), galley, colour);
    let hint = match kind {
        Kind::Map if !active => format!("{name}: a click shows its legend and settings at the right"),
        _ => name.clone(),
    };
    let row = row.on_hover_text(format!(
        "{hint}; a double click zooms to it; drag it over or under the others; a right click: more"
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
    // its legend and settings, under it
    if open {
        ui.horizontal(|ui| {
            ui.add_space(28.0);
            ui.vertical(|ui| {
                settings(ui, s, sh, vectors, (i, kind), (active, table_open), look.as_ref(), salt, asked);
            });
        });
        ui.add_space(3.0);
    }
    ui.data_mut(|d| d.insert_temp(open_id, open));
    let whole = Rect::from_min_max(Pos2::new(rect.left(), top), Pos2::new(rect.right(), ui.cursor().top()));
    (whole, started, released)
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

/// A layer's legend and settings, under its row: of a map, its palette (a click on it gives the others), its
/// classes or range, how opaque, which of its maps, its histogram (the range of its colours set there); of
/// features, their style; of a difference, what it is.
#[allow(clippy::too_many_arguments)]
fn settings(
    ui: &mut Ui,
    s: &mut Session,
    sh: &mut Shown,
    vectors: &[(String, String, Arc<terraflux_raster::vector::Vector>)],
    (i, kind): (usize, Kind),
    (active, table_open): (bool, bool),
    look: Option<&LayerLook>,
    salt: &str,
    asked: &mut Asked,
) {
    // (the slider, its number and its name in the room there is: wider, the panel would grow at every frame)
    ui.spacing_mut().slider_width = (ui.available_width() - 130.0).clamp(40.0, 160.0);
    let l = &mut sh.layers[i];
    match kind {
        Kind::Difference => {
            ui.add(egui::Slider::new(&mut l.opacity, 0.0..=1.0).text("opacity"));
            match made_of(s, l) {
                Some(d) => {
                    ui.label(RichText::new(&d.summary).weak().size(11.0));
                    legend(ui, &d.coloring, &d.counts, &|v| d.name(v));
                }
                None => {
                    match s.layer_differences.get(&l.difference_key().unwrap_or_default()).and_then(|d| d.made.as_ref())
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
            if active {
                ui.add(egui::Slider::new(&mut l.opacity, 0.0..=1.0).text("opacity"));
                ui.label(RichText::new("its legend and settings: at the right").weak().size(10.5));
                return;
            }
            match look {
                Some((src, lk, first)) => {
                    let names = crate::legend_view::legend_of(s, &l.node, &l.port, src);
                    palette_legend(ui, s, &key, lk, names.as_deref(), 8);
                    ui.add(egui::Slider::new(&mut l.opacity, 0.0..=1.0).text("opacity"));
                    // of a stack, a series or rasters on disk: the map it shows
                    let names = s.value_of(&l.node, &l.port).map(|v| maps_of(&v).1).unwrap_or_default();
                    if names.len() > 1 {
                        let raster_chosen = match s.value_of(&l.node, &l.port) {
                            Some(Value::Raster(r)) => r.chosen,
                            _ => 0,
                        };
                        let at = l.band.unwrap_or(raster_chosen).min(names.len() - 1);
                        egui::ComboBox::from_id_salt(("layer band", salt, &l.node, &l.port))
                            .selected_text(format!("map: {}", names[at]))
                            .width(160.0)
                            .show_ui(ui, |ui| {
                                for (b, name) in names.iter().enumerate() {
                                    if ui.selectable_label(b == at, name).clicked() {
                                        l.band = Some(b);
                                    }
                                }
                            });
                    }
                    // the values: their histogram, the range of the colours set on it
                    if matches!(lk.coloring, Coloring::Continuous { .. }) {
                        let id = egui::Id::new(("layer histogram", salt, &l.node, &l.port));
                        egui::CollapsingHeader::new("histogram").id_salt(id).show(ui, |ui| {
                            range_histogram(ui, s, &key, src, &lk.coloring, *first, id);
                        });
                    }
                }
                None => {
                    ui.add(egui::Slider::new(&mut l.opacity, 0.0..=1.0).text("opacity"));
                    if s.value_of(&l.node, &l.port).is_some() {
                        ui.spinner();
                    } else {
                        ui.label(RichText::new("not run yet").weak().size(11.0));
                    }
                }
            }
        }
    }
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
                        p.circle_filled(r.center() + Vec2::new(dx, dy), 2.5, c);
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
    }
}

/// The palettes a map can be shown in, each drawn: by what the values are, or a ramp, on a scale of logarithms
/// or not (kept for the output).
pub(super) fn palette_menu(ui: &mut Ui, s: &mut Session, key: &(String, String), now: &Coloring) {
    ui.set_min_width(250.0);
    let chosen = s.palettes.get(key).copied();
    let (ramp_now, log_now) = match now {
        Coloring::Continuous { ramp, log, .. } => (Some(*ramp), *log),
        Coloring::Classes(_) => (None, false),
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
    let Coloring::Continuous { min, max, .. } = *coloring else { return };
    let held = id.with("span");
    let span = ui.data(|d| d.get_temp::<(f64, f64)>(held)).unwrap_or_else(|| {
        let (a, b) = first.unwrap_or((min, max));
        (a.min(min), b.max(max))
    });
    match s.histograms.get(ui.ctx(), src, span.0, span.1) {
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
