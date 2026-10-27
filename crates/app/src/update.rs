//! Terraflux's updates. The latest release is asked of GitHub (when the application starts, at most once a day,
//! unless turned off in the settings; or from the settings, at once), and a newer one is told in the toolbar.
//! A Terraflux installed so that it can be replaced ([`Install`]) updates itself: the release's package downloaded,
//! checked against the release's `SHA256SUMS.txt` (and unpacked beside the one it replaces), installed once the
//! application has closed, the application opened again with its model. That is Windows' installer (in
//! `%LOCALAPPDATA%\Programs\Terraflux`, installed without the administrator: only its progress shown), macOS's
//! application and Linux's folder of the `.tar.gz` in a folder the user may write (replaced by the new ones), and
//! Linux's packages (installed by the system's manager, the administrator's password asked). Elsewhere (Windows'
//! zip, a build) the release's page is opened. An update that did not install is told when the application opens
//! again, with its log.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex, OnceLock};
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

/// A release: its version, its page, and its files.
#[derive(Clone, Debug, PartialEq)]
pub struct Release {
    pub version: Version,
    pub page: String,
    /// Its files: the name and where to download it.
    pub assets: Vec<(String, String)>,
    /// The checksums of its files.
    pub sums: Option<String>,
}

impl Release {
    /// The file of it that updates an installation (its name and where to download it).
    pub fn package(&self, how: &Install) -> Option<(String, String)> {
        self.assets.iter().find(|(name, _)| how.takes(name)).cloned()
    }
}

/// A release from GitHub's answer (`releases/latest`).
pub fn release_from_json(v: &serde_json::Value) -> Option<Release> {
    let version = parse_version(v.get("tag_name")?.as_str()?)?;
    let page = v.get("html_url").and_then(|u| u.as_str()).unwrap_or_default().to_string();
    let assets: Vec<(String, String)> = v
        .get("assets")
        .and_then(|a| a.as_array())
        .into_iter()
        .flatten()
        .filter_map(|a| Some((a.get("name")?.as_str()?.into(), a.get("browser_download_url")?.as_str()?.into())))
        .collect();
    let sums = assets.iter().find(|(n, _)| n == "SHA256SUMS.txt").map(|(_, url)| url.clone());
    Some(Release { version, page, assets, sums })
}

/// The checksum of a file in a `SHA256SUMS.txt` (`<sha256>  <name>` a line).
pub fn sum_of(sums: &str, name: &str) -> Option<String> {
    sums.lines().find_map(|l| {
        let (sum, file) = l.trim().split_once(char::is_whitespace)?;
        (file.trim().trim_start_matches('*') == name).then(|| sum.to_lowercase())
    })
}

/// How this Terraflux replaces itself: by the way it was installed.
#[derive(Clone, Debug, PartialEq)]
pub enum Install {
    /// Windows' installer, for the user (`%LOCALAPPDATA%\Programs\Terraflux`): the new `.msi` run. The program.
    Msi(PathBuf),
    /// macOS's application (`Terraflux.app`) in a folder the user may write: replaced by the release's (its `.zip`).
    App(PathBuf),
    /// Linux's folder of the `.tar.gz`, in a folder the user may write: replaced by the release's.
    Folder(PathBuf),
    /// Linux's package (`/opt/terraflux`): the new one installed by the system's manager, the administrator's
    /// password asked (`pkexec`).
    Package(Manager),
}

/// A manager of Linux's packages.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Manager {
    Deb,
    Rpm,
    Arch,
}

impl Install {
    /// Whether a file of a release is this installation's package (for this processor).
    pub fn takes(&self, name: &str) -> bool {
        let arch = std::env::consts::ARCH;
        let (short, debian) = match arch {
            "x86_64" => ("x64", "amd64"),
            "aarch64" => ("arm64", "arm64"),
            a => (a, a),
        };
        match self {
            Install::Msi(_) => name.ends_with("-x64.msi"),
            Install::App(_) => name.ends_with(&format!("-macos-{short}.zip")),
            Install::Folder(_) => name.ends_with(&format!("-linux-{short}.tar.gz")),
            Install::Package(Manager::Deb) => name.ends_with(&format!("_{debian}.deb")),
            Install::Package(Manager::Rpm) => name.ends_with(&format!(".{arch}.rpm")),
            Install::Package(Manager::Arch) => name.ends_with(&format!("-{arch}.pkg.tar.zst")),
        }
    }

    /// What updating does, told before it is asked.
    pub fn tell(&self) -> String {
        match self {
            Install::Msi(_) => "Terraflux closes, installs it (its progress shown, nothing to answer, no \
                                administrator's permission) and opens again with this model."
                .into(),
            Install::App(app) => format!(
                "Terraflux closes, puts the new one in the place of this one ({}) and opens again with this model.",
                app.display()
            ),
            Install::Folder(dir) => format!(
                "Terraflux closes, puts the new one in the place of this one (its folder {}) and opens again with \
                 this model.",
                dir.display()
            ),
            Install::Package(_) => "Terraflux closes, installs the new package (the system asks for the \
                                    administrator's password) and opens again with this model."
                .into(),
        }
    }
}

/// How this Terraflux can be updated (none: a build, Windows' zip, a folder the user may not write); found once.
pub fn install() -> Option<Install> {
    static HOW: OnceLock<Option<Install>> = OnceLock::new();
    HOW.get_or_init(|| find_install(&std::env::current_exe().ok()?)).clone()
}

fn find_install(exe: &Path) -> Option<Install> {
    let real = exe.canonicalize().ok()?;
    let dir = real.parent()?;
    if cfg!(windows) {
        let programs = PathBuf::from(std::env::var_os("LOCALAPPDATA")?).join("Programs").join("Terraflux");
        // the program as it was started (a canonical path is \\?\'s, which not every program takes)
        return (programs.canonicalize().ok()? == dir).then(|| Install::Msi(exe.to_path_buf()));
    }
    if cfg!(target_os = "macos") {
        // Terraflux.app/Contents/MacOS/terraflux
        let app = dir.parent()?.parent()?;
        let bundle = dir.ends_with("Contents/MacOS") && app.extension().is_some_and(|e| e == "app");
        return (bundle && writable(app.parent()?)).then(|| Install::App(app.to_path_buf()));
    }
    if dir == Path::new("/opt/terraflux") {
        return manager().map(Install::Package);
    }
    // the folder of the .tar.gz: the template of its menu entry beside the program
    (dir.join("terraflux.desktop.in").is_file() && writable(dir.parent()?)).then(|| Install::Folder(dir.to_path_buf()))
}

/// Whether this user may write in a folder (a folder made in it and removed).
fn writable(dir: &Path) -> bool {
    let probe = dir.join(format!(".terraflux-{}", std::process::id()));
    std::fs::create_dir(&probe).is_ok() && std::fs::remove_dir(&probe).is_ok()
}

fn on_path(program: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|p| std::env::split_paths(&p).any(|d| d.join(program).is_file()))
}

/// The manager that installed `/opt/terraflux`, when the administrator's password can be asked (`pkexec`).
fn manager() -> Option<Manager> {
    if !on_path("pkexec") {
        return None;
    }
    if Path::new("/var/lib/dpkg/info/terraflux.list").is_file() {
        return Some(Manager::Deb);
    }
    let pacman = std::fs::read_dir("/var/lib/pacman/local")
        .is_ok_and(|d| d.flatten().any(|e| e.file_name().to_string_lossy().starts_with("terraflux-")));
    if pacman {
        return Some(Manager::Arch);
    }
    let rpm =
        on_path("rpm") && Command::new("rpm").args(["-q", "terraflux"]).output().is_ok_and(|o| o.status.success());
    rpm.then_some(Manager::Rpm)
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
    /// The package being downloaded: the bytes so far, of so many (when told).
    Downloading(u64, Option<u64>),
    /// The package of this version downloaded and checked (the installer, or the application or folder unpacked):
    /// installed once the application closes.
    Ready(Version, PathBuf),
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
        if matches!(self.state(), State::Checking | State::Downloading(..) | State::Ready(..)) {
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

    /// Download the release's package for this installation, check it and make it ready (then [`State::Ready`]).
    pub fn download(&self, r: Release, wake: impl Fn() + Send + 'static) {
        let Some(how) = install() else { return };
        let Some((name, url)) = r.package(&how) else { return };
        self.set(State::Downloading(0, None), &wake);
        let me = self.clone();
        std::thread::spawn(move || {
            let got = fetch(&r, &name, &url, |done, total| {
                *me.state.lock().unwrap() = State::Downloading(done, total);
                wake();
            });
            let s = match got.and_then(|file| prepare(&how, &file)) {
                Ok(path) => State::Ready(r.version, path),
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

/// The folder the packages are downloaded to.
fn downloads() -> PathBuf {
    std::env::temp_dir().join("terraflux-update")
}

/// The folder a new application or folder is unpacked in: beside the one it replaces (moved into its place, not
/// copied).
fn staging(old: &Path) -> PathBuf {
    old.with_file_name(".terraflux-update")
}

/// The log of the last installation (outside the folder of the downloads, removed when the application starts).
pub fn log_file() -> PathBuf {
    std::env::temp_dir().join("terraflux-update.log")
}

/// The packages of the updates before, removed (when the application starts).
pub fn clean() {
    let _ = std::fs::remove_dir_all(downloads());
    if let Some(Install::App(old) | Install::Folder(old)) = install() {
        let _ = std::fs::remove_dir_all(staging(&old));
    }
}

/// The package downloaded into the temporary folder and checked against the release's checksums.
fn fetch(r: &Release, name: &str, url: &str, progress: impl Fn(u64, Option<u64>)) -> Result<PathBuf, String> {
    let sums_url = r.sums.as_ref().ok_or("the release has no checksums (SHA256SUMS.txt) to check its package")?;
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

/// The package made ready to install: an installer as it is; macOS's application (`ditto`) and Linux's folder
/// (`tar`) unpacked beside the ones they replace, the program checked in them.
fn prepare(how: &Install, file: &Path) -> Result<PathBuf, String> {
    let (old, mut unpack, program) = match how {
        Install::Msi(_) | Install::Package(_) => return Ok(file.to_path_buf()),
        Install::App(old) => (old, Command::new("ditto"), "Contents/MacOS/terraflux"),
        Install::Folder(old) => (old, Command::new("tar"), "terraflux"),
    };
    let stage = staging(old);
    let _ = std::fs::remove_dir_all(&stage);
    std::fs::create_dir_all(&stage).map_err(|e| format!("{}: {e}", stage.display()))?;
    if matches!(how, Install::App(_)) {
        unpack.args(["-x", "-k"]).arg(file).arg(&stage);
    } else {
        unpack.arg("-xzf").arg(file).arg("-C").arg(&stage);
    }
    let name = file.file_name().unwrap_or_default().to_string_lossy().into_owned();
    let out = unpack.output().map_err(|e| format!("{name} could not be unpacked: {e}"))?;
    if !out.status.success() {
        return Err(format!("{name} could not be unpacked: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    std::fs::read_dir(&stage)
        .map_err(|e| format!("{}: {e}", stage.display()))?
        .flatten()
        .map(|e| e.path())
        .find(|p| p.join(program).is_file())
        .ok_or_else(|| format!("{name} has no Terraflux in it: not installed"))
}

/// The command that installs the installer once this process has ended (its log kept), then opens Terraflux again
/// (with a model): PowerShell's, its script encoded (no quoting of the paths to get wrong).
pub fn install_script(pid: u32, msi: &Path, exe: &Path, model: Option<&Path>, log: &Path) -> String {
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
        lit(&format!("/i \"{}\" /passive /norestart /l*v \"{}\"", msi.display(), log.display())),
    )
}

/// The shell's script that installs an update once a process has ended, then opens Terraflux again. Its
/// arguments: the process, the package (or the application or folder unpacked), what it replaces (or the
/// program), then the model if one.
pub fn unix_script(how: &Install) -> String {
    let wait = "pid=$1 new=$2 old=$3; shift 3\n\
                while kill -0 \"$pid\" 2>/dev/null; do sleep 0.2; done\n";
    // moved into the place of the old one (the old one moved back if that fails), its staging folder removed
    let swap = "if mv \"$old\" \"$old.old\"; then\n\
                \x20 if mv \"$new\" \"$old\"; then rm -rf \"$old.old\"; echo \"installed $old\";\n\
                \x20 else mv \"$old.old\" \"$old\"; echo \"$new could not be moved to $old: not installed\"; fi\n\
                else echo \"$old could not be moved: not installed\"; fi\n\
                rm -rf \"$(dirname \"$new\")\"\n";
    let manager = |command: &str| format!("pkexec {command} \"$new\" || echo \"not installed\"\n");
    let (install, open) = match how {
        Install::Msi(_) => return String::new(),
        Install::App(_) => (swap.to_string(), "exec open -n \"$old\" --args \"$@\"\n"),
        Install::Folder(_) => (swap.to_string(), "exec \"$old/terraflux\" \"$@\"\n"),
        Install::Package(Manager::Deb) => (manager("dpkg -i"), "exec \"$old\" \"$@\"\n"),
        Install::Package(Manager::Rpm) => (manager("rpm -U"), "exec \"$old\" \"$@\"\n"),
        Install::Package(Manager::Arch) => (manager("pacman -U --noconfirm"), "exec \"$old\" \"$@\"\n"),
    };
    format!("{wait}{install}{open}")
}

/// Install the update made ready after this application closes, and open it again (with a model). The version
/// tried is kept: an update that did not install is told when Terraflux opens again.
pub fn install_after_exit(how: &Install, version: Version, ready: &Path, model: Option<&Path>) -> Result<(), String> {
    let log = log_file();
    let _ = std::fs::remove_file(&log);
    crate::settings::update(|s| s.update_tried = Some(show(version)));
    let started = match how {
        Install::Msi(exe) => run_powershell(&install_script(std::process::id(), ready, exe, model, &log)),
        _ => {
            let old = match how {
                Install::App(old) | Install::Folder(old) => old.clone(),
                _ => PathBuf::from("/opt/terraflux/terraflux"),
            };
            run_shell(&unix_script(how), ready, &old, model, &log)
        }
    };
    started.map_err(|e| {
        crate::settings::update(|s| s.update_tried = None);
        format!("the installation could not be started: {e}")
    })
}

/// PowerShell, with a console of its own not shown, living on after this process (out of its job, when the job
/// lets it). Not detached: PowerShell without a console ends before it runs anything.
fn run_powershell(script: &str) -> std::io::Result<()> {
    let utf16: Vec<u8> = script.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
    let encoded = base64(&utf16);
    let command = |breakaway: bool| {
        let mut c = Command::new("powershell.exe");
        c.args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-WindowStyle", "Hidden"]);
        c.args(["-EncodedCommand", &encoded]);
        c.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
            const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            let away = if breakaway { CREATE_BREAKAWAY_FROM_JOB } else { 0 };
            c.creation_flags(CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW | away);
        }
        #[cfg(not(windows))]
        let _ = breakaway;
        c
    };
    // a job that does not let its processes out refuses the first
    command(true).spawn().or_else(|_| command(false).spawn()).map(|_| ())
}

/// The shell, in a group of processes of its own (the terminal's hang-up not sent to it), its output in the log.
fn run_shell(script: &str, new: &Path, old: &Path, model: Option<&Path>, log: &Path) -> std::io::Result<()> {
    let out = std::fs::File::create(log)?;
    let mut c = Command::new("/bin/sh");
    c.arg("-c").arg(script).arg("terraflux-update").arg(std::process::id().to_string()).arg(new).arg(old);
    c.args(model);
    c.stdin(Stdio::null()).stdout(out.try_clone()?).stderr(out);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        c.process_group(0);
    }
    c.spawn().map(|_| ())
}

/// The version an update was to install, when Terraflux is still older: the update did not install (told, with
/// its log). Asked once, when the application starts.
pub fn not_installed() -> Option<String> {
    let tried = crate::settings::get().update_tried?;
    crate::settings::update(|s| s.update_tried = None);
    let v = parse_version(&tried)?;
    (v > current()).then(|| {
        format!(
            "Terraflux {tried} was not installed (this is still {}); its log: {}",
            env!("CARGO_PKG_VERSION"),
            log_file().display()
        )
    })
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
        Command::new("explorer.exe").arg(url).spawn()
    } else if cfg!(target_os = "macos") {
        Command::new("open").arg(url).spawn()
    } else {
        Command::new("xdg-open").arg(url).spawn()
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

    fn release() -> Release {
        let assets = [
            "SHA256SUMS.txt",
            "terraflux-1.20.0-1-x86_64.pkg.tar.zst",
            "terraflux-1.20.0-1.x86_64.rpm",
            "terraflux-1.20.0-linux-x64.tar.gz",
            "terraflux-1.20.0-macos-arm64.dmg",
            "terraflux-1.20.0-macos-arm64.zip",
            "terraflux-1.20.0-windows-x64.zip",
            "terraflux-1.20.0-x64.msi",
            "terraflux_1.20.0_amd64.deb",
        ]
        .map(|n| serde_json::json!({ "name": n, "browser_download_url": format!("https://x/{n}") }));
        let v = serde_json::json!({
            "tag_name": "v1.20.0",
            "html_url": "https://github.com/sacridini/terraflux/releases/tag/v1.20.0",
            "assets": assets,
        });
        release_from_json(&v).unwrap()
    }

    #[test]
    fn a_release_is_read_from_github_s_answer() {
        let r = release();
        assert_eq!(r.version, (1, 20, 0));
        assert_eq!(r.assets.len(), 9);
        assert_eq!(r.sums.as_deref(), Some("https://x/SHA256SUMS.txt"));
        assert!(release_from_json(&serde_json::json!({ "message": "Not Found" })).is_none());
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn each_installation_takes_its_package() {
        let r = release();
        let name = |how: Install| r.package(&how).map(|(n, _)| n);
        assert_eq!(name(Install::Msi(PathBuf::new())).as_deref(), Some("terraflux-1.20.0-x64.msi"));
        assert_eq!(name(Install::Folder(PathBuf::new())).as_deref(), Some("terraflux-1.20.0-linux-x64.tar.gz"));
        assert_eq!(name(Install::Package(Manager::Deb)).as_deref(), Some("terraflux_1.20.0_amd64.deb"));
        assert_eq!(name(Install::Package(Manager::Rpm)).as_deref(), Some("terraflux-1.20.0-1.x86_64.rpm"));
        let arch = name(Install::Package(Manager::Arch));
        assert_eq!(arch.as_deref(), Some("terraflux-1.20.0-1-x86_64.pkg.tar.zst"));
        // only arm64's application is published
        assert_eq!(name(Install::App(PathBuf::new())), None);
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
            Path::new(r"C:\Temp\terraflux-update.log"),
        );
        assert_eq!(
            s,
            "Wait-Process -Id 42 -ErrorAction SilentlyContinue; Start-Process msiexec.exe -ArgumentList \
             '/i \"C:\\Temp\\terraflux-update\\terraflux-1.19.0-x64.msi\" /passive /norestart \
             /l*v \"C:\\Temp\\terraflux-update.log\"' -Wait; \
             Start-Process 'C:\\Users\\O''Neil\\AppData\\Local\\Programs\\Terraflux\\terraflux.exe' \
             -ArgumentList '\"C:\\My models\\a.toml\"'"
        );
    }

    /// PowerShell started as the application starts it runs its script after the application has ended (a
    /// detached one ran nothing: the update of 1.19.0).
    #[test]
    #[cfg(windows)]
    fn powershell_runs_after_the_application_ends() {
        let dir = std::env::temp_dir().join(format!("terraflux-ps-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mark = dir.join("ran it.txt");
        let mut child = Command::new("cmd.exe").args(["/c", "ping -n 2 127.0.0.1 >nul"]).spawn().unwrap();
        let script = format!(
            "Wait-Process -Id {} -ErrorAction SilentlyContinue; Set-Content -LiteralPath '{}' -Value ok",
            child.id(),
            mark.display()
        );
        run_powershell(&script).unwrap();
        child.wait().unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        while !mark.is_file() && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(100));
        }
        assert!(mark.is_file(), "PowerShell did not run its script");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The shell's script waits for the application, moves the new folder into the old one's place (its staging
    /// folder removed) and opens the new program with the model.
    #[test]
    #[cfg(unix)]
    fn the_folder_is_replaced_once_the_application_ends() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("terraflux-swap-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let old = dir.join("terraflux-1.19.0-linux-x64");
        let new = staging(&old).join("terraflux-1.20.0-linux-x64");
        let mark = dir.join("opened.txt");
        for (folder, version) in [(&old, "1.19.0"), (&new, "1.20.0")] {
            std::fs::create_dir_all(folder).unwrap();
            let program = folder.join("terraflux");
            std::fs::write(&program, format!("#!/bin/sh\necho {version} \"$1\" > '{}'\n", mark.display())).unwrap();
            std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let mut app = Command::new("sleep").arg("0.5").spawn().unwrap();
        let log = dir.join("update.log");
        let out = std::fs::File::create(&log).unwrap();
        let mut c = Command::new("/bin/sh");
        c.arg("-c").arg(unix_script(&Install::Folder(old.clone()))).arg("terraflux-update");
        c.arg(app.id().to_string()).arg(&new).arg(&old).arg("/models/a b.toml");
        c.stdout(out.try_clone().unwrap()).stderr(out);
        let mut sh = c.spawn().unwrap();
        // not before the application has ended
        std::thread::sleep(Duration::from_millis(200));
        assert!(!mark.exists());
        app.wait().unwrap();
        assert!(sh.wait().unwrap().success(), "{}", std::fs::read_to_string(&log).unwrap());
        assert_eq!(std::fs::read_to_string(&mark).unwrap(), "1.20.0 /models/a b.toml\n");
        assert!(!staging(&old).exists() && !dir.join("terraflux-1.19.0-linux-x64.old").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// GitHub asked, the package downloaded and checked: `cargo test -p terraflux-app update -- --ignored`.
    #[test]
    #[ignore = "asks GitHub and downloads the installer"]
    fn the_latest_installer_is_downloaded_and_checked() {
        let r = latest().unwrap();
        assert!(r.version >= (1, 18, 0), "{r:?}");
        let (name, url) = r.package(&Install::Msi(PathBuf::new())).expect("an installer");
        let seen = std::sync::atomic::AtomicU64::new(0);
        let path = fetch(&r, &name, &url, |done, _| seen.store(done, std::sync::atomic::Ordering::Relaxed)).unwrap();
        assert_eq!(std::fs::metadata(&path).unwrap().len(), seen.into_inner());
        // a file that is not the one published is refused
        let wrong = fetch(&r, "SHA256SUMS.txt", r.sums.as_deref().unwrap(), |_, _| {});
        assert!(wrong.is_err());
    }

    #[test]
    fn base64_as_powershell_reads_it() {
        assert_eq!(base64(b"Man"), "TWFu");
        assert_eq!(base64(b"Ma"), "TWE=");
        assert_eq!(base64(b"M"), "TQ==");
    }
}
