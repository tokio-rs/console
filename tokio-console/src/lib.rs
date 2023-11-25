// this file is here to make a binary target so that cargo metadata works with this crate
#![doc = include_str!("../README.md")]

/// # Configuration Reference
///
/// `tokio-console`'s behavior can be configured in two ways: via command-line
/// arguments, or using a [TOML] config file.
///
/// ## Command-Line Arguments
///
/// The following is the complete list of command-line arguments accepted by
/// `tokio-console`:
///
/// ```text
#[doc = include_str!("../tests/ui/cli-ui.stdout")]
/// ```
///
/// This text can also be displayed by running `tokio-console help`.
///
///
/// ## Configuration File
///
/// In addition to command-line arguments, the console can also be configured by a
/// [TOML] configuration file. All settings that can be configured by the
/// command line (with the exception of the target address to connect to) can
/// also be set by the config file.
///
/// The `tokio-console gen-config` subcommand generates a config file based on
/// the default configuration, overridden by any command-line arguments passed
/// by the user.
///
/// ### Examples
///
/// The default configuration:
///
/// ```toml
#[doc = include_str!("../console.example.toml")]
/// ```
///
/// ### Config File Locations
///
/// Configuration files are read from two locations:
///
/// 1. A `tokio-console` directory in the system default configuration
///    directory (as determined by the  [`dirs` crate]). This directory depends
///    on the operating system:
///
///    |Platform | Value                                                             |
///    | ------- | ----------------------------------------------------------------- |
///    | Linux   | `$XDG_CONFIG_HOME/tokio-console` or `$HOME/.config/tokio-console` |
///    | macOS   | `$HOME/Library/Application Support/tokio-console`                 |
///    | Windows | `{FOLDERID_RoamingAppData}\tokio-console`                         |
///
/// 2. The current working directory.
///
///    If both the current working directory *and* the system default config directory
///    contain a `console.toml` file, any values set in the current working directory
///    will override those set in the system config directory. This allows overriding
///    the user-level default configuration with project specific configurations. Some
///    projects may wish to check project-specific configurations into source control
///    so that they may be shared by multiple developers.
///
/// Any command-line arguments will override the configuration set in both config files.
///
/// [TOML]: https://github.com/toml-lang/toml
/// [`dirs` crate]: https://docs.rs/dirs/4.0.0/dirs/fn.config_dir.html
pub mod config_reference {
    // empty module, used only for documentation
}
