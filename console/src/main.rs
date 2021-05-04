use chrono::Local;
use console_api::tasks::{tasks_client::TasksClient, TasksRequest};
use futures::stream::StreamExt;
use std::io;
use tokio::select;

use tui::{
    backend::TermionBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Terminal,
};

use tui::backend::CrosstermBackend;
use tui::style::*;
use tui::text::*;
use tui::widgets::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args();
    args.next(); // drop the first arg (the name of the binary)
    let target = args.next().unwrap_or_else(|| {
        eprintln!("using default address (http://127.0.0.1:6669)");
        String::from("http://127.0.0.1:6669")
    });

    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let mut client = TasksClient::connect(target).await?;
    let request = tonic::Request::new(TasksRequest {});
    let mut stream = client.watch_tasks(request).await?.into_inner();

    while let Some(update) = stream.next().await {
        match update {
            Ok(update) => {
                eprintln!("UPDATE {:?}", update);
            }
            Err(e) => {
                eprintln!("update stream error: {}", e);
                return Err(e.into());
            }
        }
    }

    loop {
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(0)
                .constraints([Constraint::Length(10), Constraint::Percentage(95)].as_ref())
                .split(f.size());

            let header_block = Block::default()
                .title(vec![Span::styled(
                    "desolation",
                    Style::default().add_modifier(Modifier::BOLD),
                )])
                .borders(Borders::ALL);

            let text = vec![Spans::from(vec![
                Span::styled("controls: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw("up/down = scroll, q = quit"),
            ])];
            let header = Paragraph::new(text)
                .block(header_block)
                .wrap(Wrap { trim: true });

            f.render_widget(header, chunks[0]);
        })?;
    }

    Ok(())
}
