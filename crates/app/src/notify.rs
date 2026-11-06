//! The system's notifications, when a long run ends while Terraflux is in the background: on Windows a toast (the
//! application's name registered for it in the user's registry, as an application not installed from the Store
//! does), on macOS the Notification Center (`osascript`), on Linux the desktop's notifications (`notify-send`, else
//! `gdbus`). By programs every system has, in a thread: nothing to install, and nothing that waits.
//!
//! A click on one brings Terraflux to the front: on Windows the toast opens `terraflux-front:<process>`, a protocol
//! of the user's registry that runs `terraflux front` (which asks that Terraflux, by [`crate::handoff`]); on Linux
//! `notify-send --wait` says the click (a `notify-send` too old for it: the notification without). Not on macOS
//! (`osascript` cannot be told the click).

/// The name the notifications of Windows come under.
#[cfg(windows)]
const APP_ID: &str = "Terraflux.Terraflux";

/// The script of PowerShell that shows a toast: its title and text (and the picture) in the environment, not in the
/// script (no quoting to get wrong). One statement a line: read from its input, PowerShell runs a block written
/// over several lines only after an empty line.
#[cfg(windows)]
const TOAST: &str = r#"
$ErrorActionPreference = 'Stop'
$id = $env:TF_APP
$key = 'HKCU:\Software\Classes\AppUserModelId\' + $id
if (-not (Test-Path $key)) { New-Item -Path $key -Force | Out-Null }
Set-ItemProperty -Path $key -Name DisplayName -Value 'Terraflux'
if ($env:TF_ICON) { Set-ItemProperty -Path $key -Name IconUri -Value $env:TF_ICON }
[Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] | Out-Null
[Windows.Data.Xml.Dom.XmlDocument, Windows.Data.Xml.Dom.XmlDocument, ContentType = WindowsRuntime] | Out-Null
$t = [System.Security.SecurityElement]::Escape($env:TF_TITLE)
$b = [System.Security.SecurityElement]::Escape($env:TF_BODY)
$p = 'HKCU:\Software\Classes\terraflux-front'
$c = $p + '\shell\open\command'
if ($env:TF_EXE -and -not (Test-Path $c)) { New-Item -Path $c -Force | Out-Null }
if ($env:TF_EXE) { Set-ItemProperty -Path $p -Name '(default)' -Value 'URL:Terraflux' }
if ($env:TF_EXE) { Set-ItemProperty -Path $p -Name 'URL Protocol' -Value '' }
if ($env:TF_EXE) { Set-ItemProperty -Path $c -Name '(default)' -Value ('"' + $env:TF_EXE + '" front "%1"') }
$launch = ''
if ($env:TF_EXE) { $launch = " activationType='protocol' launch='terraflux-front:" + $env:TF_PID + "'" }
$xml = New-Object Windows.Data.Xml.Dom.XmlDocument
$xml.LoadXml("<toast$launch><visual><binding template='ToastGeneric'><text>$t</text><text>$b</text></binding></visual></toast>")
$toast = [Windows.UI.Notifications.ToastNotification]::new($xml)
[Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier($id).Show($toast)
"#;

/// A way to notify: a program, its arguments, what it reads from the environment and from its input; whether it
/// waits for the notification to go and says on its output when it was clicked.
#[derive(Debug, Clone, PartialEq)]
pub struct Way {
    pub program: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    pub input: Option<String>,
    pub clicks: bool,
}

fn way(program: &str, args: &[&str]) -> Way {
    Way {
        program: program.into(),
        args: args.iter().map(|a| a.to_string()).collect(),
        env: Vec::new(),
        input: None,
        clicks: false,
    }
}

/// The ways to notify on a system, the first that works used. `front`: the program a click on a toast of Windows
/// runs (`terraflux`) and the process it brings to the front.
pub fn ways(
    os: &str,
    title: &str,
    body: &str,
    icon: Option<&std::path::Path>,
    front: Option<(&std::path::Path, u32)>,
) -> Vec<Way> {
    match os {
        "windows" => {
            #[cfg(windows)]
            let (script, app) = (TOAST.to_string(), APP_ID.to_string());
            #[cfg(not(windows))]
            let (script, app) = (String::new(), String::new());
            let mut env =
                vec![("TF_APP".into(), app), ("TF_TITLE".into(), title.into()), ("TF_BODY".into(), body.into())];
            if let Some(icon) = icon {
                env.push(("TF_ICON".into(), icon.display().to_string()));
            }
            if let Some((exe, pid)) = front {
                env.push(("TF_EXE".into(), exe.display().to_string()));
                env.push(("TF_PID".into(), pid.to_string()));
            }
            vec![Way {
                program: "powershell".into(),
                args: ["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", "-"]
                    .iter()
                    .map(|a| a.to_string())
                    .collect(),
                env,
                input: Some(script),
                clicks: false,
            }]
        }
        // the title and the text as the script's arguments (not in its text)
        "macos" => vec![way(
            "osascript",
            &[
                "-e",
                "on run argv",
                "-e",
                "display notification (item 2 of argv) with title (item 1 of argv)",
                "-e",
                "end run",
                title,
                body,
            ],
        )],
        _ => vec![
            // (it waits, and says "default" when the notification is clicked)
            Way {
                clicks: true,
                ..way(
                    "notify-send",
                    &["--app-name=Terraflux", "--icon=terraflux", "--action=default=Open", "--wait", title, body],
                )
            },
            way("notify-send", &["--app-name=Terraflux", "--icon=terraflux", title, body]),
            way(
                "gdbus",
                &[
                    "call",
                    "--session",
                    "--dest=org.freedesktop.Notifications",
                    "--object-path=/org/freedesktop/Notifications",
                    "--method=org.freedesktop.Notifications.Notify",
                    "Terraflux",
                    "0",
                    "terraflux",
                    title,
                    body,
                    "[]",
                    "{}",
                    "-1",
                ],
            ),
        ],
    }
}

/// The picture of the notifications of Windows: the logo, written once in the user's application data.
fn icon() -> Option<std::path::PathBuf> {
    if !cfg!(windows) {
        return None;
    }
    let f = crate::settings::data_dir().join("terraflux-notification.png");
    if f.is_file() {
        return Some(f);
    }
    std::fs::create_dir_all(f.parent()?).ok()?;
    std::fs::write(&f, crate::logo::ICON_PNG).ok()?;
    Some(f)
}

/// Run a way to notify: whether it worked, and whether it was clicked (a way that says so).
fn run(w: &Way) -> (bool, bool) {
    use std::io::{Read, Write};
    use std::process::{Command, Stdio};
    let mut c = Command::new(&w.program);
    c.args(&w.args).envs(w.env.iter().cloned()).stderr(Stdio::null());
    c.stdout(if w.clicks { Stdio::piped() } else { Stdio::null() });
    c.stdin(if w.input.is_some() { Stdio::piped() } else { Stdio::null() });
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW: no console flashing up
        c.creation_flags(0x0800_0000);
    }
    let Ok(mut child) = c.spawn() else { return (false, false) };
    if let (Some(text), Some(mut stdin)) = (&w.input, child.stdin.take()) {
        let _ = stdin.write_all(text.as_bytes());
    }
    let mut said = String::new();
    if let Some(mut out) = child.stdout.take() {
        let _ = out.read_to_string(&mut said);
    }
    let ok = child.wait().is_ok_and(|s| s.success());
    (ok, ok && said.lines().any(|l| l.trim() == "default"))
}

/// The program a click on a toast runs: this one, when it is `terraflux` (not a test's).
fn front_program() -> Option<(std::path::PathBuf, u32)> {
    let exe = std::env::current_exe().ok()?;
    (exe.file_stem()? == "terraflux").then(|| (exe, std::process::id()))
}

/// Show a notification of the system (in a thread; nothing said if no way works); `clicked` called when it is
/// clicked where the system tells it in the process (Linux; on Windows the click comes through [`crate::handoff`]).
pub fn send(title: &str, body: &str, clicked: impl FnOnce() + Send + 'static) {
    let front = front_program();
    let all =
        ways(std::env::consts::OS, title, body, icon().as_deref(), front.as_ref().map(|(e, p)| (e.as_path(), *p)));
    std::thread::spawn(move || {
        for w in &all {
            let (ok, click) = run(w);
            if click {
                clicked();
                break;
            }
            if ok {
                break;
            }
        }
    });
}

/// What `terraflux front` is given by a toast clicked (`terraflux-front:1234`, maybe with a `/` after): the process
/// to bring to the front.
pub fn front_of(uri: &str) -> Option<u32> {
    uri.trim().strip_prefix("terraflux-front:")?.trim_matches('/').parse().ok()
}

/// What a run that ended says: its title and text. `failed`: why (and where), none if it did its work.
pub fn run_ended(model: &str, seconds: f64, failed: Option<&str>) -> (String, String) {
    let took = crate::eta::words(crate::eta::Left { seconds, by_times: true });
    let took = took.trim_start_matches('~').trim_end_matches(" left").to_string();
    match failed {
        None => (format!("{model}: done"), format!("The run ended in {took}.")),
        Some(why) => (format!("{model}: failed"), format!("The run failed after {took}: {why}")),
    }
}

/// Whether the end of a run is told: one asked for (not the application's own), long enough, Terraflux not in front.
pub fn worth_telling(on: bool, quiet: bool, seconds: f64, least: f64, in_front: bool) -> bool {
    on && !quiet && seconds >= least && !in_front
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ways_of_each_system() {
        let mac = ways("macos", "land: done", "The run ended in 3 min.", None, None);
        assert_eq!(mac.len(), 1);
        assert_eq!(mac[0].program, "osascript");
        // (the texts as arguments, not in the script)
        assert_eq!(&mac[0].args[mac[0].args.len() - 2..], ["land: done", "The run ended in 3 min."]);
        let linux = ways("linux", "a \"b\"", "c", None, None);
        let programs: Vec<&str> = linux.iter().map(|w| w.program.as_str()).collect();
        assert_eq!(programs, ["notify-send", "notify-send", "gdbus"]);
        assert!(linux[0].args.contains(&"a \"b\"".to_string()));
        // the first waits for a click; the one after for a notify-send too old for that
        assert!(linux[0].clicks && linux[0].args.contains(&"--wait".to_string()));
        assert!(!linux[1].clicks && !linux[1].args.contains(&"--wait".to_string()));
        let win = ways("windows", "t", "b", None, Some((std::path::Path::new(r"C:\tf\terraflux.exe"), 42)));
        assert_eq!(win[0].program, "powershell");
        assert!(win[0].env.contains(&("TF_TITLE".to_string(), "t".to_string())));
        assert!(win[0].env.contains(&("TF_EXE".to_string(), r"C:\tf\terraflux.exe".to_string())));
        assert!(win[0].env.contains(&("TF_PID".to_string(), "42".to_string())));
        if cfg!(windows) {
            let script = win[0].input.as_deref().unwrap();
            assert!(script.contains("CreateToastNotifier") && script.contains(r"Classes\AppUserModelId"));
            assert!(script.contains("activationType='protocol'") && script.contains(r"Classes\terraflux-front"));
        }
        assert!(!ways("windows", "t", "b", None, None)[0].env.iter().any(|(k, _)| k == "TF_EXE"));
        // what a toast clicked gives terraflux front
        assert_eq!(front_of("terraflux-front:1234"), Some(1234));
        assert_eq!(front_of("terraflux-front:1234/"), Some(1234));
        assert_eq!(front_of("http://x"), None);
    }

    #[test]
    fn what_a_run_that_ended_says() {
        assert_eq!(run_ended("land", 840.0, None), ("land: done".to_string(), "The run ended in 14 min.".to_string()));
        assert_eq!(
            run_ended("land", 30.0, Some("allocation: no cell can change")),
            ("land: failed".to_string(), "The run failed after 30 s: allocation: no cell can change".to_string())
        );
    }

    #[test]
    fn told_only_when_worth_it() {
        assert!(worth_telling(true, false, 60.0, 10.0, false));
        assert!(!worth_telling(false, false, 60.0, 10.0, false), "turned off");
        assert!(!worth_telling(true, true, 60.0, 10.0, false), "a run the application started");
        assert!(!worth_telling(true, false, 5.0, 10.0, false), "short");
        assert!(!worth_telling(true, false, 60.0, 10.0, true), "Terraflux in front");
    }
}

#[cfg(test)]
mod by_hand {
    /// A notification of the system shown for real (`cargo test -p terraflux-app notify::by_hand -- --ignored`).
    #[test]
    #[ignore]
    fn a_notification_shown() {
        let all =
            super::ways(std::env::consts::OS, "Terraflux: notification test", "The run ended in 14 min.", None, None);
        assert!(all.iter().any(|w| super::run(w).0), "no way to notify worked");
    }

    /// Windows: a toast shown for real, its protocol registered for `TF_TEST_EXE` (a `terraflux.exe` built), then
    /// opened as a click on the toast does: `terraflux front` asks this process to come to the front
    /// (`TF_TEST_EXE=... cargo test -p terraflux-app notify::by_hand -- --ignored`).
    #[cfg(windows)]
    #[test]
    #[ignore]
    fn a_click_on_a_toast_reaches_terraflux() {
        let exe = std::path::PathBuf::from(std::env::var("TF_TEST_EXE").expect("TF_TEST_EXE: a terraflux.exe"));
        let listener = crate::handoff::listen(|| {}).expect("listening");
        let pid = std::process::id();
        let all = super::ways("windows", "Terraflux: click test", "Click me", None, Some((&exe, pid)));
        assert!(super::run(&all[0]).0, "the toast not shown");
        let opened = std::process::Command::new("cmd")
            .args(["/c", "start", "", &format!("terraflux-front:{pid}")])
            .status()
            .unwrap();
        assert!(opened.success());
        let r = listener.requests.recv_timeout(std::time::Duration::from_secs(30)).expect("no request to the front");
        assert!(r.path.is_none());
        r.answer.send(Ok(())).unwrap();
    }
}
