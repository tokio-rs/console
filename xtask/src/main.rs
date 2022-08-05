use clap::Parser;
use color_eyre::{
    eyre::{ensure, WrapErr},
    Result,
};
use std::{fs, path::PathBuf};

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

fn main() -> Result<()> {
    color_eyre::install()?;
    Args::parse().cmd.run()
}

impl Command {
    fn run(&self) -> Result<()> {
        match self {
            Self::GenProto => gen_proto(),
        }
    }
}

fn gen_proto() -> Result<()> {
    eprintln!("generating `console-api` protos...");

    let api_dir = {
        let mut mydir = PathBuf::from(std::env!("CARGO_MANIFEST_DIR"));
        ensure!(mydir.pop(), "manifest path should not be relative!");
        mydir.join("console-api")
    };

    let proto_dir = api_dir.join("proto");
    let proto_ext = std::ffi::OsStr::new("proto");
    let proto_files = fs::read_dir(&proto_dir)
        .with_context(|| {
            format!(
                "failed to read protobuf directory `{}`",
                proto_dir.display()
            )
        })?
        .filter_map(|entry| {
            (|| {
                let entry = entry?;
                if entry.file_type()?.is_dir() {
                    return Ok(None);
                }

                let path = entry.path();
                if path.extension() != Some(proto_ext) {
                    return Ok(None);
                }

                Ok(Some(path))
            })()
            .transpose()
        })
        .collect::<Result<Vec<_>>>()?;

    let out_dir = api_dir.join("src").join("generated");

    tonic_build::configure()
        .build_client(true)
        .build_server(true)
        .emit_rerun_if_changed(false)
        .protoc_arg("--experimental_allow_proto3_optional")
        .out_dir(&out_dir)
        .compile(&proto_files[..], &[proto_dir])
        .context("failed to compile protobuf files")
}
