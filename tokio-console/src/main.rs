use color_eyre::{eyre::eyre, Help, SectionExt};
use console_api::tasks::TaskDetails;
use state::State;

use futures::{
    future,
    stream::{StreamExt, TryStreamExt},
};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::Color,
    text::{Span, Spans},
    widgets::{Paragraph, Wrap},
};
use tokio::sync::{mpsc, watch};

use crate::{
    input::{Event, KeyEvent, KeyEventKind},
    view::{bold, UpdateKind},
};

mod config;
mod conn;
mod input;
mod intern;
mod state;
mod term;
mod util;
mod view;
mod warnings;

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    let args = config::Config::parse()?;
    // initialize error handling first, in case panics occur while setting up
    // other stuff.
    let styles = view::Styles::from_config(args.view_options.clone());
    styles.error_init(&args)?;

    args.trace_init()?;
    tracing::debug!(?args.target_addr, ?args.view_options);

    match args.subcmd {
        Some(config::OptionalCmd::GenConfig) => {
            // Generate a default config file and exit.
            let toml = args.gen_config_file()?;
            println!("{}", toml);
            return Ok(());
        }
        Some(config::OptionalCmd::GenCompletion { install, shell }) => {
            return config::gen_completion(install, shell);
        }
        None => {}
    }

    let target = args.target_addr()?;
    tracing::info!(?target, "using target addr");

    let retain_for = args.retain_for();
    let (mut terminal, _cleanup) = term::init_crossterm()?;
    terminal.clear()?;
    let mut conn = conn::Connection::new(target);
    // A channel to send the outcome of `View::update_input` to the watch_details_stream task.
    let (update_tx, update_rx) = watch::channel(UpdateKind::Other);
    // A channel to send the task details update stream (no need to keep outdated details in the memory)
    let (details_tx, mut details_rx) = mpsc::channel::<TaskDetails>(2);

    let mut state = State::default()
        .with_task_linters(args.warns.iter().map(|lint| lint.into()))
        .with_retain_for(retain_for);
    let mut input = Box::pin(input::EventStream::new().try_filter(|event| {
        future::ready(!matches!(
            event,
            Event::Key(KeyEvent {
                kind: KeyEventKind::Release,
                ..
            })
        ))
    }));
    let mut view = view::View::new(styles);

    loop {
        tokio::select! { biased;
            input = input.next() => {
                let input = input
                    .ok_or_else(|| eyre!("keyboard input stream ended early"))
                    .with_section(|| "this is probably a bug".header("Note:"))??;
                if input::should_quit(&input) {
                    return Ok(());
                }

                if input::is_space(&input) {
                    if state.is_paused() {
                        conn.resume().await;
                        state.resume();
                    } else {
                        conn.pause().await;
                        state.pause();
                    }
                }

                let update_kind = view.update_input(input, &state);
                // Using the result of update_input to manage the details watcher task
                let _ = update_tx.send(update_kind);
                match update_kind {
                    UpdateKind::SelectTask(task_id) => {
                        tracing::info!(task_id, "starting details watch");
                        match conn.watch_details(task_id).await {
                            Ok(stream) => {
                                tokio::spawn(watch_details_stream(task_id, stream, update_rx.clone(), details_tx.clone()));
                            },
                            Err(error) => {
                                tracing::warn!(%error, "error watching task details");
                                state.unset_task_details();
                        }
                        }
                    },
                    UpdateKind::ExitTaskView => {
                        state.unset_task_details();
                    }
                    _ => {}
                }
            },
            instrument_update = conn.next_update() => {
                state.update(&view.styles, view.current_view(), instrument_update);
            }
            details_update = details_rx.recv() => {
                if let Some(details_update) = details_update {
                    state.update_task_details(details_update);
                }
            },
        }
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(0)
                .constraints(
                    [
                        Constraint::Length(1),
                        Constraint::Length(1),
                        Constraint::Percentage(95),
                    ]
                    .as_ref(),
                )
                .split(f.size());

            let mut header_text = conn.render(&view.styles);
            if state.is_paused() {
                header_text
                    .0
                    .push(Span::styled(" PAUSED", view.styles.fg(Color::Red)));
            }
            let dropped_async_ops_state = state.async_ops_state().dropped_events();
            let dropped_tasks_state = state.tasks_state().dropped_events();
            let dropped_resources_state = state.resources_state().dropped_events();
            if (dropped_async_ops_state + dropped_tasks_state + dropped_resources_state) > 0 {
                let mut dropped_texts = vec![];
                if dropped_async_ops_state > 0 {
                    dropped_texts.push(format!("{} async_ops", dropped_async_ops_state))
                }
                if dropped_tasks_state > 0 {
                    dropped_texts.push(format!("{} tasks", dropped_tasks_state))
                }
                if dropped_resources_state > 0 {
                    dropped_texts.push(format!("{} resources", dropped_resources_state))
                }
                header_text.0.push(Span::styled(
                    format!(" dropped: {}", dropped_texts.join(", ")),
                    view.styles.fg(Color::Red),
                ));
            }
            let header = Paragraph::new(header_text).wrap(Wrap { trim: true });
            let view_controls = Paragraph::new(Spans::from(vec![
                Span::raw("views: "),
                bold("t"),
                Span::raw(" = tasks, "),
                bold("r"),
                Span::raw(" = resources"),
            ]))
            .wrap(Wrap { trim: true });

            f.render_widget(header, chunks[0]);
            f.render_widget(view_controls, chunks[1]);
            view.render(f, chunks[2], &mut state);
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
