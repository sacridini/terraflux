//! The user's settings, kept between sessions in the user's application data (`Terraflux/settings.json`): the
//! theme, the text's size and the interface's zoom, the threads the functions use by default, and the operators
//! in each collection of favourites (coloured, as Ableton Live's collections).

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use egui::Color32;
use serde::{Deserialize, Serialize};

/// The collections of favourites: their names and colours.
pub const COLLECTIONS: [(&str, Color32); 7] = [
    ("Red", Color32::from_rgb(232, 70, 60)),
    ("Orange", Color32::from_rgb(242, 146, 40)),
    ("Yellow", Color32::from_rgb(236, 206, 52)),
    ("Green", Color32::from_rgb(92, 196, 92)),
    ("Blue", Color32::from_rgb(66, 150, 236)),
    ("Purple", Color32::from_rgb(170, 100, 226)),
    ("Grey", Color32::from_rgb(150, 150, 150)),
];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// The theme's name (see [`crate::theme::THEMES`]).
    pub theme: String,
    /// The text's size, times egui's.
    pub text: f32,
    /// The whole interface's zoom.
    pub zoom: f32,
    /// The threads the functions use (none: all the cores but one); from the next start.
    pub threads: Option<usize>,
    /// The collections each operator is in (indices of [`COLLECTIONS`]).
    pub favourites: BTreeMap<String, Vec<u8>>,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings { theme: "Default".into(), text: 1.0, zoom: 1.0, threads: None, favourites: BTreeMap::new() }
    }
}

/// Where the settings are kept (`TERRAFLUX_SETTINGS`, else the user's application data).
pub fn file() -> Option<PathBuf> {
    if let Some(f) = std::env::var_os("TERRAFLUX_SETTINGS") {
        return Some(PathBuf::from(f));
    }
    std::env::var_os("APPDATA")
        .or_else(|| std::env::var_os("XDG_CONFIG_HOME"))
        .map(|d| PathBuf::from(d).join("Terraflux").join("settings.json"))
}

fn global() -> &'static Mutex<Settings> {
    static S: OnceLock<Mutex<Settings>> = OnceLock::new();
    S.get_or_init(|| {
        let s = file()
            .and_then(|f| std::fs::read_to_string(f).ok())
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        Mutex::new(s)
    })
}

/// The settings now.
pub fn get() -> Settings {
    global().lock().unwrap().clone()
}

/// Change the settings (kept at once).
pub fn update(f: impl FnOnce(&mut Settings)) {
    let mut s = global().lock().unwrap();
    let before = s.clone();
    f(&mut s);
    if *s != before
        && let Some(path) = file()
    {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(path, serde_json::to_string_pretty(&*s).unwrap_or_default());
    }
}

/// The threads asked for in the settings.
pub fn threads() -> Option<usize> {
    get().threads
}

/// The collections an operator is in.
pub fn collections_of(op: &str) -> Vec<u8> {
    global().lock().unwrap().favourites.get(op).cloned().unwrap_or_default()
}

/// Put an operator in a collection, or take it out.
pub fn toggle(op: &str, collection: u8) {
    update(|s| {
        let c = s.favourites.entry(op.to_string()).or_default();
        match c.iter().position(|&x| x == collection) {
            Some(i) => {
                c.remove(i);
            }
            None => {
                c.push(collection);
                c.sort_unstable();
            }
        }
        s.favourites.retain(|_, c| !c.is_empty());
    });
}

/// The operators of a collection, by name.
pub fn in_collection(collection: u8) -> Vec<String> {
    global()
        .lock()
        .unwrap()
        .favourites
        .iter()
        .filter(|(_, c)| c.contains(&collection))
        .map(|(o, _)| o.clone())
        .collect()
}
