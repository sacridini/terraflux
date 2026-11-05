//! The attributes of a vector layer as a table, in a panel under the map: a row per feature (its number, then its
//! attributes), sorted and filtered as the app's other tables are ([`crate::table_view`]). A row clicked selects
//! its feature, outlined on the map in the accent and zoomed to; a feature clicked on the map selects its row and
//! scrolls to it. Several are selected with ctrl+click (toggled) and shift+click (the range from the last row
//! clicked), on the table or on the map; a shape dragged on the map (shift: a rectangle, shift+alt: a lasso)
//! selects the features it meets (ctrl adds them). Escape lets them go, and the header zooms to all of them,
//! copies or saves only their rows, or makes a `vector.filter` node that keeps them.

use std::collections::BTreeSet;
use std::sync::Arc;

use egui::{RichText, Ui};
use terraflux_ops::Table;
use terraflux_raster::vector::{Column, Vector};

use crate::map_view::MapView;
use crate::table_view::Pick;

/// A table of attributes opened: of which layer (an output of the run, empty for features shown alone), the rows
/// selected, whether the view is to zoom to the row clicked (`zoom`) or to all the rows selected (`zoom_all`),
/// whether *Filter the selection* was clicked (`filter`), and whether a shape is being dragged on the map
/// (`dragging`: escape is for the shape, not for the selection).
#[derive(Debug, Clone, Default)]
pub struct Open {
    pub node: String,
    pub port: String,
    pub pick: Pick,
    pub zoom: bool,
    pub zoom_all: bool,
    pub filter: bool,
    pub dragging: bool,
}

impl Open {
    pub fn of(node: &str, port: &str) -> Open {
        Open { node: node.into(), port: port.into(), ..Default::default() }
    }

    /// Whether it is the table of the output `node.port`.
    pub fn is(&self, node: &str, port: &str) -> bool {
        self.node == node && self.port == port
    }
}

/// The attributes as a table: the feature's number (from 1, as the pointer tells it) then a column each. A text
/// is the number of its place among the column's texts in order, shown by the text itself (so that the column
/// sorts alphabetically); an empty text is NA.
pub fn table_of(v: &Vector) -> Table {
    let mut t = Table::new(vec!["feature".into()]);
    t.rows = (1..=v.len()).map(|i| vec![i as f64]).collect();
    for (name, col) in &v.attributes {
        t.columns.push(name.clone());
        match col {
            Column::Numbers(x) => {
                for (r, &x) in t.rows.iter_mut().zip(x) {
                    r.push(x);
                }
            }
            Column::Texts(texts) => {
                let mut names: Vec<String> = texts.iter().filter(|s| !s.is_empty()).cloned().collect();
                names.sort();
                names.dedup();
                for (r, s) in t.rows.iter_mut().zip(texts) {
                    r.push(names.binary_search(s).map_or(f64::NAN, |i| (i + 1) as f64));
                }
                t.labels.insert(name.clone(), names);
            }
        }
    }
    t
}

/// A table as text for the clipboard: tab-separated, its columns named first.
pub fn tsv(t: &Table) -> String {
    let mut s = t.columns.join("\t");
    s.push('\n');
    for r in &t.rows {
        let cells: Vec<String> = r.iter().enumerate().map(|(j, &x)| t.cell(j, x)).collect();
        s.push_str(&cells.join("\t"));
        s.push('\n');
    }
    s
}

/// The rows of a table that are selected, in their order (the columns and the names of the texts as they are).
pub fn subset(t: &Table, rows: &BTreeSet<usize>) -> Table {
    let mut s = Table::new(t.columns.clone());
    s.labels = t.labels.clone();
    s.rows = rows.iter().filter_map(|&k| t.rows.get(k).cloned()).collect();
    s
}

/// The box that holds all of these (`[x0, y0, x1, y1]`); none when there are none.
pub fn union_box(boxes: impl IntoIterator<Item = [f64; 4]>) -> Option<[f64; 4]> {
    boxes.into_iter().reduce(|a, b| [a[0].min(b[0]), a[1].min(b[1]), a[2].max(b[2]), a[3].max(b[3])])
}

/// The table of a layer, made once while the layer is the same.
fn cached(ui: &Ui, id: egui::Id, v: &Arc<Vector>) -> Arc<Table> {
    let key = id.with("made");
    let at = Arc::as_ptr(v) as usize;
    if let Some((k, t)) = ui.data(|d| d.get_temp::<(usize, Arc<Table>)>(key))
        && k == at
    {
        return t;
    }
    let t = Arc::new(table_of(v));
    ui.data_mut(|d| d.insert_temp(key, (at, t.clone())));
    t
}

/// The table in a panel under the map: copied or saved as CSV from its header, closed by its x. False once it is
/// closed.
pub fn panel(ui: &mut Ui, id: egui::Id, v: &Arc<Vector>, name: &str, open: &mut Open) -> bool {
    let table = cached(ui, id, v);
    let mut keep = true;
    let failed = id.with("failed");
    egui::Panel::bottom(id.with("panel"))
        .resizable(true)
        .default_size(240.0)
        .min_size(90.0)
        .frame(egui::Frame::new().fill(ui.visuals().panel_fill).inner_margin(egui::Margin::same(8)))
        .show(ui, |ui| {
            // escape lets the selection go (when no box has the focus, and no shape is being dragged on the map)
            if !open.pick.rows.is_empty()
                && !open.dragging
                && !ui.ctx().text_edit_focused()
                && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
            {
                open.pick.clear();
            }
            // ctrl+A selects the rows shown (when the pointer is on the table, or the keyboard focus is in it, and
            // no box has the focus)
            let table_id = id.with("table");
            let focus_here = ui
                .ctx()
                .memory(|m| m.focused())
                .and_then(|f| ui.ctx().read_response(f))
                .is_some_and(|r| ui.max_rect().contains(r.rect.center()));
            if (ui.ui_contains_pointer() || focus_here)
                && !ui.ctx().text_edit_focused()
                && ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::A))
            {
                open.pick.all(&crate::table_view::shown_rows(ui, &table, table_id));
            }
            let n = open.pick.rows.len();
            let picked = open.pick.rows.clone();
            ui.horizontal(|ui| {
                let title = if name.is_empty() { "Attributes".to_string() } else { format!("Attributes of {name}") };
                ui.label(RichText::new(title).strong());
                if n > 0 {
                    ui.label(format!("{n} selected"));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("x").on_hover_text("close the table").clicked() {
                        keep = false;
                    }
                    let save = |ui: &mut Ui, t: &Table| {
                        let file = rfd::FileDialog::new()
                            .set_title("Save the attributes")
                            .add_filter("CSV", &["csv"])
                            .set_file_name(format!("{}.csv", if name.is_empty() { "attributes" } else { name }))
                            .save_file();
                        if let Some(path) = file {
                            let e = std::fs::write(&path, t.to_csv()).err().map(|e| format!("{}: {e}", path.display()));
                            ui.data_mut(|d| d.insert_temp(failed, e.unwrap_or_default()));
                        }
                    };
                    if ui.small_button("Save as CSV").on_hover_text("the table in a file").clicked() {
                        save(ui, &table);
                    }
                    if n > 0 && ui.small_button("Save selected").on_hover_text("the selected rows in a file").clicked()
                    {
                        save(ui, &subset(&table, &picked));
                    }
                    if ui.small_button("Copy").on_hover_text("the table, tab-separated, to paste in a sheet").clicked()
                    {
                        ui.ctx().copy_text(tsv(&table));
                    }
                    if n > 0
                        && ui
                            .small_button("Copy selected")
                            .on_hover_text("the selected rows, tab-separated, to paste in a sheet")
                            .clicked()
                    {
                        ui.ctx().copy_text(tsv(&subset(&table, &picked)));
                    }
                });
            });
            ui.horizontal(|ui| {
                // (first: where it is whatever the selection, and not pushed out of a narrow panel)
                let (can, why) = can_filter(open, v, n);
                if ui.add_enabled(can, egui::Button::new("Filter the selection").small()).on_hover_text(why).clicked() {
                    open.filter = true;
                }
                if n > 0 {
                    if ui
                        .small_button("Zoom to selection")
                        .on_hover_text("the view on all the selected features")
                        .clicked()
                    {
                        open.zoom_all = true;
                    }
                    if ui.small_button("Clear selection").on_hover_text("let them go (escape)").clicked() {
                        open.pick.clear();
                    }
                }
                if ui
                    .small_button("Select all shown")
                    .on_hover_text("every row the filter leaves (ctrl+A with the pointer on the table)")
                    .clicked()
                {
                    open.pick.all(&crate::table_view::shown_rows(ui, &table, table_id));
                }
            });
            if let Some(e) = ui.data(|d| d.get_temp::<String>(failed)).filter(|e| !e.is_empty()) {
                ui.label(RichText::new(e).color(ui.visuals().error_fg_color));
            }
            crate::table_view::show_picking(ui, &table, table_id, Some(&mut open.pick));
        });
    // a row clicked alone: the view goes to its feature (with ctrl or shift: the selection grows or shrinks, the
    // view stays, also when one row is left)
    if open.pick.clicked {
        open.pick.clicked = false;
        open.zoom = open.pick.zooms();
    }
    keep
}

/// The features selected shown on the map (`at`: the index of the table's layer among the view's layers, none when
/// the layer is not drawn), and the view brought to them when asked.
pub fn link(open: &mut Open, view: &mut MapView, ctx: &egui::Context, v: &Arc<Vector>, at: Option<usize>) {
    view.selected = at.zip(Some(open.pick.rows.iter().copied().collect::<Vec<_>>())).filter(|(_, r)| !r.is_empty());
    open.dragging = view.area.is_some();
    if open.zoom_all {
        let rows: Vec<usize> = open.pick.rows.iter().copied().collect();
        open.zoom_all = !zoom_to(view, ctx, v, &rows);
        open.zoom = false;
    } else if open.zoom {
        if let Some(k) = open.pick.row {
            open.zoom = !zoom_to(view, ctx, v, &[k]);
        } else {
            open.zoom = false;
        }
    }
}

/// A feature clicked on the map selects its row, and the table scrolls to it; with ctrl it toggles the row.
pub fn clicked(open: &mut Open, view: &mut MapView, at: Option<usize>) {
    let clicked = std::mem::take(&mut view.feature_clicked);
    let toggle = std::mem::take(&mut view.feature_toggle);
    if let Some(l) = at
        && let Some(&(_, k)) = clicked.iter().find(|(i, _)| *i == l)
    {
        if toggle {
            open.pick.click(&[k], k, true, false);
            open.pick.clicked = false;
        } else {
            open.pick.only(k);
        }
        open.pick.scroll = open.pick.rows.contains(&k);
    }
}

/// A shape dragged on the map (shift+drag) selects the features of the table's layer that it meets: they replace
/// the selection, or join it when ctrl was held. Waits (and asks for a frame) while the layer is prepared for the
/// map; the shape of a layer not drawn is let go. The view stays where it is.
pub fn area(open: &mut Open, view: &mut MapView, ctx: &egui::Context, v: &Arc<Vector>, at: Option<usize>) {
    let Some(done) = view.area_done.take() else { return };
    let (Some(_), Some(grid)) = (at, view.shown_grid().cloned()) else { return };
    match view.vectors.layer(ctx, v, &grid) {
        Ok(Some(layer)) => open.pick.area(&layer.in_shape(&done.shape), done.add),
        Ok(None) => {
            view.area_done = Some(done);
            ctx.request_repaint();
        }
        Err(_) => {}
    }
}

/// Whether the selection can be made a `vector.filter` (the layer an output of the model, a selection, no
/// attribute named `feature`, which the filter would read instead of the feature's number), and what the button
/// says.
fn can_filter(open: &Open, v: &Vector, selected: usize) -> (bool, &'static str) {
    if open.node.is_empty() {
        (false, "the features are not an output of the model: there is nothing to put a filter after")
    } else if v.column("feature").is_some() {
        (false, "the features have an attribute named feature, which a filter would read instead of their number")
    } else if selected == 0 {
        (false, "select features first (on the table, or shift+drag on the map)")
    } else {
        (true, "a vector.filter node after this layer that keeps the selected features, shown in the Viewer")
    }
}

/// The condition of a `vector.filter` that keeps exactly the features of the rows `rows` (from 0) among `total`: by
/// their numbers (`feature`, from 1), as ranges (`3 <= feature <= 9`, `feature == 12`) joined by `or`, in a
/// balanced tree of brackets when they are many; the ranges of the other features under `not` when they are
/// fewer. Rows past `total` are left out.
pub fn filter_expression(rows: &BTreeSet<usize>, total: usize) -> String {
    let ranges = |numbers: &mut dyn Iterator<Item = usize>| -> Vec<(usize, usize)> {
        let mut out: Vec<(usize, usize)> = Vec::new();
        for k in numbers {
            match out.last_mut() {
                Some((_, b)) if *b + 1 == k => *b = k,
                _ => out.push((k, k)),
            }
        }
        out
    };
    let chosen = ranges(&mut rows.iter().filter(|&&k| k < total).map(|&k| k + 1));
    let others = ranges(&mut (1..=total).filter(|k| !rows.contains(&(k - 1))));
    let text = |r: &[(usize, usize)]| {
        let terms: Vec<String> = r
            .iter()
            .map(|&(a, b)| if a == b { format!("feature == {a}") } else { format!("{a} <= feature <= {b}") })
            .collect();
        join_or(&terms)
    };
    match (chosen.len(), others.len()) {
        (0, _) => "feature < 0".into(),
        (_, 0) => "feature >= 1".into(),
        (c, o) if o < c => format!("not ({})", text(&others)),
        _ => text(&chosen),
    }
}

/// Conditions joined by `or`: in one line when few, else halves in brackets, so that a long list is a shallow
/// tree for the calculator.
fn join_or(terms: &[String]) -> String {
    if terms.len() <= 8 {
        return terms.join(" or ");
    }
    let (a, b) = terms.split_at(terms.len() / 2);
    format!("({}) or ({})", join_or(a), join_or(b))
}

/// The view on features, with room around them (more for a point, which has no size). True when done: false while
/// the layer is being prepared for the map.
fn zoom_to(view: &mut MapView, ctx: &egui::Context, v: &Arc<Vector>, rows: &[usize]) -> bool {
    let Some(grid) = view.shown_grid().cloned() else { return true };
    let layer = match view.vectors.layer(ctx, v, &grid) {
        Ok(Some(l)) => l,
        Ok(None) => return false,
        Err(_) => return true,
    };
    let Some(b) = union_box(rows.iter().filter_map(|&k| layer.feature_box(k))) else { return true };
    let side = grid.width.max(grid.height) as f64;
    let m = 0.1 * (b[2] - b[0]).max(b[3] - b[1]) + 0.01 * side;
    view.zoom_to([(b[0] - m) as f32, (b[1] - m) as f32, (b[2] + m) as f32, (b[3] + m) as f32]);
    true
}

#[cfg(test)]
mod tests {
    use terraflux_raster::vector::Geometry;

    use super::*;

    fn features() -> Vector {
        Vector {
            geometries: vec![Geometry::Point([0.0, 0.0]), Geometry::Point([1.0, 1.0]), Geometry::Point([2.0, 2.0])],
            attributes: vec![
                ("area".into(), Column::Numbers(vec![2.5, f64::NAN, 0.5])),
                ("use".into(), Column::Texts(vec!["pasture".into(), "forest".into(), String::new()])),
            ],
            crs: None,
        }
    }

    #[test]
    fn attributes_as_a_table() {
        let t = table_of(&features());
        assert_eq!(t.columns, ["feature", "area", "use"]);
        // the number of each feature, from 1, then the numbers as they are
        assert_eq!(t.rows.iter().map(|r| r[0]).collect::<Vec<_>>(), [1.0, 2.0, 3.0]);
        assert_eq!(t.rows[0][1], 2.5);
        assert!(t.rows[1][1].is_nan());
        // the texts by their place in order, shown as themselves; an empty one is NA
        assert_eq!(t.rows.iter().map(|r| r[2]).take(2).collect::<Vec<_>>(), [2.0, 1.0]);
        assert!(t.rows[2][2].is_nan());
        assert_eq!(crate::table_view::cell_of(&t, 2, t.rows[0][2]), "pasture");
        assert_eq!(crate::table_view::cell_of(&t, 2, t.rows[1][2]), "forest");
        assert_eq!(crate::table_view::cell_of(&t, 2, t.rows[2][2]), "NA");
        // sorted by the texts: alphabetical
        let st = crate::table_view::TableState { sort: Some((2, true)), ..Default::default() };
        assert_eq!(crate::table_view::rows_shown(&t, &st).unwrap(), [1, 0, 2]);
        // filtered by a word of a text
        let st = crate::table_view::TableState { filter: "past".into(), ..Default::default() };
        assert_eq!(crate::table_view::rows_shown(&t, &st).unwrap(), [0]);
    }

    #[test]
    fn copied_tab_separated() {
        assert_eq!(tsv(&table_of(&features())), "feature\tarea\tuse\n1\t2.5\tpasture\n2\t\tforest\n3\t0.5\t\n");
    }

    fn rows(p: &Pick) -> Vec<usize> {
        p.rows.iter().copied().collect()
    }

    #[test]
    fn rows_toggled_and_ranged() {
        let shown = [4, 2, 0, 1, 3];
        let mut p = Pick::default();
        // a plain click: only that row
        p.click(&shown, 2, false, false);
        assert_eq!((rows(&p), p.row, p.clicked), (vec![2], Some(2), true));
        // ctrl toggles: added, then taken out (the row left is the last one)
        p.click(&shown, 3, true, false);
        assert_eq!((rows(&p), p.row), (vec![2, 3], Some(3)));
        p.click(&shown, 3, true, false);
        assert_eq!((rows(&p), p.row), (vec![2], Some(2)));
        p.click(&shown, 2, true, false);
        assert_eq!((rows(&p), p.row), (vec![], None));
        // shift: the range from the row clicked before, in the order shown, whichever way
        p.click(&shown, 2, false, false);
        p.click(&shown, 3, false, true);
        assert_eq!(rows(&p), [0, 1, 2, 3]);
        p.click(&shown, 4, false, true);
        assert_eq!(rows(&p), [2, 4]);
        p.click(&shown, 0, false, true);
        assert_eq!(rows(&p), [0, 2]);
        // a range with ctrl keeps the rows already selected
        p.click(&shown, 4, true, false);
        p.click(&shown, 1, true, true);
        assert_eq!(rows(&p), [0, 1, 2, 4]);
        // shift with nothing clicked before: just the row
        let mut q = Pick::default();
        q.click(&shown, 1, false, true);
        assert_eq!(rows(&q), [1]);
        // let go
        p.clear();
        assert_eq!((rows(&p), p.row, p.anchor), (vec![], None, None));
        p.only(3);
        assert_eq!((rows(&p), p.row), (vec![3], Some(3)));
    }

    #[test]
    fn all_the_rows_shown_selected() {
        let mut p = Pick::default();
        p.click(&[2, 0, 1], 0, false, false);
        // the rows the filter leaves, in the order shown: the first the anchor of a range, the last the row
        p.all(&[2, 0, 1]);
        assert_eq!((rows(&p), p.row, p.anchor), (vec![0, 1, 2], Some(1), Some(2)));
        // a range from the anchor (the table is as it was shown)
        p.click(&[2, 0, 1], 0, false, true);
        assert_eq!(rows(&p), [0, 2]);
        // none shown: nothing selected
        p.all(&[]);
        assert_eq!((rows(&p), p.row, p.anchor), (vec![], None, None));
    }

    #[test]
    fn only_a_plain_click_zooms() {
        let shown = [0, 1, 2];
        let mut p = Pick::default();
        p.click(&shown, 1, false, false);
        assert!(p.zooms());
        // ctrl+click adds a row, then another click takes it out: one row is left, no zoom
        p.click(&shown, 2, true, false);
        assert!(!p.zooms());
        p.click(&shown, 2, true, false);
        assert_eq!(rows(&p), [1]);
        assert!(!p.zooms(), "one row left by a ctrl+click");
        p.click(&shown, 0, false, true);
        p.click(&shown, 1, false, true);
        assert!(!p.zooms());
        p.click(&shown, 2, false, false);
        assert!(p.zooms());
        // let go, or all selected: nothing to zoom to
        p.clear();
        assert!(!p.zooms());
        p.all(&[1]);
        assert!(!p.zooms(), "select all is no click on a row");
    }

    #[test]
    fn selected_rows_as_a_table() {
        let t = table_of(&features());
        let s = subset(&t, &BTreeSet::from([2, 0, 9]));
        assert_eq!(tsv(&s), "feature\tarea\tuse\n1\t2.5\tpasture\n3\t0.5\t\n");
        assert_eq!(subset(&t, &BTreeSet::new()).rows.len(), 0);
    }

    #[test]
    fn selection_box_is_the_union() {
        let a = [0.0, 1.0, 2.0, 3.0];
        let b = [-1.0, 2.0, 1.0, 5.0];
        assert_eq!(union_box([a, b]), Some([-1.0, 1.0, 2.0, 5.0]));
        assert_eq!(union_box([a]), Some(a));
        assert_eq!(union_box([]), None);
    }

    #[test]
    fn a_shape_selects_or_adds() {
        let mut p = Pick::default();
        p.click(&[0, 1, 2, 3], 3, false, false);
        // replaces: the rows found, the view stays (no click, no zoom)
        p.area(&[0, 1], false);
        assert_eq!((rows(&p), p.row, p.clicked, p.zooms()), (vec![0, 1], Some(1), false, false));
        // adds
        p.area(&[3], true);
        assert_eq!(rows(&p), [0, 1, 3]);
        // adds none: as it was; replaces with none: let go
        p.area(&[], true);
        assert_eq!(rows(&p), [0, 1, 3]);
        p.area(&[], false);
        assert_eq!((rows(&p), p.row, p.anchor), (vec![], None, None));
    }

    /// The rows a condition keeps, by the calculator of vector.filter.
    fn kept(expression: &str, n: usize) -> Vec<usize> {
        let v = Vector {
            geometries: vec![Geometry::Point([0.0, 0.0]); n],
            attributes: vec![("a".into(), Column::Numbers(vec![1.0; n]))],
            crs: None,
        };
        let script = terraflux_ops::algebra::parse_script(expression).unwrap();
        terraflux_ops::vector_algebra::kept(&v, &Default::default(), &script).unwrap()
    }

    #[test]
    fn the_filter_keeps_the_selected_features() {
        let set = |k: &[usize]| k.iter().copied().collect::<BTreeSet<usize>>();
        // numbers from 1, a range where they run, one alone as ==
        assert_eq!(filter_expression(&set(&[2]), 10), "feature == 3");
        assert_eq!(filter_expression(&set(&[0, 1, 2, 3, 4, 7]), 10), "1 <= feature <= 5 or feature == 8");
        assert_eq!(
            filter_expression(&set(&[0, 1, 5, 6, 9]), 20),
            "1 <= feature <= 2 or 6 <= feature <= 7 or feature == 10"
        );
        // most of them: the others under not; all: every feature; none: none; rows past the end are none
        assert_eq!(filter_expression(&set(&[0, 1, 2, 4, 5, 6, 8, 9]), 10), "not (feature == 4 or feature == 8)");
        assert_eq!(filter_expression(&set(&[0, 1, 2]), 3), "feature >= 1");
        assert_eq!(filter_expression(&set(&[]), 3), "feature < 0");
        assert_eq!(filter_expression(&set(&[12]), 3), "feature < 0");
        // each condition keeps exactly its rows, as the calculator reads it
        for (rows, n) in [
            (vec![2], 10),
            (vec![0, 1, 2, 3, 4, 7], 10),
            (vec![0, 1, 5, 6, 9], 20),
            (vec![0, 1, 2, 4, 5, 6, 8, 9], 10),
            (vec![0, 1, 2], 3),
            (vec![], 3),
        ] {
            assert_eq!(kept(&filter_expression(&set(&rows), n), n), rows, "{rows:?} of {n}");
        }
        // a large selection stays compact (ranges), also scattered ones: bracketed halves, read as they are
        let many: Vec<usize> = (0..5000).filter(|k| k % 3 != 0).collect();
        let text = filter_expression(&set(&many), 5000);
        assert!(text.len() < 5000 * 12, "{} characters", text.len());
        assert_eq!(kept(&text, 5000), many);
        let block: Vec<usize> = (100..90_000).collect();
        assert_eq!(filter_expression(&set(&block), 100_000), "101 <= feature <= 90000");
    }

    #[test]
    fn no_attributes_is_the_numbers_only() {
        let v = Vector { geometries: vec![Geometry::Empty; 2], attributes: vec![], crs: None };
        let t = table_of(&v);
        assert_eq!((t.columns.len(), t.rows.len()), (1, 2));
    }
}
