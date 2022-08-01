use clap::Parser;
use std::path::PathBuf;

/// tokio-console dev tasks
#[derive(Debug, clap::Parser)]
struct Args {
    #[clap(subcommand)]
    cmd: Command,
}

#[derive(Debug, clap::Subcommand)]
enum Command {
    /// Generate `console-api` protobuf bindings.
    GenProto,
}

fn main() {
    let args = Args::parse();
    if let Err(error) = args.cmd.run() {
        eprintln!("{error}");
        std::process::exit(1)
    }
}

impl Command {
    fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        match self {
            Self::GenProto => gen_proto(),
        }
    }
}

fn gen_proto() -> Result<(), Box<dyn std::error::Error>> {
    eprintln!("generating `console-api` protos...");

    let api_dir = {
        let mut mydir = PathBuf::from(std::env!("CARGO_MANIFEST_DIR"));
        assert!(mydir.pop(), "manifest path should not be relative!");
        mydir.join("console-api")
    };

    let proto_dir = api_dir.join("proto");
    let out_dir = api_dir.join("src").join("generated");

    let iface_files = &[
        proto_dir.join("trace.proto"),
        proto_dir.join("common.proto"),
        proto_dir.join("tasks.proto"),
        proto_dir.join("instrument.proto"),
        proto_dir.join("resources.proto"),
        proto_dir.join("async_ops.proto"),
    ];

    tonic_build::configure()
        .build_client(true)
        .build_server(true)
        .protoc_arg("--experimental_allow_proto3_optional")
        .out_dir(format!("{}", out_dir.display()))
        .compile(iface_files, &[proto_dir])?;

    eprintln!("protos regenerated!");

    Ok(())
}
