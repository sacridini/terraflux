//! Terraflux's updates. The latest release is asked of GitHub (when the application starts, at most once a day,
//! unless turned off in the settings; or from the settings, at once), and a newer one is told in the toolbar.
//! On Windows, a Terraflux of the installer (in `%LOCALAPPDATA%\Programs\Terraflux`, installed without the
//! administrator) updates itself: the new installer downloaded, checked against the release's `SHA256SUMS.txt`,
//! installed once the application has closed (only its progress shown, no question asked), the application opened
//! again with its model. Elsewhere (the zip, macOS, Linux's packages) the release's page is opened.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};

/// The repository the releases are published in.
pub const REPO: &str = "sacridini/terraflux";

/// A version: major, minor, patch.
pub type Version = (u64, u64, u64);

/// `1.19.0` or `v1.19.0` (anything after a `-` or `+` left out).
pub fn parse_version(s: &str) -> Option<Version> {
    let s = s.trim().trim_start_matches('v');
    let s = s.split(['-', '+']).next()?;
    let mut parts = s.split('.').map(|p| p.parse::<u64>().ok());
    let v = (parts.next()??, parts.next()??, parts.next().unwrap_or(Some(0))?);
    parts.next().is_none().then_some(v)
}

/// This Terraflux's version.
pub fn current() -> Version {
    parse_version(env!("CARGO_PKG_VERSION")).unwrap_or_default()
}

pub fn show(v: Version) -> String {
    format!("{}.{}.{}", v.0, v.1, v.2)
}

/// A release: its version, its page, and the files of it this computer can use.
#[derive(Clone, Debug, PartialEq)]
pub struct Release {
    pub version: Version,
    pub page: String,
    /// The Windows installer (its name and where to download it).
    pub installer: Option<(String, String)>,
    /// The checksums of its files.
    pub sums: Option<String>,
}

/// A release from GitHub's answer (`releases/latest`).
pub fn release_from_json(v: &serde_json::Value) -> Option<Release> {
    let version = parse_version(v.get("tag_name")?.as_str()?)?;
    let page = v.get("html_url").and_then(|u| u.as_str()).unwrap_or_default().to_string();
    let assets = v.get("assets").and_then(|a| a.as_array()).cloned().unwrap_or_default();
    let asset = |pred: &dyn Fn(&str) -> bool| {
        assets.iter().find_map(|a| {
            let name = a.get("name")?.as_str()?;
            let url = a.get("browser_download_url")?.as_str()?;
            pred(name).then(|| (name.to_string(), url.to_string()))
        })
    };
    Some(Release {
        version,
        page,
        installer: asset(&|n| n.ends_with("-x64.msi")),
        sums: asset(&|n| n == "SHA256SUMS.txt").map(|(_, url)| url),
    })
}

/// The checksum of a file in a `SHA256SUMS.txt` (`<sha256>  <name>` a line).
pub fn sum_of(sums: &str, name: &str) -> Option<String> {
    sums.lines().find_map(|l| {
        let (sum, file) = l.trim().split_once(char::is_whitespace)?;
        (file.trim().trim_start_matches('*') == name).then(|| sum.to_lowercase())
    })
}

/// Where the updates stand.
#[derive(Clone, Debug, PartialEq, Default)]
pub enum State {
    #[default]
    Idle,
    Checking,
    /// The latest is this one (or older).
    UpToDate,
    Available(Release),
    /// The installer being downloaded: the bytes so far, of so many (when told).
    Downloading(u64, Option<u64>),
    /// The installer downloaded and checked: installed once the application closes.
    Ready(PathBuf),
    Failed(String),
}

/// The updates: asked and downloaded by threads of their own.
#[derive(Clone, Default)]
pub struct Updater {
    state: Arc<Mutex<State>>,
}

impl Updater {
    pub fn state(&self) -> State {
        self.state.lock().unwrap().clone()
    }

    fn set(&self, s: State, wake: &(impl Fn() + Send + 'static)) {
        *self.state.lock().unwrap() = s;
        wake();
    }

    pub fn fail(&self, e: String) {
        *self.state.lock().unwrap() = State::Failed(e);
    }

    /// Ask for the latest release, when the settings let it and the last asking was a day ago or more.
    pub fn check_if_due(&self, wake: impl Fn() + Send + 'static) {
        let st = crate::settings::get();
        let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
        if st.check_updates && st.updates_checked.is_none_or(|t| now.saturating_sub(t) >= 24 * 3600) {
            self.check(wake);
        }
    }

    /// Ask for the latest release now.
    pub fn check(&self, wake: impl Fn() + Send + 'static) {
        if matches!(self.state(), State::Checking | State::Downloading(..) | State::Ready(_)) {
            return;
        }
        self.set(State::Checking, &wake);
        let me = self.clone();
        std::thread::spawn(move || {
            let s = match latest() {
                Ok(r) => {
                    let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
                    crate::settings::update(|s| s.updates_checked = Some(now));
                    if r.version > current() { State::Available(r) } else { State::UpToDate }
                }
                Err(e) => State::Failed(e),
            };
            me.set(s, &wake);
        });
    }

    /// Download the release's installer and check it (then [`State::Ready`]).
    pub fn download(&self, r: Release, wake: impl Fn() + Send + 'static) {
        let Some((name, url)) = r.installer.clone() else { return };
        self.set(State::Downloading(0, None), &wake);
        let me = self.clone();
        std::thread::spawn(move || {
            let s = match fetch_installer(&r, &name, &url, |done, total| {
                *me.state.lock().unwrap() = State::Downloading(done, total);
                wake();
            }) {
                Ok(path) => State::Ready(path),
                Err(e) => State::Failed(e),
            };
            me.set(s, &wake);
        });
    }
}

fn agent() -> ureq::Agent {
    use ureq::tls::{RootCerts, TlsConfig};
    ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(15)))
        .timeout_recv_response(Some(Duration::from_secs(60)))
        .tls_config(TlsConfig::builder().root_certs(RootCerts::PlatformVerifier).build())
        .user_agent(format!("Terraflux/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .into()
}

/// The latest release published.
fn latest() -> Result<Release, String> {
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let mut res = agent()
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| format!("GitHub could not be asked for the latest Terraflux: {e}"))?;
    let text = res.body_mut().read_to_string().map_err(|e| format!("GitHub's answer: {e}"))?;
    let v: serde_json::Value = serde_json::from_str(&text).map_err(|e| format!("GitHub's answer: {e}"))?;
    release_from_json(&v).ok_or_else(|| "GitHub's answer has no release".to_string())
}

/// The folder the installers are downloaded to.
fn downloads() -> PathBuf {
    std::env::temp_dir().join("terraflux-update")
}

/// The installers of the updates before, removed (when the application starts).
pub fn clean() {
    let _ = std::fs::remove_dir_all(downloads());
}

/// The installer downloaded into the temporary folder and checked against the release's checksums.
fn fetch_installer(r: &Release, name: &str, url: &str, progress: impl Fn(u64, Option<u64>)) -> Result<PathBuf, String> {
    let sums_url = r.sums.as_ref().ok_or("the release has no checksums (SHA256SUMS.txt) to check the installer")?;
    let sums = agent()
        .get(sums_url)
        .call()
        .and_then(|mut res| res.body_mut().read_to_string())
        .map_err(|e| format!("the release's checksums: {e}"))?;
    let want = sum_of(&sums, name).ok_or_else(|| format!("the release's checksums do not have {name}"))?;
    let dir = downloads();
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let path = dir.join(name);
    let mut res = agent().get(url).call().map_err(|e| format!("{name}: {e}"))?;
    let total = res.headers().get("content-length").and_then(|v| v.to_str().ok()).and_then(|v| v.parse().ok());
    let mut body = res.body_mut().with_config().limit(u64::MAX).reader();
    let mut file = std::fs::File::create(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let (mut hash, mut buf, mut done) = (Sha256::new(), vec![0u8; 1 << 16], 0u64);
    loop {
        let n = body.read(&mut buf).map_err(|e| format!("{name}: {e}"))?;
        if n == 0 {
            break;
        }
        hash.update(&buf[..n]);
        file.write_all(&buf[..n]).map_err(|e| format!("{}: {e}", path.display()))?;
        done += n as u64;
        progress(done, total);
    }
    drop(file);
    let got = hex::encode(hash.finalize());
    if got != want {
        let _ = std::fs::remove_file(&path);
        return Err(format!("{name} downloaded is not the one published (its checksum differs): not installed"));
    }
    Ok(path)
}

/// The folder of the installer's Terraflux (`%LOCALAPPDATA%\Programs\Terraflux`), when this program runs from it:
/// the Terraflux an installer can update.
pub fn installed_here() -> Option<PathBuf> {
    if !cfg!(windows) {
        return None;
    }
    let exe = std::env::current_exe().ok()?;
    let dir = PathBuf::from(std::env::var_os("LOCALAPPDATA")?).join("Programs").join("Terraflux");
    let same = |a: &Path, b: &Path| match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    };
    same(exe.parent()?, &dir).then_some(exe)
}

/// The command that installs the installer once this process has ended, then opens Terraflux again (with a model):
/// PowerShell's, its script encoded (no quoting of the paths to get wrong).
pub fn install_script(pid: u32, msi: &Path, exe: &Path, model: Option<&Path>) -> String {
    // a PowerShell string ('' for a '), and a path in double quotes in it (Start-Process joins its arguments
    // as they are: a path with spaces needs them)
    let lit = |s: &str| format!("'{}'", s.replace('\'', "''"));
    let quoted = |p: &Path| lit(&format!("\"{}\"", p.display()));
    let open = match model {
        Some(m) => format!("Start-Process {} -ArgumentList {}", lit(&exe.display().to_string()), quoted(m)),
        None => format!("Start-Process {}", lit(&exe.display().to_string())),
    };
    format!(
        "Wait-Process -Id {pid} -ErrorAction SilentlyContinue; \
         Start-Process msiexec.exe -ArgumentList {} -Wait; {open}",
        lit(&format!("/i \"{}\" /passive /norestart", msi.display())),
    )
}

/// Install the installer downloaded after this application closes, and open it again (Windows).
pub fn install_after_exit(msi: &Path, exe: &Path, model: Option<&Path>) -> Result<(), String> {
    let script = install_script(std::process::id(), msi, exe, model);
    let utf16: Vec<u8> = script.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
    let mut c = std::process::Command::new("powershell.exe");
    c.args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-WindowStyle", "Hidden"]);
    c.args(["-EncodedCommand", &base64(&utf16)]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // its own, without a window, living on after this process
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        c.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW);
    }
    c.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    c.spawn().map(|_| ()).map_err(|e| format!("the installer could not be started: {e}"))
}

fn base64(bytes: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for c in bytes.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            if i <= c.len() {
                s.push(A[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                s.push('=');
            }
        }
    }
    s
}

/// Open a page in the system's browser.
pub fn open_page(url: &str) {
    let r = if cfg!(windows) {
        std::process::Command::new("explorer.exe").arg(url).spawn()
    } else if cfg!(target_os = "macos") {
        std::process::Command::new("open").arg(url).spawn()
    } else {
        std::process::Command::new("xdg-open").arg(url).spawn()
    };
    let _ = r;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_are_read_and_compared() {
        assert_eq!(parse_version("v1.19.0"), Some((1, 19, 0)));
        assert_eq!(parse_version("1.2"), Some((1, 2, 0)));
        assert_eq!(parse_version("2.0.1-rc.1"), Some((2, 0, 1)));
        assert_eq!(parse_version("one"), None);
        assert_eq!(parse_version("1.2.3.4"), None);
        assert!(parse_version("1.10.0") > parse_version("1.9.9"));
        assert_eq!(show(current()), env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn a_release_is_read_from_github_s_answer() {
        let v = serde_json::json!({
            "tag_name": "v1.19.0",
            "html_url": "https://github.com/sacridini/terraflux/releases/tag/v1.19.0",
            "assets": [
                { "name": "terraflux-1.19.0-windows-x64.zip", "browser_download_url": "https://x/zip" },
                { "name": "terraflux-1.19.0-x64.msi", "browser_download_url": "https://x/msi" },
                { "name": "SHA256SUMS.txt", "browser_download_url": "https://x/sums" }
            ]
        });
        let r = release_from_json(&v).unwrap();
        assert_eq!(r.version, (1, 19, 0));
        assert_eq!(r.installer, Some(("terraflux-1.19.0-x64.msi".into(), "https://x/msi".into())));
        assert_eq!(r.sums.as_deref(), Some("https://x/sums"));
        assert!(release_from_json(&serde_json::json!({ "message": "Not Found" })).is_none());
    }

    #[test]
    fn checksums_are_found_by_name() {
        let sums = "45f1  terraflux-1.18.0-1-x86_64.pkg.tar.zst\nABCD *terraflux-1.18.0-x64.msi\n";
        assert_eq!(sum_of(sums, "terraflux-1.18.0-x64.msi").as_deref(), Some("abcd"));
        assert_eq!(sum_of(sums, "terraflux-1.18.0-x64.zip"), None);
    }

    #[test]
    fn the_script_waits_installs_and_opens_again() {
        let s = install_script(
            42,
            Path::new(r"C:\Temp\terraflux-update\terraflux-1.19.0-x64.msi"),
            Path::new(r"C:\Users\O'Neil\AppData\Local\Programs\Terraflux\terraflux.exe"),
            Some(Path::new(r"C:\My models\a.toml")),
        );
        assert_eq!(
            s,
            "Wait-Process -Id 42 -ErrorAction SilentlyContinue; Start-Process msiexec.exe -ArgumentList \
             '/i \"C:\\Temp\\terraflux-update\\terraflux-1.19.0-x64.msi\" /passive /norestart' -Wait; \
             Start-Process 'C:\\Users\\O''Neil\\AppData\\Local\\Programs\\Terraflux\\terraflux.exe' \
             -ArgumentList '\"C:\\My models\\a.toml\"'"
        );
    }

    /// GitHub asked, the installer downloaded and checked: `cargo test -p terraflux-app update -- --ignored`.
    #[test]
    #[ignore = "asks GitHub and downloads the installer"]
    fn the_latest_installer_is_downloaded_and_checked() {
        let r = latest().unwrap();
        assert!(r.version >= (1, 18, 0), "{r:?}");
        let (name, url) = r.installer.clone().expect("an installer");
        let seen = std::sync::atomic::AtomicU64::new(0);
        let path =
            fetch_installer(&r, &name, &url, |done, _| seen.store(done, std::sync::atomic::Ordering::Relaxed)).unwrap();
        assert_eq!(std::fs::metadata(&path).unwrap().len(), seen.into_inner());
        // a file that is not the one published is refused
        let other = Release { sums: r.sums.clone(), ..r.clone() };
        let wrong = fetch_installer(&other, "SHA256SUMS.txt", r.sums.as_deref().unwrap(), |_, _| {});
        assert!(wrong.is_err());
    }

    #[test]
    fn base64_as_powershell_reads_it() {
        assert_eq!(base64(b"Man"), "TWFu");
        assert_eq!(base64(b"Ma"), "TWE=");
        assert_eq!(base64(b"M"), "TQ==");
    }
}
