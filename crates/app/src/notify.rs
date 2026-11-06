//! The system's notifications, when a long run ends while Terraflux is in the background: on Windows a toast (the
//! application's name registered for it in the user's registry, as an application not installed from the Store
//! does), on macOS the Notification Center (`osascript`), on Linux the desktop's notifications (`notify-send`, else
//! `gdbus`). By programs every system has, in a thread: nothing to install, and nothing that waits.

/// The name the notifications of Windows come under.
#[cfg(windows)]
const APP_ID: &str = "Terraflux.Terraflux";

/// The script of PowerShell that shows a toast: its title and text (and the picture) in the environment, not in the
/// script (no quoting to get wrong).
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
$xml = New-Object Windows.Data.Xml.Dom.XmlDocument
$xml.LoadXml("<toast><visual><binding template='ToastGeneric'><text>$t</text><text>$b</text></binding></visual></toast>")
$toast = [Windows.UI.Notifications.ToastNotification]::new($xml)
[Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier($id).Show($toast)
"#;

/// A way to notify: a program, its arguments, what it reads from the environment and from its input.
#[derive(Debug, Clone, PartialEq)]
pub struct Way {
    pub program: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    pub input: Option<String>,
}

fn way(program: &str, args: &[&str]) -> Way {
    Way { program: program.into(), args: args.iter().map(|a| a.to_string()).collect(), env: Vec::new(), input: None }
}

/// The ways to notify on a system, the first that works used.
pub fn ways(os: &str, title: &str, body: &str, icon: Option<&std::path::Path>) -> Vec<Way> {
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
            vec![Way {
                program: "powershell".into(),
                args: ["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", "-"]
                    .iter()
                    .map(|a| a.to_string())
                    .collect(),
                env,
                input: Some(script),
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

/// Run a way to notify; whether it worked.
fn run(w: &Way) -> bool {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let mut c = Command::new(&w.program);
    c.args(&w.args).envs(w.env.iter().cloned()).stdout(Stdio::null()).stderr(Stdio::null());
    c.stdin(if w.input.is_some() { Stdio::piped() } else { Stdio::null() });
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW: no console flashing up
        c.creation_flags(0x0800_0000);
    }
    let Ok(mut child) = c.spawn() else { return false };
    if let (Some(text), Some(mut stdin)) = (&w.input, child.stdin.take()) {
        let _ = stdin.write_all(text.as_bytes());
    }
    child.wait().is_ok_and(|s| s.success())
}

/// Show a notification of the system (in a thread; nothing said if no way works).
pub fn send(title: &str, body: &str) {
    let all = ways(std::env::consts::OS, title, body, icon().as_deref());
    std::thread::spawn(move || {
        for w in &all {
            if run(w) {
                break;
            }
        }
    });
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
        let mac = ways("macos", "land: done", "The run ended in 3 min.", None);
        assert_eq!(mac.len(), 1);
        assert_eq!(mac[0].program, "osascript");
        // (the texts as arguments, not in the script)
        assert_eq!(&mac[0].args[mac[0].args.len() - 2..], ["land: done", "The run ended in 3 min."]);
        let linux = ways("linux", "a \"b\"", "c", None);
        assert_eq!(linux.iter().map(|w| w.program.as_str()).collect::<Vec<_>>(), ["notify-send", "gdbus"]);
        assert!(linux[0].args.contains(&"a \"b\"".to_string()));
        let win = ways("windows", "t", "b", None);
        assert_eq!(win[0].program, "powershell");
        assert!(win[0].env.contains(&("TF_TITLE".to_string(), "t".to_string())));
        if cfg!(windows) {
            let script = win[0].input.as_deref().unwrap();
            assert!(script.contains("CreateToastNotifier") && script.contains(r"Classes\AppUserModelId"));
        }
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
        let all = super::ways(std::env::consts::OS, "Terraflux: notification test", "The run ended in 14 min.", None);
        assert!(all.iter().any(super::run), "no way to notify worked");
    }
}
