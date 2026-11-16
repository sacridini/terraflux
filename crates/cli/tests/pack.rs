//! `terraflux pack` and `terraflux unpack`: a model packed with its data in one file, unpacked and checked.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn folder(name: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("pack-{name}"));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn write(file: &Path, text: &str) {
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(file, text).unwrap();
}

fn terraflux(args: &[&Path]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_terraflux")).args(args).env("TERRAFLUX_THREADS", "4").output().unwrap()
}

fn text(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

fn model(d: &Path, other: &str) -> PathBuf {
    write(&d.join("a/land.tif"), "first");
    write(&d.join("b/land.tif"), "second");
    write(&d.join("gis/roads.shp"), "shp");
    write(&d.join("gis/roads.shx"), "shx");
    write(&d.join("gis/roads.dbf"), "dbf");
    write(&d.join("output/result.tif"), "last");
    let file = d.join("model.toml");
    write(
        &file,
        &format!(
            "format = \"terraflux/1\"\n\
             [[nodes]]\nid = \"a\"\nop = \"raster.read\"\nparams = {{ path = \"a/land.tif\" }}\n\
             [[nodes]]\nid = \"b\"\nop = \"raster.read\"\nparams = {{ path = \"b/land.tif\" }}\n\
             [[nodes]]\nid = \"v\"\nop = \"vector.read\"\nparams = {{ path = \"gis/roads.shp\" }}\n\
             [[nodes]]\nid = \"o\"\nop = \"raster.read\"\nparams = {{ path = \"{other}\" }}\n"
        ),
    );
    file
}

#[test]
fn a_model_is_packed_and_unpacked_checked() {
    let d = folder("whole");
    let file = model(&d, "a/land.tif");
    let package = d.join("study.tfpack");
    let o = terraflux(&[Path::new("pack"), &file, Path::new("-o"), &package, Path::new("--outputs")]);
    assert!(o.status.success(), "{}", text(&o.stderr));
    assert!(text(&o.stdout).contains("wrote"), "{}", text(&o.stdout));
    assert!(text(&o.stderr).contains("files"), "the total told before it is written: {}", text(&o.stderr));
    let out = d.join("there");
    let o = terraflux(&[Path::new("unpack"), &package, Path::new("-o"), &out]);
    assert!(o.status.success(), "{}", text(&o.stderr));
    for f in [
        "model.toml",
        "manifest.json",
        "data/land.tif",
        "data/2/land.tif",
        "data/roads.shp",
        "data/roads.shx",
        "data/roads.dbf",
        "outputs/result.tif",
    ] {
        assert!(out.join(f).is_file(), "{f}");
    }
    assert_eq!(std::fs::read_to_string(out.join("data/2/land.tif")).unwrap(), "second");
    let m = std::fs::read_to_string(out.join("model.toml")).unwrap();
    assert!(m.contains("data/2/land.tif") && !m.contains(&d.to_string_lossy().replace('\\', "/")), "{m}");
    // by default the folder is beside the package, named like it
    let o = terraflux(&[Path::new("unpack"), &package]);
    assert!(o.status.success(), "{}", text(&o.stderr));
    assert!(d.join("study/model.toml").is_file());
    // a package changed after it was made is told, with a status that fails
    let mut bytes = std::fs::read(&package).unwrap();
    let at = bytes.len() / 3;
    bytes[at] ^= 0x5a;
    let bad = d.join("bad.tfpack");
    std::fs::write(&bad, bytes).unwrap();
    let o = terraflux(&[Path::new("unpack"), &bad, Path::new("-o"), &d.join("bad-out")]);
    assert!(!o.status.success(), "{}", text(&o.stdout));
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn a_file_that_is_not_there_is_listed_and_fails_the_status() {
    let d = folder("missing");
    let file = model(&d, "nowhere/none.tif");
    let package = d.join("m.tfpack");
    let o = terraflux(&[Path::new("pack"), &file, Path::new("-o"), &package]);
    assert_eq!(o.status.code(), Some(1), "{}", text(&o.stderr));
    assert!(text(&o.stderr).contains("nowhere/none.tif"), "{}", text(&o.stderr));
    assert!(package.is_file(), "the package is written all the same");
    let _ = std::fs::remove_dir_all(&d);
}
