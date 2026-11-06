//! The computer kept from sleeping while Terraflux works (a run, a batch of Many runs, this computer lent to
//! another's batch): a run left for the night does not stop when the system would sleep. The screen may still go
//! dark. On Windows by the system's call (`SetThreadExecutionState`, held by a thread of its own); on macOS by
//! `caffeinate`, and on Linux by `systemd-inhibit`, as processes that end when Terraflux lets go, or when Terraflux
//! itself ends (a crash leaves nothing behind).

/// Why the computer is kept awake, as the system's tools show it.
const WHY: &str = "Terraflux is running a model";

/// What keeps the computer awake while it is held.
pub enum Hold {
    /// The thread that asked Windows (it asks again for nothing when told to end, then ends).
    #[cfg(windows)]
    Thread(std::sync::mpsc::Sender<()>, std::thread::JoinHandle<()>),
    /// The process that keeps it (`caffeinate`, `systemd-inhibit`).
    #[allow(dead_code)]
    Child(std::process::Child),
}

impl Drop for Hold {
    fn drop(&mut self) {
        match self {
            #[cfg(windows)]
            Hold::Thread(tx, _) => {
                let _ = tx.send(());
            }
            Hold::Child(c) => {
                let _ = c.kill();
                let _ = c.wait();
            }
        }
    }
}

/// The programs and their arguments that keep this system awake while process `pid` lives, the first that the system
/// lets run (none on Windows, where a call does it, and on other systems).
pub fn commands(os: &str, pid: u32) -> Vec<(String, Vec<String>)> {
    match os {
        // -i: no idle sleep; -w: until the process ends
        "macos" => vec![("caffeinate".into(), vec!["-i".into(), "-w".into(), pid.to_string()])],
        // a lock held while `tail` waits for Terraflux to end: on idle and sleep (a desktop's session may), else on
        // idle only (the automatic sleep; without a session of its own, as in WSL, blocking sleep is refused)
        "linux" => ["idle:sleep", "idle"]
            .iter()
            .map(|what| {
                let args = [
                    format!("--what={what}"),
                    "--who=Terraflux".into(),
                    format!("--why={WHY}"),
                    "--mode=block".into(),
                    "tail".into(),
                    format!("--pid={pid}"),
                    "-f".into(),
                    "/dev/null".into(),
                ];
                ("systemd-inhibit".to_string(), args.to_vec())
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// Keep the computer awake until the hold is dropped (`attempt`: the command tried, after those refused).
pub fn hold(attempt: usize) -> Result<Hold, String> {
    #[cfg(windows)]
    {
        let _ = attempt;
        use windows_sys::Win32::System::Power::{ES_CONTINUOUS, ES_SYSTEM_REQUIRED, SetThreadExecutionState};
        let (tx, rx) = std::sync::mpsc::channel::<()>();
        let (ok_tx, ok_rx) = std::sync::mpsc::channel::<bool>();
        let t = std::thread::Builder::new()
            .name("keep awake".into())
            .spawn(move || {
                // SAFETY: a call of the system with flags only; it asks for this thread
                let before = unsafe { SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED) };
                let _ = ok_tx.send(before != 0);
                let _ = rx.recv();
                // SAFETY: as above; the request let go
                unsafe { SetThreadExecutionState(ES_CONTINUOUS) };
            })
            .map_err(|e| e.to_string())?;
        if ok_rx.recv().unwrap_or(false) {
            Ok(Hold::Thread(tx, t))
        } else {
            Err("Windows refused to keep the computer awake".into())
        }
    }
    #[cfg(not(windows))]
    {
        let os = std::env::consts::OS;
        let all = commands(os, std::process::id());
        let (program, args) = all.get(attempt).ok_or(format!("not done on {os}"))?;
        let child = std::process::Command::new(program)
            .args(args)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| format!("{program} could not be started: {e}"))?;
        Ok(Hold::Child(child))
    }
}

/// The hold taken while Terraflux works and the setting allows, let go when it stops; why it could not be taken.
#[derive(Default)]
pub struct Awake {
    held: Option<Hold>,
    /// The command tried now (the ones before it refused).
    attempt: usize,
    /// Why the hold could not be taken (not asked again until the work stops).
    pub failed: Option<String>,
}

impl Awake {
    /// Once a frame: held while `working`, let go after.
    pub fn want(&mut self, working: bool) {
        if !working {
            self.held = None;
            self.failed = None;
            self.attempt = 0;
            return;
        }
        if self.held.is_none() && self.failed.is_none() {
            match hold(self.attempt) {
                Ok(h) => self.held = Some(h),
                Err(e) => self.failed = Some(e),
            }
        }
        // (a process that ended at once: the system refused it, as systemd-inhibit without the right)
        if let Some(Hold::Child(c)) = &mut self.held
            && let Ok(Some(status)) = c.try_wait()
        {
            self.held = None;
            self.attempt += 1;
            // (the next command tried at the next frame, if there is one)
            if self.attempt >= commands(std::env::consts::OS, 0).len() {
                self.failed = Some(format!("the system did not let it keep the computer awake ({status})"));
            }
        }
    }

    /// Whether the computer is kept awake now.
    pub fn held(&self) -> bool {
        self.held.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_commands_of_each_system() {
        let mac = commands("macos", 42);
        assert_eq!(mac, vec![("caffeinate".to_string(), vec!["-i".to_string(), "-w".into(), "42".into()])]);
        let linux = commands("linux", 42);
        assert_eq!(linux.len(), 2, "idle and sleep, else idle only");
        for ((p, a), what) in linux.iter().zip(["idle:sleep", "idle"]) {
            assert_eq!(p, "systemd-inhibit");
            assert!(a.contains(&format!("--what={what}")) && a.contains(&"--mode=block".to_string()));
            // (the lock ends with Terraflux: tail waits for its process)
            assert_eq!(&a[a.len() - 4..], ["tail", "--pid=42", "-f", "/dev/null"]);
        }
        assert!(commands("windows", 42).is_empty());
    }

    #[test]
    fn held_while_working_and_let_go_after() {
        let mut a = Awake::default();
        a.want(false);
        assert!(!a.held() && a.failed.is_none());
        // (a command refused ends at once: the next one tried a few frames later)
        for _ in 0..20 {
            a.want(true);
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        // (on Linux without systemd-inhibit, as some containers: told why, not held)
        assert!(a.held() || a.failed.is_some(), "held, or why not");
        if cfg!(windows) {
            assert!(a.held(), "{:?}", a.failed);
        }
        a.want(true);
        a.want(false);
        assert!(!a.held() && a.failed.is_none());
    }
}
