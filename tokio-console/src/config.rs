use crate::view::Palette;
use clap::{ArgGroup, IntoApp, Parser as Clap, Subcommand, ValueHint};
use clap_complete::Shell;
use color_eyre::eyre::WrapErr;
use serde::{Deserialize, Serialize};
use std::fmt;
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
    ///
    /// [default: http://127.0.0.1:6669]
    #[clap(value_hint = ValueHint::Url)]
    pub(crate) target_addr: Option<Uri>,

    /// Log level filter for the console's internal diagnostics.
    ///
    /// Logs are written to a new file at the path given by the `--log-dir`
    /// argument (or its default value), or to the system journal if
    /// `systemd-journald` support is enabled.
    ///
    /// If this is set to 'off' or is not set, no logs will be written.
    ///
    /// [default: off]
    #[clap(long = "log", env = "RUST_LOG")]
    pub(crate) env_filter: Option<tracing_subscriber::EnvFilter>,

    /// Path to a directory to write the console's internal logs to.
    ///
    /// [default: /tmp/tokio-console/logs]
    #[clap(long = "log-dir", value_hint = ValueHint::DirPath)]
    pub(crate) log_directory: Option<PathBuf>,

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
    ///
    /// [default: 6s]
    #[clap(long = "retain-for")]
    retain_for: Option<RetainFor>,

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

    /// Generate shell completions
    ///
    /// The completion script will be written to stdout.
    /// The completion script should be saved in the shell's completion directory.
    /// This depends on which shell is in use.
    GenCompletion {
        #[clap(name = "install", long = "install")]
        install: bool,
        #[clap(arg_enum)]
        shell: Shell,
    },
}

#[derive(Debug, Clone, Copy, Deserialize)]
struct RetainFor(Option<Duration>);

impl fmt::Display for RetainFor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            None => write!(f, ""),
            Some(duration) => write!(f, "{:?}", duration),
        }
    }
}

impl Serialize for RetainFor {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

#[derive(Clap, Debug, Clone)]
#[clap(group = ArgGroup::new("colors").conflicts_with("no-colors"))]
pub struct ViewOptions {
    /// Disable ANSI colors entirely.
    #[clap(name = "no-colors", long = "no-colors", takes_value = false)]
    no_colors: bool,

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
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
struct ConfigFile {
    default_target_addr: Option<String>,
    log: Option<String>,
    log_directory: Option<PathBuf>,
    retention: Option<RetainFor>,
    charset: Option<CharsetConfig>,
    colors: Option<ColorsConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CharsetConfig {
    lang: Option<String>,
    ascii_only: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
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
        let home = Self::from_path(ConfigPath::Home)?;
        let current = Self::from_path(ConfigPath::Current)?;
        let base = match (home, current) {
            (None, None) => None,
            (Some(home), None) => Some(home),
            (None, Some(current)) => Some(current),
            (Some(home), Some(current)) => Some(home.merge_with(current)),
        };
        let config = <Self as Clap>::parse();
        let config = match base {
            None => config,
            Some(base) => base.merge_with(config),
        };
        Ok(config)
    }

    pub fn gen_config_file(self) -> color_eyre::Result<String> {
        let defaults = Self::default().merge_with(self);
        let config: ConfigFile = defaults.into();
        toml::to_string_pretty(&config).map_err(Into::into)
    }

    pub fn trace_init(&mut self) -> color_eyre::Result<()> {
        use tracing_subscriber::prelude::*;
        let filter = match self.env_filter.take() {
            // if logging is totally disabled, don't bother even constructing
            // the subscriber
            None => return Ok(()),
            Some(filter) => filter,
        };

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

        // Otherwise, log to a file.
        let fmt = if should_fmt {
            let dir = self
                .log_directory
                .take()
                .unwrap_or_else(default_log_directory);

            // first ensure that the log directory exists
            fs::create_dir_all(&dir)
                .with_context(|| format!("creating log directory '{}'", dir.display()))?;
            color_eyre::eyre::ensure!(
                dir.is_dir(),
                "log directory path '{}' is not a directory",
                dir.display()
            );

            // now, open a log file
            let now = std::time::SystemTime::now();
            // format the current time in a way that's appropriate for a
            // filename (strip the `:` character, as it is an invalid filename
            // char on windows)
            let filename =
                format!("{}.log", humantime::format_rfc3339_seconds(now)).replace(':', "");
            let path = dir.join(filename);
            let file = fs::File::options()
                .create_new(true)
                .write(true)
                .open(&path)
                .with_context(|| format!("creating log file '{}'", path.display()))?;

            // finally, construct a `fmt` layer to write to that log file
            let fmt = tracing_subscriber::fmt::layer()
                .with_ansi(false)
                .with_writer(file);
            Some(fmt)
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
        self.retain_for.as_ref().and_then(|value| value.0)
    }

    pub(crate) fn target_addr(&self) -> Uri {
        self.target_addr
            .as_ref()
            .unwrap_or(&default_target_addr())
            .clone()
    }

    pub(crate) fn add_issue_metadata(
        &self,
        mut builder: color_eyre::config::HookBuilder,
    ) -> color_eyre::config::HookBuilder {
        macro_rules! add_issue_metadata {
            ($self:ident, $builder:ident =>
                $(
                    $($name:ident).+
                ),+
                $(,)?
            ) => {
                $(
                    $builder = $builder.add_issue_metadata(concat!("config", $(".", stringify!($name)),+), format!("`{:?}`", $self$(.$name)+));
                )*
            }
        }

        add_issue_metadata! {
            self, builder =>
                subcmd,
                target_addr,
                env_filter,
                log_directory,
                retain_for,
                view_options.no_colors,
                view_options.lang,
                view_options.ascii_only,
                view_options.truecolor,
                view_options.palette,
                view_options.toggles.color_durations,
                view_options.toggles.color_terminated,
        }

        builder
    }

    fn from_path(config_path: ConfigPath) -> color_eyre::Result<Option<Self>> {
        ConfigFile::from_path(config_path)?
            .map(|config| config.try_into())
            .transpose()
    }

    fn merge_with(self, other: Self) -> Self {
        Self {
            log_directory: other.log_directory.or(self.log_directory),
            target_addr: other.target_addr.or(self.target_addr),
            env_filter: other.env_filter.or(self.env_filter),
            retain_for: other.retain_for.or(self.retain_for),
            view_options: self.view_options.merge_with(other.view_options),
            subcmd: other.subcmd.or(self.subcmd),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            target_addr: Some(default_target_addr()),
            env_filter: Some(tracing_subscriber::EnvFilter::new("off")),
            log_directory: Some(default_log_directory()),
            retain_for: Some(RetainFor(Some(Duration::from_secs(6)))),
            view_options: ViewOptions::default(),
            subcmd: None,
        }
    }
}

fn default_target_addr() -> Uri {
    "http://127.0.0.1:6669"
        .parse::<Uri>()
        .expect("default target address should be a valid URI")
}

fn default_log_directory() -> PathBuf {
    ["/", "tmp", "tokio-console", "logs"].iter().collect()
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

    fn merge_with(self, command_line: ViewOptions) -> Self {
        Self {
            no_colors: command_line.no_colors || self.no_colors,
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
            no_colors: false,
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
    fn from_path(path: ConfigPath) -> color_eyre::Result<Option<Self>> {
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

    fn target_addr(&self) -> color_eyre::Result<Option<Uri>> {
        let uri = self
            .default_target_addr
            .as_ref()
            .map(|addr| addr.parse::<Uri>())
            .transpose()
            .wrap_err_with(|| {
                format!(
                    "failed to parse target address {:?} as URI",
                    self.default_target_addr
                )
            })?;
        Ok(uri)
    }

    fn env_filter(&self) -> color_eyre::Result<Option<tracing_subscriber::EnvFilter>> {
        let filter_str = self.log.as_deref();

        // If logging is totally disabled, may as well bail completely.
        if filter_str == Some("off") {
            return Ok(None);
        }

        let env_filter = filter_str
            .map(|directive| directive.parse::<tracing_subscriber::EnvFilter>())
            .transpose()
            .wrap_err_with(|| format!("failed to parse log filter {:?}", self.log))?;
        Ok(env_filter)
    }

    fn retain_for(&self) -> Option<RetainFor> {
        self.retention
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

impl From<Config> for ConfigFile {
    fn from(config: Config) -> Self {
        Self {
            default_target_addr: config.target_addr.map(|addr| addr.to_string()),
            log: config.env_filter.map(|filter| filter.to_string()),
            log_directory: config.log_directory,
            retention: config.retain_for,
            charset: Some(CharsetConfig {
                lang: config.view_options.lang,
                ascii_only: config.view_options.ascii_only,
            }),
            colors: Some(ColorsConfig {
                enabled: Some(!config.view_options.no_colors),
                truecolor: config.view_options.truecolor,
                palette: config.view_options.palette,
                enable: Some(config.view_options.toggles),
            }),
        }
    }
}

impl TryFrom<ConfigFile> for Config {
    type Error = color_eyre::eyre::Error;

    fn try_from(mut value: ConfigFile) -> Result<Self, Self::Error> {
        Ok(Config {
            target_addr: value.target_addr()?,
            env_filter: value.env_filter()?,
            log_directory: value.log_directory.take(),
            retain_for: value.retain_for(),
            view_options: ViewOptions {
                no_colors: value.no_colors().unwrap_or(false),
                lang: value
                    .charset
                    .as_ref()
                    .and_then(|config| config.lang.clone()),
                ascii_only: value.charset.as_ref().and_then(|config| config.ascii_only),
                truecolor: value.colors.as_ref().and_then(|config| config.truecolor),
                palette: value.colors.as_ref().and_then(|config| config.palette),
                toggles: ColorToggles {
                    color_durations: value.color_durations(),
                    color_terminated: value.color_terminated(),
                },
            },
            subcmd: None,
        })
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

/// Generete completion scripts for each specified shell.
pub fn gen_completion(install: bool, shell: Shell) -> color_eyre::Result<()> {
    let mut app = Config::command();
    let mut buf: Box<dyn std::io::Write> = if install {
        color_eyre::eyre::bail!(
            "Automatically installing completion scripts is not currently supported on {}",
            shell
        )
    } else {
        Box::new(std::io::stdout())
    };
    clap_complete::generate(shell, &mut app, "tokio-console", &mut buf);
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        env,
        fs::File,
        io::{BufWriter, Cursor, Write},
        path::{Path, PathBuf},
        process,
    };

    use super::*;

    #[test]
    fn args_example_changed() {
        use clap::CommandFactory;

        // Override env vars that may effect the defaults.
        clobber_env_vars();

        let path = PathBuf::from(std::env!("CARGO_MANIFEST_DIR")).join("args.example");

        let mut cmd = Config::command();
        let mut helptext = Vec::new();
        // Format the help text to a string.
        cmd.write_long_help(&mut Cursor::new(&mut helptext))
            .expect("generating help should succeed");
        let helptext = String::from_utf8(helptext).expect("help text is UTF-8");

        let mut file = {
            let file = File::create(&path).expect("failed to open file");
            BufWriter::new(file)
        };
        // Drop the first four lines of the help text, as they include the
        // version number, and it seems like a pain to have to re-generate the
        // file every time the version changes...
        for line in helptext.lines().skip(4) {
            writeln!(file, "{}", line).expect("writing to file succeeds");
        }

        file.flush().expect("flushing should succeed");
        drop(file);

        if let Err(diff) = git_diff(&path) {
            panic!(
                "\n/!\\ command line arguments have changed!\n\
                you should commit the new version of `{}`\n\n\
                git diff output:\n\n{}\n",
                path.display(),
                diff
            );
        }
    }

    #[test]
    // The example output includes paths, so skip this test on windows. :/
    #[cfg_attr(windows, ignore)]
    fn toml_example_changed() {
        // Override env vars that may effect the defaults.
        clobber_env_vars();

        let path = PathBuf::from(std::env!("CARGO_MANIFEST_DIR")).join("console.example.toml");

        let generated = Config::try_parse_from(std::iter::empty::<std::ffi::OsString>())
            .expect("should parse empty config")
            .gen_config_file()
            .expect("generating config file should succeed");

        File::create(&path)
            .expect("failed to open file")
            .write_all(generated.as_bytes())
            .expect("failed to write to file");
        if let Err(diff) = git_diff(&path) {
            panic!(
                "\n/!\\ default config file has changed!\n\
                you should commit the new version of `tokio-console/{}`\n\n\
                git diff output:\n\n{}\n",
                path.display(),
                diff
            );
        }
    }

    fn git_diff(path: impl AsRef<Path>) -> Result<(), String> {
        let output = process::Command::new("git")
            .arg("diff")
            .arg("--exit-code")
            .arg(format!(
                "--color={}",
                env::var("CARGO_TERM_COLOR")
                    .as_ref()
                    .map(String::as_str)
                    .unwrap_or("always")
            ))
            .arg("--")
            .arg(path.as_ref().display().to_string())
            .output()
            .unwrap();

        let diff = String::from_utf8(output.stdout).expect("git diff output not utf8");
        if output.status.success() {
            println!("git diff:\n{}", diff);
            return Ok(());
        }

        Err(diff)
    }

    /// Override any env vars that may effect the generated defaults for CLI
    /// arguments.
    fn clobber_env_vars() {
        use std::sync::Once;

        // `set_env` is unsafe in a multi-threaded environment, so ensure that
        // this only happens once...
        static ENV_VARS_CLOBBERED: Once = Once::new();

        ENV_VARS_CLOBBERED.call_once(|| {
            env::set_var("COLORTERM", "truecolor");
            env::set_var("LANG", "en_US.UTF-8");
        })
    }
}
