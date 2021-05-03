use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let iface_files = &[
        "../proto/trace.proto",
        "../proto/common.proto",
        "../proto/tasks.proto",
    ];
    let dirs = &["../proto"];

    tonic_build::configure()
        .build_client(true)
        .build_server(true)
        .protoc_arg("--experimental_allow_proto3_optional")
        .compile(iface_files, dirs)?;

    // recompile protobufs only if any of the proto files changes.
    for file in iface_files {
        println!("cargo:rerun-if-changed={}", file);
    }

    Ok(())
}
