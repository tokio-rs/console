use crate::{input, tasks::Task};
use std::{cell::RefCell, rc::Rc, time::SystemTime};
use tui::{
    layout::{self, Layout},
    style::{self, Style},
    text::{Span, Spans, Text},
    widgets::{Block, Borders, Paragraph},
};

pub(crate) struct TaskView {
    task: Rc<RefCell<Task>>,
}

impl TaskView {
    pub(super) fn new(task: Rc<RefCell<Task>>) -> Self {
        TaskView { task }
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
                    layout::Constraint::Length(5),
                    layout::Constraint::Percentage(60),
                ]
                .as_ref(),
            )
            .split(area);

        let stats_area = Layout::default()
            .direction(layout::Direction::Horizontal)
            .constraints(
                [
                    layout::Constraint::Percentage(50),
                    layout::Constraint::Percentage(50),
                ]
                .as_ref(),
            )
            .split(chunks[0]);

        let fields_area = chunks[1];

        let attrs = Spans::from(vec![
            Span::styled("ID: ", Style::default().add_modifier(style::Modifier::BOLD)),
            Span::raw(task.id_hex()),
        ]);

        let metrics = Spans::from(vec![
            Span::styled(
                "Total Time: ",
                Style::default().add_modifier(style::Modifier::BOLD),
            ),
            Span::from(format!("{:.prec$?}", task.total(now), prec = DUR_PRECISION,)),
            Span::raw(", "),
            Span::styled(
                "Busy: ",
                Style::default().add_modifier(style::Modifier::BOLD),
            ),
            Span::from(format!("{:.prec$?}", task.busy(), prec = DUR_PRECISION,)),
            Span::raw(", "),
            Span::styled(
                "Idle: ",
                Style::default().add_modifier(style::Modifier::BOLD),
            ),
            Span::from(format!("{:.prec$?}", task.idle(now), prec = DUR_PRECISION,)),
        ]);

        let wakers = Spans::from(vec![
            Span::styled(
                "Current wakers: ",
                Style::default().add_modifier(style::Modifier::BOLD),
            ),
            Span::from(format!("{} (", task.waker_count())),
            Span::styled(
                "clones: ",
                Style::default().add_modifier(style::Modifier::BOLD),
            ),
            Span::from(format!("{}, ", task.waker_clones())),
            Span::styled(
                "drops: ",
                Style::default().add_modifier(style::Modifier::BOLD),
            ),
            Span::from(format!("{})", task.waker_drops())),
        ]);

        let mut wakeups = vec![
            Span::styled(
                "Woken: ",
                Style::default().add_modifier(style::Modifier::BOLD),
            ),
            Span::from(format!("{} times", task.wakes())),
        ];

        // If the task has been woken, add the time since wake to its stats as well.
        if let Some(since) = task.since_wake(now) {
            wakeups.push(Span::raw(", "));
            wakeups.push(Span::styled("last woken:", bold()));
            wakeups.push(Span::from(format!(" {:?} ago", since)));
        }

        let wakeups = Spans::from(wakeups);

        fn block_for(title: &str) -> Block {
            Block::default().borders(Borders::ALL).title(title)
        }

        fn bold() -> Style {
            Style::default().add_modifier(style::Modifier::BOLD)
        }

        let mut fields = Text::default();
        fields.extend(task.formatted_fields().iter().cloned().map(Spans::from));

        let task_widget = Paragraph::new(vec![attrs, metrics]).block(block_for("Task"));
        let wakers_widget = Paragraph::new(vec![wakers, wakeups]).block(block_for("Waker"));
        let fields_widget = Paragraph::new(fields).block(block_for("Fields"));

        frame.render_widget(task_widget, stats_area[0]);
        frame.render_widget(wakers_widget, stats_area[1]);
        frame.render_widget(fields_widget, fields_area);
    }
}
