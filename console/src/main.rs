use color_eyre::{eyre::eyre, Help, SectionExt};
use console_api::tasks::{tasks_client::TasksClient, DetailsRequest, TaskDetails, TasksRequest};
use futures::{future::OptionFuture, stream::StreamExt};

use tui::{
    layout::{Constraint, Direction, Layout},
    style::{Modifier, Style},
    text::{Span, Spans},
    widgets::{Block, Paragraph, Wrap},
};

use crate::view::UpdateKind;

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
    let mut tasks_stream = client.watch_tasks(request).await?.into_inner();
    let mut details_stream: Option<tonic::Streaming<TaskDetails>> = None;
    let mut tasks = tasks::State::default();
    let mut input = input::EventStream::new();
    let mut view = view::View::default();
    loop {
        let details_stream_next: OptionFuture<_> =
            details_stream.as_mut().map(|stream| stream.next()).into();

        tokio::select! { biased;
            input = input.next() => {
                let input = input
                    .ok_or_else(|| eyre!("keyboard input stream ended early"))
                    .with_section(|| "this is probably a bug".header("Note:"))??;
                if input::should_quit(&input) {
                    return Ok(());
                }
                match view.update_input(input, &tasks) {
                    UpdateKind::SelectTask(task) => {
                        if let Some(task) = task.upgrade() {
                            let task_id = task.borrow().id();
                            let request = tonic::Request::new(DetailsRequest {
                                id: Some(task_id.into()),
                            });
                            match client.watch_task_details(request).await {
                                Ok(stream) => {
                                    details_stream = Some(stream.into_inner());
                                },
                                Err(_) => {
                                    // TODO: handle the error somehow
                                    details_stream = None;
                                }
                            }
                        }
                    },
                    UpdateKind::ExitTaskView => {
                        details_stream = None;
                        tasks.unset_task_details();
                    },
                    _ => {}
                }
            },
            task_update = tasks_stream.next() => {
                let update = task_update
                    .ok_or_else(|| eyre!("data stream closed by server"))
                    .with_section(|| "in the future, this should be reconnected automatically...".header("Note:"))?;
                tasks.update_tasks(update?);
            },
            details_update = details_stream_next => {
                if let Some(details_update) = details_update {
                    match details_update {
                        Some(Ok(update)) => {
                            tasks.update_task_details(update);
                        },
                        Some(Err(_)) => {
                            // TODO: handle the error somehow
                            details_stream = None;
                        },
                        None => {
                            details_stream = None;
                        },
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
            view.render(f, chunks[1], &mut tasks);
        })?;
    }
}
