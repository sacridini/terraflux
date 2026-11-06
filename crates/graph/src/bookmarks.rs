//! The places of a model (`[[bookmarks]]`): views of the Viewer's map or of the canvas kept by name, in a slot (1 to
//! 9) or not, outside the graph. Saved with the model, so they are there on another computer. (`Model::places` is
//! something else: where the canvas draws the boxes of what a scope reads.)

use serde::{Deserialize, Serialize};

use crate::Model;

/// The most slots there are: the digits 1 to 9.
pub const SLOTS: u8 = 9;

/// A view kept by name.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Place {
    pub name: String,
    /// The digit that goes to it (1 to 9), one place of each kind to a slot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slot: Option<u8>,
    pub view: PlaceView,
}

/// What a place keeps: the part of a map in view, or where the canvas looks.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum PlaceView {
    /// The Viewer: a box (west, south, east, north) in the coordinates of the map shown (`crs`: its system, as the
    /// map tells it; none for a map without one), and the layers visible when it was saved (`from` of each, as the
    /// Plots tell them), when they were kept.
    Map {
        extent: [f64; 4],
        #[serde(default, skip_serializing_if = "Option::is_none")]
        crs: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        layers: Option<Vec<String>>,
    },
    /// The canvas: the container whose body it shows (its path joined by `/`; empty for the top), the graph point
    /// at the middle of the view and the zoom.
    Canvas {
        #[serde(default)]
        scope: String,
        x: f32,
        y: f32,
        zoom: f32,
    },
}

impl PlaceView {
    pub fn is_map(&self) -> bool {
        matches!(self, PlaceView::Map { .. })
    }
}

impl Place {
    pub fn is_map(&self) -> bool {
        self.view.is_map()
    }
}

impl Model {
    /// The places of a kind (the Viewer's or the canvas'), in the order saved.
    pub fn bookmarks_of(&self, map: bool) -> impl Iterator<Item = &Place> + '_ {
        self.bookmarks.iter().filter(move |p| p.is_map() == map)
    }

    /// The place of a slot of a kind.
    pub fn bookmark_at(&self, map: bool, slot: u8) -> Option<&Place> {
        self.bookmarks_of(map).find(|p| p.slot == Some(slot))
    }

    /// A name for a new place of a kind: `Place 1`, `Place 2`... not taken.
    pub fn new_bookmark_name(&self, map: bool) -> String {
        (1..).map(|k| format!("Place {k}")).find(|n| !self.bookmarks_of(map).any(|p| &p.name == n)).unwrap()
    }

    /// A place saved. With a name taken by a place of its kind, that one is replaced (its slot kept unless the new one
    /// has its own); with a slot taken by another of its kind, the other loses the slot. Names are not empty.
    pub fn save_bookmark(&mut self, mut place: Place) {
        let map = place.is_map();
        place.name = place.name.trim().to_string();
        if place.name.is_empty() {
            place.name = self.new_bookmark_name(map);
        }
        place.slot = place.slot.filter(|s| (1..=SLOTS).contains(s));
        if let Some(k) = self.bookmarks.iter().position(|p| p.is_map() == map && p.name == place.name) {
            let old = self.bookmarks.remove(k);
            if place.slot.is_none() {
                place.slot = old.slot;
            }
        }
        if let Some(s) = place.slot {
            for p in self.bookmarks.iter_mut().filter(|p| p.is_map() == map && p.slot == Some(s)) {
                p.slot = None;
            }
        }
        self.bookmarks.push(place);
    }

    /// The name for a view put in a slot: that of the place there, else `Place N` (N the slot) when that name is
    /// free, else a new one.
    pub fn slot_name(&self, map: bool, slot: u8) -> String {
        match self.bookmark_at(map, slot) {
            Some(p) => p.name.clone(),
            None => {
                let by_slot = format!("Place {slot}");
                if self.bookmarks_of(map).any(|p| p.name == by_slot) { self.new_bookmark_name(map) } else { by_slot }
            }
        }
    }

    /// The view put in a slot: the place of the slot is replaced (it keeps its name), else a new one is made.
    pub fn save_bookmark_slot(&mut self, slot: u8, view: PlaceView) {
        let name = self.slot_name(view.is_map(), slot);
        self.save_bookmark(Place { name, slot: Some(slot), view });
    }

    /// A place of a kind taken off by its name; whether there was one.
    pub fn remove_bookmark(&mut self, map: bool, name: &str) -> bool {
        let before = self.bookmarks.len();
        self.bookmarks.retain(|p| !(p.is_map() == map && p.name == name));
        self.bookmarks.len() != before
    }

    /// A place of a kind renamed. A name taken by another place of the kind is refused.
    pub fn rename_bookmark(&mut self, map: bool, old: &str, new: &str) -> Result<(), String> {
        let new = new.trim();
        if new.is_empty() {
            return Err("a place needs a name".into());
        }
        if new != old && self.bookmarks_of(map).any(|p| p.name == new) {
            return Err(format!("there is a place named {new:?} already"));
        }
        match self.bookmarks.iter_mut().find(|p| p.is_map() == map && p.name == old) {
            Some(p) => {
                p.name = new.to_string();
                Ok(())
            }
            None => Err(format!("no place named {old:?}")),
        }
    }

    /// The slot of a place of a kind set (none: it has no slot); another place of the kind in that slot loses it.
    pub fn set_bookmark_slot(&mut self, map: bool, name: &str, slot: Option<u8>) {
        let slot = slot.filter(|s| (1..=SLOTS).contains(s));
        if let Some(s) = slot {
            for p in self.bookmarks.iter_mut().filter(|p| p.is_map() == map && p.slot == Some(s)) {
                p.slot = None;
            }
        }
        if let Some(p) = self.bookmarks.iter_mut().find(|p| p.is_map() == map && p.name == name) {
            p.slot = slot;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(x: f64) -> PlaceView {
        PlaceView::Map { extent: [x, 0.0, x + 10.0, 10.0], crs: Some("EPSG:32633".into()), layers: None }
    }

    fn canvas(x: f32) -> PlaceView {
        PlaceView::Canvas { scope: String::new(), x, y: 5.5, zoom: 0.75 }
    }

    #[test]
    fn places_are_written_and_read_in_both_formats() {
        let mut m = Model::new("t");
        m.save_bookmark(Place { name: "North".into(), slot: Some(1), view: map(0.5) });
        m.save_bookmark(Place {
            name: "Wet".into(),
            slot: None,
            view: PlaceView::Map {
                extent: [1.0, 2.0, 3.0, 4.0],
                crs: None,
                layers: Some(vec!["a.out".into(), "$land".into()]),
            },
        });
        m.save_bookmark(Place { name: "Inputs".into(), slot: Some(2), view: canvas(120.25) });
        m.save_bookmark(Place {
            name: "In the loop".into(),
            slot: None,
            view: PlaceView::Canvas { scope: "loop".into(), x: -3.0, y: 8.0, zoom: 2.0 },
        });
        let toml = m.to_toml().unwrap();
        assert!(toml.contains("[[bookmarks]]"), "{toml}");
        let back = Model::from_toml(&toml).unwrap();
        assert_eq!(back.bookmarks, m.bookmarks);
        let json = m.to_json().unwrap();
        assert_eq!(Model::from_json(&json).unwrap().bookmarks, m.bookmarks);
        // none: nothing written, and a model without them reads
        let plain = Model::new("t").to_toml().unwrap();
        assert!(!plain.contains("bookmarks"), "{plain}");
        assert!(Model::from_toml(&plain).unwrap().bookmarks.is_empty());
    }

    #[test]
    fn a_slot_belongs_to_one_place_of_a_kind() {
        let mut m = Model::new("t");
        m.save_bookmark_slot(1, map(0.0));
        assert_eq!(m.bookmark_at(true, 1).unwrap().name, "Place 1");
        // the canvas has its own slot 1
        m.save_bookmark_slot(1, canvas(1.0));
        assert_eq!(m.bookmark_at(false, 1).unwrap().name, "Place 1");
        assert_eq!(m.bookmarks.len(), 2);
        // the slot saved again: the place replaced, its name kept
        m.rename_bookmark(true, "Place 1", "North").unwrap();
        m.save_bookmark_slot(1, map(50.0));
        assert_eq!(m.bookmarks_of(true).count(), 1);
        let p = m.bookmark_at(true, 1).unwrap();
        assert_eq!((p.name.as_str(), &p.view), ("North", &map(50.0)));
        // another place given the slot takes it
        m.save_bookmark(Place { name: "South".into(), slot: Some(1), view: map(9.0) });
        assert_eq!(m.bookmark_at(true, 1).unwrap().name, "South");
        assert_eq!(m.bookmarks_of(true).find(|p| p.name == "North").unwrap().slot, None);
        // a slot's default name: Place N by the slot when free
        m.save_bookmark_slot(5, map(5.0));
        assert_eq!(m.bookmark_at(true, 5).unwrap().name, "Place 5");
        // a slot out of 1 to 9 is none
        m.save_bookmark(Place { name: "Far".into(), slot: Some(10), view: map(1.0) });
        assert_eq!(m.bookmarks_of(true).find(|p| p.name == "Far").unwrap().slot, None);
        m.set_bookmark_slot(true, "North", Some(1));
        assert_eq!(m.bookmark_at(true, 1).unwrap().name, "North");
        assert_eq!(m.bookmarks_of(true).find(|p| p.name == "South").unwrap().slot, None);
    }

    #[test]
    fn names_are_new_unique_and_renamed() {
        let mut m = Model::new("t");
        assert_eq!(m.new_bookmark_name(true), "Place 1");
        m.save_bookmark(Place { name: " ".into(), slot: None, view: map(0.0) });
        m.save_bookmark(Place { name: "".into(), slot: None, view: map(1.0) });
        let names: Vec<&str> = m.bookmarks.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["Place 1", "Place 2"]);
        assert!(m.rename_bookmark(true, "Place 1", "Place 2").is_err());
        assert!(m.rename_bookmark(true, "Place 1", " ").is_err());
        assert!(m.rename_bookmark(true, "Nothing", "x").is_err());
        m.rename_bookmark(true, "Place 1", "Lake").unwrap();
        // the same name of a kind replaces
        m.save_bookmark(Place { name: "Lake".into(), slot: None, view: map(7.0) });
        assert_eq!(m.bookmarks_of(true).count(), 2);
        assert!(m.remove_bookmark(true, "Lake") && !m.remove_bookmark(true, "Lake"));
        assert!(!m.remove_bookmark(false, "Place 2"), "the kinds are apart");
    }
}
