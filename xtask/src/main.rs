use std::{
    fmt::Write,
    fs,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
};

use clap::Parser;
use color_eyre::{
    eyre::{ensure, eyre, WrapErr},
    Result,
};
use regex::Regex;

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

    /// Check images needed for tokio-console docs.rs main page
    CheckDocsImages,
}

fn main() -> Result<()> {
    color_eyre::install()?;
    Args::parse().cmd.run()
}

impl Command {
    fn run(&self) -> Result<()> {
        match self {
            Self::GenProto => gen_proto(),
            Self::CheckDocsImages => check_docs_rs_images(),
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

    tonic_prost_build::configure()
        .build_client(true)
        .build_server(true)
        .emit_rerun_if_changed(false)
        .protoc_arg("--experimental_allow_proto3_optional")
        .out_dir(out_dir)
        .compile_protos(&proto_files[..], &[proto_dir])
        .context("failed to compile protobuf files")
}

fn check_docs_rs_images() -> Result<()> {
    eprintln!("checking images for tokio-console docs.rs page...");

    let base_dir = {
        let mut mydir = PathBuf::from(std::env!("CARGO_MANIFEST_DIR"));
        ensure!(mydir.pop(), "manifest path should not be relative!");
        mydir
    };

    let readme_path = base_dir.join("tokio-console/README.md");
    let file =
        fs::File::open(&readme_path).expect("couldn't open tokio-console README.md for reading");

    let regex_line = line!() + 1;
    let re = Regex::new(
        r"https://raw.githubusercontent.com/tokio-rs/console/main/(assets/tokio-console-[\d\.]+\/\w+\.png)",
    )
    .expect("couldn't compile regex");
    let reader = BufReader::new(file);
    let mut readme_images = Vec::new();
    for line in reader.lines() {
        let Ok(line) = line else {
            break;
        };

        let Some(image_match) = re.captures(&line) else {
            continue;
        };

        let image_path = image_match.get(1).unwrap().as_str();
        readme_images.push(image_path.to_string());
    }

    if readme_images.is_empty() {
        let regex_file = file!();
        let readme_path = readme_path.to_string_lossy();
        return Err(eyre!(
            "No images found in tokio-console README.md!\n\n\
            The README that was read is located at: {readme_path}\n\n\
            This probably means that there is a problem with the regex defined at \
            {regex_file}:{regex_line}."
        ));
    }

    let mut missing = Vec::new();
    for image_path in &readme_images {
        if !Path::new(image_path).exists() {
            missing.push(image_path.to_string());
        }
    }

    if missing.is_empty() {
        eprintln!(
            "OK: verified existance of image files in README, count: {}",
            readme_images.len()
        );

        Ok(())
    } else {
        let mut error_buffer = "Tokio console README images missing:\n".to_string();
        for path in missing {
            writeln!(&mut error_buffer, " - {path}")?;
        }

        Err(eyre!("{}", error_buffer))
    }
}
