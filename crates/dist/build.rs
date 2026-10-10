//! The gRPC code of `proto/terraflux.proto`, with the protoc of `protoc-bin-vendored`.

fn main() {
    let protoc = protoc_bin_vendored::protoc_bin_path().expect("no protoc for this platform");
    // SAFETY: the build script is single-threaded
    unsafe { std::env::set_var("PROTOC", protoc) };
    tonic_prost_build::configure()
        .compile_protos(&["proto/terraflux.proto"], &["proto"])
        .expect("compiling the protocol");
    println!("cargo:rerun-if-changed=proto/terraflux.proto");
}
