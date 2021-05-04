use console_api::tasks::{tasks_client::TasksClient, TasksRequest};
use futures::stream::StreamExt;
use std::io;

use tui::{
    layout::{Constraint, Direction, Layout},
    style::{Modifier, Style},
    text::{Span, Spans},
    widgets::{Block, Paragraph, Wrap},
    Terminal,
};

use tui::backend::CrosstermBackend;

mod tasks;

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

    let mut client = TasksClient::connect(target.clone()).await?;
    let request = tonic::Request::new(TasksRequest {});
    let mut stream = client.watch_tasks(request).await?.into_inner();
    let mut tasks = tasks::State::default();
    while let Some(update) = stream.next().await {
        match update {
            Ok(update) => {
                tasks.update(update);
            }
            Err(e) => {
                eprintln!("update stream error: {}", e);
                return Err(e.into());
            }
        }

        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(0)
                .constraints([Constraint::Length(2), Constraint::Percentage(95)].as_ref())
                .split(f.size());

            let header_block = Block::default().title(vec![
                Span::raw("connected to: "),
                Span::styled(
                    target.as_str(),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
            ]);

            let text = vec![Spans::from(vec![
                Span::styled(
                    format!("{}", tasks.len()),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw(" tasks"),
            ])];
            let header = Paragraph::new(text)
                .block(header_block)
                .wrap(Wrap { trim: true });
            f.render_widget(header, chunks[0]);
            tasks.render(f, chunks[1]);
            tasks.retain_active();
        })?;
    }

    Ok(())
}
