//! Builds landschaft's C++ libraries (CMake, `LANDSCHAFT_PYTHON=OFF`) and the shim between them and Rust.
//!
//! The landschaft sources are the submodule `extern/landschaft`, or the folder in `LANDSCHAFT_DIR`. A
//! local copy of Eigen can be given in `LANDSCHAFT_EIGEN_DIR` (else CMake downloads it). On macOS the OpenMP
//! runtime is Homebrew's libomp (`brew install libomp`), or the one in `OpenMP_ROOT`.

use std::path::PathBuf;

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let source = std::env::var_os("LANDSCHAFT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest.join("..").join("..").join("extern").join("landschaft"));
    if !source.join("CMakeLists.txt").exists() {
        panic!(
            "landschaft's sources were not found in {}: run `git submodule update --init` or set LANDSCHAFT_DIR",
            source.display()
        );
    }
    println!("cargo:rerun-if-env-changed=LANDSCHAFT_DIR");
    println!("cargo:rerun-if-env-changed=LANDSCHAFT_EIGEN_DIR");
    println!("cargo:rerun-if-changed={}", source.join("CMakeLists.txt").display());
    println!("cargo:rerun-if-changed={}", source.join("src").join("cpp").join("sim").display());
    println!("cargo:rerun-if-changed={}", source.join("src").join("cpp").join("cost").display());
    println!("cargo:rerun-if-changed={}", source.join("src").join("cpp").join("gtb").display());
    println!("cargo:rerun-if-changed={}", source.join("src").join("cpp").join("lsm").display());
    println!("cargo:rerun-if-changed={}", source.join("src").join("cpp").join("rest").display());
    println!("cargo:rerun-if-changed={}", source.join("src").join("cpp").join("opt").display());
    println!("cargo:rerun-if-changed={}", source.join("src").join("cpp").join("prio").display());
    println!("cargo:rerun-if-changed={}", source.join("src").join("cpp").join("surf").display());
    println!("cargo:rerun-if-changed={}", source.join("src").join("cpp").join("hydro").display());
    println!("cargo:rerun-if-changed={}", source.join("src").join("cpp").join("serv").display());

    let target = std::env::var("TARGET").unwrap();
    if target.contains("msvc") && std::env::var_os("CMAKE").is_none() {
        // the cmake crate picks the generator of the newest Visual Studio, which an older CMake on PATH may not
        // know: take the CMake that Visual Studio brings
        if let Some(cmake) = visual_studio_cmake() {
            // SAFETY: the build script is single-threaded here
            unsafe { std::env::set_var("CMAKE", cmake) };
        }
    }
    println!("cargo:rerun-if-env-changed=CMAKE");

    // Apple's clang has no OpenMP runtime of its own: the kernels and the shim are built against one libomp,
    // linked statically so that the program needs none next to it
    let openmp = target.contains("apple").then(apple_openmp);
    println!("cargo:rerun-if-env-changed=OpenMP_ROOT");

    let mut cfg = cmake::Config::new(&source);
    // the MILP prioritization too (optcore, with HiGHS)
    cfg.define("LANDSCHAFT_PYTHON", "OFF").define("LANDSCHAFT_MILP", "ON").profile("Release");
    if let Some(eigen) = std::env::var_os("LANDSCHAFT_EIGEN_DIR") {
        cfg.define("FETCHCONTENT_SOURCE_DIR_EIGEN", eigen);
    }
    if let Some(root) = &openmp {
        cfg.define("OpenMP_ROOT", root);
    }
    let dst = cfg.build();
    let include = dst.join("include");

    println!("cargo:rustc-link-search=native={}", dst.join("lib").display());
    for lib in [
        "sdmcore",
        "surfcore",
        "hydrocore",
        "servcore",
        "simcore",
        "costcore",
        "biocore",
        "circuitcore",
        "iocore",
        "lsmcore",
        "nlmcore",
        "conncore",
        "gtbcore",
        "priocore",
        "restcore",
        "optcore",
        "highs",
        "highs_extras",
    ] {
        println!("cargo:rustc-link-lib=static={lib}");
    }
    // the OpenMP runtime of the kernels
    if target.contains("msvc") {
        println!("cargo:rustc-link-lib=dylib=vcomp");
    } else if let Some(root) = &openmp {
        println!("cargo:rustc-link-search=native={}", root.join("lib").display());
        println!("cargo:rustc-link-lib=static=omp");
    } else {
        println!("cargo:rustc-link-lib=dylib=gomp");
    }

    // Eigen's headers, as landschaft's build fetched them (the sparse solves of the shim)
    let eigen = std::env::var_os("LANDSCHAFT_EIGEN_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| dst.join("build").join("_deps").join("eigen-src"));
    let mut bridge = cxx_build::bridge("src/lib.rs");
    bridge.file("shim/shim.cc").include(&include).include(&manifest).include(&eigen).std("c++17");
    bridge.define("EIGEN_MPL2_ONLY", None).define("EIGEN_DONT_PARALLELIZE", None);
    if target.contains("msvc") {
        bridge.flag("/fp:precise").flag("/EHsc").flag("/permissive-").flag("/openmp");
    } else if let Some(root) = &openmp {
        bridge.include(root.join("include"));
        bridge.flag("-ffp-contract=off").flag("-fno-fast-math").flag("-Xpreprocessor").flag("-fopenmp");
    } else {
        bridge.flag("-ffp-contract=off").flag("-fno-fast-math").flag("-fopenmp");
    }
    bridge.compile("terraflux_shim");
    println!("cargo:rerun-if-changed=shim/shim.cc");
    println!("cargo:rerun-if-changed=shim/shim.h");
    println!("cargo:rerun-if-changed=src/lib.rs");
}

/// The libomp of macOS: `OpenMP_ROOT`, else Homebrew's.
fn apple_openmp() -> PathBuf {
    let root = std::env::var_os("OpenMP_ROOT").map(PathBuf::from).or_else(|| {
        let out = std::process::Command::new("brew").args(["--prefix", "libomp"]).output().ok()?;
        out.status.success().then(|| PathBuf::from(String::from_utf8_lossy(&out.stdout).trim()))
    });
    match root {
        Some(r) if r.join("include").join("omp.h").exists() => r,
        _ => panic!("no OpenMP runtime: run `brew install libomp`, or set OpenMP_ROOT to a libomp's prefix"),
    }
}

/// The CMake of the newest Visual Studio (vswhere), if it has one.
fn visual_studio_cmake() -> Option<PathBuf> {
    let pf = std::env::var_os("ProgramFiles(x86)").or_else(|| std::env::var_os("ProgramFiles"))?;
    let vswhere = PathBuf::from(pf).join("Microsoft Visual Studio").join("Installer").join("vswhere.exe");
    let out = std::process::Command::new(vswhere)
        .args(["-latest", "-products", "*", "-property", "installationPath"])
        .output()
        .ok()?;
    let root = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if root.is_empty() {
        return None;
    }
    let cmake = PathBuf::from(root)
        .join("Common7")
        .join("IDE")
        .join("CommonExtensions")
        .join("Microsoft")
        .join("CMake")
        .join("CMake")
        .join("bin")
        .join("cmake.exe");
    cmake.exists().then_some(cmake)
}
