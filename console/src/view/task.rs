use crate::{
    input,
    tasks::{DetailsRef, Task},
    view::bold,
};
use std::{cell::RefCell, rc::Rc, time::SystemTime};
use tui::{
    layout::{self, Layout},
    text::{Span, Spans, Text},
    widgets::{Block, Borders, Paragraph},
};

pub(crate) struct TaskView {
    task: Rc<RefCell<Task>>,
    details: DetailsRef,
}

impl TaskView {
    pub(super) fn new(task: Rc<RefCell<Task>>, details: DetailsRef) -> Self {
        TaskView { task, details }
    }

    pub(crate) fn update_input(&mut self, _event: input::Event) {
        // TODO :D
    }

    pub(crate) fn render<B: tui::backend::Backend>(
        &mut self,
        frame: &mut tui::terminal::Frame<B>,
        area: layout::Rect,
        now: SystemTime,
    ) {
        // Rows with the following info:
        // - Task main attributes
        // - task metadata
        // - metrics
        // - logs?

        let task = &*self.task.borrow();
        const DUR_PRECISION: usize = 4;

        let chunks = Layout::default()
            .direction(layout::Direction::Vertical)
            .constraints(
                [
                    layout::Constraint::Length(1),
                    layout::Constraint::Length(6),
                    layout::Constraint::Percentage(60),
                ]
                .as_ref(),
            )
            .split(area);

        let controls_area = chunks[0];
        let stats_area = Layout::default()
            .direction(layout::Direction::Horizontal)
            .constraints(
                [
                    layout::Constraint::Percentage(50),
                    layout::Constraint::Percentage(50),
                ]
                .as_ref(),
            )
            .split(chunks[1]);

        let fields_area = chunks[2];

        let controls = Spans::from(vec![
            Span::raw("controls: "),
            bold("esc"),
            Span::raw(" = return to task list, "),
            bold("q"),
            Span::raw(" = quit"),
        ]);

        let attrs = Spans::from(vec![bold("ID: "), Span::raw(task.id_hex())]);

        let mut total = vec![
            bold("Total Time: "),
            Span::from(format!("{:.prec$?}", task.total(now), prec = DUR_PRECISION,)),
        ];

        // TODO(eliza): maybe surface how long the task has been completed, as well?
        if task.is_completed() {
            total.push(Span::raw(" (completed)"));
        };

        let total = Spans::from(total);

        let busy = Spans::from(vec![
            bold("Busy: "),
            Span::from(format!("{:.prec$?}", task.busy(), prec = DUR_PRECISION,)),
        ]);
        let idle = Spans::from(vec![
            bold("Idle: "),
            Span::from(format!("{:.prec$?}", task.idle(now), prec = DUR_PRECISION,)),
        ]);

        let metrics = vec![attrs, total, busy, idle];

        let wakers = Spans::from(vec![
            bold("Current wakers: "),
            Span::from(format!("{} (", task.waker_count())),
            bold("clones: "),
            Span::from(format!("{}, ", task.waker_clones())),
            bold("drops: "),
            Span::from(format!("{})", task.waker_drops())),
        ]);

        let mut wakeups = vec![
            bold("Woken: "),
            Span::from(format!("{} times", task.wakes())),
        ];

        // If the task has been woken, add the time since wake to its stats as well.
        if let Some(since) = task.since_wake(now) {
            wakeups.push(Span::raw(", "));
            wakeups.push(bold("last woken:"));
            wakeups.push(Span::from(format!(" {:?} ago", since)));
        }

        let wakeups = Spans::from(wakeups);

        fn block_for(title: &str) -> Block {
            Block::default().borders(Borders::ALL).title(title)
        }

        let mut fields = Text::default();
        fields.extend(task.formatted_fields().iter().cloned().map(Spans::from));

        let has_details = self
            .details
            .borrow()
            .as_ref()
            .map(|details| details.task_id() == task.id())
            .unwrap_or(false);
        let details_last_updated = self
            .details
            .borrow()
            .as_ref()
            .and_then(|details| details.last_updated_at())
            .map(|time| {
                let datetime: chrono::DateTime<chrono::offset::Local> = time.into();
                datetime.format("%c").to_string()
            })
            .unwrap_or_else(|| "never".to_owned());
        let l = vec![
            bold("Has details?"),
            Span::from(format!("{:?}", has_details)),
            bold("Details last updated:"),
            Span::from(details_last_updated),
        ];
        fields.extend(l.into_iter().map(Spans::from));

        let task_widget = Paragraph::new(metrics).block(block_for("Task"));
        let wakers_widget = Paragraph::new(vec![wakers, wakeups]).block(block_for("Waker"));
        let fields_widget = Paragraph::new(fields).block(block_for("Fields"));

        frame.render_widget(Block::default().title(controls), controls_area);
        frame.render_widget(task_widget, stats_area[0]);
        frame.render_widget(wakers_widget, stats_area[1]);
        frame.render_widget(fields_widget, fields_area);
    }
}
