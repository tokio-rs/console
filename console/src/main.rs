use color_eyre::{eyre::eyre, Help, SectionExt};
use console_api::tasks::TaskDetails;
use tasks::State;

use futures::stream::StreamExt;
use tokio::sync::{mpsc, watch};
use tui::{
    layout::{Constraint, Direction, Layout},
    style::{Modifier, Style},
    text::{Span, Spans},
    widgets::{Block, Paragraph, Wrap},
};

use crate::view::UpdateKind;

mod conn;
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
    let mut conn = conn::Connection::new(target);
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
                        match conn.watch_details(task_id).await {
                            Ok(stream) => {
                                tokio::spawn(watch_details_stream(task_id, stream, update_rx.clone(), details_tx.clone()));
                            },
                            Err(error) => {
                                tracing::warn!(%error, "error watching task details");
                                tasks.unset_task_details();
                        }
                        }
                    },
                    UpdateKind::ExitTaskView => {
                        tasks.unset_task_details();
                    }
                    _ => {}
                }
            },
            instrument_update = conn.next_update() => {
                let now = instrument_update.now.map(Into::into);
                if let Some(task_update) = instrument_update.task_update {
                    tasks.update_tasks(task_update, instrument_update.new_metadata, now);
                }
            }
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

            let header_block = Block::default().title(conn.render());

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

/// Given the task details stream for the given task id, sends the updates
/// to the `details_tx` channel until the currently-viewed task changes.
///
/// This is a separate task from the main program loop mainly because there isn't
/// always a details stream to poll and we need to react to user inputs to
/// replace the details stream with another one.
async fn watch_details_stream(
    task_id: u64,
    mut details_stream: tonic::Streaming<TaskDetails>,
    mut watch_rx: watch::Receiver<UpdateKind>,
    details_tx: mpsc::Sender<TaskDetails>,
) {
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
}
