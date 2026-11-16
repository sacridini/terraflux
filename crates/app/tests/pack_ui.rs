//! The model packed with its data from the File menu, and the package opened (a part of the `ui` tests).

use super::*;

/// *File > Pack for sharing...* writes the package beside the model; a package opened (as when it is dropped on the
/// window) is unpacked, checked, and its model opened from the folder it was unpacked into.
#[test]
fn a_model_is_packed_from_the_file_menu_and_the_package_opened() {
    let dir = tmp("pack-ui");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut w = Wizard::default();
    w.landuse_picked(Picked::read(&data("sibuyan_landuse.tif")).unwrap());
    w.drivers = Some(Picked::read(&data("sibuyan_drivers.tif")).unwrap());
    w.increment = Some(250.0);
    let file = w.save(&dir.join("ui-pack.toml")).unwrap();
    let mut h = harness(new_app(Some(file.clone())));
    loaded(&mut h);
    h.run_steps(2);
    h.get_by_label("File").click();
    h.run_steps(2);
    h.get_by_label("Pack for sharing...").click();
    h.run_steps(3);
    assert!(h.state().pack.is_some(), "the window opened");
    assert!(h.query_by_label_contains("files,").is_some(), "what is packed, told before it is written");
    shot(&mut h, "pack-1");
    let package = dir.join("ui-pack.tfpack");
    h.get_by_label("Pack").click();
    until(&mut h, "the package", |_| package.is_file());
    h.run_steps(3);
    assert!(h.query_by_label_contains("wrote").is_some());
    h.state_mut().pack = None;
    // opened: unpacked beside it, named like it, and the model opened from there
    h.state_mut().pack = Some(terraflux_app::pack_view::State::open_package(&package, true));
    let folder = dir.join("ui-pack");
    until(&mut h, "the package opened", |a| {
        a.pack.is_none() && a.session.as_ref().and_then(|s| s.path.clone()).is_some_and(|p| p.starts_with(&folder))
    });
    assert!(folder.join("manifest.json").is_file() && folder.join("ui-pack.toml").is_file());
    let unpacked = terraflux_graph::Model::load(folder.join("ui-pack.toml")).unwrap();
    for (name, i) in &unpacked.inputs {
        if let terraflux_graph::Input::Map { path, .. } | terraflux_graph::Input::Stack { path, .. } = i {
            assert!(folder.join(path).is_file(), "{name}: {path}");
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}
