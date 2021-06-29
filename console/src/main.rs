use color_eyre::{eyre::eyre, Help, SectionExt};
use console_api::tasks::{tasks_client::TasksClient, TasksRequest};
use futures::stream::StreamExt;
use tasks::{ConnectionState, State};

use tui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{Block, Paragraph, Wrap},
};

mod input;
mod tasks;
mod term;
mod view;

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    let mut args = std::env::args();
    args.next(); // drop the first arg (the name of the binary)
    let target = args.next().unwrap_or_else(|| {
        eprintln!("using default address (http://127.0.0.1:6669)");
        String::from("http://127.0.0.1:6669")
    });

    let (mut terminal, _cleanup) = term::init_crossterm()?;
    terminal.clear()?;

    let mut client = TasksClient::connect(target.clone()).await?;
    let request = tonic::Request::new(TasksRequest {});
    let mut tasks = State::default();
    let mut stream = client.watch_tasks(request).await?.into_inner();
    tasks.set_connection_state(ConnectionState::Connected(target));
    let mut input = input::EventStream::new();
    let mut view = view::View::default();
    loop {
        tokio::select! { biased;
            input = input.next() => {
                let input = input
                    .ok_or_else(|| eyre!("keyboard input stream ended early"))
                    .with_section(|| "this is probably a bug".header("Note:"))??;
                if input::should_quit(&input) {
                    return Ok(());
                }
                view.update_input(input);
            },
            task_update = stream.next() => {
                match task_update {
                    Some(Ok(update)) => {
                        tasks.update_tasks(update);
                    },
                    Some(Err(status)) => {
                        tracing::error!(%status, "error fro stream");
                        tasks.set_connection_state(ConnectionState::Disconnected);
                    },
                    None => {
                        tracing::error!("data stream closed by server");
                        tasks.set_connection_state(ConnectionState::Disconnected);
                    }

                }
            }
        }
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(0)
                .constraints([Constraint::Length(2), Constraint::Percentage(95)].as_ref())
                .split(f.size());

            let header_block = Block::default().title(vec![
                Span::raw("connection: "),
                match tasks.connection_state() {
                    ConnectionState::Connected(t) => Span::styled(
                        t.as_str(),
                        Style::default()
                            .add_modifier(Modifier::BOLD)
                            .fg(Color::Green),
                    ),
                    ConnectionState::Disconnected => Span::styled(
                        "DISCONNECTED",
                        Style::default().add_modifier(Modifier::BOLD).fg(Color::Red),
                    ),
                    ConnectionState::Pending => Span::styled(
                        "PENDING",
                        Style::default()
                            .add_modifier(Modifier::BOLD)
                            .fg(Color::Yellow),
                    ),
                },
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
            view.render(f, chunks[1], &mut tasks);
        })?;
    }
}
