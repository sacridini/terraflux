//! How long the computer's user has been away (no key pressed, the mouse not moved): a worker that lends a
//! computer may take tasks only then. Known on Windows (`GetLastInputInfo`) and macOS (CoreGraphics); elsewhere
//! unknown, and the worker works whenever it is asked.

/// The seconds since the user last used the keyboard or the mouse (none: not known here).
pub fn seconds() -> Option<f64> {
    imp::seconds()
}

#[cfg(windows)]
mod imp {
    #[repr(C)]
    struct LastInputInfo {
        size: u32,
        time: u32,
    }

    #[link(name = "user32")]
    unsafe extern "system" {
        fn GetLastInputInfo(info: *mut LastInputInfo) -> i32;
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetTickCount() -> u32;
    }

    pub fn seconds() -> Option<f64> {
        let mut info = LastInputInfo { size: std::mem::size_of::<LastInputInfo>() as u32, time: 0 };
        // SAFETY: the struct is the one the call fills, its size set
        let (ok, now) = unsafe { (GetLastInputInfo(&mut info), GetTickCount()) };
        (ok != 0).then(|| f64::from(now.wrapping_sub(info.time)) / 1000.0)
    }
}

#[cfg(target_os = "macos")]
mod imp {
    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGEventSourceSecondsSinceLastEventType(state: i32, event: u32) -> f64;
    }

    pub fn seconds() -> Option<f64> {
        // the combined session's state, any input event
        // SAFETY: a call without pointers
        let s = unsafe { CGEventSourceSecondsSinceLastEventType(0, u32::MAX) };
        s.is_finite().then_some(s)
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
mod imp {
    pub fn seconds() -> Option<f64> {
        None
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn known_where_it_can_be() {
        let s = super::seconds();
        if cfg!(any(windows, target_os = "macos")) {
            assert!(s.is_some_and(|x| x >= 0.0), "{s:?}");
        }
    }
}
