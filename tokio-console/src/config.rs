use crate::view::Palette;
use clap::{ArgGroup, Parser as Clap, Subcommand, ValueHint};
use color_eyre::eyre::WrapErr;
use serde::{Deserialize, Serialize};
use std::fs;
use std::ops::Not;
use std::path::PathBuf;
use std::process::Command;
use std::str::FromStr;
use std::time::Duration;
use tonic::transport::Uri;

#[derive(Clap, Debug)]
#[clap(
    name = clap::crate_name!(),
    author,
    about,
    version,
    propagate_version = true,
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

    /// How long to continue displaying completed tasks and dropped resources
    /// after they have been closed.
    ///
    /// This accepts either a duration, parsed as a combination of time spans
    /// (such as `5days 2min 2s`), or `none` to disable removing completed tasks
    /// and dropped resources.
    ///
    /// Each time span is an integer number followed by a suffix. Supported suffixes are:
    ///
    /// * `nsec`, `ns` -- nanoseconds
    ///
    /// * `usec`, `us` -- microseconds
    ///
    /// * `msec`, `ms` -- milliseconds
    ///
    /// * `seconds`, `second`, `sec`, `s`
    ///
    /// * `minutes`, `minute`, `min`, `m`
    ///
    /// * `hours`, `hour`, `hr`, `h`
    ///
    /// * `days`, `day`, `d`
    ///
    /// * `weeks`, `week`, `w`
    ///
    /// * `months`, `month`, `M` -- defined as 30.44 days
    ///
    /// * `years`, `year`, `y` -- defined as 365.25 days
    #[clap(long = "retain-for", default_value = "6s")]
    retain_for: RetainFor,

    /// An optional subcommand.
    ///
    /// If one of these is present, the console CLI will do something other than
    /// attempting to connect to a remote server.
    #[clap(subcommand)]
    pub subcmd: Option<OptionalCmd>,
}

#[derive(Debug, Subcommand, PartialEq, Eq)]
pub enum OptionalCmd {
    /// Generate a `console.toml` config file with the default configuration
    /// values, overridden by any provided command-line arguments.
    ///
    /// By default, the config file is printed to stdout. It can be redirected
    /// to a file to generate an new configuration file:
    ///
    ///
    ///     $ tokio-console gen-config > console.toml
    ///
    GenConfig,
}

#[derive(Debug)]
struct RetainFor(Option<Duration>);

#[derive(Clap, Debug, Clone)]
#[clap(group = ArgGroup::new("colors").conflicts_with("no-colors"))]
pub struct ViewOptions {
    /// Disable ANSI colors entirely.
    #[clap(name = "no-colors", long = "no-colors")]
    no_colors: Option<bool>,

    /// Overrides the terminal's default language.
    #[clap(long = "lang", env = "LANG")]
    lang: Option<String>,

    /// Explicitly use only ASCII characters.
    #[clap(long = "ascii-only")]
    ascii_only: Option<bool>,

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
#[derive(Clap, Debug, Copy, Clone, Deserialize, Serialize)]
pub struct ColorToggles {
    /// Disable color-coding for duration units.
    #[clap(long = "no-duration-colors", group = "colors")]
    #[serde(rename = "durations")]
    color_durations: Option<bool>,

    /// Disable color-coding for terminated tasks.
    #[clap(long = "no-terminated-colors", group = "colors")]
    #[serde(rename = "terminated")]
    color_terminated: Option<bool>,
}

/// A sturct used to parse the toml config file
#[derive(Debug, Clone, Deserialize, Serialize)]
struct ConfigFile {
    charset: Option<CharsetConfig>,
    colors: Option<ColorsConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct CharsetConfig {
    lang: Option<String>,
    ascii_only: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct ColorsConfig {
    enabled: Option<bool>,
    truecolor: Option<bool>,
    palette: Option<Palette>,
    enable: Option<ColorToggles>,
}

// === impl Config ===

impl Config {
    /// Parse from config files and command line options.
    pub fn parse() -> color_eyre::Result<Self> {
        let home = ViewOptions::from_config(ConfigPath::Home)?;
        let current = ViewOptions::from_config(ConfigPath::Current)?;
        let base = match (home, current) {
            (None, None) => None,
            (Some(home), None) => Some(home),
            (None, Some(current)) => Some(current),
            (Some(home), Some(current)) => Some(home.merge_with(current)),
        };
        let mut config = <Self as Clap>::parse();
        let view_options = match base {
            None => config.view_options,
            Some(base) => base.merge_with(config.view_options),
        };
        config.view_options = view_options;
        Ok(config)
    }

    pub fn gen_config_file(self) -> color_eyre::Result<String> {
        let defaults = ViewOptions::default().merge_with(self.view_options);
        let config = ConfigFile::from_view_options(defaults);
        toml::to_string_pretty(&config).map_err(Into::into)
    }

    pub fn trace_init(&mut self) -> color_eyre::Result<()> {
        let filter = std::mem::take(&mut self.env_filter);
        use tracing_subscriber::prelude::*;

        // If we're on a Linux distro with journald, try logging to the system
        // journal so we don't interfere with text output.
        #[cfg(all(feature = "tracing-journald", target_os = "linux"))]
        let (journald, should_fmt) = {
            let journald = tracing_journald::layer().ok();
            let should_fmt = journald.is_none();
            (journald, should_fmt)
        };

        #[cfg(not(all(feature = "tracing-journald", target_os = "linux")))]
        let should_fmt = true;

        // Otherwise, log to stderr and rely on the user redirecting output.
        let fmt = if should_fmt {
            Some(
                tracing_subscriber::fmt::layer()
                    .with_writer(std::io::stderr)
                    .with_ansi(atty::is(atty::Stream::Stderr)),
            )
        } else {
            None
        };

        let registry = tracing_subscriber::registry().with(fmt).with(filter);

        #[cfg(all(feature = "tracing-journald", target_os = "linux"))]
        let registry = registry.with(journald);

        registry.try_init()?;

        Ok(())
    }

    pub(crate) fn retain_for(&self) -> Option<Duration> {
        self.retain_for.0
    }
}

// === impl ViewOptions ===

impl ViewOptions {
    pub fn is_utf8(&self) -> bool {
        if !self.ascii_only.unwrap_or(true) {
            return false;
        }
        self.lang.as_deref().unwrap_or_default().ends_with("UTF-8")
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
        if self.no_colors.unwrap_or(true) {
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
                .and_then(|s| {
                    let palette = s.parse::<Palette>();
                    tracing::debug!(?palette, "parsed `tput colors`");
                    palette.map_err(|_| tracing::warn!(palette = ?s, "invalid color palette from `tput colors`"))
                })
                .unwrap_or_default();
        }

        Palette::NoColors
    }

    pub(crate) fn toggles(&self) -> ColorToggles {
        self.toggles
    }

    fn from_config(path: ConfigPath) -> color_eyre::Result<Option<Self>> {
        let options = ConfigFile::from_config(path)?.map(|config| config.into_view_options());
        Ok(options)
    }

    fn merge_with(self, command_line: ViewOptions) -> Self {
        Self {
            no_colors: command_line.no_colors.or(self.no_colors),
            lang: command_line.lang.or(self.lang),
            ascii_only: command_line.ascii_only.or(self.ascii_only),
            truecolor: command_line.truecolor.or(self.truecolor),
            palette: command_line.palette.or(self.palette),
            toggles: ColorToggles {
                color_durations: command_line
                    .toggles
                    .color_durations
                    .or(self.toggles.color_durations),
                color_terminated: command_line
                    .toggles
                    .color_terminated
                    .or(self.toggles.color_terminated),
            },
        }
    }
}

impl Default for ViewOptions {
    fn default() -> Self {
        Self {
            no_colors: Some(false),
            lang: Some("en_us.UTF8".to_string()),
            ascii_only: Some(false),
            truecolor: Some(true),
            palette: Some(Palette::All),
            toggles: ColorToggles {
                color_durations: Some(true),
                color_terminated: Some(true),
            },
        }
    }
}

fn parse_true_color(s: &str) -> bool {
    let s = s.trim();
    s.eq_ignore_ascii_case("truecolor") || s.eq_ignore_ascii_case("24bit")
}

impl FromStr for RetainFor {
    type Err = humantime::DurationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            s if s.eq_ignore_ascii_case("none") => Ok(RetainFor(None)),
            _ => s
                .parse::<humantime::Duration>()
                .map(|duration| RetainFor(Some(duration.into()))),
        }
    }
}

// === impl ColorToggles ===

impl ColorToggles {
    /// Return true when disabling color-coding for duration units.
    pub fn color_durations(&self) -> bool {
        self.color_durations.map(Not::not).unwrap_or(true)
    }

    /// Return true when disabling color-coding for terminated tasks.
    pub fn color_terminated(&self) -> bool {
        self.color_durations.map(Not::not).unwrap_or(true)
    }
}

// === impl ColorToggles ===

impl ConfigFile {
    fn from_config(path: ConfigPath) -> color_eyre::Result<Option<Self>> {
        let config = path
            .into_path()
            .and_then(|path| fs::read_to_string(path).ok())
            .map(|raw| toml::from_str::<ConfigFile>(&raw))
            .transpose()
            .wrap_err_with(|| {
                format!(
                    "failed to parse {}",
                    path.into_path().unwrap_or_default().display()
                )
            })?;
        Ok(config)
    }

    fn into_view_options(self) -> ViewOptions {
        ViewOptions {
            no_colors: self.no_colors(),
            lang: self.charset.as_ref().and_then(|config| config.lang.clone()),
            ascii_only: self.charset.as_ref().and_then(|config| config.ascii_only),
            truecolor: self.colors.as_ref().and_then(|config| config.truecolor),
            palette: self.colors.as_ref().and_then(|config| config.palette),
            toggles: ColorToggles {
                color_durations: self.color_durations(),
                color_terminated: self.color_terminated(),
            },
        }
    }

    fn from_view_options(view_options: ViewOptions) -> Self {
        Self {
            charset: Some(CharsetConfig {
                lang: view_options.lang,
                ascii_only: view_options.ascii_only,
            }),
            colors: Some(ColorsConfig {
                enabled: view_options.no_colors.map(Not::not),
                truecolor: view_options.truecolor,
                palette: view_options.palette,
                enable: Some(view_options.toggles),
            }),
        }
    }

    fn no_colors(&self) -> Option<bool> {
        self.colors
            .as_ref()
            .and_then(|config| config.enabled.map(Not::not))
    }

    fn color_durations(&self) -> Option<bool> {
        self.colors
            .as_ref()
            .and_then(|config| config.enable.map(|toggles| toggles.color_durations()))
    }

    fn color_terminated(&self) -> Option<bool> {
        self.colors
            .as_ref()
            .and_then(|config| config.enable.map(|toggles| toggles.color_terminated()))
    }
}

#[derive(Debug, Clone, Copy)]
enum ConfigPath {
    Home,
    Current,
}

impl ConfigPath {
    fn into_path(self) -> Option<PathBuf> {
        match self {
            Self::Home => {
                let mut path = dirs::config_dir();
                if let Some(path) = path.as_mut() {
                    path.push("tokio-console/console.toml");
                }
                path
            }
            Self::Current => {
                let mut path = PathBuf::new();
                path.push("./console.toml");
                Some(path)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{ffi::OsString, fs::File, io::Write, process};

    use super::*;

    #[test]
    fn toml_example_changed() {
        const PATH: &str = "../console.example.toml";
        // Don't parse the locale or terminal color settings from the user's
        // environment, as this may differ from the one in the example.
        let defaults = Config::try_parse_from(vec![
            OsString::from("--lang"),
            OsString::from("en_US.UTF-8"),
            OsString::from("--colorterm"),
            OsString::from("truecolor"),
        ])
        .expect("defaults should parse");

        let generated = defaults
            .gen_config_file()
            .expect("generating config file should succeed");

        File::create(PATH)
            .expect("failed to open file")
            .write_all(generated.as_bytes())
            .expect("failed to write to file");
        if let Err(diff) = git_diff(PATH) {
            panic!(
                "default config file has changed!\n\
                you should commit the new version of `tokio-console/console.example.toml`\n\n\
                git diff output:\n{}",
                diff
            );
        }
    }

    fn git_diff(path: &str) -> Result<(), String> {
        let output = process::Command::new("git")
            .arg("diff")
            .arg("--exit-code")
            .arg("--")
            .arg(path)
            .output()
            .unwrap();

        if output.status.success() {
            return Ok(());
        }

        let diff = String::from_utf8(output.stdout).expect("git diff output not utf8");

        Err(diff)
    }
}
