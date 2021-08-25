use crate::view::Palette;
use clap::{ArgGroup, Clap, ValueHint};
use std::process::Command;
use tonic::transport::Uri;

#[derive(Clap, Debug)]
#[clap(
    name = clap::crate_name!(),
    author,
    about,
    version,
)]
#[deny(missing_docs)]
pub struct Config {
    /// The address of a console-enabled process to connect to.
    ///
    /// This may be an IP address and port, or a DNS name.
    #[clap(default_value = "http://127.0.0.1:6669", value_hint = ValueHint::Url)]
    pub(crate) target_addr: Uri,

    /// Log level filter for the console's internal diagnostics.
    ///
    /// The console will log to stderr if a log level filter is provided. Since
    /// the console application runs interactively, stderr should generally be
    /// redirected to a file to avoid interfering with the console's text output.
    #[clap(long = "log", env = "RUST_LOG", default_value = "off")]
    pub(crate) env_filter: tracing_subscriber::EnvFilter,

    #[clap(flatten)]
    pub(crate) view_options: ViewOptions,
}

#[derive(Clap, Debug, Clone)]
#[clap(group = ArgGroup::new("colors").conflicts_with("no-colors"))]
pub struct ViewOptions {
    /// Disable ANSI colors entirely.
    #[clap(name = "no-colors", long = "no-colors")]
    no_colors: bool,

    /// Overrides the terminal's default language.
    #[clap(long = "lang", env = "LANG")]
    lang: String,

    /// Explicitly use only ASCII characters.
    #[clap(long = "ascii-only")]
    ascii_only: bool,

    /// Overrides the value of the `COLORTERM` environment variable.
    ///
    /// If this is set to `24bit` or `truecolor`, 24-bit RGB color support will be enabled.
    #[clap(
        long = "colorterm",
        name = "truecolor",
        env = "COLORTERM",
        parse(from_str = parse_true_color),
        possible_values = &["24bit", "truecolor"],
    )]
    truecolor: Option<bool>,

    /// Explicitly set which color palette to use.
    #[clap(
        long,
        possible_values = &["8", "16", "256", "all", "off"],
        group = "colors",
        conflicts_with_all = &["no-colors", "truecolor"]
    )]
    palette: Option<Palette>,

    #[clap(flatten)]
    toggles: ColorToggles,
}

/// Toggles on and off color coding for individual UI elements.
#[derive(Clap, Debug, Copy, Clone)]
pub struct ColorToggles {
    /// Disable color-coding for duration units.
    #[clap(long = "no-duration-colors", parse(from_flag = std::ops::Not::not), group = "colors")]
    pub(crate) color_durations: bool,

    /// Disable color-coding for terminated tasks.
    #[clap(long = "no-terminated-colors", parse(from_flag = std::ops::Not::not), group = "colors")]
    pub(crate) color_terminated: bool,
}

// === impl Config ===

impl Config {
    pub fn trace_init(&mut self) -> color_eyre::Result<()> {
        let filter = std::mem::take(&mut self.env_filter);
        use tracing_subscriber::prelude::*;

        // If we're on a Linux distro with journald, try logging to the system
        // journal so we don't interfere with text output.
        let journald = tracing_journald::layer().ok();

        // Otherwise, log to stderr and rely on the user redirecting output.
        let fmt = if journald.is_none() {
            Some(
                tracing_subscriber::fmt::layer()
                    .with_writer(std::io::stderr)
                    .with_ansi(atty::is(atty::Stream::Stderr)),
            )
        } else {
            None
        };

        tracing_subscriber::registry()
            .with(journald)
            .with(fmt)
            .with(filter)
            .try_init()?;

        Ok(())
    }
}

// === impl ViewOptions ===

impl ViewOptions {
    pub fn is_utf8(&self) -> bool {
        self.lang.ends_with("UTF-8") && !self.ascii_only
    }

    /// Determines the color palette to use.
    ///
    /// The color palette is determined based on the following (in order):
    /// - Any palette explicitly set via the command-line options
    /// - The terminal's advertised support for true colors via the `COLORTERM`
    ///   env var.
    /// - Checking the `terminfo` database via `tput`
    pub(crate) fn determine_palette(&self) -> Palette {
        // Did the user explicitly disable colors?
        if self.no_colors {
            tracing::debug!("colors explicitly disabled by `--no-colors`");
            return Palette::NoColors;
        }

        // Did the user explicitly select a palette?
        if let Some(palette) = self.palette {
            tracing::debug!(?palette, "colors selected via `--palette`");
            return palette;
        }

        // Does the terminal advertise truecolor support via the COLORTERM env var?
        if self.truecolor.unwrap_or(false) {
            tracing::debug!("millions of colors enabled via `COLORTERM=truecolor`");
            return Palette::All;
        }

        // Okay, try to use `tput` to ask the terminfo database how many colors
        // are supported...
        let tput = Command::new("tput").arg("colors").output();
        tracing::debug!(?tput, "checking `tput colors`");
        if let Ok(output) = tput {
            let stdout = String::from_utf8(output.stdout);
            tracing::debug!(?stdout, "`tput colors` succeeded");
            return stdout
                .map_err(|err| tracing::warn!(%err, "`tput colors` stdout was not utf-8 (this shouldn't happen)"))
                .and_then(|s| s.parse::<Palette>().map_err(|_| tracing::warn!(palette = ?s, "invalid color palette from `tput colors`")))
                .unwrap_or_default();
        }

        Palette::NoColors
    }

    pub(crate) fn toggles(&self) -> ColorToggles {
        self.toggles
    }
}

fn parse_true_color(s: &str) -> bool {
    let s = s.trim();
    s.eq_ignore_ascii_case("truecolor") || s.eq_ignore_ascii_case("24bit")
}
