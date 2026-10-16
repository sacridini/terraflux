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
    /// Other computers may work for this one's batches: those given its code.
    pub share: bool,
    pub share_code: Option<String>,
    /// This computer works for another's batches: the other's code, how many threads, and only when nobody uses
    /// it for so many minutes (none: whenever).
    pub lend: bool,
    pub lend_code: String,
    pub lend_threads: Option<usize>,
    pub lend_idle: Option<f64>,
    /// The folder the Files pane shows.
    pub files_dir: Option<PathBuf>,
    /// On the model's canvas, the nodes of data (the model's inputs, what reads, writes, stacks or plots) with a
    /// header in the colour of the type they give.
    pub node_colours: bool,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            theme: "Default".into(),
            text: 1.0,
            zoom: 1.0,
            threads: None,
            favourites: BTreeMap::new(),
            share: false,
            share_code: None,
            lend: false,
            lend_code: String::new(),
            lend_threads: None,
            lend_idle: None,
            files_dir: None,
            node_colours: false,
        }
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

/// The folder of the user's application data (where the settings are).
pub fn data_dir() -> PathBuf {
    file().and_then(|f| f.parent().map(PathBuf::from)).unwrap_or_else(|| std::env::temp_dir().join("Terraflux"))
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
