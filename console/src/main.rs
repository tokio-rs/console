use color_eyre::{eyre::eyre, Help, SectionExt};
use console_api::tasks::{tasks_client::TasksClient, TaskUpdate, TasksRequest};
use futures::stream::StreamExt;
use tasks::State;

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

enum ConnectionState {
    Connected,
    Disconnected,
}

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
    let mut tasks = State::default();
    let mut stream: Option<tonic::Streaming<TaskUpdate>> = None;
    let mut connection_state: Option<ConnectionState> = None;
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
            connection = connect(target.clone(), connection_state.is_some()), if stream.is_none() => {
                match connection {
                    Ok(s) => {
                        stream = Some(s);
                        connection_state = Some(ConnectionState::Connected);
                    },
                    Err(err) => {
                        tracing::error!(%err, "connection unsuccessful");
                        stream = None;
                    }
                }
            },
            task_update = async { match stream.as_mut() {
                Some(s) => s.next().await,
                None => None,
            }}, if stream.is_some() => {
                match task_update {
                    Some(Ok(update)) => {
                        tasks.update_tasks(update);
                    },
                    Some(Err(status)) => {
                        tracing::error!(%status, "error from stream");
                        stream = None;
                        connection_state = Some(ConnectionState::Disconnected);
                    },
                    None => {
                        tracing::error!("stream closed by server");
                        stream = None;
                        connection_state = Some(ConnectionState::Disconnected);
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
                Span::raw(format!("connection: {} ", target)),
                match connection_state {
                    Some(ConnectionState::Connected) => Span::styled(
                        "(CONNECTED)",
                        Style::default()
                            .add_modifier(Modifier::BOLD)
                            .fg(Color::Green),
                    ),
                    Some(ConnectionState::Disconnected) | None => Span::styled(
                        "(DISCONNECTED)",
                        Style::default().add_modifier(Modifier::BOLD).fg(Color::Red),
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

async fn connect(
    target: String,
    is_reconnect: bool,
) -> Result<tonic::Streaming<TaskUpdate>, Box<dyn std::error::Error>> {
    if is_reconnect {
        tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
    }

    let mut client = TasksClient::connect(target).await?;
    let request = tonic::Request::new(TasksRequest {});
    let rsp = client.watch_tasks(request).await?;
    Ok(rsp.into_inner())
}
