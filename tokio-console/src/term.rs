pub use color_eyre::eyre::WrapErr;
use std::io;
pub use ratatui::{backend::CrosstermBackend, Terminal};

pub fn init_crossterm() -> color_eyre::Result<(Terminal<CrosstermBackend<io::Stdout>>, OnShutdown)>
{
    use crossterm::terminal::{self, EnterAlternateScreen};
    terminal::enable_raw_mode().wrap_err("Failed to enable crossterm raw mode")?;

    let mut stdout = std::io::stdout();
    crossterm::execute!(stdout, EnterAlternateScreen)
        .wrap_err("Failed to enable crossterm alternate screen")?;
    let backend = CrosstermBackend::new(io::stdout());
    let term = Terminal::new(backend).wrap_err("Failed to create crossterm terminal")?;

    let cleanup = OnShutdown::new(exit_crossterm);

    Ok((term, cleanup))
}

pub(crate) fn exit_crossterm() -> color_eyre::Result<()> {
    use crossterm::terminal::{self, LeaveAlternateScreen};
    // Be a good terminal citizen...
    let mut stdout = std::io::stdout();
    crossterm::execute!(stdout, LeaveAlternateScreen)
        .wrap_err("Failed to disable crossterm alternate screen")?;
    terminal::disable_raw_mode().wrap_err("Failed to enable crossterm raw mode")?;
    Ok(())
}

pub struct OnShutdown {
    action: fn() -> color_eyre::Result<()>,
}

impl OnShutdown {
    fn new(action: fn() -> color_eyre::Result<()>) -> Self {
        Self { action }
    }
}

impl Drop for OnShutdown {
    fn drop(&mut self) {
        if let Err(error) = (self.action)() {
            tracing::error!(%error, "error running terminal cleanup");
        }
    }
}
