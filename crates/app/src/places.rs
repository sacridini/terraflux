//! Saved places: the part of the Viewer's map in view, or where the canvas looks, kept by name in the model
//! ([`terraflux_graph::Place`]) and gone back to with a click or a digit. The Places menu of the Viewer's header and
//! of the canvas' background menu list them, save the view in sight, rename, give a slot (the digits 1 to 9) and
//! delete; ctrl+1 to ctrl+9 save the view in a slot and 1 to 9 go to it, with the pointer over the Viewer or the
//! canvas and no field being typed in. Saving, renaming and deleting are edits of the model, one step of undo each.

use std::time::Instant;

use egui::{RichText, Ui, Vec2};
use terraflux_graph::bookmarks::SLOTS;
use terraflux_graph::{Model, Place, PlaceView};

use crate::app::Session;
use crate::builder::Layer;
use crate::map_view::MapView;

/// What a place's menu asked for.
#[derive(Debug, Clone, PartialEq)]
pub enum Ask {
    /// Go to a place, by its name.
    Go(String),
    /// Keep the view in sight under a name (the map's visible layers too, when `layers`).
    Save {
        name: String,
        layers: bool,
    },
    Rename(String, String),
    Delete(String),
    Slot(String, Option<u8>),
}

/// The digit keys, the first for slot 1.
const DIGITS: [egui::Key; 9] = [
    egui::Key::Num1,
    egui::Key::Num2,
    egui::Key::Num3,
    egui::Key::Num4,
    egui::Key::Num5,
    egui::Key::Num6,
    egui::Key::Num7,
    egui::Key::Num8,
    egui::Key::Num9,
];

/// The digit pressed, taken: its slot and whether ctrl was held (save the view there) or not (go to it). Alt and
/// shift alone are not a place's keys.
pub fn read_key(i: &mut egui::InputState) -> Option<(u8, bool)> {
    for (k, key) in DIGITS.iter().enumerate() {
        if i.consume_key(egui::Modifiers::COMMAND, *key) {
            return Some((k as u8 + 1, true));
        }
    }
    if i.modifiers.alt || i.modifiers.shift || i.modifiers.command {
        return None;
    }
    for (k, key) in DIGITS.iter().enumerate() {
        if i.consume_key(egui::Modifiers::NONE, *key) {
            return Some((k as u8 + 1, false));
        }
    }
    None
}

/// A coordinate system as a place keeps it: its EPSG code, else its WKT.
pub fn crs_name(g: &terraflux_raster::Grid) -> Option<String> {
    g.geo.epsg().map(|c| format!("EPSG:{c}")).or_else(|| g.geo.wkt.clone())
}

/// A box of the coordinates of the system `from` (none: a map without one) in those of the grid `g`'s map: as it is
/// when the systems are the same, carried by GDAL (the box's edges sampled) when they are not. An error says why it
/// cannot be.
pub fn extent_in(extent: [f64; 4], from: Option<&str>, g: &terraflux_raster::Grid) -> Result<[f64; 4], String> {
    match (from, crs_name(g)) {
        (None, None) => Ok(extent),
        (Some(a), Some(b)) if a == b || terraflux_gdal::same_crs(a, &b).unwrap_or(false) => Ok(extent),
        (Some(a), Some(b)) => {
            let tr = terraflux_gdal::Transform::new(a, &b).map_err(|e| e.to_string())?;
            // (a grid of points over the box: the edges of a box bend in another system)
            const N: usize = 6;
            let (mut xs, mut ys) = (Vec::new(), Vec::new());
            for i in 0..N {
                for j in 0..N {
                    xs.push(extent[0] + (extent[2] - extent[0]) * i as f64 / (N - 1) as f64);
                    ys.push(extent[1] + (extent[3] - extent[1]) * j as f64 / (N - 1) as f64);
                }
            }
            tr.points(&mut xs, &mut ys);
            let mut out = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
            for (x, y) in xs.iter().zip(&ys).filter(|(x, y)| x.is_finite() && y.is_finite()) {
                out = [out[0].min(*x), out[1].min(*y), out[2].max(*x), out[3].max(*y)];
            }
            if out[0] > out[2] || out[1] > out[3] {
                return Err("the place has no coordinates in the map's system".into());
            }
            Ok(out)
        }
        (None, Some(_)) => Err("the place was kept on a map with no coordinate system, and this one has one".into()),
        (Some(_), None) => Err("the place was kept on a map with a coordinate system, and this one has none".into()),
    }
}

/// The Viewer's view in sight as a place (the layers shown too, when asked for); none before a map was drawn.
pub fn map_now(view: &MapView, layers: &[Layer], with_layers: bool) -> Option<PlaceView> {
    let g = view.shown_grid()?;
    let extent = view.view_extent()?;
    let layers = with_layers.then(|| {
        layers
            .iter()
            .filter(|l| l.visible && l.minus.is_none())
            .map(|l| format!("{}.{}", l.node, l.port))
            .collect::<Vec<_>>()
    });
    Some(PlaceView::Map { extent, crs: crs_name(g), layers })
}

/// The Viewer taken to a map place: the view on its box (in the map's coordinate system), and the layers it kept made
/// visible, the others hidden.
pub fn go_map(view: &mut MapView, layers: &mut [Layer], place: &Place) -> Result<(), String> {
    let PlaceView::Map { extent, crs, layers: kept } = &place.view else {
        return Err(format!("{} is a place of the canvas", place.name));
    };
    let g = view.shown_grid().ok_or("no map is shown")?;
    let extent = extent_in(*extent, crs.as_deref(), g)?;
    if !view.go_extent(extent) {
        return Err("the place is not on this map".into());
    }
    if let Some(kept) = kept {
        for l in layers.iter_mut().filter(|l| l.minus.is_none()) {
            l.visible = kept.contains(&format!("{}.{}", l.node, l.port));
        }
    }
    Ok(())
}

/// Where the canvas looks as a place: the container shown, the graph point at the middle of the view and the zoom.
pub fn canvas_now(s: &Session) -> Option<PlaceView> {
    let cam = s.editor.camera()?;
    let rect = s.editor.canvas()?;
    let c = (rect.size() / 2.0 - cam.pan) / cam.zoom;
    (c.x.is_finite() && c.y.is_finite() && cam.zoom > 0.0).then(|| PlaceView::Canvas {
        scope: s.editor.path.join("/"),
        x: c.x,
        y: c.y,
        zoom: cam.zoom,
    })
}

/// The canvas taken to a place of the canvas, into its container when it is another.
pub fn go_canvas(s: &mut Session, place: &Place) -> Result<(), String> {
    let PlaceView::Canvas { scope, x, y, zoom } = &place.view else {
        return Err(format!("{} is a place of the Viewer", place.name));
    };
    let path: Vec<String> = if scope.is_empty() { Vec::new() } else { scope.split('/').map(String::from).collect() };
    if path != s.editor.path {
        if s.model.scope(&path).is_none() {
            return Err(format!("the container of {} is not in the model any more", place.name));
        }
        s.editor.path = path;
    }
    let size = s.editor.canvas().ok_or("the canvas has not been drawn yet")?.size();
    s.editor.set_camera(size / 2.0 - Vec2::new(*x, *y) * *zoom, *zoom);
    Ok(())
}

/// A place of a kind by its name.
fn named(s: &Session, map: bool, name: &str) -> Option<Place> {
    s.source.bookmarks_of(map).find(|p| p.name == name).cloned()
}

/// A message under the window for a moment.
fn tell(s: &mut Session, text: impl Into<String>) {
    s.notice = Some((text.into(), Instant::now()));
}

/// An edit of the places: the model changed as one step of undo with this name.
fn edit(s: &mut Session, name: &str, f: impl FnOnce(&mut Model)) {
    f(&mut s.source);
    s.history.name(name);
    s.edited();
}

/// What the menu asked that is the same for both kinds: rename, delete, the slot. False for the others.
fn change(s: &mut Session, map: bool, ask: &Ask) -> bool {
    match ask {
        Ask::Rename(old, new) => match s.source.rename_bookmark(map, old, new) {
            Ok(()) => {
                s.history.name(&format!("Rename the place {old} to {}", new.trim()));
                s.edited();
            }
            Err(e) => tell(s, e),
        },
        Ask::Delete(name) => {
            let name = name.clone();
            edit(s, &format!("Delete the place {name}"), |m| {
                m.remove_bookmark(map, &name);
            });
        }
        Ask::Slot(name, slot) => {
            let (name, slot) = (name.clone(), *slot);
            let step = match slot {
                Some(k) => format!("Put the place {name} in slot {k}"),
                None => format!("Take the place {name} off its slot"),
            };
            edit(s, &step, |m| m.set_bookmark_slot(map, &name, slot));
        }
        _ => return false,
    }
    true
}

/// A place saved under a name (a name already taken replaces its place; none: `Place N`), in `slot` or not.
fn keep(s: &mut Session, name: &str, slot: Option<u8>, view: PlaceView) {
    let map = view.is_map();
    let name = match (name.trim(), slot) {
        ("", Some(k)) => s.source.slot_name(map, k),
        ("", None) => s.source.new_bookmark_name(map),
        (n, _) => n.to_string(),
    };
    let step = format!("Save the place {name}");
    edit(s, &step, |m| m.save_bookmark(Place { name: name.clone(), slot, view }));
    tell(s, format!("saved the place {name}"));
}

/// A place gone to by its name, from the palette of commands: the Viewer's (the map in view now) or the canvas'.
pub fn go_named(s: &mut Session, map: bool, name: &str) {
    let Some(p) = named(s, map, name) else {
        return tell(s, format!("no place named {name}"));
    };
    let done = if map { go_map(&mut s.preview_view, &mut s.preview_layers, &p) } else { go_canvas(s, &p) };
    if let Err(e) = done {
        tell(s, e);
    }
}

/// What the Viewer's Places menu asked.
pub fn map_ask(s: &mut Session, view: &mut MapView, layers: &mut [Layer], ask: Ask) {
    if change(s, true, &ask) {
        return;
    }
    match ask {
        Ask::Go(name) => match named(s, true, &name) {
            Some(p) => {
                if let Err(e) = go_map(view, layers, &p) {
                    tell(s, e);
                }
            }
            None => tell(s, format!("no place named {name}")),
        },
        Ask::Save { name, layers: with } => match map_now(view, layers, with) {
            Some(v) => keep(s, &name, None, v),
            None => tell(s, "no map is in view to keep"),
        },
        _ => {}
    }
}

/// A digit pressed over the Viewer: the view kept in the slot (ctrl), or the slot's place gone to.
pub fn map_key(s: &mut Session, view: &mut MapView, layers: &mut [Layer], (slot, save): (u8, bool)) {
    if save {
        // (the layers kept again when the place kept them)
        let with =
            s.source.bookmark_at(true, slot).is_some_and(|p| matches!(&p.view, PlaceView::Map { layers: Some(_), .. }));
        match map_now(view, layers, with) {
            Some(v) => {
                let name = s.source.bookmark_at(true, slot).map(|p| p.name.clone()).unwrap_or_default();
                keep(s, &name, Some(slot), v);
            }
            None => tell(s, "no map is in view to keep"),
        }
        return;
    }
    match s.source.bookmark_at(true, slot).cloned() {
        Some(p) => {
            if let Err(e) = go_map(view, layers, &p) {
                tell(s, e);
            }
        }
        None => tell(s, format!("no place of the Viewer in slot {slot}: ctrl+{slot} keeps the view there")),
    }
}

/// What the canvas' Places menu asked.
pub fn canvas_ask(s: &mut Session, ask: Ask) {
    if change(s, false, &ask) {
        return;
    }
    match ask {
        Ask::Go(name) => match named(s, false, &name) {
            Some(p) => {
                if let Err(e) = go_canvas(s, &p) {
                    tell(s, e);
                }
            }
            None => tell(s, format!("no place named {name}")),
        },
        Ask::Save { name, .. } => match canvas_now(s) {
            Some(v) => keep(s, &name, None, v),
            None => tell(s, "the canvas has not been drawn yet"),
        },
        _ => {}
    }
}

/// A digit pressed over the canvas: the view kept in the slot (ctrl), or the slot's place gone to.
pub fn canvas_key(s: &mut Session, (slot, save): (u8, bool)) {
    if save {
        match canvas_now(s) {
            Some(v) => {
                let name = s.source.bookmark_at(false, slot).map(|p| p.name.clone()).unwrap_or_default();
                keep(s, &name, Some(slot), v);
            }
            None => tell(s, "the canvas has not been drawn yet"),
        }
        return;
    }
    match s.source.bookmark_at(false, slot).cloned() {
        Some(p) => {
            if let Err(e) = go_canvas(s, &p) {
                tell(s, e);
            }
        }
        None => tell(s, format!("no place of the canvas in slot {slot}: ctrl+{slot} keeps the view there")),
    }
}

/// The text of a place in a list: its name, and its slot.
pub fn label(p: &Place) -> String {
    match p.slot {
        Some(k) => format!("{}   [{k}]", p.name),
        None => p.name.clone(),
    }
}

/// The Places button with its menu. The menu stays open for the name to be typed and the box ticked: it closes with
/// a click outside it, or when something is chosen.
pub fn menu_button(ui: &mut Ui, model: &Model, map: bool, can_save: bool, asks: &mut Vec<Ask>) -> egui::Response {
    use egui::containers::menu::{MenuButton, MenuConfig, SubMenuButton, is_in_menu};
    let config = MenuConfig::new().close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside);
    let body = |ui: &mut Ui| menu(ui, model, map, can_save, asks);
    // (in the canvas' own menu: a submenu of it)
    if is_in_menu(ui) {
        SubMenuButton::new("Places").config(config).ui(ui, body).0
    } else {
        MenuButton::new("Places").config(config).ui(ui, body).0
    }
}

/// The body of a Places menu: the places of a kind listed (a click goes to one; its "..." renames it, gives it a
/// slot or deletes it), then the name to keep the view in sight under. What was asked is pushed to `asks`.
/// `can_save`: there is a view to keep.
pub fn menu(ui: &mut Ui, model: &Model, map: bool, can_save: bool, asks: &mut Vec<Ask>) {
    let c = crate::commands::cmd();
    let places: Vec<&Place> = model.bookmarks_of(map).collect();
    if places.is_empty() {
        ui.label(RichText::new("no place kept yet").weak());
    }
    for p in places {
        ui.horizontal(|ui| {
            if ui.button(label(p)).on_hover_text("go to this place").clicked() {
                asks.push(Ask::Go(p.name.clone()));
                ui.close();
            }
            ui.menu_button("...", |ui| {
                let id = egui::Id::new(("place rename", map, &p.name));
                let mut text: String = ui.data(|d| d.get_temp(id)).unwrap_or_else(|| p.name.clone());
                ui.horizontal(|ui| {
                    let edit = ui.add(egui::TextEdit::singleline(&mut text).desired_width(140.0));
                    let go = ui.button("Rename").clicked()
                        || (edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)));
                    if go && text.trim() != p.name {
                        asks.push(Ask::Rename(p.name.clone(), text.clone()));
                        ui.data_mut(|d| d.remove_temp::<String>(id));
                        ui.close();
                    }
                });
                ui.data_mut(|d| d.insert_temp(id, text));
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new(format!("slot ({c}1 to {c}9 keep, 1 to 9 go)")).weak());
                });
                ui.horizontal(|ui| {
                    if ui.selectable_label(p.slot.is_none(), "none").clicked() {
                        asks.push(Ask::Slot(p.name.clone(), None));
                        ui.close();
                    }
                    for k in 1..=SLOTS {
                        if ui.selectable_label(p.slot == Some(k), k.to_string()).clicked() {
                            asks.push(Ask::Slot(p.name.clone(), Some(k)));
                            ui.close();
                        }
                    }
                });
                if ui.button("Delete this place").clicked() {
                    asks.push(Ask::Delete(p.name.clone()));
                    ui.close();
                }
            });
        });
    }
    ui.separator();
    let id = egui::Id::new(("place new", map));
    let mut name: String = ui.data(|d| d.get_temp(id)).unwrap_or_default();
    let mut layers: bool = ui.data(|d| d.get_temp(egui::Id::new(("place layers", map)))).unwrap_or(false);
    let hint = model.new_bookmark_name(map);
    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(&mut name).hint_text(hint).desired_width(140.0));
        if ui
            .add_enabled(can_save, egui::Button::new("Save this place..."))
            .on_hover_text(if map {
                "the part of the map in view, kept under this name (the same name replaces its place)"
            } else {
                "where the canvas looks, kept under this name (the same name replaces its place)"
            })
            .clicked()
        {
            asks.push(Ask::Save { name: std::mem::take(&mut name), layers });
            ui.close();
        }
    });
    if map {
        ui.checkbox(&mut layers, "with the layers shown")
            .on_hover_text("going to the place shows these layers and hides the others");
        ui.data_mut(|d| d.insert_temp(egui::Id::new(("place layers", map)), layers));
    }
    ui.data_mut(|d| d.insert_temp(id, name));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utm() -> terraflux_raster::Grid {
        let mut g = terraflux_raster::Grid::new(100, 100);
        g.transform = [400_000.0, 30.0, 0.0, 4_430_000.0, 0.0, -30.0];
        g.geo.geokeys = Some(vec![1, 1, 0, 3, 1024, 0, 1, 1, 3072, 0, 1, 32633, 3076, 0, 1, 9001]);
        g
    }

    fn plain() -> terraflux_raster::Grid {
        let mut g = terraflux_raster::Grid::new(10, 10);
        g.transform = [0.0, 1.0, 0.0, 10.0, 0.0, -1.0];
        g
    }

    #[test]
    fn a_box_of_the_same_system_is_kept_as_it_is() {
        let e = [400_100.0, 4_429_000.0, 401_000.0, 4_429_900.0];
        assert_eq!(crs_name(&utm()).as_deref(), Some("EPSG:32633"));
        assert_eq!(extent_in(e, Some("EPSG:32633"), &utm()), Ok(e));
        assert_eq!(extent_in(e, None, &plain()), Ok(e));
    }

    #[test]
    fn a_box_of_a_system_the_map_has_not_cannot_be_carried() {
        let e = [1.0, 1.0, 2.0, 2.0];
        assert!(extent_in(e, None, &utm()).unwrap_err().contains("no coordinate system"));
        assert!(extent_in(e, Some("EPSG:32633"), &plain()).unwrap_err().contains("has none"));
    }

    #[test]
    fn a_box_in_degrees_is_carried_to_the_maps_system() {
        if terraflux_gdal::gdal().is_err() {
            return;
        }
        // 14 to 14.1 east, 40 to 40.1 north: west of zone 33's meridian (15 E), some 4430 km from the equator
        let e = extent_in([14.0, 40.0, 14.1, 40.1], Some("EPSG:4326"), &utm()).unwrap();
        assert!((410_000.0..430_000.0).contains(&e[0]) && e[2] > e[0], "{e:?}");
        assert!((4_420_000.0..4_450_000.0).contains(&e[1]) && e[3] > e[1], "{e:?}");
        // about 8.5 km wide, 11 km high
        assert!((7_000.0..10_000.0).contains(&(e[2] - e[0])), "{e:?}");
        assert!((10_000.0..12_500.0).contains(&(e[3] - e[1])), "{e:?}");
    }

    #[test]
    fn the_digits_are_slots_and_ctrl_keeps() {
        let mut i = egui::InputState::default();
        let key = |k: egui::Key, modifiers: egui::Modifiers| egui::Event::Key {
            key: k,
            physical_key: Some(k),
            pressed: true,
            repeat: false,
            modifiers,
        };
        i.events = vec![key(egui::Key::Num3, egui::Modifiers::COMMAND)];
        i.modifiers = egui::Modifiers::COMMAND;
        assert_eq!(read_key(&mut i), Some((3, true)));
        assert_eq!(read_key(&mut i), None, "taken once");
        i.modifiers = egui::Modifiers::NONE;
        i.events = vec![key(egui::Key::Num9, egui::Modifiers::NONE)];
        assert_eq!(read_key(&mut i), Some((9, false)));
        // a letter, or a digit with alt, is not a place's
        i.events = vec![key(egui::Key::A, egui::Modifiers::NONE), key(egui::Key::Num2, egui::Modifiers::ALT)];
        i.modifiers = egui::Modifiers::ALT;
        assert_eq!(read_key(&mut i), None);
    }
}
