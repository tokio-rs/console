use crate::{input, tasks::Task};
use std::{cell::RefCell, fmt::Write, rc::Rc, time::SystemTime};
use tui::{
    layout,
    style::{self, Style},
    text::{Span, Spans},
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

        let fields: String = task
            .fields()
            .iter()
            .fold(String::new(), |mut res, f| {
                write!(&mut res, "{}={} ", f.name, f.value).unwrap();
                res
            })
            .trim_end()
            .into();

        let attrs = Spans::from(vec![
            Span::styled("ID: ", Style::default().add_modifier(style::Modifier::BOLD)),
            Span::raw(task.id_hex()),
            Span::raw(", "),
            Span::styled(
                "Fields: ",
                Style::default().add_modifier(style::Modifier::BOLD),
            ),
            Span::raw(fields),
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

        let lines = vec![attrs, metrics];
        let block = Block::default().borders(Borders::ALL).title("Task");
        let paragraph = Paragraph::new(lines).block(block);
        frame.render_widget(paragraph, area);
    }
}
