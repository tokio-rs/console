use crate::config;
use serde::{Deserialize, Serialize};
use std::{str::FromStr, time::Duration};
use ratatui::{
    style::{Color, Modifier, Style},
    text::Span,
};

#[derive(Debug, Clone)]
pub struct Styles {
    palette: Palette,
    toggles: config::ColorToggles,
    pub(crate) utf8: bool,
}

#[derive(Debug, PartialEq, Eq, Copy, Clone, Deserialize, Serialize)]
#[repr(u8)]
pub enum Palette {
    #[serde(rename = "off")]
    NoColors,
    /// Use ANSI 8 color palette only.
    #[serde(rename = "8")]
    Ansi8,
    /// Use ANSI 16 color palette only.
    #[serde(rename = "16")]
    Ansi16,
    /// Enable ANSI 256-color palette.
    #[serde(rename = "256")]
    Ansi256,
    /// Enable all RGB colors.
    #[serde(rename = "all")]
    All,
}

/// Represents formatted time spans.
///
/// Distinguishing between different units allows appropriate colouring.
enum FormattedDuration {
    /// Days (and no minor unit), e.g. `102d`
    Days(String),
    /// Days with hours, e.g. `12d03h`
    DaysHours(String),
    /// Hours with minutes, e.g. `14h32m`
    HoursMinutes(String),
    /// Minutes with seconds, e.g. `43m02s`
    MinutesSeconds(String),
    /// The `time::Duration` debug string which uses units ranging from
    /// picoseconds (`ps`) to seconds (`s`). May contain decimal digits
    /// (e.g. `628.76ms`) or not (e.g. `32ns`)
    Debug(String),
}

impl FormattedDuration {
    fn into_inner(self) -> String {
        match self {
            Self::Days(inner) => inner,
            Self::DaysHours(inner) => inner,
            Self::HoursMinutes(inner) => inner,
            Self::MinutesSeconds(inner) => inner,
            Self::Debug(inner) => inner,
        }
    }
}

fn fg_style(color: Color) -> Style {
    Style::default().fg(color)
}

// === impl Styles ===

impl Styles {
    pub fn from_config(config: config::ViewOptions) -> Self {
        Self {
            palette: config.determine_palette(),
            toggles: config.toggles(),
            utf8: config.is_utf8(),
        }
    }

    pub fn error_init(&self, cfg: &crate::config::Config) -> color_eyre::Result<()> {
        use color_eyre::{
            config::{HookBuilder, Theme},
            ErrorKind,
        };

        let mut builder = HookBuilder::new()
            .issue_url(concat!(env!("CARGO_PKG_REPOSITORY"), "/issues/new"))
            .issue_filter(|kind| match kind {
                // Only suggest reporting GitHub issues for panics, not for
                // errors, so people don't open GitHub issues for stuff like not
                // being able to find a config file or connections being
                // terminated by remote hosts.
                ErrorKind::NonRecoverable(_) => true,
                ErrorKind::Recoverable(_) => false,
            })
            // filter out `color-eyre`'s default set of frames to skip from
            // backtraces.
            //
            // this includes `std::rt`, `color_eyre`'s own frames, and
            // `tokio::runtime` & friends.
            .add_default_filters()
            .add_issue_metadata("version", env!("CARGO_PKG_VERSION"));
        // Add all the config values to the GitHub issue metadata
        builder = cfg.add_issue_metadata(builder);

        if self.palette == Palette::NoColors {
            // disable colors in error reports
            builder = builder.theme(Theme::new());
        }

        // We're going to wrap the panic hook in some extra code of our own, so
        // we can't use `HookBuilder::install` --- instead, split it into an
        // error hook and a panic hook.
        let (panic_hook, error_hook) = builder.into_hooks();

        // Set the panic hook.
        std::panic::set_hook(Box::new(move |panic_info| {
            // First, try to log the panic. This way, even if the more
            // user-friendly panic message isn't displayed correctly, the panic
            // will still be recorded somewhere.
            if let Some(location) = panic_info.location() {
                // If the panic has a source location, record it as structured fields.
                tracing::error!(
                    message = %panic_info,
                    panic.file = location.file(),
                    panic.line = location.line(),
                    panic.column = location.column(),
                );
            } else {
                // Otherwise, just log the whole thing.
                tracing::error!(message = %panic_info);
            }

            // After logging the panic, use the `color_eyre` panic hook to print
            // a nice, user-friendly panic message.

            // Leave crossterm before printing panic messages; otherwise, they
            // may not be displayed and the app will just crash to a blank
            // screen, which isn't great.
            let _ = crate::term::exit_crossterm();
            // Print the panic message.
            eprintln!("{}", panic_hook.panic_report(panic_info));
        }));

        // Set the error hook.
        error_hook.install()?;

        Ok(())
    }

    pub fn if_utf8<'a>(&self, utf8: &'a str, ascii: &'a str) -> &'a str {
        if self.utf8 {
            utf8
        } else {
            ascii
        }
    }

    /// Creates a span with a formatted duration inside.
    ///
    /// The formatted duration will be colored depending on the palette
    /// defined for this `Styles` object.
    ///
    /// If the `width` parameter is `None` then no padding will be
    /// added. Otherwise the text in the span will be left-padded to
    /// the specified width (right aligned). Passing `Some(0)` is
    /// equivalent to `None`.
    pub fn time_units<'a>(&self, dur: Duration, prec: usize, width: Option<usize>) -> Span<'a> {
        let formatted = self.duration_text(dur, width.unwrap_or(0), prec);

        if !self.toggles.color_durations() {
            return Span::raw(formatted.into_inner());
        }

        let style = match self.palette {
            Palette::NoColors => return Span::raw(formatted.into_inner()),
            Palette::Ansi8 | Palette::Ansi16 => match &formatted {
                FormattedDuration::Days(_) => fg_style(Color::Blue),
                FormattedDuration::DaysHours(_) => fg_style(Color::Blue),
                FormattedDuration::HoursMinutes(_) => fg_style(Color::Cyan),
                FormattedDuration::MinutesSeconds(_) => fg_style(Color::Green),
                FormattedDuration::Debug(s) if s.ends_with("ps") => fg_style(Color::Gray),
                FormattedDuration::Debug(s) if s.ends_with("ns") => fg_style(Color::Gray),
                FormattedDuration::Debug(s) if s.ends_with("µs") || s.ends_with("us") => {
                    fg_style(Color::Magenta)
                }
                FormattedDuration::Debug(s) if s.ends_with("ms") => fg_style(Color::Red),
                FormattedDuration::Debug(s) if s.ends_with('s') => fg_style(Color::Yellow),
                _ => Style::default(),
            },
            Palette::Ansi256 | Palette::All => match &formatted {
                FormattedDuration::Days(_) => fg_style(Color::Indexed(33)), // dodger blue 1
                FormattedDuration::DaysHours(_) => fg_style(Color::Indexed(33)), // dodger blue 1
                FormattedDuration::HoursMinutes(_) => fg_style(Color::Indexed(39)), // deep sky blue 1
                FormattedDuration::MinutesSeconds(_) => fg_style(Color::Indexed(45)), // turquoise 2
                FormattedDuration::Debug(s) if s.ends_with("ps") => fg_style(Color::Indexed(40)), // green 3
                FormattedDuration::Debug(s) if s.ends_with("ns") => fg_style(Color::Indexed(41)), // spring green 3
                FormattedDuration::Debug(s) if s.ends_with("µs") || s.ends_with("us") => {
                    fg_style(Color::Indexed(42))
                } // spring green 2
                FormattedDuration::Debug(s) if s.ends_with("ms") => fg_style(Color::Indexed(43)), // cyan 3
                FormattedDuration::Debug(s) if s.ends_with('s') => fg_style(Color::Indexed(44)), // dark turquoise,
                _ => Style::default(),
            },
        };

        Span::styled(formatted.into_inner(), style)
    }

    fn duration_text(&self, dur: Duration, width: usize, prec: usize) -> FormattedDuration {
        let secs = dur.as_secs();

        if secs >= 60 * 60 * 24 * 100 {
            let days = secs / (60 * 60 * 24);
            FormattedDuration::Days(format!("{days:>width$}d", days = days, width = width))
        } else if secs >= 60 * 60 * 24 {
            let hours = secs / (60 * 60);
            FormattedDuration::DaysHours(format!(
                "{days:>leading_width$}d{hours:02.0}h",
                days = hours / 24,
                hours = hours % 24,
                // Subtract the known 4 characters that trail the days value.
                leading_width = width.saturating_sub(4),
            ))
        } else if secs >= 60 * 60 {
            let mins = secs / 60;
            FormattedDuration::HoursMinutes(format!(
                "{hours:>leading_width$}h{minutes:02.0}m",
                hours = mins / 60,
                minutes = mins % 60,
                // Subtract the known 4 characters that trail the hours value.
                leading_width = width.saturating_sub(4),
            ))
        } else if secs >= 60 {
            FormattedDuration::MinutesSeconds(format!(
                "{minutes:>leading_width$}m{seconds:02.0}s",
                minutes = secs / 60,
                seconds = secs % 60,
                // Subtract the known 4 characters that trail the minutes value.
                leading_width = width.saturating_sub(4),
            ))
        } else {
            let mut text = format!("{:>width$.prec$?}", dur, width = width, prec = prec);

            if !self.utf8 {
                if let Some(mu_offset) = text.find("µs") {
                    text.replace_range(mu_offset.., "us");
                }
            }

            FormattedDuration::Debug(text)
        }
    }

    pub fn terminated(&self) -> Style {
        if !self.toggles.color_terminated() {
            return Style::default();
        }

        Style::default().add_modifier(Modifier::DIM)
    }

    pub fn fg(&self, color: Color) -> Style {
        if let Some(color) = self.color(color) {
            Style::default().fg(color)
        } else {
            Style::default()
        }
    }

    pub fn warning_wide(&self) -> Span<'static> {
        Span::styled(
            self.if_utf8("\u{26A0} ", "/!\\ "),
            self.fg(Color::LightYellow).add_modifier(Modifier::BOLD),
        )
    }

    pub fn warning_narrow(&self) -> Span<'static> {
        Span::styled(
            self.if_utf8("\u{26A0} ", "! "),
            self.fg(Color::LightYellow).add_modifier(Modifier::BOLD),
        )
    }

    pub fn selected(&self, value: &str) -> Span<'static> {
        let style = if let Some(cyan) = self.color(Color::Cyan) {
            Style::default().fg(cyan)
        } else {
            Style::default().remove_modifier(Modifier::REVERSED)
        };
        Span::styled(value.to_string(), style)
    }

    pub fn ascending(&self, value: &str) -> Span<'static> {
        let value = format!("{}{}", value, self.if_utf8("▵", "+"));
        self.selected(&value)
    }

    pub fn descending(&self, value: &str) -> Span<'static> {
        let value = format!("{}{}", value, self.if_utf8("▿", "-"));
        self.selected(&value)
    }

    pub fn color(&self, color: Color) -> Option<Color> {
        use Palette::*;
        match (self.palette, color) {
            // If colors are disabled, no colors.
            (NoColors, _) => None,
            // If true RGB color is enabled, any color is enabled.
            (All, color) => Some(color),
            // If ANSI 256 colors are enabled, we can't use RGB true colors...
            (Ansi256, Color::Rgb(_, _, _)) => None,
            // ...but we can use anything else.
            (Ansi256, color) => Some(color),
            // If we are using only ANSI 16 or ANSI 8 colors, disable RGB true
            // colors and ANSI 256 indexed colors.
            (_, Color::Rgb(_, _, _)) | (Ansi16, Color::Indexed(_)) => None,
            // If we are using ANSI 16 colors and the color is not RGB or
            // indexed, allow it.
            (Ansi16, color) => Some(color),
            // If we are using ANSI 8 colors, try to translate ANSI 16 colors
            // 'light' variants to their 8 color equivalents...
            (Ansi8, Color::LightRed) => Some(Color::Red),
            (Ansi8, Color::LightGreen) => Some(Color::Green),
            (Ansi8, Color::LightYellow) => Some(Color::Yellow),
            (Ansi8, Color::LightBlue) => Some(Color::Blue),
            (Ansi8, Color::LightMagenta) => Some(Color::Magenta),
            (Ansi8, Color::Cyan) => Some(Color::Cyan),
            // Otherwise, if a previous case didn't match, the color is enabled
            // by the current palette.
            (_, _) => Some(color),
        }
    }

    pub fn border_block(&self) -> ratatui::widgets::Block<'_> {
        if self.utf8 {
            ratatui::widgets::Block::default()
                .borders(ratatui::widgets::Borders::ALL)
                .border_type(ratatui::widgets::BorderType::Rounded)
        } else {
            // TODO(eliza): configure an ascii-art border set instead?
            Default::default()
        }
    }
}

// === impl Palette ===

impl FromStr for Palette {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim() {
            "0" => Ok(Palette::NoColors),
            "8" => Ok(Palette::Ansi8),
            "16" => Ok(Palette::Ansi16),
            "256" => Ok(Palette::Ansi256),
            s if s.eq_ignore_ascii_case("all") => Ok(Palette::All),
            s if s.eq_ignore_ascii_case("off") => Ok(Palette::NoColors),
            _ => Err("invalid color palette"),
        }
    }
}

impl Default for Palette {
    fn default() -> Self {
        Self::NoColors
    }
}
