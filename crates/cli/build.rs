//! On Windows terraflux.exe carries the application's icon (assets/logo/terraflux.ico): Explorer, the taskbar and
//! the Start menu's shortcut show it. Elsewhere nothing is done.

use std::path::Path;

fn main() {
    // the folder read when the script runs, not when it is built: the worktrees beside this one share `target`,
    // and a script built in one of them runs for the others
    let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("cargo's CARGO_MANIFEST_DIR");
    let root = Path::new(&manifest).parent().and_then(Path::parent).expect("the workspace");
    let icon = root.join("assets").join("logo").join("terraflux.ico");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={}", icon.display());
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon(icon.to_str().expect("a path in UTF-8"));
        res.compile().expect("the icon in terraflux.exe (rc.exe of the Windows SDK)");
    }
}
