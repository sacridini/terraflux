//! The computer kept awake while tasks run, so that it does not go to sleep half way through a batch: while
//! anything holds an [`Awake`], Windows is told every 20 seconds that the system is needed
//! (`SetThreadExecutionState`), macOS runs `caffeinate` and Linux `systemd-inhibit`. The screen may still turn
//! off.

#[cfg(not(windows))]
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

static HOLDERS: AtomicUsize = AtomicUsize::new(0);

/// While it lives, the computer does not sleep.
pub struct Awake(());

impl Awake {
    pub fn hold() -> Awake {
        if HOLDERS.fetch_add(1, Ordering::SeqCst) == 0 {
            start();
        }
        Awake(())
    }

    /// Whether anything keeps the computer awake now.
    pub fn held() -> bool {
        HOLDERS.load(Ordering::SeqCst) > 0
    }
}

impl Drop for Awake {
    fn drop(&mut self) {
        if HOLDERS.fetch_sub(1, Ordering::SeqCst) == 1 {
            stop();
        }
    }
}

#[cfg(windows)]
fn start() {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn SetThreadExecutionState(flags: u32) -> u32;
    }
    const ES_SYSTEM_REQUIRED: u32 = 0x1;
    static TICKING: std::sync::Once = std::sync::Once::new();
    // a thread that resets the system's idle timer while anything holds it
    TICKING.call_once(|| {
        std::thread::spawn(|| {
            loop {
                if Awake::held() {
                    // SAFETY: a plain Win32 call without pointers
                    unsafe { SetThreadExecutionState(ES_SYSTEM_REQUIRED) };
                }
                std::thread::sleep(std::time::Duration::from_secs(20));
            }
        });
    });
}

#[cfg(windows)]
fn stop() {}

#[cfg(not(windows))]
static CHILD: Mutex<Option<std::process::Child>> = Mutex::new(None);

#[cfg(not(windows))]
fn start() {
    let pid = std::process::id().to_string();
    let child = if cfg!(target_os = "macos") {
        std::process::Command::new("caffeinate").args(["-i", "-w", &pid]).spawn()
    } else {
        std::process::Command::new("systemd-inhibit")
            .args(["--what=idle:sleep", "--who=Terraflux", "--why=tasks running", "--mode=block", "sleep", "infinity"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
    };
    if let Ok(c) = child {
        *CHILD.lock().unwrap_or_else(|e| e.into_inner()) = Some(c);
    }
}

#[cfg(not(windows))]
fn stop() {
    if let Some(mut c) = CHILD.lock().unwrap_or_else(|e| e.into_inner()).take() {
        let _ = c.kill();
        let _ = c.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn held_while_anything_holds_it() {
        let a = Awake::hold();
        let b = Awake::hold();
        drop(a);
        assert!(Awake::held());
        drop(b);
    }
}
