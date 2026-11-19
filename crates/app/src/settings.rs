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

/// The theme of a settings file: the one named "Default" before (saved only because it was the default then) is
/// the default of today, Slate.
fn theme_of_file<'de, D: serde::Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    let name = String::deserialize(d)?;
    Ok(if name == "Default" { crate::theme::DEFAULT.into() } else { name })
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// The theme's name (see [`crate::theme::THEMES`]); a file of before may hold "Default", read as Slate.
    #[serde(deserialize_with = "theme_of_file")]
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
    /// On the model's canvas, a node's main settings in rows under its ports (on unless turned off).
    pub node_settings: bool,
    /// The Node pane's tab opened by itself beside the model with its first node (off unless turned on: a node's
    /// settings open in a panel beside its box on the canvas; the tab stays in the panes' menu).
    pub node_pane_opens: bool,
    /// The start page on the Model pane's canvas while the model is empty (on unless turned off).
    pub start_page: bool,
    /// The models opened or saved last, the last first.
    pub recent_models: Vec<PathBuf>,
    /// The computer kept from sleeping while a model runs, a batch goes on or it works for another's (on unless
    /// turned off).
    pub keep_awake: bool,
    /// The small outputs of each run copied into the journal: the most one run's copies take, in megabytes (0: none
    /// is copied), and the most all the copies take, the oldest going first.
    pub keep_outputs_mb: u32,
    pub keep_outputs_total_mb: u32,
    /// The models open when Terraflux closed opened again when it starts without one given (on unless turned off).
    pub reopen_last: bool,
    /// The end of a run asked for, or of a batch, told (the system's notification, the window's button in the
    /// taskbar) when it took this many seconds or more and Terraflux is not in front (on unless turned off).
    pub notify_runs: bool,
    pub notify_after: f64,
    /// The model's script of terraflux-py written beside it (`<model>.py`) each time the model is saved or a run
    /// asked for starts (off unless turned on).
    pub python_beside: bool,
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
    /// The figure's typeface and text size, and where its legend and scale bar stand (the last figure's).
    pub figure_look: crate::figure::Look,
    /// The Viewer's window of its own, where it was last: its place on the screen and its size (x, y, width,
    /// height, in points).
    pub viewer_window: Option<[f32; 4]>,
    /// The basemaps of the user's own (web tiles by an address template), the last used first; at most
    /// [`crate::basemap::MOST_CUSTOM`].
    pub custom_basemaps: Vec<CustomBasemap>,
    /// The keyboard shortcuts the user changed, by the action's id (see [`crate::keymap`]): the shortcuts as
    /// "Ctrl+Shift+S"; an empty list is an action with no shortcut. An action not here has its default.
    pub shortcuts: BTreeMap<String, Vec<String>>,
    /// The credentials of the cloud storage rasters and features are read from (`s3://`, `gs://`, `/vsiaz/`): in
    /// the system's store of credentials (the secrets) and the settings file, never in a model.
    pub cloud: Cloud,
    /// The weights of SAM this computer agreed to download (`quant`, `full`): asked once for each.
    pub sam_agreed: Vec<String>,
    /// How long each image segmenter took here against what the rates of the benchmark said, the last run's (see
    /// [`crate::time_estimate`]).
    pub time_ratios: BTreeMap<String, f64>,
}

/// The credentials of cloud storage, given to GDAL as its configuration options (those of the environment, which GDAL
/// reads by itself, are used for what is left empty). The secrets (the secret keys, the session token, Azure's key)
/// are kept in the system's store of credentials, the rest in the settings file; none is ever written in a model or a
/// script.
#[derive(Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Cloud {
    /// Amazon S3, or any S3-compatible storage (MinIO, Ceph, Wasabi...): the keys, the session token of temporary
    /// credentials, the region, the endpoint (`host:port`, or `http://host:port`; none: Amazon's), and the access
    /// of public buckets without signing the requests.
    pub aws_access_key_id: String,
    /// (the secrets are read from a settings file of before, to be moved to the system's store, but never written
    /// to it: see [`SecretStore`])
    #[serde(skip_serializing)]
    pub aws_secret_access_key: String,
    #[serde(skip_serializing)]
    pub aws_session_token: String,
    pub aws_region: String,
    pub aws_endpoint: String,
    pub aws_no_sign_request: bool,
    /// Google Cloud Storage: the interoperability (HMAC) keys, or the file of a service account's key (JSON).
    pub gs_access_key_id: String,
    #[serde(skip_serializing)]
    pub gs_secret_access_key: String,
    pub gs_credentials_file: String,
    /// The access of public buckets without signing the requests (`GS_NO_SIGN_REQUEST`).
    pub gs_no_sign_request: bool,
    /// Azure Blob Storage: the storage account and its key.
    pub azure_account: String,
    #[serde(skip_serializing)]
    pub azure_key: String,
}

/// The names the secrets are kept under in the store (after the style of the LLM keys', by what they are).
const SECRETS: [&str; 4] = [
    "cloud:AWS_SECRET_ACCESS_KEY",
    "cloud:AWS_SESSION_TOKEN",
    "cloud:GS_SECRET_ACCESS_KEY",
    "cloud:AZURE_STORAGE_ACCESS_KEY",
];

impl Cloud {
    /// The secrets by their names in the store.
    fn secrets(&self) -> [(&'static str, &str); 4] {
        [
            (SECRETS[0], &self.aws_secret_access_key),
            (SECRETS[1], &self.aws_session_token),
            (SECRETS[2], &self.gs_secret_access_key),
            (SECRETS[3], &self.azure_key),
        ]
    }

    fn secrets_mut(&mut self) -> [(&'static str, &mut String); 4] {
        [
            (SECRETS[0], &mut self.aws_secret_access_key),
            (SECRETS[1], &mut self.aws_session_token),
            (SECRETS[2], &mut self.gs_secret_access_key),
            (SECRETS[3], &mut self.azure_key),
        ]
    }
}

/// Where the secrets are kept: the system's store of credentials, as the assistant's keys are (the keychain, the
/// credential manager, the secret service). A trait so that a test keeps them in memory and never touches the
/// user's own store ([`use_store`]).
pub trait SecretStore: Send + Sync {
    fn get(&self, name: &str) -> Option<String>;
    /// Keep a secret (empty: forgotten).
    fn set(&self, name: &str, value: &str) -> Result<(), String>;
}

/// The system's store of credentials (service "Terraflux").
struct Keyring;

impl SecretStore for Keyring {
    fn get(&self, name: &str) -> Option<String> {
        keyring::Entry::new("Terraflux", name).ok()?.get_password().ok().filter(|k| !k.is_empty())
    }

    fn set(&self, name: &str, value: &str) -> Result<(), String> {
        let entry = keyring::Entry::new("Terraflux", name).map_err(|e| e.to_string())?;
        if value.trim().is_empty() {
            return match entry.delete_credential() {
                Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
                Err(e) => Err(e.to_string()),
            };
        }
        entry.set_password(value.trim()).map_err(|e| e.to_string())
    }
}

/// Secrets kept in memory (for tests).
#[derive(Default)]
pub struct Memory(Mutex<BTreeMap<String, String>>);

impl SecretStore for Memory {
    fn get(&self, name: &str) -> Option<String> {
        self.0.lock().unwrap().get(name).cloned()
    }

    fn set(&self, name: &str, value: &str) -> Result<(), String> {
        let mut m = self.0.lock().unwrap();
        if value.trim().is_empty() {
            m.remove(name);
        } else {
            m.insert(name.to_string(), value.trim().to_string());
        }
        Ok(())
    }
}

fn store() -> &'static Mutex<Box<dyn SecretStore>> {
    static S: OnceLock<Mutex<Box<dyn SecretStore>>> = OnceLock::new();
    // (the unit tests of this crate never reach the real store)
    S.get_or_init(|| Mutex::new(if cfg!(test) { Box::new(Memory::default()) } else { Box::new(Keyring) }))
}

/// Keep the secrets in another store (a test's, in memory).
pub fn use_store(s: Box<dyn SecretStore>) {
    *store().lock().unwrap() = s;
}

/// Fill the secrets from the store, and move into it those a settings file of before had (never released: the
/// secrets were written in it). Whether the file must be written again, without them.
fn secrets_from_store(c: &mut Cloud) -> bool {
    let st = store().lock().unwrap();
    let mut moved = false;
    let mut failed = false;
    for (name, value) in c.secrets_mut() {
        if !value.is_empty() {
            moved = true;
            failed |= st.set(name, value).is_err();
        } else if let Some(v) = st.get(name) {
            *value = v;
        }
    }
    moved && !failed
}

// (the secrets are not in the debug text a log or a failed assertion prints)
impl std::fmt::Debug for Cloud {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let set = |s: &str| if s.is_empty() { "" } else { "(set)" };
        f.debug_struct("Cloud")
            .field("aws_access_key_id", &set(&self.aws_access_key_id))
            .field("aws_secret_access_key", &set(&self.aws_secret_access_key))
            .field("aws_region", &self.aws_region)
            .field("aws_endpoint", &self.aws_endpoint)
            .field("aws_no_sign_request", &self.aws_no_sign_request)
            .field("gs_no_sign_request", &self.gs_no_sign_request)
            .finish_non_exhaustive()
    }
}

/// What GDAL is told of the credentials: every option it is given from them, with its value, or none to leave it
/// unset (GDAL then reads the environment's variable of the name, as the command line and the workers do).
pub fn cloud_options(c: &Cloud) -> Vec<(&'static str, Option<String>)> {
    let text = |s: &str| (!s.trim().is_empty()).then(|| s.trim().to_string());
    // an endpoint as a host (GDAL's AWS_S3_ENDPOINT has no scheme): `http://` is AWS_HTTPS=NO
    let endpoint = c.aws_endpoint.trim();
    let plain = endpoint.get(..7).is_some_and(|h| h.eq_ignore_ascii_case("http://"));
    let host = endpoint
        .strip_prefix("https://")
        .or_else(|| endpoint.strip_prefix("http://"))
        .unwrap_or(endpoint)
        .trim_end_matches('/');
    vec![
        ("AWS_ACCESS_KEY_ID", text(&c.aws_access_key_id)),
        ("AWS_SECRET_ACCESS_KEY", text(&c.aws_secret_access_key)),
        ("AWS_SESSION_TOKEN", text(&c.aws_session_token)),
        ("AWS_REGION", text(&c.aws_region)),
        ("AWS_S3_ENDPOINT", text(host)),
        ("AWS_HTTPS", (!host.is_empty() && plain).then(|| "NO".to_string())),
        // (a storage of another host is addressed by path: bucket.host would not be there)
        ("AWS_VIRTUAL_HOSTING", (!host.is_empty()).then(|| "FALSE".to_string())),
        ("AWS_NO_SIGN_REQUEST", c.aws_no_sign_request.then(|| "YES".to_string())),
        ("GS_ACCESS_KEY_ID", text(&c.gs_access_key_id)),
        ("GS_SECRET_ACCESS_KEY", text(&c.gs_secret_access_key)),
        ("GS_NO_SIGN_REQUEST", c.gs_no_sign_request.then(|| "YES".to_string())),
        ("GOOGLE_APPLICATION_CREDENTIALS", text(&c.gs_credentials_file)),
        ("AZURE_STORAGE_ACCOUNT", text(&c.azure_account)),
        ("AZURE_STORAGE_ACCESS_KEY", text(&c.azure_key)),
    ]
}

/// What the credentials' fields say of where they are used.
pub const CLOUD_HELP: &str = "Terraflux gives these to GDAL for reading s3://, gs:// and /vsiaz/ data (the Viewer's \
    layers and the runs). The secrets are kept in the system's store of credentials, the rest in the settings file, \
    never in a model. The command line (terraflux run) and \
    the workers use the environment's variables (AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY, AWS_SESSION_TOKEN, \
    AWS_REGION, AWS_S3_ENDPOINT, AWS_NO_SIGN_REQUEST, GS_ACCESS_KEY_ID, GS_SECRET_ACCESS_KEY, \
    GS_NO_SIGN_REQUEST, GOOGLE_APPLICATION_CREDENTIALS, AZURE_STORAGE_ACCOUNT, AZURE_STORAGE_ACCESS_KEY), as GDAL does by itself; a field \
    left empty here leaves the variable in charge.";

/// The fields of the cloud credentials, secrets masked.
pub fn cloud_panel(ui: &mut egui::Ui, c: &mut Cloud) {
    let field = |ui: &mut egui::Ui, label: &str, value: &mut String, secret: bool, hint: &str, hover: &str| {
        ui.label(label);
        ui.add(egui::TextEdit::singleline(value).password(secret).hint_text(hint).desired_width(260.0))
            .on_hover_text(format!("{hover}\n\n{CLOUD_HELP}"));
        ui.end_row();
    };
    egui::Grid::new("cloud credentials").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
        ui.label(egui::RichText::new("Amazon S3, or compatible").strong().size(11.5));
        ui.end_row();
        field(ui, "access key id", &mut c.aws_access_key_id, false, "AKIA...", "AWS_ACCESS_KEY_ID");
        field(ui, "secret key", &mut c.aws_secret_access_key, true, "", "AWS_SECRET_ACCESS_KEY");
        field(ui, "session token", &mut c.aws_session_token, true, "temporary credentials only", "AWS_SESSION_TOKEN");
        field(ui, "region", &mut c.aws_region, false, "us-east-1", "AWS_REGION");
        field(
            ui,
            "endpoint",
            &mut c.aws_endpoint,
            false,
            "another S3 storage: host:port, or http://host:port",
            "AWS_S3_ENDPOINT (with AWS_HTTPS=NO for http://, and the buckets addressed by path)",
        );
        ui.label("");
        ui.checkbox(&mut c.aws_no_sign_request, "public buckets, requests not signed")
            .on_hover_text(format!("AWS_NO_SIGN_REQUEST\n\n{CLOUD_HELP}"));
        ui.end_row();
        ui.label(egui::RichText::new("Google Cloud Storage").strong().size(11.5));
        ui.end_row();
        field(ui, "access key id", &mut c.gs_access_key_id, false, "", "GS_ACCESS_KEY_ID (interoperability key)");
        field(ui, "secret key", &mut c.gs_secret_access_key, true, "", "GS_SECRET_ACCESS_KEY");
        field(
            ui,
            "service account",
            &mut c.gs_credentials_file,
            false,
            "the key's .json file",
            "GOOGLE_APPLICATION_CREDENTIALS: the file of a service account's key",
        );
        ui.label("");
        ui.checkbox(&mut c.gs_no_sign_request, "public buckets, requests not signed")
            .on_hover_text(format!("GS_NO_SIGN_REQUEST\n\n{CLOUD_HELP}"));
        ui.end_row();
        ui.label(egui::RichText::new("Azure Blob Storage").strong().size(11.5));
        ui.end_row();
        field(ui, "account", &mut c.azure_account, false, "", "AZURE_STORAGE_ACCOUNT");
        field(ui, "account key", &mut c.azure_key, true, "", "AZURE_STORAGE_ACCESS_KEY");
    });
}

/// The credentials given to GDAL last (none: nothing yet).
fn applied() -> &'static Mutex<Option<Cloud>> {
    static A: OnceLock<Mutex<Option<Cloud>>> = OnceLock::new();
    A.get_or_init(|| Mutex::new(None))
}

/// Give GDAL the credentials, as its options for the whole program (not for a thread: the tile threads, the Viewer's
/// reads and the runs' threads all read their own datasets), at the start and when they change. What is empty is
/// unset again, so that the environment's variable is the value. GDAL is not loaded for nothing: with no credential
/// given and none before, nothing is done. The credentials are applied now, whatever was applied before.
pub fn apply_cloud(c: &Cloud) {
    let mut last = applied().lock().unwrap();
    if last.is_none() && *c == Cloud::default() {
        return;
    }
    if last.as_ref() == Some(c) {
        return;
    }
    for (key, value) in cloud_options(c) {
        // (GDAL not found: nothing to tell)
        if terraflux_gdal::set_config(key, value.as_deref()).is_err() {
            return;
        }
    }
    *last = Some(c.clone());
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
            node_settings: true,
            node_pane_opens: false,
            start_page: true,
            recent_models: Vec::new(),
            keep_awake: true,
            keep_outputs_mb: 50,
            keep_outputs_total_mb: 1024,
            reopen_last: true,
            notify_runs: true,
            notify_after: 10.0,
            python_beside: false,
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
            figure_look: crate::figure::Look::default(),
            viewer_window: None,
            custom_basemaps: Vec::new(),
            shortcuts: BTreeMap::new(),
            cloud: Cloud::default(),
            sam_agreed: Vec::new(),
            time_ratios: BTreeMap::new(),
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
        let mut s: Settings = file()
            .and_then(|f| std::fs::read_to_string(f).ok())
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        // (the secrets from the system's store; those the file had, moved there and the file written without them)
        if secrets_from_store(&mut s.cloud)
            && let Some(path) = file()
        {
            let _ = write_private(&path, &serde_json::to_string_pretty(&s).unwrap_or_default());
        }
        // (the cloud credentials given to GDAL at the start)
        apply_cloud(&s.cloud);
        Mutex::new(s)
    })
}

/// A file written for its owner alone where the system has such a mode (`0600` on Unix; the user's application data
/// of Windows is the user's alone already): the settings hold secrets. Written beside and then put in its place, so
/// that the file is never read half written (by another Terraflux reading it, or after a crash while writing).
fn write_private(path: &std::path::Path, text: &str) -> std::io::Result<()> {
    let mut temp = path.as_os_str().to_owned();
    temp.push(format!(".{}.tmp", std::process::id()));
    let temp = std::path::PathBuf::from(temp);
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        let mut f = std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(&temp)?;
        // (a file made before keeps its mode: set again)
        f.set_permissions(std::fs::Permissions::from_mode(0o600))?;
        f.write_all(text.as_bytes())?;
    }
    #[cfg(not(unix))]
    std::fs::write(&temp, text)?;
    std::fs::rename(&temp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&temp);
    })
}

/// The settings now.
pub fn get() -> Settings {
    global().lock().unwrap().clone()
}

/// The settings now, with `f` run under their lock: what it does with them is not overtaken by a change (the theme
/// in use, set from the settings a frame read).
pub fn get_with<R>(f: impl FnOnce(&Settings) -> R) -> (Settings, R) {
    let s = global().lock().unwrap();
    let r = f(&s);
    (s.clone(), r)
}

/// How many times the settings changed in this run (for what is worked out from them and kept: the keymap).
pub fn generation() -> u64 {
    GENERATION.load(std::sync::atomic::Ordering::Relaxed)
}

static GENERATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

/// Change the settings (kept at once).
pub fn update(f: impl FnOnce(&mut Settings)) {
    let mut s = global().lock().unwrap();
    let before = s.clone();
    f(&mut s);
    if *s != before {
        GENERATION.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    if s.cloud != before.cloud {
        // (a secret changed goes to the store, the file never has it)
        let st = store().lock().unwrap();
        for ((name, now), (_, was)) in s.cloud.secrets().into_iter().zip(before.cloud.secrets()) {
            if now != was {
                let _ = st.set(name, now);
            }
        }
        drop(st);
        apply_cloud(&s.cloud);
    }
    if *s != before
        && let Some(path) = file()
    {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = write_private(&path, &serde_json::to_string_pretty(&*s).unwrap_or_default());
    }
}

/// The folder of the user's application data (where the settings are).
pub fn data_dir() -> PathBuf {
    file().and_then(|f| f.parent().map(PathBuf::from)).unwrap_or_else(|| std::env::temp_dir().join("Terraflux"))
}

/// How much of a run's outputs the journal copies, from the settings (megabytes to bytes).
pub fn keep() -> terraflux_exec::journal::Keep {
    let s = get();
    terraflux_exec::journal::Keep {
        per_run: u64::from(s.keep_outputs_mb) << 20,
        total: u64::from(s.keep_outputs_total_mb) << 20,
    }
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

/// Whether the canvas shows a node's main settings on its box (read without copying the settings).
pub fn node_settings() -> bool {
    global().lock().unwrap().node_settings
}

/// Whether the Node pane's tab opens by itself with a model's first node (read without copying the settings).
pub fn node_pane_opens() -> bool {
    global().lock().unwrap().node_pane_opens
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The cloud credentials of a test.
    fn secrets() -> Cloud {
        Cloud {
            aws_access_key_id: "AKIATESTKEY".into(),
            aws_secret_access_key: "s3cr3t-of-the-test".into(),
            aws_session_token: "token-of-the-test".into(),
            aws_region: "eu-central-1".into(),
            aws_endpoint: "http://127.0.0.1:9000/".into(),
            aws_no_sign_request: true,
            gs_access_key_id: "GOOGTESTKEY".into(),
            gs_secret_access_key: "gs-secret-of-the-test".into(),
            gs_credentials_file: "C:\\keys\\service.json".into(),
            gs_no_sign_request: true,
            azure_account: "account".into(),
            azure_key: "azure-key-of-the-test".into(),
        }
    }

    #[test]
    fn the_credentials_are_gdals_options() {
        let o: BTreeMap<&str, Option<String>> = cloud_options(&secrets()).into_iter().collect();
        let v = |k: &str| o[k].as_deref();
        assert_eq!(v("AWS_ACCESS_KEY_ID"), Some("AKIATESTKEY"));
        assert_eq!(v("AWS_SECRET_ACCESS_KEY"), Some("s3cr3t-of-the-test"));
        assert_eq!(v("AWS_SESSION_TOKEN"), Some("token-of-the-test"));
        assert_eq!(v("AWS_REGION"), Some("eu-central-1"));
        // the endpoint as GDAL has it: a host, http as AWS_HTTPS=NO, the buckets by path
        assert_eq!(v("AWS_S3_ENDPOINT"), Some("127.0.0.1:9000"));
        assert_eq!((v("AWS_HTTPS"), v("AWS_VIRTUAL_HOSTING")), (Some("NO"), Some("FALSE")));
        assert_eq!(v("AWS_NO_SIGN_REQUEST"), Some("YES"));
        assert_eq!(v("GS_ACCESS_KEY_ID"), Some("GOOGTESTKEY"));
        assert_eq!(v("GS_NO_SIGN_REQUEST"), Some("YES"));
        assert_eq!(v("GOOGLE_APPLICATION_CREDENTIALS"), Some("C:\\keys\\service.json"));
        assert_eq!(
            (v("AZURE_STORAGE_ACCOUNT"), v("AZURE_STORAGE_ACCESS_KEY")),
            (Some("account"), Some("azure-key-of-the-test"))
        );
        // an https endpoint: no AWS_HTTPS; no endpoint: Amazon's, no path addressing
        let c = Cloud { aws_endpoint: "https://s3.example.org".into(), ..Cloud::default() };
        let o: BTreeMap<&str, Option<String>> = cloud_options(&c).into_iter().collect();
        assert_eq!(o["AWS_S3_ENDPOINT"].as_deref(), Some("s3.example.org"));
        assert_eq!(o["AWS_HTTPS"], None);
        // nothing given: every option left unset (the environment's variables in charge)
        assert!(cloud_options(&Cloud::default()).iter().all(|(_, v)| v.is_none()));
    }

    #[test]
    fn the_credentials_are_kept_with_the_settings_and_not_told() {
        let s = Settings { cloud: secrets(), ..Settings::default() };
        let text = serde_json::to_string_pretty(&s).unwrap();
        for secret in ["s3cr3t-of-the-test", "token-of-the-test", "gs-secret-of-the-test", "azure-key-of-the-test"] {
            assert!(!text.contains(secret), "{secret} in the settings file");
        }
        // what is not secret is kept
        let back: Settings = serde_json::from_str(&text).unwrap();
        let open = Cloud {
            aws_secret_access_key: String::new(),
            aws_session_token: String::new(),
            gs_secret_access_key: String::new(),
            azure_key: String::new(),
            ..secrets()
        };
        assert_eq!(back.cloud, open);
        // a settings file of before has none
        let old: Settings = serde_json::from_str(r#"{"theme": "Light"}"#).unwrap();
        assert_eq!(old.cloud, Cloud::default());
        // the debug text (a log, a failed assertion) leaves the secrets out
        let shown = format!("{s:?} {:?}", s.cloud);
        for secret in
            ["s3cr3t-of-the-test", "token-of-the-test", "AKIATESTKEY", "gs-secret-of-the-test", "azure-key-of-the-test"]
        {
            assert!(!shown.contains(secret), "{secret} in {shown}");
        }
    }

    /// A theme saved as "Default" (the old default) reads as Slate; the others are kept.
    #[test]
    fn a_theme_saved_as_default_reads_as_slate() {
        let old: Settings = serde_json::from_str(r#"{"theme": "Default"}"#).unwrap();
        assert_eq!(old.theme, "Slate");
        let classic: Settings = serde_json::from_str(r#"{"theme": "Classic"}"#).unwrap();
        assert_eq!(classic.theme, "Classic");
        let none: Settings = serde_json::from_str("{}").unwrap();
        assert_eq!(none.theme, crate::theme::DEFAULT);
    }

    /// The secrets go to the store, come from it, and a file of before that had them is moved over.
    #[test]
    fn the_secrets_are_in_the_store_and_a_file_of_before_is_moved_to_it() {
        static ONE: Mutex<()> = Mutex::new(());
        let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
        use_store(Box::new(Memory::default()));
        // a file of before: the secrets in it are moved (and the file is to be written again, without them)
        let old = r#"{"cloud": {"aws_access_key_id": "AKIA1", "aws_secret_access_key": "old-secret", "azure_key": "old-azure"}}"#;
        let mut s: Settings = serde_json::from_str(old).unwrap();
        assert!(secrets_from_store(&mut s.cloud), "moved");
        assert_eq!(store().lock().unwrap().get("cloud:AWS_SECRET_ACCESS_KEY").as_deref(), Some("old-secret"));
        assert_eq!(store().lock().unwrap().get("cloud:AZURE_STORAGE_ACCESS_KEY").as_deref(), Some("old-azure"));
        assert!(!serde_json::to_string(&s).unwrap().contains("old-secret"));
        // a file without them: the store fills them
        let mut fresh = Cloud { aws_access_key_id: "AKIA1".into(), ..Cloud::default() };
        assert!(!secrets_from_store(&mut fresh), "nothing to move");
        assert_eq!((fresh.aws_secret_access_key.as_str(), fresh.azure_key.as_str()), ("old-secret", "old-azure"));
        // forgotten: empty
        store().lock().unwrap().set("cloud:AWS_SECRET_ACCESS_KEY", "").unwrap();
        let mut again = Cloud::default();
        secrets_from_store(&mut again);
        assert_eq!((again.aws_secret_access_key.as_str(), again.azure_key.as_str()), ("", "old-azure"));
    }

    #[test]
    fn the_settings_file_is_written_for_its_owner() {
        let dir = std::env::temp_dir().join(format!("terraflux-settings-private-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("settings.json");
        // a file that was there, open to others, is closed
        std::fs::write(&file, "{}").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();
        }
        write_private(&file, "{\"cloud\": 1}").unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "{\"cloud\": 1}");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&file).unwrap().permissions().mode() & 0o777, 0o600);
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    /// The options are GDAL's for the whole program: read back with CPLGetConfigOption, from another thread than the
    /// one that set them (the tile threads and the runs' threads read their own datasets), and unset when cleared.
    #[test]
    fn applying_the_credentials_sets_gdals_options_for_every_thread() {
        if terraflux_gdal::gdal().is_err() {
            return;
        }
        static ONE: Mutex<()> = Mutex::new(());
        let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
        apply_cloud(&secrets());
        let read = || {
            [
                "AWS_ACCESS_KEY_ID",
                "AWS_SECRET_ACCESS_KEY",
                "AWS_SESSION_TOKEN",
                "AWS_REGION",
                "AWS_S3_ENDPOINT",
                "AWS_HTTPS",
                "AWS_NO_SIGN_REQUEST",
                "GS_ACCESS_KEY_ID",
                "AZURE_STORAGE_ACCOUNT",
            ]
            .map(terraflux_gdal::config)
        };
        let own = read();
        let other = std::thread::spawn(read).join().unwrap();
        assert_eq!(own, other, "the same in another thread");
        let want = [
            "AKIATESTKEY",
            "s3cr3t-of-the-test",
            "token-of-the-test",
            "eu-central-1",
            "127.0.0.1:9000",
            "NO",
            "YES",
            "GOOGTESTKEY",
            "account",
        ];
        assert_eq!(own.each_ref().map(|v| v.as_deref().unwrap_or("")), want);
        // cleared: unset, GDAL reads the environment's variable of the name, as the command line does
        apply_cloud(&Cloud::default());
        for (k, v) in cloud_options(&secrets()) {
            if v.is_some() {
                assert_eq!(terraflux_gdal::config(k), std::env::var(k).ok(), "{k}");
            }
        }
    }
}
