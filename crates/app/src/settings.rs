//! The user's settings, kept between sessions in the user's application data (`Terraflux/settings.json`): the
//! theme, the text's size and the interface's zoom, the threads the functions use by default, and the operators
//! in each collection of favourites (coloured, as Ableton Live's collections), the assistant's servers of chat
//! models, the checks for updates.

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
    /// The threads the functions use (none: the most Terraflux takes, a quarter of the cores left free); from
    /// the next start.
    pub threads: Option<usize>,
    /// The collections each operator is in (indices of [`COLLECTIONS`]).
    pub favourites: BTreeMap<String, Vec<u8>>,
    /// The names given to the collections, by index (none: its colour's).
    pub collection_names: BTreeMap<u8, String>,
    /// Other computers may work for this one's batches: those given its code.
    pub share: bool,
    pub share_code: Option<String>,
    /// This computer works for another's batches: the other's code, how many threads, and only when nobody uses
    /// it for so many minutes (none: whenever).
    pub lend: bool,
    pub lend_code: String,
    pub lend_threads: Option<usize>,
    pub lend_idle: Option<f64>,
    /// The folder the Files pane shows, the folders pinned to its places, the folders it showed last.
    pub files_dir: Option<PathBuf>,
    pub files_pinned: Vec<PathBuf>,
    pub files_recent: Vec<PathBuf>,
    /// On the model's canvas, the nodes of data (the model's inputs, what reads, writes, stacks or plots) with a
    /// header in the colour of the type they give.
    pub node_colours: bool,
    /// On the model's canvas, a minimap in the corner when the model does not fit the view (on unless turned off).
    pub minimap: bool,
    /// On the model's canvas, a wire that crosses another node's box goes around it (on unless turned off).
    pub wires_around: bool,
    /// The models opened or saved last, the last first.
    pub recent_models: Vec<PathBuf>,
    /// The computer kept from sleeping while a model runs, a batch goes on or it works for another's (on unless
    /// turned off).
    pub keep_awake: bool,
    /// The end of a run asked for, or of a batch, told (the system's notification, the window's button in the
    /// taskbar) when it took this many seconds or more and Terraflux is not in front (on unless turned off).
    pub notify_runs: bool,
    pub notify_after: f64,
    /// The assistant's servers of chat models (their keys in the system's store of credentials), and the one
    /// chosen (none: not chosen yet).
    pub providers: Vec<crate::llm::Provider>,
    pub provider: Option<usize>,
    /// The size each cache of rasters made and data downloaded is held under, in gigabytes (none: 10; 0: no limit).
    pub cache_gb: Option<f64>,
    /// Ask GitHub for a newer Terraflux when the application starts (at most once a day), and when it was asked
    /// last (seconds since 1970).
    pub check_updates: bool,
    pub updates_checked: Option<u64>,
    /// The version that ran last: another one now, the application says it was updated.
    pub seen_version: Option<String>,
    /// The version an update was to install when the application closed: told if it is not the one that opens.
    pub update_tried: Option<String>,
    /// The libraries of nodes shared with others (a group's folder on a shared drive, a repository cloned), beside
    /// the user's own.
    pub node_libraries: Vec<PathBuf>,
    /// The remote libraries of nodes, by address: git repositories, folders on the web (only read; their copies
    /// pulled when the application starts).
    pub node_remotes: Vec<String>,
    /// On the Viewer's map: the scale bar (on unless turned off) and the coordinate grid (off unless turned on).
    pub scale_bar: bool,
    pub map_grid: bool,
    /// The Viewer's window of its own, where it was last: its place on the screen and its size (x, y, width,
    /// height, in points).
    pub viewer_window: Option<[f32; 4]>,
    /// The basemaps of the user's own (web tiles by an address template), the last used first; at most
    /// [`crate::basemap::MOST_CUSTOM`].
    pub custom_basemaps: Vec<CustomBasemap>,
}

/// A basemap of the user's: where its tiles are (`https://host/{z}/{x}/{y}.png`), whose map it is, its deepest zoom.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CustomBasemap {
    pub template: String,
    #[serde(default)]
    pub attribution: String,
    pub max_zoom: u32,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            theme: crate::theme::DEFAULT.into(),
            text: 1.0,
            zoom: 1.0,
            threads: None,
            favourites: BTreeMap::new(),
            collection_names: BTreeMap::new(),
            share: false,
            share_code: None,
            lend: false,
            lend_code: String::new(),
            lend_threads: None,
            lend_idle: None,
            files_dir: None,
            files_pinned: Vec::new(),
            files_recent: Vec::new(),
            node_colours: false,
            minimap: true,
            wires_around: true,
            recent_models: Vec::new(),
            keep_awake: true,
            notify_runs: true,
            notify_after: 10.0,
            providers: crate::llm::presets(),
            provider: None,
            cache_gb: None,
            check_updates: true,
            updates_checked: None,
            seen_version: None,
            update_tried: None,
            node_libraries: Vec::new(),
            node_remotes: Vec::new(),
            scale_bar: true,
            map_grid: false,
            viewer_window: None,
            custom_basemaps: Vec::new(),
        }
    }
}

/// Where the settings are kept (`TERRAFLUX_SETTINGS`, else the user's application data; on macOS
/// `~/Library/Application Support`, on Linux `~/.config` when `XDG_CONFIG_HOME` is not set).
pub fn file() -> Option<PathBuf> {
    if let Some(f) = std::env::var_os("TERRAFLUX_SETTINGS") {
        return Some(PathBuf::from(f));
    }
    std::env::var_os("APPDATA")
        .or_else(|| std::env::var_os("XDG_CONFIG_HOME"))
        .map(PathBuf::from)
        .or_else(|| {
            let home = PathBuf::from(std::env::var_os("HOME")?);
            Some(if cfg!(target_os = "macos") {
                home.join("Library").join("Application Support")
            } else {
                home.join(".config")
            })
        })
        .map(|d| d.join("Terraflux").join("settings.json"))
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

/// The models opened or saved last, the last first (those whose file is gone left out).
pub fn recent_models() -> Vec<PathBuf> {
    get().recent_models.into_iter().filter(|p| p.is_file()).collect()
}

/// A model opened or saved: first of the recent ones (ten kept).
pub fn opened_model(path: &std::path::Path) {
    let path = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    update(|s| {
        s.recent_models.retain(|p| *p != path);
        s.recent_models.insert(0, path);
        s.recent_models.truncate(10);
    });
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

/// A collection's name: the one given to it, else its colour's.
pub fn collection_name(collection: u8) -> String {
    global()
        .lock()
        .unwrap()
        .collection_names
        .get(&collection)
        .cloned()
        .unwrap_or_else(|| COLLECTIONS.get(collection as usize).map_or("", |c| c.0).to_string())
}

/// Name a collection (an empty name: its colour's again).
pub fn rename_collection(collection: u8, name: &str) {
    update(|s| {
        let name = name.trim();
        if name.is_empty() || COLLECTIONS.get(collection as usize).is_some_and(|c| c.0 == name) {
            s.collection_names.remove(&collection);
        } else {
            s.collection_names.insert(collection, name.to_string());
        }
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
