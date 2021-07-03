use color_eyre::{eyre::eyre, Help, SectionExt};
use console_api::tasks::{
    tasks_client::TasksClient, DetailsRequest, TaskDetails, TaskUpdate, TasksRequest,
};
use futures::stream::StreamExt;
use tasks::State;

use tokio::sync::{mpsc, watch};
use tonic::transport::Channel;
use tui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{Block, Paragraph, Wrap},
};

use crate::view::UpdateKind;

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
    let mut stream: Option<tonic::Streaming<TaskUpdate>> = None;
    let mut client: Option<TasksClient<Channel>> = None;
    let mut connection_state: Option<ConnectionState> = None;
    // A channel to send the outcome of `View::update_input` to the watch_details_stream task.
    let (update_tx, update_rx) = watch::channel(UpdateKind::Other);
    // A channel to send the task details update stream (no need to keep outdated details in the memory)
    let (details_tx, mut details_rx) = mpsc::channel::<TaskDetails>(2);

    let mut tasks = State::default();
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
                let update_kind = view.update_input(input, &tasks);
                // Using the result of update_input to manage the details watcher task
                let _ = update_tx.send(update_kind);
                match update_kind {
                    UpdateKind::SelectTask(task_id) => {
                        tokio::spawn(watch_details_stream(task_id, client.clone(), update_rx.clone(), details_tx.clone()));
                    }
                    UpdateKind::ExitTaskView => {
                        tasks.unset_task_details();
                    }
                    _ => {}
                }
            },
            connection = connect(target.clone(), connection_state.is_some()), if stream.is_none() => {
                match connection {
                    Ok((s, c)) => {
                        stream = Some(s);
                        client = Some(c);
                        connection_state = Some(ConnectionState::Connected);
                    },
                    Err(err) => {
                        tracing::error!(%err, "connection unsuccessful");
                        stream = None;
                        client = None;
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
            },
            details_update = details_rx.recv() => {
                if let Some(details_update) = details_update {
                    tasks.update_task_details(details_update);
                }
            },
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
) -> Result<(tonic::Streaming<TaskUpdate>, TasksClient<Channel>), Box<dyn std::error::Error>> {
    if is_reconnect {
        tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
    }

    let mut client = TasksClient::connect(target).await?;
    let request = tonic::Request::new(TasksRequest {});
    let rsp = client.watch_tasks(request).await?;
    Ok((rsp.into_inner(), client))
}

/// Connects to the task details stream for the given task id, sends the updates
/// to the `details_tx` channel until the currently-viewed task changes.
///
/// This is a separate task from the main program loop mainly because there isn't
/// always a details stream to poll and we need to react to user inputs to
/// replace the details stream with another one.
async fn watch_details_stream(
    task_id: u64,
    mut client: Option<TasksClient<Channel>>,
    mut watch_rx: watch::Receiver<UpdateKind>,
    details_tx: mpsc::Sender<TaskDetails>,
) {
    let request = tonic::Request::new(DetailsRequest {
        id: Some(task_id.into()),
    });
    if let Some(c) = client.as_mut() {
        if let Ok(streaming) = c.watch_task_details(request).await {
            let mut details_stream = streaming.into_inner();
            loop {
                tokio::select! { biased;
                    details = details_stream.next() => {
                        match details {
                            Some(Ok(details)) => {
                                if details_tx.send(details).await.is_err() {
                                    break;
                                }
                            },
                            _ => {
                                break;
                            }
                        }
                    },
                    update = watch_rx.changed() => {
                        if update.is_ok() {
                            match *watch_rx.borrow() {
                                UpdateKind::ExitTaskView => {
                                    break;
                                },
                                UpdateKind::SelectTask(new_id) if new_id != task_id => {
                                    break;
                                },
                                _ => {}
                            }
                        } else {
                            break;
                        }
                    },
                }
            }
        } else {
            // TODO: handle connection error, print details somewhere? Related to Issue #30
        }
    }
}
